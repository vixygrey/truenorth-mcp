# Troubleshooting

This page covers the common startup problems and their fixes.

## The server cannot resolve the repository root

The runtime resolves the repository root by looking for a directory that contains
`.agent/`, `specs/`, and `skills/`. A run from a subdirectory, or from a directory without
all three markers, fails to resolve the root and exits with an error like this:

```text
no valid repository root among the evaluated candidates. A valid root must directly
contain a `.agent/`, a `specs/`, and a `skills/` directory. Set the `TRUENORTH_ROOT`
environment variable to the repository root, or run the server from inside the repository.
```

To fix it, do one of the following:

- Run the client from the repository root.
- Set `TRUENORTH_ROOT` to the repository root in the client config.

```json
{
  "mcpServers": {
    "truenorth": {
      "command": "npx",
      "args": ["truenorth-mcp"],
      "env": { "TRUENORTH_ROOT": "/absolute/path/to/repo" }
    }
  }
}
```

## The layout contract is incomplete

At startup the runtime validates the `.agent/` layout contract. A missing required entry
logs a warning that names the absent path, and the runtime keeps serving on the last valid
state rather than crashing.

To fix it, add the named entry. The full set of required entries is on
[The .agent workspace](The-agent-workspace). For a new project, run
`truenorth_scaffold_project` to seed the complete tree.

## An ontology tool or resource is missing

When the ontology feature is disabled, the two ontology tools and the `truenorth://ontology`
resource are absent by design. A read of the ontology URI is an unknown resource. This is
not a fault. To enable the surface, set `features.ontology: true` in
`.agent/config/rules.yml`. See [The ontology feature](The-ontology-feature).

## A resource read returns an error

A parse or validation failure on a backing file returns a resource read error that names
the file and the cause. The runtime retains the last good content and keeps serving the
other resources. Fix the YAML in the named file. The watcher revalidates on the next edit.

## A verify gate fails with an allowlist rejection

`truenorth_verify_gate` in execute mode runs the command in a sandbox with a command
allowlist on the first token. A command whose binary is not on the allowlist is rejected
with a hint. Add the binary to the allowlist, or run the gate in `evidence` mode. See
[The lifecycle](The-lifecycle).
