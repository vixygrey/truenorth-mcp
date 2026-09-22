# Release note contract

Every tagged release needs a committed `v<version>.md` file in this directory. The release
workflow validates the file before it builds or stages any package.

Use this structure:

```md
# v1.2.3

## Highlights

- Describe a user-visible change.

## Migration

No migration required.
```

For a breaking change, replace `No migration required.` with command-first migration steps,
the replacement surface, and rollback guidance. The workflow adds GitHub-generated change notes
after this audited preamble.
