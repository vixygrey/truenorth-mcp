# SkillOpt environment adapters for the pilot

SkillOpt runs a benchmark through an environment package under `skillopt/envs/<name>/` in
the SkillOpt install. Each package provides an adapter, a data loader, a scored rollout
helper, a YAML config, and a seed skill (see the SkillOpt "Add a New Benchmark" guide).

This directory does not vendor SkillOpt code. It documents how the pilot's fixtures and
graders map onto that contract, so the adapter is a thin wrapper the maintainer drops into
their SkillOpt checkout at run time.

## The contract, mapped

For each pilot skill (`commit_message`, `simple_english`), the adapter wires four things:

1. **The data loader** reads the split files under `tasks/<skill>/{train,val,test}.json`.
   Each item has an `id`, a `prompt` (the text the target model sees), and the grading
   metadata the grader needs. The config points `env.split_dir` at the task directory.

2. **The rollout** sends the item's `prompt` to the target model with the skill under test
   loaded, and captures the model's text output as the candidate.

3. **The scored grader** calls the matching grader in `../graders/` and returns its
   `score` in `[0, 1]`:
   - `commit_message` uses `graders/commit_message_grader.py`, `score_candidate(candidate, task)`.
   - `simple_english` uses `graders/simple_english_grader.py`, `score_candidate(candidate, task)`.
     Both accept the task dict verbatim from the split file, so no field remapping is needed.

4. **The seed skill** is the current hand-authored `SKILL.md`:
   - `commit_message`: `skills/commit-message/SKILL.md`.
   - `simple_english`: `skills/simple-english/SKILL.md`.
     SkillOpt trains from this seed, so the baseline is the real skill, and the measured lift
     is against the document we ship today.

## The grading call

Both graders expose the same Python entry point, so the adapter's scored rollout helper is
one shared shape:

```python
from graders.commit_message_grader import score_candidate  # or simple_english_grader
score = score_candidate(model_output_text, task_dict)       # float in [0, 1]
```

The graders are deterministic and make no model calls, so grading adds no inference cost
and no non-determinism to the rollout score.

## Why the adapter is not committed here

The adapter imports SkillOpt's environment base classes, which exist only inside the
SkillOpt install and evolve with its version. Committing a copy here would pin a stale
copy of someone else's interface. The runbook gives the exact steps to create the adapter
in the SkillOpt checkout from this mapping.
