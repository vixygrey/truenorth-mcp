'use strict';

async function run(ctx) {
  await ctx.runModel(ctx.scenario.model_prompt);
}

module.exports = { run };
