'use strict';

const fs = require('node:fs');
const actual = fs.readFileSync(process.env.TRUENORTH_BENCHMARK_STDOUT, 'utf8');
const expected = fs.readFileSync('expected.txt', 'utf8').trimEnd();
process.exitCode = actual === expected ? 0 : 1;
