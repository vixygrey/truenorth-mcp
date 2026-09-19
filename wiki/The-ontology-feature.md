# The ontology feature

The ontology surface is a per-project choice. A single flag gates the two ontology tools
and the ontology resource. The default is enabled (ADR-0012).

## The flag

Set `features.ontology` in `.agent/config/rules.yml`:

```yaml
features:
  ontology: false
```

The runtime resolves the flag once at startup. An absent `.agent/`, an absent `config/`, or
an absent `rules.yml` all resolve to enabled. A present file with no `features` block or no
`ontology` key resolves to enabled. A present-but-broken file returns a typed error that
names the path.

## The enabled surface

When the feature is enabled, the runtime serves:

- `truenorth_generate_ontology`: seed `.agent/ontology.yml` from sources.
- `truenorth_verify_ontology`: scan changed code against the ontology and reject a
  prohibited alias or a constraint violation, citing the constraint id.
- The `truenorth://ontology` resource, backed by `.agent/ontology.yml`.

The scan baseline is a language-agnostic regex scan, with an optional tree-sitter AST
plug-in for a supported language (ADR-2).

## The disabled surface

When the feature is disabled:

- Neither ontology tool is registered, so neither is advertised or callable.
- `truenorth://ontology` is not listed, and a read of its URI is an unknown resource.
- No `.agent/ontology.yml` file is seeded.

Every other tool and resource is unchanged. To confirm the disabled surface, list the tools
and the resources from your client: no tool name contains `ontology`, and the resource list
omits `truenorth://ontology`.

## The backing path

The ontology tools and the resource read and write one backing path, `.agent/ontology.yml`,
through the write guard. A legacy `specs/ontology.yaml` is read as a fallback when the
primary file is absent, but a write never mutates the legacy file (ADR-0013).
