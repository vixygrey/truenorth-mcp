'use strict';

function configuration(env = process.env) {
  const command = env.TRUENORTH_EVAL_MODEL_COMMAND;
  const provider = env.TRUENORTH_EVAL_MODEL_PROVIDER;
  const model = env.TRUENORTH_EVAL_MODEL_ID;
  if (!command || !provider || !model) {
    return {
      available: false,
      reason:
        'set TRUENORTH_EVAL_MODEL_COMMAND, TRUENORTH_EVAL_MODEL_PROVIDER, and TRUENORTH_EVAL_MODEL_ID',
      provider: provider ?? null,
      model: model ?? null,
    };
  }
  return { available: true, reason: null, provider, model, command };
}

module.exports = { configuration };
