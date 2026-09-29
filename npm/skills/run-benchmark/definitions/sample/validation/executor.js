'use strict';

const fs = require('node:fs');
if (fs.existsSync('.prior-run')) throw new Error('benchmark workspace was reused');
fs.writeFileSync('.prior-run', process.env.TRUENORTH_BENCHMARK_MODE);

const input = fs.readFileSync('input.txt', 'utf8');
const skillPath = process.env.TRUENORTH_BENCHMARK_SKILL_PATH;
const hasDirective =
  skillPath &&
  fs.existsSync(skillPath) &&
  fs.readFileSync(skillPath, 'utf8').includes('BENCHMARK_DIRECTIVE: normalize-lowercase');
process.stdout.write(hasDirective ? input.trim().toLowerCase() : input);
