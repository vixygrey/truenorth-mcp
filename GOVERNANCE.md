# Governance

This policy defines how TrueNorth-MCP merges changes and authorizes releases during
its solo-maintainer phase.

## Merge controls

Every change uses a pull request and a short-lived branch. Direct pushes, force
pushes, and branch deletion are disabled for `main`. `CI Gate` is required before
merge. Pull-request conversations must be resolved.

The repository owner can merge an owner-authored pull request after those checks
pass. This keeps normal solo development unblocked.

An external pull request must also pass the `External contribution review` check.
That check requires a current-head approval from an allowed maintainer. A later
push resets the requirement. The workflow runs only trusted code from the default
branch and never checks out pull-request code.

`CODEOWNERS` requests the maintainer review for every path. The external-review
check is the enforceable merge condition while the repository remains
owner-operated.

## Release controls

A release starts only from a protected `v*` tag that resolves to the current
`main` commit. The release workflow verifies release metadata, builds all native
packages, and smoke-tests the staged binary and packed wrapper before it can
request publication.

One `release-publication` GitHub Environment approval authorizes all npm staging
jobs. The publication jobs run only after that approval. npm trusted publishing
then stages packages through OIDC; npm maintainer 2FA remains the final registry
approval.

The current owner can approve a routine release. This is an explicit recorded
release action, separate from tag creation. When a second trusted maintainer is
available, configure that maintainer or the `release-maintainers` team as the
required Environment reviewer and prevent self-review.

## Emergency authority

The owner may bypass the `release-publication` Environment only for an active
security or availability incident. A merge emergency requires an intentional
temporary protection change; the `main` branch does not grant an administrator
bypass. Record the reason, affected version, and timestamp in the associated
private advisory, incident issue, or release issue. Complete normal review and
follow-up fixes after the incident is stabilized.

Repository owners can change repository settings. These controls prevent
accidental bypass and make an intentional exception visible; they do not protect
against a malicious repository owner.

## Organization migration

Migrate the repository to an organization when a second maintainer needs regular
merge or release authority. Replace the solo external-review workflow with a
team-backed code-owner ruleset, require independent release approval, and narrow
bypass access to named emergency actors.
