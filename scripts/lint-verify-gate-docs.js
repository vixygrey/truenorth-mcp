'use strict';

const fs = require('node:fs');
const path = require('node:path');

const repoRoot = path.resolve(__dirname, '..');
const publicDocPaths = ['.agent/product/scope.yml', 'README.md', 'wiki', 'site'];
const forbiddenPatterns = [
  /\bevidence\s+mode\b/i,
  /["']mode["']\s*:\s*["']evidence["']/i,
  /["']test_evidence["']\s*:/i,
  /\baccepts?\s+(?:model[-\s]+supplied\s+)?evidence\b/i,
];
const requiredConfigurationNames = [
  'TRUENORTH_ROOT',
  'TRUENORTH_VERIFY_CMD',
  'TRUENORTH_GATE_ALLOWLIST',
];

const files = publicDocPaths.flatMap((docPath) => collectFiles(path.join(repoRoot, docPath)));
const violations = [];

for (const file of files) {
  const content = fs.readFileSync(file, 'utf8');
  for (const pattern of forbiddenPatterns) {
    const match = content.match(pattern);
    if (match) {
      violations.push(
        `${relative(file)}:${lineNumber(content, match.index)}: deprecated verify-gate wording: ${match[0]}`,
      );
    }
  }
}

const configurationGuide = path.join(repoRoot, 'wiki', 'Install-and-connect.md');
const configurationContent = fs.readFileSync(configurationGuide, 'utf8');
for (const name of requiredConfigurationNames) {
  if (!configurationContent.includes(name)) {
    violations.push(
      `${relative(configurationGuide)}: missing required verify-gate configuration: ${name}`,
    );
  }
}

if (violations.length > 0) {
  console.error('Verify-gate documentation contract failed:');
  for (const violation of violations) {
    console.error(`- ${violation}`);
  }
  process.exitCode = 1;
}

function collectFiles(target) {
  const stat = fs.statSync(target);
  if (stat.isFile()) {
    return [target];
  }

  return fs.readdirSync(target, { withFileTypes: true }).flatMap((entry) => {
    const child = path.join(target, entry.name);
    return entry.isDirectory() ? collectFiles(child) : [child];
  });
}

function lineNumber(content, index) {
  return content.slice(0, index).split('\n').length;
}

function relative(file) {
  return path.relative(repoRoot, file);
}
