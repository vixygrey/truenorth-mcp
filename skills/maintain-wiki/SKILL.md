---
name: maintain-wiki
description: "Maintain the concept-wiki bundle so it stays consistent with its source docs. Ingest source docs, lint for issues, and query across the concept pages. Use it to keep the wiki current with the skills, the conventions, and the agent guide."
---

# Maintain Wiki

Three operations that keep the concept-wiki bundle consistent with its source docs.

## Ingest

Read a source doc (the conventions, the agent guide, or a SKILL.md) and write or
update the concept pages it maps to. Regenerate the skills wiki from the SKILL.md
files, the conventions wiki from the conventions doc, and the agent-guide wiki from
the agent guide.

## Lint

Check for common wiki issues.

1. **Stale concept**: the source file is newer than its concept page. Compare the
   modification times. When the source is newer, the concept is stale.
2. **Orphan concept**: the concept page exists but the source section no longer does.
3. **Missing cross-reference**: a skill concept page with no enforcing or referencing
   link.
4. **Contradiction**: two concept pages that make opposite claims about the same
   topic.
5. **Broken link**: an internal link that points to a non-existent concept page.

For the stale check, compare each concept page's modification time against its source
SKILL.md, and report a page that is older than its source.

## Query

Search across the concept pages to answer a question. Find every concept about a
topic, trace a convention to its enforcing skills, or find a skill by a description
keyword. Use the git-context tool or a text search over the wiki directories.

## Verify

Confirm the wiki directories exist and no concept page is older than its source doc.
