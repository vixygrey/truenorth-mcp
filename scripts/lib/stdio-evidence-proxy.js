'use strict';

const fs = require('node:fs');
const { spawn } = require('node:child_process');

const [transcript, lifecycle, stderrFile, command, ...args] = process.argv.slice(2);
const stderr = fs.openSync(stderrFile, 'a');
const child = spawn(command, args, {
  stdio: ['pipe', 'pipe', stderr],
  env: process.env,
});
const append = (target, value) => fs.appendFileSync(target, `${JSON.stringify(value)}\n`);

append(lifecycle, { event: 'start', pid: child.pid });

function forward(source, target, direction) {
  let pending = '';
  source.on('data', (chunk) => {
    target.write(chunk);
    pending += chunk.toString('utf8');
    for (;;) {
      const index = pending.indexOf('\n');
      if (index < 0) break;
      const line = pending.slice(0, index).trim();
      pending = pending.slice(index + 1);
      if (!line) continue;
      try {
        const frame = JSON.parse(line);
        append(transcript, {
          direction,
          method: frame.method || null,
          id: frame.id ?? null,
          protocol_version: frame.params?.protocolVersion || frame.result?.protocolVersion || null,
          client_info: frame.params?.clientInfo || null,
          tools: Array.isArray(frame.result?.tools)
            ? frame.result.tools.map((tool) => tool.name).filter(Boolean)
            : [],
          resources: Array.isArray(frame.result?.resources)
            ? frame.result.resources.map((resource) => resource.uri).filter(Boolean)
            : [],
          tool_call:
            frame.method === 'tools/call' && typeof frame.params?.name === 'string'
              ? frame.params.name
              : null,
          response:
            direction === 'server_to_client' &&
            frame.id != null &&
            ('result' in frame || 'error' in frame),
          response_error: Boolean(frame.error || frame.result?.isError),
        });
      } catch {}
    }
  });
}

forward(process.stdin, child.stdin, 'client_to_server');
forward(child.stdout, process.stdout, 'server_to_client');
process.stdin.on('end', () => child.stdin.end());
child.on('exit', (code, signal) => {
  fs.closeSync(stderr);
  append(lifecycle, { event: 'exit', pid: child.pid, code, signal });
  process.exitCode = code ?? 1;
});
for (const signal of ['SIGTERM', 'SIGINT']) {
  process.on(signal, () => child.kill(signal));
}
