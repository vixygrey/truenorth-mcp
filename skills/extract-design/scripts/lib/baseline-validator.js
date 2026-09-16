// In-repo baseline design-token validator.
//
// This is the offline fallback and the always-available floor for the DESIGN.md HARD GATE.
// It reuses the checks the skill genuinely needs: DESIGN.md front-matter structure and a
// WCAG AA contrast check on the extracted color pairs. It never shells out and never
// touches the network. The external `@google/design.md` CLI, when present, is a richer
// enhancement layered on top (see validator.js). Both return the same result shape:
//   lint -> { summary: { errors, warnings, info }, findings: [...], skipped: false }
//   diff -> { tokens: { colors, typography }, regression, skipped: false }
// A finding is { severity: 'error'|'warning'|'info', rule, message }.

import { readFileSync, existsSync } from 'node:fs';

// WCAG 2.1 contrast thresholds. Normal-size text must meet AA at 4.5:1; the large-text
// and AAA thresholds are intentionally out of scope for this baseline.
const WCAG_AA_NORMAL = 4.5;

// The color roles a DESIGN.md must define to anchor a usable palette. A foreground role
// (`on-*`) is meaningful only against its matching surface, so the pairs below drive both
// the required-role check and the contrast check.
const REQUIRED_ROLES = ['surface', 'on-surface'];

// Foreground/background role pairs to contrast-check when both are present.
const CONTRAST_PAIRS = [
  ['on-surface', 'surface'],
  ['on-surface', 'background'],
  ['on-primary', 'primary'],
  ['on-secondary', 'secondary'],
  ['on-tertiary', 'tertiary'],
  ['on-error', 'error'],
  ['on-background', 'background'],
];

/// Parse a `#rgb`, `#rrggbb`, `rgb(...)`, or `rgba(...)` string to `{ r, g, b }` in 0..255.
/// Returns null when the string is not a color this baseline understands.
export function parseColor(value) {
  if (typeof value !== 'string') return null;
  const s = value.trim();
  const hex = s.match(/^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/);
  if (hex) {
    let h = hex[1];
    if (h.length === 3) h = h[0] + h[0] + h[1] + h[1] + h[2] + h[2];
    return {
      r: parseInt(h.slice(0, 2), 16),
      g: parseInt(h.slice(2, 4), 16),
      b: parseInt(h.slice(4, 6), 16),
    };
  }
  const rgb = s.match(/^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/);
  if (rgb) {
    return { r: parseInt(rgb[1], 10), g: parseInt(rgb[2], 10), b: parseInt(rgb[3], 10) };
  }
  return null;
}

