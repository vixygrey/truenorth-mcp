# TrueNorth-MCP

TrueNorth-MCP is an active, protocol-first MCP execution runtime for spec-driven
engineering discipline. The runtime is a Rust binary. A thin Node.js wrapper distributes
it. An agent connects over stdio, calls a tool to advance a lifecycle phase, verify a
gate, or read the project cockpit, and reads state through a resource.

This Wiki is the usage manual. The landing page presents the project. This manual explains
how to run it.

## What it is

The runtime delivers engineering discipline through typed MCP tools and resources, not
through a large instruction file dumped into a context window. It advances lifecycle
state, runs quality gates in a sandbox, and drives a Red-Green-Refactor loop. The governed
project can be any language.

## The two-layer model

The repository has two layers.

- The machine-facing layer is `.agent/`. The runtime reads, watches, and writes only under
  `.agent/`. A single write guard rejects any write outside `.agent/`.
- The human-facing layer is `specs/`, which holds human-authored narrative such as
  Architecture Decision Records. The runtime reads a file under `specs/`, but never writes
  there.

## A reading path

Read the pages in this order for a first run.

1. [Install and connect](Install-and-connect): install the wrapper and connect a client.
2. [The .agent workspace](The-agent-workspace): the layout contract the runtime validates.
3. [Methodology profiles](Methodology-profiles): the workflow shape for your project.
4. [The lifecycle](The-lifecycle): a worked Discover-to-Integrate run.
5. [Resources](Resources): the live `truenorth://` resources.
6. [The ontology feature](The-ontology-feature): the optional ontology surface.
7. [Skills and tiers](Skills-and-tiers): the skill library and tiered rendering.
8. [Troubleshooting](Troubleshooting): repo-root resolution and a broken layout contract.

## The skill reference

The runtime serves 80 skills across the lifecycle. For the full reference:

- [The skill workflow](The-skill-workflow): how the skills chain from one to the next.
- [Skill index](Skill-index): every skill, alphabetical, with its phase page.
- Phase pages: [Discover](Skills-Discover), [Design](Skills-Design), [Plan](Skills-Plan),
  [Build](Skills-Build), [Verify](Skills-Verify), [Release](Skills-Release),
  [Sustain](Skills-Sustain), and [Utility](Skills-Utility).

## Design references

The design lives in the repository, not this Wiki. The Architecture Decision Records under
`specs/adr/` record the load-bearing decisions. This manual links them rather than
restating them.
