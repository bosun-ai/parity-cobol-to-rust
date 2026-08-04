import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).parent))
from assert_defect import assert_defect


def proof(*roots, verdict="failed", error=None):
    return {
        "cases": [
            {
                "verdict": verdict,
                "legacy": {"error": error},
                "target": {"error": None},
                "differences": [
                    {
                        "surface": "observations",
                        "path": f"/views/{root}/added/0",
                    }
                    for root in roots
                ],
            }
        ]
    }


class DefectContractTest(unittest.TestCase):
    contract = {"defects": {"changed": ["views.postgres"]}}

    def test_exact_difference_matches(self):
        assert_defect(self.contract, proof("postgres"), "changed")

    def test_unrelated_difference_fails(self):
        with self.assertRaisesRegex(ValueError, "defect differences differ"):
            assert_defect(self.contract, proof("result"), "changed")

    def test_expected_pass_text_cannot_hide_an_unrelated_difference(self):
        changed = proof("result")
        changed["display"] = "PASS views.postgres"
        with self.assertRaisesRegex(ValueError, "defect differences differ"):
            assert_defect(self.contract, changed, "changed")

    def test_execution_error_fails(self):
        with self.assertRaisesRegex(ValueError, "execution error"):
            assert_defect(self.contract, proof("postgres", error="broken"), "changed")


if __name__ == "__main__":
    unittest.main()