/// Relative luminance per WCAG 2.1, matching the sRGB linearization used in
/// classify-colors.js. Input channels are 0..255.
export function relativeLuminance({ r, g, b }) {
  const lin = (c) => {
    const v = c / 255;
    return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

/// WCAG contrast ratio between two colors, in the range 1..21. Returns null when either
/// color cannot be parsed.
export function contrastRatio(fg, bg) {
  const a = parseColor(fg);
  const b = parseColor(bg);
  if (!a || !b) return null;
  const la = relativeLuminance(a);
  const lb = relativeLuminance(b);
  const lighter = Math.max(la, lb);
  const darker = Math.min(la, lb);
  return (lighter + 0.05) / (darker + 0.05);
}

/// Parse the DESIGN.md front matter that write-designd.js produces. Returns
/// `{ frontMatterPresent, name, colors, colorsDark, typography }`, hand-parsed to avoid a
/// YAML dependency. `colors` and `colors-dark` are flat `role -> value` maps. `typography`
/// is a nested `level -> { prop -> value }` map.
export function parseFrontMatter(text) {
  const out = { frontMatterPresent: false, name: null, colors: {}, colorsDark: {}, typography: {} };
  if (!text.startsWith('---\n')) return out;
  const end = text.indexOf('\n---', 4);
  if (end === -1) return out;
  out.frontMatterPresent = true;
  const body = text.slice(4, end);
  // section: null | 'colors' | 'colors-dark' | 'typography'. typographyLevel tracks the
  // current nested key while inside the typography block.
  let section = null;
  let typographyLevel = null;
  for (const raw of body.split('\n')) {
    if (raw === '') continue;
    const indent = raw.length - raw.replace(/^\s+/, '').length;
    const line = raw.trim();

    if (indent === 0) {
      // A top-level key opens or closes a section.
      typographyLevel = null;
      if (line === 'colors:') section = 'colors';
      else if (line === 'colors-dark:') section = 'colors-dark';
      else if (line === 'typography:') section = 'typography';
      else {
        section = null;
        const nameMatch = line.match(/^name:\s*(.+)$/);
        if (nameMatch) out.name = stripQuotes(nameMatch[1]);
      }
      continue;
    }

    if (section === 'colors' || section === 'colors-dark') {
      const pair = line.match(/^([\w-]+):\s*(.+)$/);
      if (!pair) continue;
      const target = section === 'colors' ? out.colors : out.colorsDark;
      target[pair[1]] = stripQuotes(pair[2]);
      continue;
    }

    if (section === 'typography') {
      // A level header has no value (`  body-md:`). A property line has a value and sits at
      // a deeper indent (`    fontSize: 16px`).
      const levelHeader = line.match(/^([\w-]+):$/);
      if (levelHeader) {
        typographyLevel = levelHeader[1];
        out.typography[typographyLevel] = {};
        continue;
      }
      const prop = line.match(/^([\w-]+):\s*(.+)$/);
      if (prop && typographyLevel) {
        out.typography[typographyLevel][prop[1]] = stripQuotes(prop[2]);
      }
    }
  }
  return out;
}

function stripQuotes(v) {
  const t = v.trim();
  if (t.length >= 2 && t[0] === '"' && t[t.length - 1] === '"') return t.slice(1, -1);
  return t;
}

/// Lint a DESIGN.md file with the in-repo baseline. Mirrors the CLI lint result shape.
export function baselineLint(filePath) {
  const findings = [];
  if (!existsSync(filePath)) {
    findings.push({
      severity: 'error',
      rule: 'file-missing',
      message: `DESIGN.md not found at ${filePath}.`,
    });
    return toResult(findings);
  }
  const text = readFileSync(filePath, 'utf8');
  const fm = parseFrontMatter(text);

  if (!fm.frontMatterPresent) {
    findings.push({
      severity: 'error',
      rule: 'front-matter-missing',
      message: 'DESIGN.md has no YAML front matter delimited by `---`.',
    });
    return toResult(findings);
  }

  if (!fm.name) {
    findings.push({
      severity: 'warning',
      rule: 'name-missing',
      message: 'The front matter has no `name` field.',
    });
  }

  for (const role of REQUIRED_ROLES) {
    if (!fm.colors[role]) {
      findings.push({
        severity: 'error',
        rule: 'role-missing',
        message: `The required color role \`${role}\` is not defined.`,
      });
    }
  }

  findings.push(...contrastFindings(fm.colors, 'colors'));
  if (Object.keys(fm.colorsDark).length) {
    findings.push(...contrastFindings(fm.colorsDark, 'colors-dark'));
  }

  findings.push(...unparseableColorFindings(fm.colors, 'colors'));
  if (Object.keys(fm.colorsDark).length) {
    findings.push(...unparseableColorFindings(fm.colorsDark, 'colors-dark'));
  }

  findings.push(...typographyFindings(fm.typography));

  return toResult(findings);
}

/// A CSS length the baseline understands: a number with a unit, or a bare `0`.
function isLength(value) {
  if (typeof value !== 'string') return false;
  return /^-?\d*\.?\d+(px|rem|em|%|pt|vh|vw)$/.test(value.trim()) || value.trim() === '0';
}

/// A color value that does not parse is a defect: it cannot render and it silently drops
/// out of the contrast check. Report it directly rather than leaving it as a skipped pair.
function unparseableColorFindings(colors, label) {
  const findings = [];
  for (const [role, value] of Object.entries(colors)) {
    if (parseColor(value) === null) {
      findings.push({
        severity: 'error',
        rule: 'color-unparseable',
        message: `${label}: \`${role}\` value \`${value}\` is not a parseable color.`,
      });
    }
  }
  return findings;
}

/// Typography completeness and value checks. An empty typography block, or one with no
/// body-tier level, is a warning: the palette has no defined reading text. A `fontSize`
/// that is not a length is a warning, since the value cannot render.
function typographyFindings(typography) {
  const findings = [];
  const levels = Object.keys(typography);
  if (levels.length === 0) {
    findings.push({
      severity: 'warning',
      rule: 'typography-empty',
      message: 'The front matter defines no typography levels.',
    });
    return findings;
  }
  if (!levels.some((l) => l.startsWith('body-'))) {
    findings.push({
      severity: 'warning',
      rule: 'typography-no-body',
      message: 'Typography defines no body-tier level (a `body-*` level for reading text).',
    });
  }
  for (const [level, props] of Object.entries(typography)) {
    if ('fontSize' in props && !isLength(props.fontSize)) {
      findings.push({
        severity: 'warning',
        rule: 'typography-fontsize-invalid',
        message: `typography: \`${level}\` fontSize \`${props.fontSize}\` is not a length.`,
      });
    }
  }
  return findings;
}

/// Contrast findings for one color map. An `on-*` role present without its background, or
/// the reverse, is an info-level note. A parsed pair below the AA threshold is an error.
function contrastFindings(colors, label) {
  const findings = [];
  for (const [fgRole, bgRole] of CONTRAST_PAIRS) {
    const fg = colors[fgRole];
    const bg = colors[bgRole];
    if (!fg && !bg) continue;
    if (!fg || !bg) {
      findings.push({
        severity: 'info',
        rule: 'contrast-pair-incomplete',
        message: `${label}: \`${fgRole}\` and \`${bgRole}\` are not both defined, so contrast was not checked.`,
      });
      continue;
    }
    const ratio = contrastRatio(fg, bg);
    if (ratio === null) {
      // An unparseable value is reported once by `color-unparseable` (an error). Do not
      // also emit a redundant info here.
      continue;
    }
    if (ratio < WCAG_AA_NORMAL) {
      findings.push({
        severity: 'error',
        rule: 'contrast-aa',
        message: `${label}: \`${fgRole}\` on \`${bgRole}\` is ${ratio.toFixed(2)}:1, below the WCAG AA minimum of ${WCAG_AA_NORMAL}:1.`,
      });
    }
  }
  return findings;
}

/// Diff two DESIGN.md files with the in-repo baseline. Mirrors the CLI diff result shape.
/// Reports added, removed, and modified color roles, and flags a regression when the new
/// file has more lint errors than the old one.
export function baselineDiff(oldPath, newPath) {
  const oldFm = existsSync(oldPath) ? parseFrontMatter(readFileSync(oldPath, 'utf8')) : emptyFrontMatter();
  const newFm = existsSync(newPath) ? parseFrontMatter(readFileSync(newPath, 'utf8')) : emptyFrontMatter();

  const oldErrors = existsSync(oldPath) ? baselineLint(oldPath).summary.errors : 0;
  const newErrors = existsSync(newPath) ? baselineLint(newPath).summary.errors : 0;

  return {
    tokens: {
      colors: diffFlatMap(oldFm.colors, newFm.colors),
      typography: diffNestedMap(oldFm.typography, newFm.typography),
    },
    regression: newErrors > oldErrors,
    skipped: false,
  };
}

function emptyFrontMatter() {
  return { frontMatterPresent: false, name: null, colors: {}, colorsDark: {}, typography: {} };
}

/// Diff two flat `key -> value` maps. A key present in both with a changed value is
/// modified.
function diffFlatMap(oldMap, newMap) {
  const added = [];
  const removed = [];
  const modified = [];
  for (const key of Object.keys(newMap)) {
    if (!(key in oldMap)) added.push(key);
    else if (oldMap[key] !== newMap[key]) modified.push(key);
  }
  for (const key of Object.keys(oldMap)) {
    if (!(key in newMap)) removed.push(key);
  }
  return { added, removed, modified };
}

/// Diff two nested `key -> { prop -> value }` maps. A key present in both is modified when
/// any property is added, removed, or changed.
function diffNestedMap(oldMap, newMap) {
  const added = [];
  const removed = [];
  const modified = [];
  for (const key of Object.keys(newMap)) {
    if (!(key in oldMap)) added.push(key);
    else if (!shallowEqual(oldMap[key], newMap[key])) modified.push(key);
  }
  for (const key of Object.keys(oldMap)) {
    if (!(key in newMap)) removed.push(key);
  }
  return { added, removed, modified };
}

function shallowEqual(a, b) {
  const ak = Object.keys(a);
  const bk = Object.keys(b);
  if (ak.length !== bk.length) return false;
  return ak.every((k) => a[k] === b[k]);
}

/// Fold a findings list into the CLI-compatible lint result shape.
function toResult(findings) {
  const summary = { errors: 0, warnings: 0, info: 0 };
  for (const f of findings) {
    if (f.severity === 'error') summary.errors++;
    else if (f.severity === 'warning') summary.warnings++;
    else summary.info++;
  }
  return { summary, findings, skipped: false };
}
