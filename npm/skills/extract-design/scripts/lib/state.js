import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';
import { log } from './logging.js';
const STATE_PATH = '.agent/tasks/state.yml';

export function writeGrillMeHandoff(ctx = {}) {
  const decisions = (ctx.uncertainDecisions || []).map((d) => `    - ${d}`);
  const hb = [
    'handoff:',
    '  next_skill: grill-me',
    '  last_step_completed: extract-design',
    '  context: >',
    `    Design extraction produced ${ctx.tokenCount || '?'} tokens and ${ctx.componentCount || '?'} components; ${ctx.uncertainCount || 0} decisions remain open.`,
    decisions.length > 0 ? '  open_decisions:' : '  open_decisions: []',
    ...decisions,
    '  required_reading:',
    '    - .agent/product/design.md',
    '  group_id: null',
    '  artifacts_summary: null',
    '  git_context: null',
  ];
  try {
    const d = dirname(STATE_PATH);
    if (!existsSync(d)) mkdirSync(d, { recursive: true });
    let c = existsSync(STATE_PATH) ? readFileSync(STATE_PATH, 'utf8') : '';
    if (!c.includes('next_skill:')) {
      c = c.trimEnd() + '\n' + hb.join('\n') + '\n';
      writeFileSync(STATE_PATH, c, 'utf8');
      log.info('handoff-written', { nextSkill: 'grill-me' });
    }
  } catch (e) {
    log.warn('handoff-failed', { error: e.message });
  }
}
