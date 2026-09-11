---
inclusion: always
---

# House Writing Rules (Simplified Technical English)

These are the authoritative writing rules for this repository. They apply to all prose
this project produces: documentation, commit messages, PR titles and bodies, issues,
specs, changelogs, error messages, CLI output, UI copy, and instructions for AI agents.
They override any writing style implied by the inherited upstream files.

The rules paraphrase ASD-STE100 Issue 9 (the aerospace maintenance-documentation
standard) with software examples. The core mechanic is one word, one meaning, one part
of speech. Do not claim STE compliance: no tool can guarantee it, and final approval
rests with the writer.

## Voice and tone (house style)

- Be calm, technically sharp, and warm. Clarity over flourish.
- Keep answers concise by default. Expand when the task benefits from detail.
- Recommend the smallest high-leverage next step first.
- Do not use em dashes in user-facing prose. Limit hyphen-heavy phrasing. Prefer simpler
  punctuation.
- Avoid AI writing tropes: filler praise, sales language, inflated certainty, canned
  encouragement. Do not write "great question", "absolutely", "certainly", "game changer",
  "seamless", or "hope this helps" unless the wording is genuinely necessary.
- Do not narrate intent at length. Act, then summarize results.
- Keep confidence proportional to evidence. State uncertainty plainly when it exists.
- Use American English spelling.
- Use inclusive language (primary and replica, not master and slave).

## Scope: two tiers

Membership is by document type. Decide the tier before anything else.

| Tier | Applies to |
|---|---|
| **Strict** | Commit messages. PR titles and bodies. Specs. Technical documentation (READMEs, runbooks, procedures, API guides, architecture notes). Changelogs and release notes. Incident reports. Error messages and CLI output. UI copy. Instructions for AI agents. |
| **Loose** | Issues and their comments. Wikis. Chat replies. |

- **Strict** means every rule below, including passage classification, the 20-word and
  25-word limits, and Rule 4.2 (no contractions).
- **Loose** means the mechanical subset only: no slop words, no filler adverbs, no Latin
  abbreviations, no hedging, one term per concept. The sentence-length limits and Rule
  4.2 do not apply.

Note the asymmetry: an issue body is loose, but its PR body is strict. The issue argues
for a change while the shape is still open. The PR records what the change is, for someone
who reads it later.

**Safety carve-out (overrides the tier).** A warning about data loss, an irreversible
action, or a destructive flag follows Section 7 wherever it appears, including inside a
loose document. Command or condition first, risk second. The tier controls register. It
does not control safety.

## Before you write

1. Classify each passage as procedural or descriptive. Every length limit and verb form
   depends on this choice.
2. Fix the vocabulary first. Pick one noun and one verb per concept, then hold them for
   the whole document.
3. Leave code, identifiers, CLI flags, file paths, quoted error messages, and proper nouns
   exactly as they are. These are untouchable.

| | Procedural (instructions) | Descriptive (explanations) |
|---|---|---|
| Purpose | Tell the reader what to do | Explain what a thing is or does |
| Verb form | Imperative: "Install the pump." | Simple present, past, or future |
| Sentence limit | **20 words** | **25 words** |
| Unit rule | One instruction per sentence | One topic per paragraph, max six sentences |

## Section 1: Words

- Use domain words as technical nouns ("webhook", "commit", "endpoint") and domain verbs
  as technical verbs ("deploy", "compile", "merge"). Your domain vocabulary is legal.
- Do not use technical nouns as verbs (not "webhook the event").
- Do not use technical verbs as nouns (not "do a deploy").
- One item, one name. Do not call it "config" here and "settings" there.
- Pick short, clear technical nouns. No regional, slang, or jargon words.
- Use American English spelling.

## Section 2: Multi-word nouns

- Write multi-word nouns of three words or fewer.
- When a noun needs more than three words, write it in full once, then give a short form.
- Break long noun chains with prepositions. "the timeout value for the connection pool",
  not "the connection pool timeout configuration value".

## Section 3: Verbs

- Use only: infinitive, imperative, simple present, simple past, simple future, and past
  participle as an adjective ("the cached response").
- No auxiliary verb constructions. No present perfect. No "is to be installed".
- Use an "-ing" form only as a technical noun or inside one ("logging"). Never as a verb.
- Active voice. In descriptive text, passive is legal only when the agent is unknown.
- Describe an action with a verb, not a noun. "compress the file", not "perform compression
  of the file".

## Section 4: Sentences

