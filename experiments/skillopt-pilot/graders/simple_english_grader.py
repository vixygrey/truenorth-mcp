#!/usr/bin/env python3
"""Objective grader for the simple-english skill (issue #259).

The grader wraps the simple-english skill's own ASD-STE100 lint,
`skills/simple-english/scripts/ste_lint.py`, and scores a candidate rewrite by its violation
density. It reuses the skill's gate as the grader, so the pilot measures the exact standard
the skill claims to enforce.

The score is in [0, 1]. It is `1.0` at zero violations and decays with the violations per
100 words:

    score = max(0.0, 1.0 - violations_per_100w / DECAY)

with DECAY chosen so a text at the decay rate scores 0. The lint runs with `--no-vale`, so
the grader needs no external Vale install and stays deterministic. It makes no model calls.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

# The violations-per-100-words rate at which the score reaches 0. A clean rewrite scores 1.0;
# a text with this density or worse scores 0.0. Ten per 100 words is a heavy-slop threshold.
DECAY = 10.0

# The skill's own lint, resolved relative to the repo root (two levels above this file's dir).
REPO_ROOT = Path(__file__).resolve().parents[3]
STE_LINT = REPO_ROOT / "skills" / "simple-english" / "scripts" / "ste_lint.py"


def _run_ste_lint(text: str, text_type: str) -> dict:
    """Run the skill's STE lint over `text` and return its JSON report."""
    proc = subprocess.run(
        [
            sys.executable,
            str(STE_LINT),
            "-",
            "--type",
            text_type,
            "--json",
            "--no-vale",
        ],
        input=text,
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode not in (0, 1):
        # The lint exits 0 (clean) or 1 (violations). Any other code is a harness failure.
        raise RuntimeError(
            f"ste_lint.py failed (exit {proc.returncode}): {proc.stderr.strip()}"
        )
    return json.loads(proc.stdout)


def score_candidate(candidate: str, task: dict) -> float:
    """The public grader entry point SkillOpt calls: a float score in [0, 1].

    The task's `text_type` selects the procedural or descriptive rule set. It defaults to
    descriptive, the more common case for the pilot fixtures.
    """
    text_type = task.get("text_type", "descriptive")
    if not candidate.strip():
        return 0.0
    report = _run_ste_lint(candidate, text_type)
    per_100w = float(report.get("violations_per_100w", 0.0))
    return max(0.0, 1.0 - per_100w / DECAY)


def _self_test() -> int:
    """Deterministic assertions against the real STE lint, no API key required."""
    if not STE_LINT.is_file():
        print(f"simple-english grader self-test: FAIL (missing {STE_LINT})")
        return 1

    clean = "Adjust the temperature. Then close the valve. The system is ready."
    slop = (
        "The temperature must be adjusted and the migration has completed; "
        "it's basically going to seamlessly leverage the robust pipeline, e.g. the queue."
    )

    clean_score = score_candidate(clean, {"text_type": "descriptive"})
    slop_score = score_candidate(slop, {"text_type": "descriptive"})
    empty_score = score_candidate("", {"text_type": "descriptive"})

    ok = True
    checks = [
        ("clean text scores high", clean_score >= 0.8),
        ("slop text scores lower than clean", slop_score < clean_score),
        ("empty text scores 0", empty_score == 0.0),
        ("scores stay in [0, 1]", all(0.0 <= s <= 1.0 for s in (clean_score, slop_score))),
    ]
    for name, passed in checks:
        flag = "ok " if passed else "BAD"
        if not passed:
            ok = False
        print(f"  {flag} {name}")
    print(f"  clean={clean_score:.3f} slop={slop_score:.3f} empty={empty_score:.3f}")
    print("simple-english grader self-test:", "PASS" if ok else "FAIL")
    return 0 if ok else 1


def main() -> int:
    parser = argparse.ArgumentParser(description="Grade a simple-english candidate.")
    parser.add_argument("--self-test", action="store_true", help="run deterministic assertions")
    parser.add_argument("--task", help="path to a task JSON file")
    parser.add_argument("--candidate", help="candidate text, or omit to read from stdin")
    args = parser.parse_args()

    if args.self_test:
        return _self_test()

    task = {"text_type": "descriptive"}
    if args.task:
        with open(args.task, encoding="utf-8") as fh:
            task = json.load(fh)
    candidate = args.candidate if args.candidate is not None else sys.stdin.read()
    print(json.dumps({"score": score_candidate(candidate, task)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
