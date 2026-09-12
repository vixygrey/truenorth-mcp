---
name: deepen-architecture
description: 'Find deepening opportunities in a codebase, informed by the domain language in the tech-architecture notes and the decisions in the ADRs. Use it to improve architecture, find refactoring opportunities, consolidate tightly-coupled modules, or make a codebase more testable and navigable.'
---

# Deepen Architecture

Surface architectural friction and propose deepening opportunities: refactors that
turn a shallow module into a deep one. The aim is testability and navigability.

Distinct from `define-language` and `model-domain`. Use this skill to find
module-level refactoring opportunities in the codebase. Use `define-language` to
produce a canonical glossary. Use `model-domain` to stress-test a plan through a
domain-model interview.

> **HARD GATE**: a deep module MUST solve a forcing function, not just be a nice abstraction. When you cannot articulate why the abstraction exists, it is premature.

## Glossary

Use these terms exactly in every suggestion. Consistent language is the point. Do
not drift into "component", "service", "API", or "boundary". Full definitions in
[LANGUAGE.md](LANGUAGE.md).

- **Module**: anything with an interface and an implementation (a function, class,
  package, or slice).
- **Interface**: everything a caller must know to use the module: the types,
  invariants, error modes, ordering, and config. Not just the type signature.
- **Implementation**: the code inside.
- **Depth**: leverage at the interface, a lot of behavior behind a small interface.
  Deep is high leverage. Shallow means the interface is nearly as complex as the
  implementation.
- **Seam**: where an interface lives, a place behavior can be altered without
  editing in place.
- **Adapter**: a concrete thing that satisfies an interface at a seam.
- **Leverage**: what callers get from depth.
- **Locality**: what maintainers get from depth, with change, bugs, and knowledge
  concentrated in one place.

Key principles (see [LANGUAGE.md](LANGUAGE.md) for the full list):

- **Deletion test**: imagine deleting the module. When complexity vanishes, it was
  a pass-through. When complexity reappears across N callers, it was earning its
  keep.
- The interface is the test surface.
- One adapter is a hypothetical seam. Two adapters is a real seam.

This skill is informed by the project domain model, the tech-architecture notes and
the ADRs. The domain language gives names to good seams. An ADR records a decision
the skill must not re-litigate.

## Process

### 1. Explore

Read the existing documentation first: the tech-architecture notes and the relevant
ADRs. When a file does not exist, proceed silently. Do not flag its absence.

Look here first (a churn heuristic): before organic exploration, rank the candidate
modules by recent commit frequency. A high-churn file is an architectural-friction
magnet. Start there. Use the git-context tool or `git log` to rank the recently
changed files.

Then walk the codebase. Explore organically and note where you experience friction.

- Where does understanding one concept require bouncing between many small modules?
- Where is a module shallow, the interface nearly as complex as the implementation?
- Where has a pure function been extracted just for testability, while the real bug
  hides in how it is called?
- Where do tightly-coupled modules leak across their seams?
- Which part of the codebase is untested, or hard to test through its current
  interface?

Apply the deletion test to anything you suspect is shallow.

### 2. Module-depth score

For each candidate, assign a module-depth score of 1 to 5.

| Score | Meaning                                                   |
| ----- | --------------------------------------------------------- |
| 1     | Shallow, interface complexity close to the implementation |
| 3     | Balanced                                                  |
| 5     | Deep, a small interface with substantial hidden behavior  |

Include the score in each candidate row. Prioritize a score of 2 or less for
deepening.

### 3. Present the candidates

Present a numbered list of deepening opportunities. For each candidate, give the
files involved, the problem (why the current architecture causes friction), the
solution (a plain-English description of the change), and the benefits (in terms of
locality and leverage, and how the tests improve).

Use the tech-architecture vocabulary for the domain and the [LANGUAGE.md](LANGUAGE.md)
vocabulary for the architecture.

When a candidate contradicts an existing ADR, surface it only when the friction is
real enough to warrant revisiting the ADR. Mark it clearly.

Do NOT propose an interface yet. Ask the user which candidate to explore.

### 4. Grilling loop

Once the user picks a candidate, drop into a grilling conversation. Walk the design
tree with them: the constraints, the dependencies, the shape of the deepened module,
what sits behind the seam, and what tests survive.

Side effects happen inline as decisions crystallize.

- Naming a deepened module after a concept not in the tech-architecture notes? Add
  the term there, the same discipline as `model-domain`.
- Sharpening a fuzzy term during the conversation? Update the notes right there.
- The user rejects the candidate with a load-bearing reason? Offer to record it as
  an ADR, so a future architecture review does not re-suggest it. Offer only when a
  future explorer would need the reason.
- Want to explore alternative interfaces? See [INTERFACE-DESIGN.md](INTERFACE-DESIGN.md).

### 5. Import-boundary hygiene

When a deepening move splits or merges a module, update the project's declared
module-dependency allowlist to reflect which module may depend on which peer. A
convention doc alone does not authorize a new import. The allowlist must list it.
Check the boundaries before you propose a cross-module dependency edge.

## Verify

Confirm the module-dependency allowlist is present and the current imports satisfy
it.
