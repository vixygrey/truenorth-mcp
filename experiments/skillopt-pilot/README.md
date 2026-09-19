# SkillOpt pilot (issue #259)

This directory stages a scoped SkillOpt pilot. The pilot measures whether an optimized
skill beats the hand-authored skill on a held-out set, on two skills with objective
graders. It answers one question and returns a go or no-go, per issue #259.

> **CAUTION: the optimizer run makes paid model API calls.** Do not run
> `skillopt-train` without an intended inference budget. Every rollout is a billed call.
> This directory stages the run. The maintainer executes it with their own credentials.

## What is here

- `graders/commit_message_grader.py`: an objective grader for the `commit-message` skill.
  It scores a candidate commit message against a fixed diff: Conventional Commits format
  and the correct SemVer bump.
- `graders/simple_english_grader.py`: an objective grader for the `simple-english` skill.
  It wraps the skill's own `scripts/ste_lint.py` and scores by the ASD-STE100 violation
  count.
- `tasks/commit-message/`: the scored task fixtures (train, validation, test).
- `tasks/simple-english/`: the scored task fixtures (train, validation, test).
- `envs/`: notes on the SkillOpt environment adapter each skill needs.
- `configs/`: the SkillOpt YAML config, defaulted to the OpenAI `openai_chat` backend.
- `RUNBOOK.md`: the exact install and run steps, and the environment variable to set.

## The graders are testable without a model

Both graders are deterministic. They take a candidate text and return a score in `[0, 1]`.
Run their self-tests without any API key:

```bash
python3 experiments/skillopt-pilot/graders/commit_message_grader.py --self-test
python3 experiments/skillopt-pilot/graders/simple_english_grader.py --self-test
```

## Scope

The pilot covers two skills. Scaling to the full catalog waits on the pilot result. The
mechanical lints (issues #250, #253) stay the correctness floor regardless of the outcome.
