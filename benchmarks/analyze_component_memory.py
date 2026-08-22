#!/usr/bin/env python3
"""Summarize component-memory evidence from decision-quality publications."""

import argparse
import hashlib
import json
import statistics
from pathlib import Path

try:
    from benchmarks.validate_baseline_suite import _load_publication
except ModuleNotFoundError:
    from validate_baseline_suite import _load_publication


ROOT = Path(__file__).resolve().parent
COMPONENT_MEMORY_FIELDS = (
    "component_count",
    "active_node_count",
    "active_edge_count",
    "largest_component_node_count",
    "largest_component_edge_count",
    "partition_node_capacity",
    "partition_edge_capacity",
    "global_graph_node_capacity",
    "global_graph_edge_capacity",
    "global_graph_directed_edge_capacity",
    "global_graph_adjacency_capacity",
    "global_graph_adjacency_row_capacity",
    "global_graph_csr_offset_count",
    "global_graph_csr_directed_edge_count",
    "scratch_instance_count",
    "execution_worker_count",
    "max_scratch_node_capacity",
    "max_scratch_edge_capacity",
    "max_scratch_directed_edge_capacity",
    "max_scratch_adjacency_capacity",
    "max_scratch_global_node_capacity",
    "max_scratch_local_node_capacity",
    "max_scratch_global_dir_edge_capacity",
    "max_merged_output_item_count",
    "max_merged_output_coordinate_capacity",
)


def _median(records, field):
    return statistics.median_low(
        record["work"]["component_memory"][field] for record in records
    )


def _ratio(numerator, denominator):
    return numerator / denominator if denominator else 0.0


def _vec_vec_storage_words(component_memory):
    return (
        component_memory["global_graph_adjacency_row_capacity"] * 3
        + component_memory["global_graph_adjacency_capacity"]
    )


def _csr_storage_words(component_memory):
    return (
        component_memory["global_graph_csr_offset_count"]
        + component_memory["global_graph_csr_directed_edge_count"]
    )


def _component_summary(context):
    baseline = context["baseline"]
    records = context["records"]
    component_memory = {
        field: _median(records, field) for field in COMPONENT_MEMORY_FIELDS
    }
    active_nodes = component_memory["active_node_count"]
    active_edges = component_memory["active_edge_count"]
    workers = component_memory["execution_worker_count"]
    return {
        "workload_id": baseline["workload_id"],
        "lane": baseline["lane"],
        "publication_id": context["publication"]["publication_id"],
        "publication_sha256": context["publication_sha256"],
        "artifact_sha256": baseline["artifact_sha256"],
        "record_count": len(records),
        "input_segments": baseline["work"]["input_segments"],
        "median_p50_ms": statistics.median(
            record["measurement"]["p50_ms"] for record in records
        ),
        "median_allocated_bytes": statistics.median(
            record["measurement"]["allocations"]["bytes"] for record in records
        ),
        "median_peak_rss_bytes": statistics.median(
            record["measurement"]["peak_rss_bytes"] for record in records
        ),
        "component_memory": component_memory,
        "derived": {
            "largest_component_node_fraction": _ratio(
                component_memory["largest_component_node_count"], active_nodes
            ),
            "largest_component_edge_fraction": _ratio(
                component_memory["largest_component_edge_count"], active_edges
            ),
            "partition_node_capacity_over_active": _ratio(
                component_memory["partition_node_capacity"], active_nodes
            ),
            "partition_edge_capacity_over_active": _ratio(
                component_memory["partition_edge_capacity"], active_edges
            ),
            "scratch_instances_per_worker": _ratio(
                component_memory["scratch_instance_count"], workers
            ),
            "vec_vec_storage_words": _vec_vec_storage_words(component_memory),
            "csr_storage_words": _csr_storage_words(component_memory),
            "csr_to_vec_vec_storage_ratio": _ratio(
                _csr_storage_words(component_memory),
                _vec_vec_storage_words(component_memory),
            ),
        },
    }


def analyze_component_memory(publication_paths):
    if not publication_paths:
        raise ValueError("at least one publication is required")
    contexts = [_load_publication(path) for path in publication_paths]
    first = contexts[0]["baseline"]
    environment_fields = ("architecture", "os", "compiler", "commit_sha")
    environment = {
        field: first["environment"][field] for field in environment_fields
    }
    implementation = first["implementation"]
    for context in contexts:
        baseline = context["baseline"]
        if {
            field: baseline["environment"][field] for field in environment_fields
        } != environment:
            raise ValueError("publications must use one environment")
        if baseline["implementation"] != implementation:
            raise ValueError("publications must use one implementation")
        for record in context["records"]:
            if "component_memory" not in record["work"]:
                raise ValueError(
                    f"{context['publication']['publication_id']}: component memory is required"
                )

    publications = sorted(
        (_component_summary(context) for context in contexts),
        key=lambda value: (value["workload_id"], value["lane"]),
    )
    digest = hashlib.sha256(
        "".join(value["publication_sha256"] for value in publications).encode()
    ).hexdigest()
    return {
        "schema_version": 1,
        "report_id": f"component-memory-{digest[:16]}",
        "policy_id": "benchmark-decision-v1",
        "measurement_class": "decision-quality",
        "runner_class": "dedicated",
        "publication_count": len(publications),
        "environment": environment,
        "publications": publications,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--publication", action="append", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    report = analyze_component_memory(args.publication)
    args.output.write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")


if __name__ == "__main__":
    main()
