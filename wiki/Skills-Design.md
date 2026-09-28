# Skills: Design

The Design phase turns a clear spec into a sound model and interface before code. The
skills here sharpen the domain language, stress-test the plan, find architectural
deepening opportunities, and explore interface options.

For the full arc across phases, see [The skill workflow](The-skill-workflow). For the
alphabetical list, see [Skill index](Skill-index).

Three of these skills are easy to confuse, so the distinction is stated up front:

- `define-language` produces a canonical glossary of terms.
- `model-domain` stress-tests a plan through a domain-model interview and captures
  invariants.
- `deepen-architecture` finds module-level refactoring opportunities in the code.

---

## define-language

Extract a DDD-style ubiquitous-language glossary from the conversation.

- **What it does**: scans the conversation for domain nouns, verbs, and concepts;
  identifies ambiguity (one word, many concepts) and synonyms (many words, one concept);
  proposes opinionated canonical terms; and writes the project glossary with tables,
  relationships, an example dialogue, and flagged ambiguities.
- **When to use it**: to define domain terms, build a glossary, harden terminology, or
  create a ubiquitous language.
- **Inputs**: the current conversation.
- **Outputs**: the project glossary.
- **Hard gate**: ubiquitous language is not optional. Every term that could be
  misunderstood must be glossed, because ambiguity means rework.
- **Re-running**: on a second invocation it reads the existing glossary, incorporates new
  terms, updates definitions, and re-flags ambiguities.

## model-domain

A grilling session that challenges a plan against the domain model and captures the
invariants and state machines for core entities.

- **What it does**: interviews relentlessly, one question at a time, resolving domain
  decisions. It challenges terms against the glossary, sharpens fuzzy language, stress-
  tests relationships with concrete scenarios, cross-references the code, and updates the
  tech-stack note and ADRs inline. It includes a concurrency-safety audit for shared
  state.
- **When to use it**: to stress-test a plan against the project domain language and the
  documented decisions.
- **Inputs**: the plan, the glossary, the tech-stack note, and the ADRs.
- **Outputs**: updated tech-stack note and ADRs, captured invariants and state machines.
- **Hard gate**: capture the invariants (what must always be true) and the legal state
  transitions for core entities. Fuzzy invariants mean a failed design.
- **Feeds the ontology**: when the ontology feature is enabled, the canonical terms and
  invariants seed the ontology through `truenorth_generate_ontology`, then
  `truenorth_verify_ontology` enforces them. When the feature is off, those tools are
  absent and the domain work stands on its own. See
  [The ontology feature](The-ontology-feature).

## grill-me

Interactive assumption-surfacing Q&A that stress-tests a plan through relentless
questioning until every decision is resolved. It owns two explicit modes.

- **Context-only mode** is the default. It uses the conversation, plan, repository, and
  local artifacts without fetching external documentation.
- **Docs mode** applies when the plan depends on an external library, framework, or API.
  Every external factual challenge cites an official source URL and quotes the relevant
  API detail, such as a signature, parameter, version, deprecation, quota, timeout, or
  documented error.
- **What it does**: walks the design tree one decision at a time and provides a
  recommended answer. Docs mode validates plan assumptions against official,
  version-matched sources and records each as confirmed, corrected, or unresolved.
- **When to use it**: use context-only mode to challenge a plan from conversation and
  repository evidence. Use docs mode for “grill me with docs” or whenever correctness
  depends on external behavior.
- **Inputs**: the plan and repository; in docs mode, official library or API
  documentation.
- **Outputs**: a stress-tested plan with resolved tensions. Docs mode also records source
  URLs, quoted details, verdicts, and required plan corrections.
- **Facts versus decisions**: the agent researches discoverable facts. It asks the user
  only for judgment about tradeoffs, preferences, or priorities.
- **Hard gates**: do not accept a design until every hard decision is stress-tested.
  Docs mode rejects uncited or unquoted external claims. Do not enact the plan until the
  user explicitly confirms shared understanding.

## deepen-architecture

Surface architectural friction and propose deepening opportunities: refactors that turn a
shallow module into a deep one. It is informed by "A Philosophy of Software Design".

- **What it does**: reads the tech-architecture notes and ADRs, ranks candidate modules by
  churn, walks the codebase for friction, scores each candidate for module depth (1 to 5),
  presents numbered deepening opportunities, then drops into a grilling loop on the chosen
  candidate. It keeps the import-boundary allowlist current.
- **When to use it**: to improve architecture, find refactoring opportunities, consolidate
  tightly-coupled modules, or make a codebase more testable.
- **Inputs**: the tech-architecture notes, the ADRs, and the codebase.
- **Outputs**: scored deepening candidates and a chosen deepening plan.
- **Hard gate**: a deep module must solve a forcing function, not just be a nice
  abstraction. When you cannot articulate why the abstraction exists, it is premature.
- **Key tools**: the deletion test (delete the module; if complexity reappears across
  callers, it earns its keep) and the two-adapters rule (one adapter is hypothetical, two
  is a real seam).

## design-interface

Generate multiple radically different interface designs for a module, then compare the
trade-offs. Based on "Design It Twice".

- **What it does**: gathers requirements, spawns three or more parallel sub-agents each
  under a different constraint (minimize methods, maximize flexibility, optimize the common
  case, borrow a paradigm), presents each design with its signature, usage, and hidden
  complexity, then compares them on simplicity, generality, efficiency, and depth, and
  synthesizes.
- **When to use it**: to design an API, explore interface options, or compare module
  shapes.
- **Inputs**: the module requirements and its callers.
- **Outputs**: several contrasting interface designs and a synthesized recommendation.
- **Hard gate**: multiple options must be explored. Do not settle on the first idea, and
  do not implement here. This is purely interface shape.
