import json
import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).parent))
from assert_defect import assert_defect


def result(*roots, verdict="failed", errors=0):
    return {
        "verdict": verdict,
        "failed": 1 if verdict == "failed" else 0,
        "errors": errors,
        "cases": [
            {
                "verdict": verdict,
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
    contract_path = Path(__file__).parents[1] / "audit.contract.json"
    contract = json.loads(contract_path.read_text())

    def test_exact_difference_matches(self):
        for defect, expected in self.contract["defects"].items():
            with self.subTest(defect=defect):
                roots = [root.removeprefix("views.") for root in expected]
                assert_defect(self.contract, result(*roots), defect)

    def test_contract_describes_the_parent_repository_checks(self):
        self.assertEqual(self.contract["schema"], 2)
        self.assertTrue(self.contract["live"])
        self.assertTrue(self.contract["offline"])
        self.assertEqual(set(self.contract["views"]), set(self.contract["proofs"]))

    def test_execution_view_path_matches(self):
        changed = result()
        changed["cases"][0]["differences"] = [
            {
                "surface": "observations",
                "path": "/steps/exercise/effects/postgres/delta/added/0",
            }
        ]
        assert_defect(self.contract, changed, "split-reservation")

    def test_failed_assertions_are_not_behavior_roots(self):
        changed = result("postgres")
        changed["cases"][0]["differences"].append(
            {"surface": "assertions", "path": "/2"}
        )
        assert_defect(self.contract, changed, "split-reservation")

    def test_unrelated_difference_fails(self):
        with self.assertRaisesRegex(ValueError, "defect differences differ"):
            assert_defect(self.contract, result("result"), "split-reservation")

    def test_expected_pass_text_cannot_hide_an_unrelated_difference(self):
        changed = result("result")
        changed["display"] = "PASS views.postgres"
        with self.assertRaisesRegex(ValueError, "defect differences differ"):
            assert_defect(self.contract, changed, "split-reservation")

    def test_execution_error_fails(self):
        with self.assertRaisesRegex(ValueError, "execution error"):
            assert_defect(
                self.contract,
                result("postgres", errors=1),
                "split-reservation",
            )


if __name__ == "__main__":
    unittest.main()
