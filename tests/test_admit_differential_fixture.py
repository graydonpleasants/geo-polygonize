import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "scripts/admit_differential_fixture.py"
SPEC = importlib.util.spec_from_file_location("admit_differential_fixture", SCRIPT)
ADMISSION = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ADMISSION)
reviewed_fixture = ADMISSION.reviewed_fixture
write_fixture = ADMISSION.write_fixture


def candidate():
    return {
        "schema_version": 1,
        "producer": "adapter_differential",
        "input": [
            {
                "start": {
                    "x": "0x0000000000000000",
                    "y": "0x0000000000000000",
                    "z": "0x4024000000000000",
                },
                "end": {
                    "x": "0x3ff0000000000000",
                    "y": "0x0000000000000000",
                    "z": "0x4026000000000000",
                },
                "line_id": "0x00000007",
            }
        ],
        "options": {"node_input": False},
        "versions": {"geo-polygonize-core": "0.76.2"},
        "baseline": {
            "implementation": "one_shot",
            "outcome": {"status": "success", "value": {"polygons": []}},
        },
        "comparison": {
            "implementation": "workspace",
            "outcome": {"status": "error", "value": {"kind": "topology"}},
        },
    }


class ReviewedAdmissionTests(unittest.TestCase):
    def test_review_preserves_the_exact_candidate_without_rewriting_it(self):
        source = candidate()
        fixture = reviewed_fixture(source, "adapter-mixed-v2", "invalid_ambiguous")

        self.assertIs(fixture["candidate"], source)
        self.assertEqual(fixture["schema_version"], 2)
        self.assertEqual(fixture["case_id"], "adapter-mixed-v2")
        self.assertEqual(fixture["classification"], "invalid_ambiguous")
        self.assertEqual(fixture["candidate"]["input"][0]["line_id"], "0x00000007")
        self.assertEqual(
            fixture["candidate"]["input"][0]["start"]["z"],
            "0x4024000000000000",
        )

    def test_review_rejects_unknown_producers_and_matching_outcomes(self):
        unknown = candidate()
        unknown["producer"] = "benchmark_record"
        with self.assertRaisesRegex(ValueError, "unknown producer"):
            reviewed_fixture(unknown, "unknown", "expected_divergence")

        matching = candidate()
        matching["comparison"]["outcome"] = matching["baseline"]["outcome"]
        with self.assertRaisesRegex(ValueError, "outcomes must differ"):
            reviewed_fixture(matching, "matching", "expected_parity")

    def test_admission_refuses_to_overwrite_a_fixture(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "case.json"
            write_fixture(path, {"schema_version": 2})
            with self.assertRaises(FileExistsError):
                write_fixture(path, {"schema_version": 2})
            self.assertEqual(json.loads(path.read_text()), {"schema_version": 2})


if __name__ == "__main__":
    unittest.main()
