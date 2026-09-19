#!/usr/bin/env python3
"""Objective grader for the commit-message skill (issue #259).

The grader scores a candidate commit message against a task. A task carries a diff summary
and the expected Conventional Commits type and SemVer bump. The score is in [0, 1] and is
the mean of three deterministic checks:

  1. format:  the subject matches the Conventional Commits pattern `type(scope): description`.
  2. type:    the subject's type equals the task's expected type.
  3. bump:    the message states the expected SemVer bump (major, minor, or patch).

The grader is deterministic. It makes no model calls, so its self-test runs with no API key.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass

# The Conventional Commits type set (mirrors the commit-message skill and the commit-msg hook).
CONVENTIONAL_TYPES = (
    "feat",
    "fix",
    "docs",
    "style",
    "refactor",
    "perf",
    "test",
    "build",
    "ci",
    "chore",
    "revert",
)

# `type(optional-scope)!: description`. The `!` marks a breaking change.
SUBJECT_RE = re.compile(
    r"^(?P<type>" + "|".join(CONVENTIONAL_TYPES) + r")"
    r"(?:\([a-z0-9._-]+\))?"
    r"(?P<breaking>!)?: .+"
)

# The type-to-bump mapping from Conventional Commits and SemVer, absent a breaking change.
TYPE_TO_BUMP = {"feat": "minor", "fix": "patch"}
BUMP_WORDS = ("major", "minor", "patch")


@dataclass
class TaskResult:
    """The per-check outcome for one candidate against one task."""

    format_ok: bool
    type_ok: bool
    bump_ok: bool

    @property
    def score(self) -> float:
        """The mean of the three checks, in [0, 1]."""
        return (int(self.format_ok) + int(self.type_ok) + int(self.bump_ok)) / 3.0


def expected_bump(task: dict) -> str:
    """Resolve the expected SemVer bump for a task.

    A breaking change is a major bump. Otherwise the expected type maps through
    TYPE_TO_BUMP, defaulting to `none` for a type that implies no release bump.
    """
    if task.get("breaking"):
        return "major"
    return TYPE_TO_BUMP.get(task["expected_type"], "none")


def grade(candidate: str, task: dict) -> TaskResult:
    """Score a candidate commit message against a task."""
    subject = candidate.strip().splitlines()[0] if candidate.strip() else ""
    match = SUBJECT_RE.match(subject)

    format_ok = match is not None
    type_ok = bool(match) and match.group("type") == task["expected_type"]

    want_bump = expected_bump(task)
    if want_bump == "none":
        # A no-bump type states no bump word, or explicitly says the bump is none.
        bump_ok = not _mentions_any_bump(candidate) or "none" in candidate.lower()
    elif want_bump == "major":
        # A breaking change is a major bump, signaled by the word or a `!`/`BREAKING CHANGE`.
        bump_ok = (
            "major" in candidate.lower()
            or "breaking change" in candidate.lower()
            or (match is not None and match.group("breaking") == "!")
        )
    else:
        bump_ok = want_bump in candidate.lower()

    return TaskResult(format_ok=format_ok, type_ok=type_ok, bump_ok=bump_ok)


def _mentions_any_bump(text: str) -> bool:
    low = text.lower()
    return any(w in low for w in BUMP_WORDS)


def score_candidate(candidate: str, task: dict) -> float:
    """The public grader entry point SkillOpt calls: a float score in [0, 1]."""
    return grade(candidate, task).score


def _self_test() -> int:
    """Deterministic assertions, no API key required."""
    feat_task = {"expected_type": "feat", "breaking": False}
    fix_task = {"expected_type": "fix", "breaking": False}
    docs_task = {"expected_type": "docs", "breaking": False}
    breaking_task = {"expected_type": "feat", "breaking": True}

    cases = [
        # (candidate, task, expected score)
        ("feat(api): add pagination\n\nThis is a minor bump.", feat_task, 1.0),
        ("fix(auth): correct token refresh\n\npatch", fix_task, 1.0),
        ("docs(readme): expand the install section", docs_task, 1.0),
        ("feat(api)!: drop the v1 endpoint\n\nBREAKING CHANGE: removes v1.", breaking_task, 1.0),
        # Wrong type: format ok, type wrong, bump word still minor so bump ok.
        ("fix(api): add pagination\n\nminor", feat_task, 2 / 3),
        # Not Conventional at all: all three fail.
        ("added some stuff", feat_task, 0.0),
        # Right type and format, missing bump statement.
        ("feat(api): add pagination", feat_task, 2 / 3),
    ]
    ok = True
    for candidate, task, want in cases:
        got = score_candidate(candidate, task)
        flag = "ok " if abs(got - want) < 1e-9 else "BAD"
        if flag == "BAD":
            ok = False
        print(f"  {flag} score={got:.3f} want={want:.3f}  {candidate!r}")
    print("commit-message grader self-test:", "PASS" if ok else "FAIL")
    return 0 if ok else 1


def main() -> int:
    parser = argparse.ArgumentParser(description="Grade a commit-message candidate.")
    parser.add_argument("--self-test", action="store_true", help="run deterministic assertions")
    parser.add_argument("--task", help="path to a task JSON file")
    parser.add_argument(
        "--candidate", help="candidate text, or omit to read from stdin"
    )
    args = parser.parse_args()

    if args.self_test:
        return _self_test()

    if not args.task:
        parser.error("--task is required unless --self-test is set")
    with open(args.task, encoding="utf-8") as fh:
        task = json.load(fh)
    candidate = args.candidate if args.candidate is not None else sys.stdin.read()
    print(json.dumps({"score": score_candidate(candidate, task)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
