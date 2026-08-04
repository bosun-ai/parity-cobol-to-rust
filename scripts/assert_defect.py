#!/usr/bin/env python3
"""Require one example defect to fail at its reviewed proof views."""

import json
import pathlib
import sys


def difference_root(difference: dict) -> str:
    path = difference.get("path", "").strip("/").split("/")
    if len(path) >= 2 and path[0] == "views":
        return f"views.{path[1]}"
    if "effects" in path:
        effects = path.index("effects")
        if len(path) > effects + 2 and path[effects + 1] == "views":
            return f"views.{path[effects + 2]}"
    surface = difference.get("surface", "")
    return surface.split("/", 1)[0]


def assert_defect(contract: dict, result: dict, defect: str) -> None:
    expected = set(contract["defects"][defect])
    if result.get("verdict") != "failed" or result.get("failed") != 1:
        raise ValueError(
            f"expected one failed proof, observed {result.get('verdict')} "
            f"with {result.get('failed')} failures"
        )
    if result.get("errors") != 0:
        raise ValueError(f"proof ended with {result.get('errors')} execution errors")
    cases = result.get("cases", [])
    if len(cases) != 1:
        raise ValueError(f"expected one defect case, observed {len(cases)}")
    case = cases[0]
    if case.get("verdict") != "failed":
        raise ValueError(f"expected a failed proof, observed {case.get('verdict')}")
    observed = {difference_root(value) for value in case.get("differences", [])}
    if observed != expected:
        raise ValueError(
            f"defect differences differ: expected {sorted(expected)}, observed {sorted(observed)}"
        )


def main() -> None:
    defect = sys.argv[1]
    result = json.loads(pathlib.Path(sys.argv[2]).read_text())
    contract = json.loads(pathlib.Path(sys.argv[3]).read_text())
    assert_defect(contract, result, defect)
    print(f"Expected mismatch: {defect} ({', '.join(contract['defects'][defect])})")


if __name__ == "__main__":
    try:
        main()
    except (KeyError, OSError, ValueError) as error:
        raise SystemExit(str(error)) from error
