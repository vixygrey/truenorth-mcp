# ADR-0018: Native Artifact Attestations, SBOMs, and Code Signing Policy

**Status:** Accepted
**Date:** 2026-09-29

## Context

TrueNorth-MCP distributes both as an npm wrapper package (`npx truenorth-mcp`) and as direct standalone native binaries (`.tar.gz` archives on GitHub Releases).

The npm distribution path provides registry integrity (SHA-512 hashes) and OIDC-based trusted publishing provenance. In contrast, direct native downloads from GitHub Releases previously relied only on SHA-256 checksums and HTTPS transport integrity without independent build provenance or component inventory.

Additionally, compiled macOS binaries are ad-hoc linker signed without an Apple Developer ID signature or Gatekeeper notarization.

## Decision

1. **Adopt Keyless GitHub Artifact Attestations**:
   - Use GitHub's Sigstore-backed provenance attestation (`actions/attest-build-provenance`) during the release workflow.
   - Attest both the individual native release archives and the root `SHA256SUMS` manifest.
   - This provides verifiable cryptographic supply-chain provenance tied to the repository's immutable OIDC identity without requiring long-lived private signing keys or certificate rotation.

2. **Publish CycloneDX Software Bill of Materials (SBOM)**:
   - Generate a standardized CycloneDX JSON SBOM for each release covering workspace dependencies (Cargo lockfile and npm package lockfile).
   - Attach the SBOM to the public GitHub Release alongside `RELEASE-VERIFICATION.json`.

3. **Defer Apple Developer ID Signing and Notarization**:
   - The primary distribution path is `npx truenorth-mcp`, which downloads and executes platform binaries directly without triggering macOS Gatekeeper quarantine.
   - Maintaining an Apple Developer Program subscription ($99/year), provisioning certificates, App Store Connect API keys, and notarization stapling introduces operational burden and secret leak risk for solo and small-team maintainers without providing security value to npm-installed users.
   - Developer ID signing and notarization remain deferred until a direct GUI application, PKG installer, or Homebrew tap becomes a primary supported distribution channel.

4. **Provide Explicit Verification Instructions**:
   - Document copy-paste verification instructions (`gh attestation verify` and `sha256sum`) in release notes and workspace connection guides.

## Consequences

- Direct native download users can cryptographically verify that release binaries were built by the official GitHub Actions workflow from the exact release tag commit (`gh attestation verify <archive> --owner vixygrey`).
- Automated vulnerability scanners and enterprise supply-chain audit tools can ingest the CycloneDX SBOM.
- Zero paid certificate infrastructure or long-lived private keys are introduced into CI secrets.
- Releases remain fully automated and verifiable.
