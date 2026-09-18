# Lessons

Durable lessons the project has learned. Each entry records what happened and the rule it
produced. Add a new lesson when a decision or a failure is worth carrying forward.

## Methodology

- The `.agent/` layout contract requires the config, spec, tasks, memories, and telemetry
  areas with their files. The runtime validates the contract at startup and keeps serving
  on the last valid state, so an incomplete tree fails quietly. Keep the contract complete.
- The ontology feature is disabled for this project. The two ontology tools and the
  ontology resource are absent. Do not reference them in a workflow here.
