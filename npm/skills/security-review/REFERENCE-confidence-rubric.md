# Confidence Scoring Rubric

Every candidate that survives initial tracing receives a confidence score from 1
(speculative) to 10 (certain). Confidence measures evidence strength. It does not
replace impact severity and does not by itself decide whether a potentially
high-impact path is visible.

## Score 9–10: Certain exploit path

**Criteria:**

- Concrete, testable exploit with clear reproduction steps.
- No assumptions about uncommon configurations.
- No chain of multiple unlikely conditions.
- Attacker has control over the input vector.

**Examples:**

- User-supplied SQL reaches a query with no parameterization.
- `os.system(f"rm {user_path}")` receives an attacker-controlled path.
- Pickle deserialization receives user-supplied data.

**Action:** report under `Confirmed findings`.

## Score 8: Clear vulnerability pattern

**Criteria:**

- Well-known vulnerability pattern with a standard exploitation method.
- Requires specific conditions that are commonly met.
- Exploitability is documented in established security guidance.

**Examples:**

- JWT without signature verification in authentication middleware.
- SSRF where an attacker controls the full URL including host.
- A hardcoded production credential in source code.

**Action:** report under `Confirmed findings` when the required conditions are
present. Otherwise use `Needs investigation` and name the missing evidence.

## Score 6–7: Suspicious path

**Criteria:**

- Code may cross a security boundary, but a condition or sanitization step is
  unresolved.
- A secure interpretation remains plausible.
- Additional repository or runtime evidence can decide the path.

**Examples:**

- User input passes through several layers before a sink and sanitization is
  unclear.
- Custom cryptography may protect sensitive data, but the data classification is
  unknown.
- Path construction may permit traversal, but canonicalization behavior is not
  established.

**Action:** when potential impact is HIGH or CRITICAL, report under `Needs
investigation`. State the uncertainty and the evidence needed to confirm or
exclude the path. For lower potential impact, report only when actionable under
the active project conventions.

## Score 1–5: Weak evidence

Weak evidence does not prove a vulnerability. Exclude a candidate only when a
documented hard exclusion or repository evidence disproves the security path.

When the potential impact is HIGH or CRITICAL and the path remains plausible,
report it under `Needs investigation` even at low confidence. Do not silently
discard it.

## Classification decision

| Evidence                                    | Potential impact | Classification                                   |
| ------------------------------------------- | ---------------- | ------------------------------------------------ |
| Supported exploit path                      | Any              | `Confirmed findings`                             |
| Unresolved plausible path                   | HIGH or CRITICAL | `Needs investigation`                            |
| Unresolved plausible path                   | MEDIUM or LOW    | Report when actionable under project conventions |
| Disproved path or documented hard exclusion | Any              | `Excluded`                                       |

## Severity mapping

Map severity from impact after tracing the security boundary:

| Severity     | Impact                                          | Examples                                                              |
| ------------ | ----------------------------------------------- | --------------------------------------------------------------------- |
| **CRITICAL** | Remote compromise or full data breach           | RCE, admin auth bypass, SQLi with broad exfiltration                  |
| **HIGH**     | Significant security boundary crossed           | Internal SSRF, production credential exposure, unsafe deserialization |
| **MEDIUM**   | Limited impact or conditional boundary crossing | Stored XSS behind auth, IDOR on non-sensitive data                    |
| **LOW**      | Defense in depth with minimal blast radius      | Missing security header, verbose non-production errors                |

## Quality gate

Assess every candidate through three lenses:

| Lens               | Question                                                                    |
| ------------------ | --------------------------------------------------------------------------- |
| **Exploitability** | Can a real attacker trigger this from a trust boundary?                     |
| **Actionability**  | What evidence or fix would resolve the candidate?                           |
| **Precedent**      | Has repository evidence or prior review confirmed or excluded this pattern? |

A confirmed unresolved HIGH or CRITICAL finding blocks. A `Needs investigation`
item with potentially HIGH or CRITICAL impact requires explicit disposition
before release. Confidence alone never suppresses either class.
