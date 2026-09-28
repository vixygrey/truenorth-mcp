# Stocktake checklist

## Structural contract

- [ ] The skill appears in `index_skills` and exists at `skills/<name>/SKILL.md`.
- [ ] `validate_skill` passes, including naming, frontmatter, kind, size, and links.
- [ ] A non-verb-noun name has a nonempty `name_exception`.
- [ ] A scripted skill has a nonempty frontmatter `verify` command.
- [ ] A prose skill is not assigned a synthetic command.

## Supplemental audit

- [ ] The description states the capability and "Use it ..." triggers.
- [ ] HARD GATE callouts match real blocking conditions.
- [ ] Handoffs, lifecycle placement, output paths, and cockpit paths are current.
- [ ] The body follows the house writing rules.
- [ ] `--verify` mode ran every scripted command and recorded its exit result.
- [ ] The source skill and packaged mirror match.

## Usage evidence

- [ ] A full report has a usage section separate from structural and semantic findings.
- [ ] Disabled collection is recorded as `status: unavailable`, with the reason,
      source, observation window, and retention set explicitly.
- [ ] Missing or legacy metrics are not converted into zero-use, ranking, or timing claims.
- [ ] The report stores no raw prompts, repository file contents, secrets, or user identifiers.
