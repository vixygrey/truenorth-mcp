# Methodology profiles

A methodology profile declares the project workflow shape: the grouping vocabulary, whether
grouping is required, the branch pattern, and whether a commit needs an id reference. The
profiles are fixed data, so a custom profile cannot be defined (ADR-0009).

## The five profiles

| Profile           | Grouping  | Grouping rule | Requires an id |
| ----------------- | --------- | ------------- | -------------- |
| `issue-per-task`  | ticket    | optional      | yes            |
| `epic-based`      | epic      | required      | yes            |
| `milestone-based` | milestone | required      | yes            |
| `kanban`          | none      | optional      | no             |
| `generic`         | none      | optional      | no             |

`issue-per-task` is the default. An absent `.agent/profile.yml` resolves to it.

## Select the profile

`.agent/profile.yml` names the active profile:

```yaml
profile: issue-per-task
```

The runtime reads the name once at startup. An unknown name returns an error that names the
value and the five valid names, and makes no partial change.

## The grouping key

A task carries a neutral grouping key, not a mandatory epic id. `truenorth_record_task`
takes an optional `group_id` with an optional `group_kind` (epic, sprint, milestone, or
ticket). A legacy `epic_id` still maps to a group with `group_kind` set to epic.

Whether the grouping key is required depends on the active profile. `epic-based` and
`milestone-based` require it. `issue-per-task`, `kanban`, and `generic` accept a task
without it.

## Scaffold a profile

`truenorth_scaffold_project` takes a profile name and seeds the `.agent/` tree, the git
hooks, and the `.github/` issue templates for that profile. An absent name uses
`issue-per-task`. The issue templates carry a grouping field only when the profile groups
by epic or milestone, and omit the id field when the profile requires no id.

See [The lifecycle](The-lifecycle) for recording and advancing work under a profile.
