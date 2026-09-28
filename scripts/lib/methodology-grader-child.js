'use strict';

const fs = require('node:fs');
const path = require('node:path');

const { readJson, stableStringify } = require('./methodology-eval.js');

async function main() {
  const [inputPath, graderPath, outputPath] = process.argv.slice(2);
  if (!inputPath || !graderPath || !outputPath) {
    throw new Error('grader child requires input, grader, and output paths');
  }
  const input = readJson(inputPath);
  const grader = require(path.resolve(graderPath));
  const assertions = await grader.grade(input);
  if (!Array.isArray(assertions)) throw new Error('grader must return an assertion array');
  fs.writeFileSync(outputPath, stableStringify({ assertions }));
}

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
