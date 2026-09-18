# Glossary

The working glossary for the agent memories area. The full domain vocabulary, with
definitions and related terms, lives at `.agent/product/glossary.yml`. This file holds
short working notes and any term the memories area needs that the product glossary does
not yet carry.

- **cockpit**: the runtime-written state under `.agent/tasks/`.
- **write guard**: the single guarded write path, `write_under_agent`, under `.agent/`.
- **methodology profile**: the workflow shape resolved from `.agent/profile.yml`. This
  project uses `issue-per-task`.
- **feature flag**: the per-project `features.ontology` boolean in
  `.agent/config/rules.yml`. This project sets it to `false`.

See `.agent/product/glossary.yml` for the full term set.
