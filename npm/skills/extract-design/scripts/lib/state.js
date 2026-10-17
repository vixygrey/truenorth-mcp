import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';
import { log } from './logging.js';
const STATE_PATH = '.agent/tasks/state.json';

export function writeGrillMeHandoff(ctx = {}) {
  try {
    const d = dirname(STATE_PATH);
    if (!existsSync(d)) mkdirSync(d, { recursive: true });
    const state = existsSync(STATE_PATH) ? JSON.parse(readFileSync(STATE_PATH, 'utf8')) : {};
    state.handoff = {
      ...(state.handoff && typeof state.handoff === 'object' && !Array.isArray(state.handoff)
        ? state.handoff
        : {}),
      next_skill: 'grill-me',
      last_step_completed: 'extract-design',
      context: `Design extraction produced ${ctx.tokenCount || '?'} tokens and ${ctx.componentCount || '?'} components; ${ctx.uncertainCount || 0} decisions remain open.`,
      open_decisions: ctx.uncertainDecisions || [],
      required_reading: ['.agent/product/design.md'],
      group_id: null,
      artifacts_summary: null,
      git_context: null,
    };
    writeFileSync(STATE_PATH, `${JSON.stringify(state, null, 2)}\n`, 'utf8');
    log.info('handoff-written', { nextSkill: 'grill-me' });
  } catch (e) {
    log.warn('handoff-failed', { error: e.message });
  }
}