- Write short, clear sentences.
- Do not omit words or use contractions. Keep articles. Keep "that" (write "make sure that
  the file exists", not "ensure file exists"). This is short sentences with complete
  grammar, not telegraph style.
- Use a vertical list for complex text.
- Use connecting words between related sentences ("Then", "As a result").
- Put an article or demonstrative adjective before nouns where applicable.

## Section 5: Procedural writing

- Maximum 20 words per sentence, warnings and cautions included.
- One instruction per sentence, unless two actions occur at the same time.
- Write instructions in the imperative.
- Put a required condition before the command, divided by a comma.
- Notes give information, never instructions. Notes get the 25-word limit.

## Section 6: Descriptive writing

- Give information gradually: one new fact per sentence.
- Maximum 25 words per sentence.
- One topic per paragraph. Maximum six sentences per paragraph.
- Do not use the imperative in descriptive text. Descriptions explain. Procedures instruct.

## Section 7: Safety instructions (overrides tier)

- Use a word that shows the risk level. "WARNING" equals injury. "CAUTION" equals damage.
- Start with a clear command or condition.
- Then give the risk or the possible result.

Do not bury the instruction after the explanation. This pattern applies to destructive CLI
flags, irreversible migrations, and dangerous API options.

Example: "CAUTION: Do not use the `--force` flag against production. The flag deletes rows
that do not match the source."

## Section 8: Punctuation and word count

- All standard punctuation is legal except the semicolon. Write two sentences instead.
- Use hyphens to connect words that act as one unit.
- Parentheses are legal for references, item numbers, abbreviations, and explanations.
- Count as one word each: numbers, numbers with units, abbreviations, identifiers, quoted
  text, titles, and proper nouns. A backticked command such as `sqlpipe run --config
  sqlpipe.yaml` is quoted text and counts as one word.
- Text inside parentheses counts as one word. A hyphenated word counts as one word.

## Section 9: Writing practices

- When a word-for-word replacement does not work, restructure the sentence.
- Do not build phrasal verbs. Write "decrease", not "go down". Write "install", not "set up".
- Keep one consistent style and terminology through the whole document.
- Keep the conjunction "that". Give pronouns clear referents. Prefer "this plus noun" over a
  bare "this".
- Avoid Latin abbreviations. Write "for example" and "that is". Name the items instead of
  "etc.".
- Use the possessive apostrophe only when you are sure it is correct.

## Approved modals and slop substitutions

Approved modals: `can`, `will`, `must`. Nothing else.

| Instead of | Write |
|---|---|
| should | `must` for a requirement, or delete it for a recommendation |
| may, might, could | can |
| leverage, utilize | use |
| in order to | to |
| prior to | before |
| ensure | make sure that |
| functionality | function, or feature |
| simply, easily, seamlessly, robust | delete, they carry no fact |

## Known part-of-speech rulings

- test, check, work: noun only. "Do a test", not "test the pump". "Check that X" becomes
  "make sure that X".
- help: verb only. For the noun, use "aid".
- follow: "to come after" only. Write "obey the instructions", not "follow the instructions".
- above, below: physical positions only. For limits, write "more than" or "less than".

## Doc-type modes

| Document | Mode | Adaptation |
|---|---|---|
| Error messages, CLI output | Procedural | State what happened in the simple past. State the cause if known. Give the command that fixes it. Delete "Oops" and "Please ensure". |
| Runbooks, SOPs | Strict procedural | Imperative every step. One instruction per step. Conditions first. Warnings before the step, command first, risk second. |
| Incident reports, postmortems | Descriptive | Simple past only. State what is known and write "unknown" for the rest. |
| Commit messages, PR bodies | Imperative subject, descriptive body | Plain past facts in the body. Delete "this PR aims to". |
| Changelogs, release notes | Descriptive | One entry, one change, one sentence where possible. Breaking entries follow the warning pattern, command first. |
| Instructions for AI agents | Procedural | One instruction per sentence. One word, one meaning, so "check", "verify", and "validate" are not read as three operations. Delete the banned modals. |
| UI copy, empty states | Procedural, hard length limits | Buttons and labels are technical names and stay exempt. Body copy follows the rules. |

## Mechanical self-check before delivery

Search the draft for each pattern. Every hit outside code blocks and quoted text is a
violation.

| Search for | Violation | Fix |
|---|---|---|
| contractions (`n't`, `'ll`, `'re`, `'ve`, `it's`) | Contraction | Expand it. |
| `has been`, `have been`, `had been` | Perfect tense | Simple past or simple present. |
| `has` or `have` plus a past participle | Present perfect | Simple past. |
| `is being`, `are being`, `was being` | Progressive passive | Active, simple tense. |
| comma plus `making`, `allowing`, `enabling`, `ensuring` | "-ing" clause as verb | New sentence with a real subject. |
| semicolon `;` | Semicolon | Two sentences. |
| `e.g.`, `i.e.`, `etc.` | Latin abbreviation | "for example", "that is", name the items. |
| `simply`, `easily`, `seamlessly`, `robust` | Filler, no fact | Delete. |
| ` if ` or ` when ` mid-sentence | Trailing condition | Move the condition to the start. Add a comma. |

Then count. Sentences: 20 words procedural, 25 words descriptive and notes. Paragraphs:
six sentences. Noun chains: three words. Instructions per sentence: one.

## When reporting writing-rule violations

Give the rule section, the offending text, and a compliant rewrite. End the report with
this statement: "No tool can guarantee ASD-STE100 compliance. Final approval rests with the
writer. The official standard is a free download at asd-ste100.org."
