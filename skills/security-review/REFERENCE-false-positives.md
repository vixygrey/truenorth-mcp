# False-Positive Exclusion Rules

Apply these rules during evidence classification. A candidate may enter
`Excluded` only when a hard exclusion or repository evidence disproves the
security path. Precedents guide investigation but do not silently suppress an
uncertain potentially high-impact path.

## Hard Exclusions

Automatically exclude findings matching these patterns:

| #   | Rule                                                                                                          | Rationale                                                       |
| --- | ------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| 1   | **Denial of Service (DOS)** — resource exhaustion, CPU/memory attacks                                         | Handled separately; not actionable in code review               |
| 2   | **Secrets on disk** if otherwise secured                                                                      | Secrets management is a separate concern                        |
| 3   | **Rate limiting** concerns                                                                                    | Operational, not a code vulnerability                           |
| 4   | **Memory consumption / CPU exhaustion**                                                                       | Not actionable in diff review                                   |
| 5   | **Input validation on non-security-critical fields** without proven exploit path                              | Theoretical, not concrete                                       |
| 6   | **GitHub Actions input sanitization** unless clearly triggerable via untrusted input                          | Most workflow vulns are not exploitable                         |
| 7   | **Lack of hardening measures**                                                                                | Code is not expected to implement all best practices            |
| 8   | **Race conditions / timing attacks** that are theoretical                                                     | Only report if concretely problematic                           |
| 9   | **Outdated third-party libraries**                                                                            | Managed separately by dependency scanners                       |
| 10  | **Memory safety** in Rust or other memory-safe languages                                                      | Impossible by language guarantees                               |
| 11  | **Hardcoded SQL with proven authorship** — migrations, seeds, static admin queries with no user interpolation | Developer-authored SQL is safe per SQL-safety doctrine (e45s41) |
| 12  | **Unit test files only**                                                                                      | Not production risk                                             |
| 13  | **Log spoofing**                                                                                              | Outputting unsanitized input to logs is not a vuln              |
| 14  | **SSRF that only controls path**                                                                              | Only host/protocol control is exploitable                       |
| 15  | **User-controlled content in AI system prompts**                                                              | Not a security vulnerability                                    |
| 16  | **Regex injection**                                                                                           | Injecting untrusted content into regex is not a vuln            |
| 17  | **Regex DOS**                                                                                                 | Excluded alongside general DOS                                  |
| 18  | **Documentation files** (.md, .txt)                                                                           | Insecure docs are not code vulnerabilities                      |
| 19  | **Lack of audit logs**                                                                                        | Not a vulnerability                                             |

## Precedent Rules

These guide borderline cases based on prior human review decisions:

| #   | Precedent                                                                                                      | Reasoning                                                   |
| --- | -------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| 1   | **Logging high-value secrets in plaintext IS a vuln.** Logging URLs is safe.                                   | Secrets in logs = credential exposure; URLs are not secrets |
| 2   | **UUIDs are unguessable** — no validation needed                                                               | Cryptographic property of UUID v4/v7                        |
| 3   | **Environment variables and CLI flags are trusted values**                                                     | Attackers cannot modify these in secure environments        |
| 4   | **Resource management issues** (memory leaks, fd leaks) are NOT valid                                          | Operational, not security                                   |
| 5   | **Tabnabbing, XS-Leaks, prototype pollution, open redirects** — do NOT report unless extremely high confidence | Subtle, low-impact, high false-positive rate                |
| 6   | **React/Angular XSS** — safe unless `dangerouslySetInnerHTML`, `bypassSecurityTrustHtml`, etc.                 | Framework auto-escapes                                      |
| 7   | **GitHub Action workflow vulns** — verify concrete attack path before reporting                                | Most are theoretical                                        |
| 8   | **Client-side JS/TS auth checks** — not a vuln; server is authoritative                                        | Client code is untrusted                                    |
| 9   | **IPython notebook vulns** — only report if concrete untrusted-input trigger                                   | Most are not exploitable                                    |
| 10  | **Logging non-PII data** — not a vuln even if sensitive. Only PII/secrets/passwords.                           | Intent: operational logging vs credential exposure          |
| 11  | **Shell script command injection** — only report if concrete untrusted-input path                              | Most shell scripts don't process untrusted input            |

## Confidence and exclusions

Confidence measures evidence strength; it is not an exclusion rule. Classify a
candidate as follows:

| State                                                          | Action                                                         |
| -------------------------------------------------------------- | -------------------------------------------------------------- |
| Exploit path supported                                         | Report under `Confirmed findings` with confidence and severity |
| Plausible path with potentially HIGH or CRITICAL impact        | Report under `Needs investigation`, regardless of confidence   |
| Candidate disproved by a hard exclusion or repository evidence | Record under `Excluded` with the cited rule                    |
| Uncertain MEDIUM or LOW path                                   | Report when actionable under active project conventions        |

Never cite a numeric confidence threshold as the reason for exclusion.

## Signal Quality Criteria

For remaining findings, assess:

1. Is there a concrete, exploitable vulnerability with a clear attack path?
2. Does this represent a real security risk (vs theoretical best practice)?
3. Are there specific code locations and reproduction steps?
4. Would this finding be actionable for a security team?
