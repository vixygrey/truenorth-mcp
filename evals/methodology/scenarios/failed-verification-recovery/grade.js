'use strict';

const result = (id, passed, evidence) => ({ id, passed, evidence });

function grade({ evidence }) {
  const gates = evidence.events.filter(
    (event) => event.type === 'mcp_tool' && event.tool === 'truenorth_verify_gate',
  );
  const mutation = evidence.events.find(
    (event) => event.type === 'fixture_mutation' && event.path.endsWith('status.txt'),
  );
  const first = gates[0];
  const second = gates[1];
  return [
    result(
      'recovery-correction-between-gates',
      first && mutation && second && first.index < mutation.index && mutation.index < second.index,
      `event order: ${first?.index}, ${mutation?.index}, ${second?.index}`,
    ),
    result(
      'recovery-first-gate-failed',
      first?.ok === false && /exited with code 1/.test(first.error?.message ?? ''),
      first?.error?.message ?? 'first gate missing',
    ),
    result(
      'recovery-failure-preserved',
      gates.length === 2 && first?.ok === false,
      'both gate events remain in the transcript',
    ),
    result(
      'recovery-second-gate-passed',
      second?.ok === true,
      second?.ok ? 'later real gate passed' : 'later gate did not pass',
    ),
  ];
}

module.exports = { grade };
