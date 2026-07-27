#!/usr/bin/env python3
"""Render schema-valid benchmark publications and decisions as Markdown."""

import argparse
import html
import json
import statistics
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

ROOT = Path(__file__).parent


def load(path):
    return json.loads(Path(path).read_text())


def cell(value):
    return html.escape(str(value)).replace("|", r"\|").replace("\n", " ")


def render(publication_paths, decision_paths):
    record_schema = load(ROOT / "benchmark-record-v1.schema.json")
    publication_schema = load(ROOT / "benchmark-publication-v1.schema.json")
    registry = Registry().with_resource(
        record_schema["$id"], Resource.from_contents(record_schema)
    )
    publication_validator = Draft202012Validator(
        publication_schema, registry=registry
    )
    publications = [load(path) for path in publication_paths]
    for publication in publications:
        publication_validator.validate(publication)

    decision_validator = Draft202012Validator(
        load(ROOT / "benchmark-decision-v1.schema.json")
    )
    decisions = [load(path) for path in decision_paths]
    for decision in decisions:
        decision_validator.validate(decision)

    lines = [
        "# Benchmark trends",
        "",
        "Generated from schema-valid decision-quality artifacts.",
        "",
        "## Publications",
        "",
    ]
    if publications:
        lines.extend(
            [
                "| Workload | Lane | Architecture | Commit | p50 ms | p95 ms | Throughput | Allocated bytes | Peak RSS bytes | Processes | MAD % |",
                "| --- | --- | --- | --- | ---: | ---: | --- | ---: | ---: | ---: | ---: |",
            ]
        )
        for publication in sorted(
            publications,
            key=lambda value: (
                value["records"][0]["workload_id"],
                value["records"][0]["lane"],
                value["records"][0]["environment"]["commit_sha"],
            ),
        ):
            records = publication["records"]
            identities = {
                (
                    record["workload_id"],
                    record["lane"],
                    json.dumps(record["environment"], sort_keys=True),
                )
                for record in records
            }
            throughput_units = {
                record["measurement"]["throughput"]["unit"] for record in records
            }
            if len(identities) != 1 or len(throughput_units) != 1:
                raise ValueError("publication records mix identities or throughput units")
            measurement = [record["measurement"] for record in records]
            throughput = [value["throughput"]["value"] for value in measurement]
            lines.append(
                "| "
                + " | ".join(
                    map(
                        cell,
                        [
                            records[0]["workload_id"],
                            records[0]["lane"],
                            records[0]["environment"]["architecture"],
                            records[0]["environment"]["commit_sha"][:12],
                            f"{statistics.median(value['p50_ms'] for value in measurement):.3f}",
                            f"{statistics.median(value['p95_ms'] for value in measurement):.3f}",
                            (
                                f"{statistics.median(throughput):.3f} "
                                f"{measurement[0]['throughput']['unit']}"
                            ),
                            round(
                                statistics.median(
                                    value["allocations"]["bytes"]
                                    for value in measurement
                                )
                            ),
                            round(
                                statistics.median(
                                    value["peak_rss_bytes"] for value in measurement
                                )
                            ),
                            publication["process_repetitions"],
                            f"{publication['p50_relative_mad_percent']:.3f}",
                        ],
                    )
                )
                + " |"
            )
    else:
        lines.append("No decision-quality publications.")

    lines.extend(["", "## Decisions", ""])
    if decisions:
        lines.extend(
            [
                "| Decision | Outcome | Targets | Crossover |",
                "| --- | --- | --- | --- |",
            ]
        )
        for decision in sorted(decisions, key=lambda value: value["decision_id"]):
            crossover = decision["crossover"]
            if crossover["status"] == "measured":
                measured = crossover["range"]
                summary = (
                    f"{measured['lower_bound']}–{measured['upper_bound']} "
                    f"{measured['unit']} ({measured['descriptor']})"
                )
            else:
                summary = f"not applicable: {crossover['reason']}"
            lines.append(
                "| "
                + " | ".join(
                    map(
                        cell,
                        [
                            decision["decision_id"],
                            decision["outcome"],
                            ", ".join(decision["target_workloads"]),
                            summary,
                        ],
                    )
                )
                + " |"
            )
    else:
        lines.append("No decision records.")
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--publication", action="append", default=[])
    parser.add_argument("--decision", action="append", default=[])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not args.publication and not args.decision:
        parser.error("at least one publication or decision is required")
    args.output.write_text(render(args.publication, args.decision))


if __name__ == "__main__":
    main()
