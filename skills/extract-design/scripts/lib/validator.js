import { execSync } from 'node:child_process';
import { baselineLint, baselineDiff } from './baseline-validator.js';

// The design-token linter has two layers. The in-repo baseline validator is the always
// available floor: it runs no subprocess and no network call, so the DESIGN.md HARD GATE
// holds offline and in locked-down environments. The external `@google/design.md` CLI, when
// present, is a richer enhancement (broader structural and reference checks). Pin the CLI
// version so a run resolves a known release, not the floating latest. A floating tag would
// let a new upstream release change lint output between runs and pull unpinned code from the
// network.
const DESIGN_MD_PKG = '@google/design.md';
const DESIGN_MD_VERSION = '0.4.0';
const PINNED = `${DESIGN_MD_PKG}@${DESIGN_MD_VERSION}`;
const BIN = `npx ${PINNED}`;

export class DesignValidator {
  constructor({ runCommand } = {}) {
    this._run = runCommand || ((c) => execSync(c, { encoding: 'utf8', timeout: 30_000 }));
    this._cliAvail = null;
  }

  // Whether the external CLI enhancement is reachable. The baseline does not depend on this.
  cliAvailable() {
    if (this._cliAvail !== null) return this._cliAvail;
    try {
      this._run(`npx -y ${PINNED} --help`);
      this._cliAvail = true;
    } catch {
      this._cliAvail = false;
    }
    return this._cliAvail;
  }

  // Lint a DESIGN.md file. Uses the external CLI when available, else the in-repo baseline.
  // Never skips: both paths return { summary, findings, skipped: false, source }.
  lint(fp) {
    if (this.cliAvailable()) {
      try {
        const r = JSON.parse(this._run(`${BIN} lint --format json ${fp}`));
        return { skipped: false, ...r, source: 'cli' };
      } catch (e) {
        if (e.stdout) {
          try {
            const r = JSON.parse(e.stdout);
            return { skipped: false, ...r, source: 'cli' };
          } catch {}
        }
        // The CLI was reachable but this invocation failed. Fall back to the baseline so the
        // gate still runs rather than throwing.
      }
    }
    return { ...baselineLint(fp), source: 'baseline' };
  }

  // Diff two DESIGN.md files. Uses the external CLI when available, else the in-repo
  // baseline. Never skips: both paths return { tokens, regression, skipped: false, source }.
  diff(a, b) {
    if (this.cliAvailable()) {
      try {
        const r = JSON.parse(this._run(`${BIN} diff --format json ${a} ${b}`));
        return { skipped: false, ...r, source: 'cli' };
      } catch {
        // Fall back to the baseline diff.
      }
    }
    return { ...baselineDiff(a, b), source: 'baseline' };
  }
}
