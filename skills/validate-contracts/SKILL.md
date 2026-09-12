---
name: validate-contracts
description: 'Assert data-shape consistency across system boundaries. Live API responses against a JSON schema, a key-set comparison across layers, and data-shape validation for migrations and exports. Catches silent data corruption before a deploy.'
---

# Validate Contracts

> **HARD GATE**: Do NOT deploy or migrate data without running validate-contracts first. Silent data divergence between system boundaries causes the hardest production bugs to debug.
>
> **HARD GATE**: a contract file MUST be version-controlled alongside the code. An outdated contract is worse than no contract. When a contract has not been reviewed in 30 days, flag it as stale.

Validate that data structures stay in sync across a system boundary: front end
versus back end, an API response versus its expected schema, a config file versus
the code assumptions, and migration output versus the target shape.

## Contract types

| Mode        | What it catches                                     | When to use                                   |
| ----------- | --------------------------------------------------- | --------------------------------------------- |
| **Schema**  | An API response shape mismatch                      | Before every deploy, after an API change      |
| **Key-set** | A missing or unexpected key across two data sources | Translation files, configs, enum definitions  |
| **Shape**   | A column type or format violation                   | After a migration, before consuming an export |

## Contract file convention

Every contract file lives in `specs/contracts/` as YAML. See
[REFERENCE.md](REFERENCE.md) for extended examples.

### Key-set example

```yaml
# specs/contracts/i18n-keys.yaml
sources:
  reference: src/locales/en.json
  target: src/messages/en.json
mode: subset
```

## Process

### 1. Define the contract

Create a YAML file in `specs/contracts/` that follows the schema for the mode.

### 2. Run the validation

Validate the contract file against its sources. A key-set contract has a
`sources:` block. The schema and shape modes are documented in REFERENCE.md for a
consumer project.

### 3. Read the report

A pass reports the satisfied contract. A failure names the divergence, for example
"key-set: N keys in reference missing from target". A key-set failure exits
non-zero.

### 4. Fix the divergence

- A missing key: add it to the target source.
- A type mismatch: update the schema or fix the producer.
- A shape violation: fix the migration or the consumer.

### 5. Re-validate

Run the validation again and confirm the pass.

## Verify arc

Part of VERIFY: `verify-work`, then `validate-contracts`, then `smoke-test`, then
`run-evals`, then `audit-code`.

## Verify

Confirm the contract validation passes for every contract in `specs/contracts/`.
Run it through the `truenorth_verify_gate` tool. A pass returns exit 0.
