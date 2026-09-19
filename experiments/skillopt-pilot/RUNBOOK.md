# SkillOpt pilot runbook (issue #259)

This runbook executes the pilot. It makes paid model API calls, so the maintainer runs it
with their own credentials and budget. The build environment does not run it.

> **CAUTION: every step under "Run the optimizer" makes billed API calls.** Start with the
> small defaults in the configs. Do not raise `train.num_epochs` or the split sizes until a
> first run confirms the cost per run.

## Prerequisites

1. Python 3.10 or later.
2. The SkillOpt package: `pip install skillopt`.
3. An OpenAI API key for the default `openai_chat` backend.

## Set the credential (never commit it)

The key is read from an environment variable at run time. Do not put it in a config file
or commit it.

```bash
export OPENAI_API_KEY="sk-..."   # your key, in the shell only
```

The config files name the model as `REPLACE_WITH_YOUR_OPENAI_MODEL`. Edit both configs to a
model your account can access, or override on the command line with `--cfg-options`.

### Alternative backends

- **Claude Code CLI (`claude_chat`).** Install and authenticate the Claude Code CLI, then
  set both `*_backend` fields in the config to `claude_chat`. The key authenticates the CLI,
  not SkillOpt.
- **OpenAI-compatible endpoint (`openai_compatible`).** Set `OPENAI_COMPATIBLE_BASE_URL` and
  `OPENAI_COMPATIBLE_API_KEY`, and switch both `*_backend` fields to `openai_compatible`. A
  raw Claude key works here against Anthropic's OpenAI-compatible endpoint.

## Create the environment adapters

Follow `envs/README.md` to add a thin `skillopt/envs/commit_message/` and
`skillopt/envs/simple_english/` adapter in your SkillOpt checkout. Each adapter loads the
split files under `tasks/<skill>/`, runs the rollout, and scores it with the matching
grader in `graders/`. The seed skill is the current `SKILL.md` for each skill.

## Verify the graders first (no API key, free)

```bash
python3 graders/commit_message_grader.py --self-test
python3 graders/simple_english_grader.py --self-test
```

Both must print PASS before spending on a run.

## Run the optimizer

Run one skill at a time. Record the wall-clock time and the token cost the run reports.

```bash
skillopt-train --config experiments/skillopt-pilot/configs/commit-message.yaml
skillopt-train --config experiments/skillopt-pilot/configs/simple-english.yaml
```

Each run trains from the seed skill and, with `evaluation.use_gate: true`, accepts an edit
only when it improves the held-out validation score. The run writes a `best_skill.md` and a
test-split score.

## Read the result

For each skill, compare two test-split scores:

1. The **baseline**: the seed `SKILL.md` scored on the test split.
2. The **optimized**: the `best_skill.md` scored on the test split.

The lift is `optimized - baseline`. Record both scores, the lift, the wall-clock time, and
the token cost in `RESULTS.md` (create it from the template below).

## Success criterion (from issue #259)

The pilot succeeds if an optimized skill shows a clear held-out lift over the hand-authored
skill on at least one of the two skills, and the optimized document still passes the
mechanical lints and the house writing rules. A flat or negative result is a valid outcome:
it says SkillOpt does not pay for the benchmark cost here.

## Check the optimized skill against the existing gates

Before trusting an optimized `best_skill.md`, run it through the mechanical lints the
project already ships, so an optimizer edit did not introduce a dangling path or handoff:

```bash
bash scripts/lint-skill-paths.sh
bash scripts/lint-skill-handoffs.sh
bash scripts/lint-skill-artifacts.sh
```

Also run the simple-english STE lint over the optimized `simple-english` skill body, since
that skill must obey the standard it enforces.

## RESULTS.md template

```markdown
# SkillOpt pilot results (#259)

Backend: openai_chat Model: <model> Date: <date>

| Skill          | Baseline | Optimized | Lift | Wall-clock | Token cost |
| -------------- | -------- | --------- | ---- | ---------- | ---------- |
| commit-message |          |           |      |            |            |
| simple-english |          |           |      |            |            |

Go or no-go: <decision>
Notes: <observations, including any lint or writing-rule regressions in best_skill.md>
```
