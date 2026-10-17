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

function shellInvocation(model) {
  if (!model?.available || !model.command) throw new Error('model command is unavailable');
  return { executable: '/bin/sh', args: ['-c', model.command] };
}

function modelEnvironment(workspace, env = process.env) {
  return { ...env, TRUENORTH_ROOT: workspace };
}

module.exports = { configuration, modelEnvironment, shellInvocation };
