'use strict';

const fs = require('node:fs');

const { executeScenarioDriver, readJson, stableStringify } = require('./methodology-eval.js');

async function main() {
  const [inputPath, outputPath] = process.argv.slice(2);
  if (!inputPath || !outputPath) throw new Error('driver child requires input and output paths');
  const evidence = await executeScenarioDriver(readJson(inputPath));
  fs.writeFileSync(outputPath, stableStringify(evidence));
  if (evidence.driver_error) process.exitCode = 1;
}

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
