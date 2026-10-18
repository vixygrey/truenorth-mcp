'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { McpError, startSession } = require('./lib/mcp-session.js');

const ROOT = path.resolve(__dirname, '..');
const FIXTURES = path.join(ROOT, 'benchmarks/jev/guard-pilot/fixtures-v2.json');
const SOURCE = path.join(ROOT, 'benchmarks/jev/guard-pilot/source-v2.json');
const RATIONALES = path.join(ROOT, 'benchmarks/jev/guard-pilot/rationales-v2.json');
const POLICY = path.join(ROOT, 'benchmarks/jev/guard-pilot/policy-v1.yml');
const CLIENT_META = {
  'io.modelcontextprotocol/protocolVersion': '2026-07-28',
  'io.modelcontextprotocol/clientInfo': { name: 'truenorth-guard-pilot', version: '1.0.0' },
  'io.modelcontextprotocol/clientCapabilities': {},
};

function sha256(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

function caseDigest(caseId) {
  return crypto.createHash('sha256').update(caseId).digest('hex');
}

function safePath(value) {
  if (
    typeof value !== 'string' ||
    !value ||
    path.posix.isAbsolute(value) ||
    value.includes('\\') ||
    value.split('/').some((part) => !part || part === '.' || part === '..') ||
    value === '.agent' ||
    value === '.github' ||
    value.startsWith('.agent/') ||
    value.startsWith('.github/') ||
    value.includes('\0')
  ) {
    throw new Error('invalid pilot fixture path');
  }
  return value;
}

function loadFixtures() {
  // These JSON pilot fixture/source/rationale files need no extra parser.
  const fixtures = JSON.parse(fs.readFileSync(FIXTURES, 'utf8'));
  const source = JSON.parse(fs.readFileSync(SOURCE, 'utf8'));
  const rationales = JSON.parse(fs.readFileSync(RATIONALES, 'utf8'));
  if (
    fixtures.schema_version !== 1 ||
    source.schema_version !== 1 ||
    source.name !== 'guard-pilot-issue344' ||
    source.version !== '2' ||
    fixtures.source?.name !== 'guard-pilot-issue344' ||
    fixtures.source?.version !== '2' ||
    source.rubric !== 'guard-pilot-review-v2' ||
    source.supersedes?.source !== 'source-v1.json' ||
    source.supersedes?.rubric !== 'guard-pilot-review-v1' ||
    source.supersedes?.status !== 'failed_unaccepted' ||
    rationales.schema_version !== 1 ||
    rationales.status !== 'candidate_for_operator_review_not_provider_acceptance' ||
    fixtures.source?.sha256 !== sha256(SOURCE) ||
    source.rationales_sha256 !== sha256(RATIONALES) ||
    !Array.isArray(fixtures.cases) ||
    fixtures.cases.length !== 10 ||
    rationales.cases?.length !== 10 ||
    source.label_status?.includes('candidate') !== true
  ) {
    throw new Error('pilot source, fixture, or rationale manifest mismatch');
  }
  const ids = new Set();
  for (const [index, testCase] of fixtures.cases.entries()) {
    if (
      typeof testCase.case_id !== 'string' ||
      ids.has(testCase.case_id) ||
      rationales.cases[index]?.case_id !== testCase.case_id ||
      !Array.isArray(testCase.files) ||
      !Array.isArray(testCase.proposal?.paths) ||
      testCase.proposal.paths.length !== 1 ||
      typeof testCase.proposal.content !== 'string' ||
      typeof testCase.proposal.task !== 'string' ||
      !testCase.proposal.task ||
      !['allow', 'annotate'].includes(testCase.expected?.decision)
    ) {
      throw new Error('invalid pilot case or rationale alignment');
    }
    ids.add(testCase.case_id);
    const files = new Set();
    for (const file of testCase.files) {
      safePath(file.path);
      if (files.has(file.path) || typeof file.content !== 'string') {
        throw new Error('invalid or duplicate pilot source');
      }
      files.add(file.path);
    }
    testCase.proposal.paths.forEach(safePath);
  }
  return fixtures;
}

function loadPricing() {
  const policy = fs.readFileSync(POLICY, 'utf8');
  const value = (key) => {
    const line = policy.match(new RegExp(`^  ${key}: ([^\\n#]+)`, 'm'));
    const parsed = Number(line?.[1].trim());
    if (!line || !Number.isFinite(parsed) || parsed < 0) {
      throw new Error(`invalid pricing ${key}`);
    }
    return parsed;
  };
  if (!/^approval: pending\s*$/m.test(policy) || !/^  currency: USD\s*$/m.test(policy)) {
    throw new Error('pilot requires pending policy and USD pricing; no acceptance is permitted');
  }
  const expectedModel = policy.match(/^concrete_model: ([a-zA-Z0-9._-]+)\s*$/m)?.[1];
  if (!expectedModel) {
    throw new Error('missing priced concrete model identity');
  }
  const maximumCaseCost = value('maximum_cost_per_case');
  if (maximumCaseCost <= 0) {
    throw new Error('invalid positive per-case cost ceiling');
  }
  return {
    input: value('input_per_million'),
    output: value('output_per_million'),
    expectedModel,
    maximumCaseCost,
    policy_sha256: sha256(POLICY),
  };
}

function requireTrustedReportPath(report) {
  const temp = process.env.RUNNER_TEMP;
  if (!temp || !path.isAbsolute(temp) || path.dirname(report) !== path.resolve(temp)) {
    throw new Error('pilot report must be a direct child of RUNNER_TEMP; no call made');
  }
  let component = path.parse(temp).root;
  for (const part of path.relative(component, temp).split(path.sep)) {
    if (!part) continue;
    component = path.join(component, part);
    const stat = fs.lstatSync(component);
    if (!stat.isDirectory() || stat.isSymbolicLink()) {
      throw new Error('pilot report parent contains a symlink or non-directory; no call made');
    }
  }
  try {
    fs.lstatSync(report);
    throw new Error('pilot report must not already exist; no call made');
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  fs.accessSync(path.dirname(report), fs.constants.W_OK);
}

function preflight() {
  const budget = Number(process.env.JEV_BENCH_MAX_ESTIMATED_USD);
  if (
    !process.env.JEV_BENCH_MAX_ESTIMATED_USD ||
    !Number.isFinite(budget) ||
    budget <= 0 ||
    budget > 0.05 ||
    process.env.JEV_BENCH_PROVIDER_SPEND_CAP_CONFIRMED !== 'true'
  ) {
    throw new Error(
      'pilot requires reviewed positive JEV_BENCH_MAX_ESTIMATED_USD at most 0.05 and JEV_BENCH_PROVIDER_SPEND_CAP_CONFIRMED=true; no call made',
    );
  }
  if (
    !process.env.TRUENORTH_JEV_ENDPOINT ||
    !process.env.TRUENORTH_JEV_API_KEY ||
    process.env.JEV_GUARD_PILOT_APPROVED !== 'true'
  ) {
    throw new Error(
      'pilot requires protected provider injection and separate explicit dispatch approval; no call made',
    );
  }
  const separator = process.argv.indexOf('--');
  if (separator < 0 || separator !== process.argv.length - 2) {
    throw new Error(
      'usage: node scripts/jev-guard-pilot.js [--report <path>] -- <prebuilt-truenorth-mcp-binary>',
    );
  }
  const args = process.argv.slice(2, separator);
  let report;
  if (args.length === 2 && args[0] === '--report') {
    report = path.resolve(args[1]);
  } else if (args.length) {
    throw new Error(
      'usage: node scripts/jev-guard-pilot.js [--report <path>] -- <prebuilt-truenorth-mcp-binary>',
    );
  }
  if (report) {
    requireTrustedReportPath(report);
  }
  const binary = path.resolve(process.argv[separator + 1]);
  if (!fs.statSync(binary).isFile()) {
    throw new Error('the prebuilt MCP binary must be a file');
  }
  if (process.env.GITHUB_SHA && !/^[0-9a-f]{40}$/i.test(process.env.GITHUB_SHA)) {
    throw new Error('invalid GITHUB_SHA code provenance; no call made');
  }
  return { budget, binary, report, codeSha: process.env.GITHUB_SHA ?? null };
}

function makeProject(testCase, pricing) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'tn-guard-pilot-'));
  try {
    fs.mkdirSync(path.join(root, '.agent/config'), { recursive: true });
    fs.mkdirSync(path.join(root, 'skills'));
    fs.mkdirSync(path.join(root, 'specs'));
    fs.writeFileSync(
      path.join(root, '.agent/config/rules.yml'),
      `features:\n  jev: true\njev:\n  retry_limit: 0\n  price:\n    input_per_million: ${pricing.input}\n    output_per_million: ${pricing.output}\n`,
      { mode: 0o600 },
    );
    for (const file of testCase.files) {
      const target = path.join(root, file.path);
      fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.writeFileSync(target, file.content);
    }
    return root;
  } catch (error) {
    fs.rmSync(root, { recursive: true, force: true });
    throw error;
  }
}

function parseReply(result) {
  const text = result?.content?.find((block) => block.type === 'text')?.text;
  if (typeof text !== 'string') {
    throw new Error('missing guard decision');
  }
  const body = JSON.parse(text);
  if (!['allow', 'annotate'].includes(body.decision)) {
    throw new Error('invalid guard decision');
  }
  return body;
}

function requireOfflineAccounting(accounting) {
  if (
    accounting?.schema_version !== 1 ||
    accounting.input_tokens !== 0 ||
    accounting.output_tokens !== 0 ||
    accounting.transport_attempts !== 0 ||
    accounting.unknown_usage_calls !== 0 ||
    !Array.isArray(accounting.observed_models) ||
    accounting.observed_models.length !== 0 ||
    !Number.isSafeInteger(accounting.elapsed_ms) ||
    accounting.elapsed_ms < 0
  ) {
    throw new Error('production MCP lacks verified zero-call accounting; no paid proposal made');
  }
}

async function preflightGuard(session, params) {
  try {
    await session.request(
      'tools/call',
      params({
        name: 'truenorth_guard_change',
        arguments: { paths: ['specs/pilot-preflight.md'], content: 'pilot preflight only' },
      }),
    );
  } catch (error) {
    if (
      error instanceof McpError &&
      error.code === -32600 &&
      error.data?.violated_check === 'protected-path'
    ) {
      requireOfflineAccounting(error.data.accounting);
      return;
    }
  }
  throw new Error('production MCP did not block protected-path preflight; no paid proposal made');
}

function parseAccounting(body, pricing) {
  const accounting = body.accounting;
  if (
    !accounting ||
    accounting.schema_version !== 1 ||
    !Number.isSafeInteger(accounting.input_tokens) ||
    accounting.input_tokens < 0 ||
    !Number.isSafeInteger(accounting.output_tokens) ||
    accounting.output_tokens < 0 ||
    !Number.isSafeInteger(accounting.transport_attempts) ||
    accounting.transport_attempts !== 2 ||
    accounting.unknown_usage_calls !== 0 ||
    !Array.isArray(accounting.observed_models) ||
    accounting.observed_models.length < 1 ||
    accounting.observed_models.some((id) => id !== pricing.expectedModel) ||
    !Number.isFinite(accounting.elapsed_ms) ||
    accounting.elapsed_ms < 0
  ) {
    throw new Error('unknown, incomplete or retried production usage; stopping without cost claim');
  }
  return {
    input_tokens: accounting.input_tokens,
    output_tokens: accounting.output_tokens,
    transport_attempts: accounting.transport_attempts,
    observed_models: accounting.observed_models,
    elapsed_ms: accounting.elapsed_ms,
    estimated_usd:
      (accounting.input_tokens * pricing.input + accounting.output_tokens * pricing.output) / 1e6,
  };
}

function classifyFindings(body, expected) {
  const findings = body.findings;
  const rigor = findings?.rigor;
  const score = (number, maximum) =>
    typeof number === 'number' && Number.isFinite(number) && number >= 0 && number <= maximum;
  const rigor_scores =
    rigor &&
    score(rigor.hallucinated_import, 1) &&
    score(rigor.violates_conventions, 1) &&
    score(rigor.contains_secrets, 1) &&
    score(rigor.complexity, 100)
      ? {
          hallucinated_import: rigor.hallucinated_import,
          violates_conventions: rigor.violates_conventions,
          contains_secrets: rigor.contains_secrets,
          complexity: rigor.complexity,
        }
      : null;
  const notes = body.notes;
  if (
    !Array.isArray(notes) ||
    notes.length > 12 ||
    notes.some((note) => typeof note !== 'string' || note.length > 1_000)
  ) {
    return {
      observed: null,
      matches: false,
      rigor_scores,
      visible_notes: [],
      unavailable: 'invalid_notes',
    };
  }
  if (body.decision === 'annotate' && notes.length === 0) {
    return {
      observed: null,
      matches: false,
      rigor_scores,
      visible_notes: notes,
      unavailable: 'missing_advisory',
    };
  }
  const contexts = findings?.unavailable_context;
  if (
    !Array.isArray(contexts) ||
    contexts.length > 20 ||
    contexts.some(
      (context) => typeof context !== 'string' || !/^[a-zA-Z0-9_./: -]{1,80}$/.test(context),
    )
  ) {
    return {
      observed: null,
      matches: false,
      rigor_scores,
      visible_notes: notes,
      unavailable: 'invalid_context',
    };
  }
  const drift = findings.drift;
  if (findings.unavailable !== null || !rigor || !drift) {
    return {
      observed: null,
      matches: false,
      rigor_scores,
      visible_notes: notes,
      unavailable:
        typeof findings.unavailable === 'string' ? 'model_unavailable' : 'incomplete_findings',
      unavailable_context: contexts,
    };
  }
  if (
    !rigor_scores ||
    !score(drift.noul, 1) ||
    typeof drift.out_of_scope !== 'boolean' ||
    typeof drift.forced_by_path_match !== 'boolean' ||
    drift.out_of_scope !== (drift.forced_by_path_match || drift.noul >= 0.5)
  ) {
    return {
      observed: null,
      matches: false,
      rigor_scores,
      visible_notes: notes,
      unavailable: 'invalid_scores',
      unavailable_context: contexts,
    };
  }
  const observed = {
    hallucinated_import: rigor.hallucinated_import >= 0.7,
    violates_conventions: rigor.violates_conventions >= 0.7,
    contains_secrets: rigor.contains_secrets >= 0.25,
    complexity_high: rigor.complexity >= 62.5,
    drift_out_of_scope: drift.out_of_scope,
  };
  const expectedEntries = Object.entries(expected);
  const matches =
    expectedEntries.length > 0 &&
    expectedEntries.every(
      ([key, value]) =>
        Object.hasOwn(observed, key) && typeof value === 'boolean' && observed[key] === value,
    );
  return {
    rigor_scores,
    observed,
    matches,
    visible_notes: notes,
    unavailable: null,
    unavailable_context: contexts,
  };
}

async function runCase(testCase, binary, pricing) {
  const root = makeProject(testCase, pricing);
  const before = [
    ...testCase.files.map((file) => [file.path, sha256(path.join(root, file.path))]),
    ['.agent/config/rules.yml', sha256(path.join(root, '.agent/config/rules.yml'))],
  ];
  const childEnv = Object.fromEntries(
    ['PATH', 'HOME', 'TRUENORTH_JEV_ENDPOINT', 'TRUENORTH_JEV_API_KEY']
      .filter((key) => process.env[key] !== undefined)
      .map((key) => [key, process.env[key]]),
  );
  const session = startSession(binary, [], root, { requestTimeoutMs: 120_000, env: childEnv });
  try {
    const params = (value) => ({ ...value, _meta: CLIENT_META });
    const tools = await session.request('tools/list', params({}));
    const guard = tools?.tools?.find((tool) => tool.name === 'truenorth_guard_change');
    if (!guard) {
      throw new Error('production MCP guard is unavailable; no paid call made');
    }
    await preflightGuard(session, params);
    let body;
    try {
      body = parseReply(
        await session.request(
          'tools/call',
          params({
            name: 'truenorth_guard_change',
            arguments: testCase.proposal,
          }),
        ),
      );
    } catch (error) {
      if (!(error instanceof McpError) || error.code !== -32600) {
        throw error;
      }
      body = {
        decision: 'block',
        accounting: error.data?.accounting,
        findings: error.data?.findings,
        notes: error.data?.notes,
      };
    }
    const accounting = parseAccounting(body, pricing);
    const classified = classifyFindings(body, testCase.expected.findings);
    for (const [relative, hash] of before) {
      if (sha256(path.join(root, relative)) !== hash) {
        throw new Error('guard mutated initial source');
      }
    }
    if (
      testCase.proposal.paths.some(
        (relative) =>
          !before.some(([original]) => original === relative) &&
          fs.existsSync(path.join(root, relative)),
      )
    ) {
      throw new Error('guard created proposed source');
    }
    let process_exit_ok = true;
    try {
      await session.close();
    } catch {
      process_exit_ok = false;
    }
    return {
      case_id_sha256: caseDigest(testCase.case_id),
      guard_config_sha256: sha256(path.join(root, '.agent/config/rules.yml')),
      expected_decision: testCase.expected.decision,
      observed_decision: body.decision,
      decision_matches: testCase.expected.decision === body.decision,
      expected_findings: testCase.expected.findings,
      rigor_scores: classified.rigor_scores,
      observed_findings: classified.observed,
      findings_match: classified.matches,
      findings_unavailable: classified.unavailable,
      unavailable_context: classified.unavailable_context ?? [],
      visible_notes: classified.visible_notes,
      process_exit_ok,
      accounting,
    };
  } finally {
    await session.terminate();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function main() {
  const { budget, binary, report, codeSha } = preflight();
  const fixtures = loadFixtures();
  const pricing = loadPricing();
  const evidence = {
    schema_version: 2,
    fixture_version: '2',
    stage: 'guard_pilot',
    accepted: false,
    policy_approval: 'pending',
    source_sha256: sha256(SOURCE),
    fixtures_sha256: sha256(FIXTURES),
    rationales_sha256: sha256(RATIONALES),
    policy_sha256: pricing.policy_sha256,
    binary_sha256: sha256(binary),
    code_sha: codeSha,
    prompt_source_sha256: {
      rigor: sha256(path.join(ROOT, 'runtime/src/engine/jev/rigor.rs')),
      drift: sha256(path.join(ROOT, 'runtime/src/engine/jev/drift.rs')),
    },
    reviewed_max_estimated_usd: budget,
    provider_spend_cap_confirmed: true,
    retry_limit: 0,
    maximum_case_estimated_usd: pricing.maximumCaseCost,
    cases: [],
    known_estimated_subtotal_usd: 0,
    estimated_total_usd: null,
    usage_complete: true,
    stopped_reason: null,
  };
  try {
    for (const testCase of fixtures.cases) {
      if (evidence.known_estimated_subtotal_usd >= budget) {
        throw new Error('reviewed estimated budget reached; no further provider call made');
      }
      const result = await runCase(testCase, binary, pricing);
      evidence.cases.push(result);
      evidence.known_estimated_subtotal_usd += result.accounting.estimated_usd;
      if (result.accounting.estimated_usd > pricing.maximumCaseCost) {
        throw new Error(
          'reviewed per-case estimated budget exceeded; no further provider call made',
        );
      }
      if (result.observed_decision === 'block') {
        throw new Error('model-driven hard block');
      }
      if (!result.process_exit_ok) {
        throw new Error('production MCP exited abnormally');
      }
      if (!result.decision_matches || !result.findings_match || result.findings_unavailable) {
        throw new Error('pilot finding or decision mismatch');
      }
      if (evidence.known_estimated_subtotal_usd > budget) {
        throw new Error('reviewed estimated budget exceeded; no further provider call made');
      }
    }
    evidence.estimated_total_usd = evidence.known_estimated_subtotal_usd;
    evidence.stopped_reason = 'pilot complete; pending policy is nonaccepting';
  } catch (error) {
    const budgetBoundary = error.message.includes('budget');
    const modelHardBlock = error.message === 'model-driven hard block';
    const findingMismatch = error.message === 'pilot finding or decision mismatch';
    if (!budgetBoundary && !modelHardBlock && !findingMismatch) {
      evidence.usage_complete = false;
    }
    if (evidence.usage_complete) {
      evidence.estimated_total_usd = evidence.known_estimated_subtotal_usd;
    }
    evidence.stopped_reason = budgetBoundary
      ? 'estimated budget boundary'
      : modelHardBlock
        ? 'model-driven hard block'
        : findingMismatch
          ? 'pilot finding or decision mismatch'
          : 'guard result, usage, or transport unavailable; operator investigation required';
  }
  if (report) {
    fs.writeFileSync(report, `${JSON.stringify(evidence, null, 2)}\n`, { flag: 'wx', mode: 0o600 });
  }
  console.log(JSON.stringify(evidence));
  if (
    evidence.cases.length !== 10 ||
    evidence.stopped_reason !== 'pilot complete; pending policy is nonaccepting'
  ) {
    process.exitCode = 1;
  }
}

if (require.main === module) {
  main().catch((error) => {
    // Never print caught transport errors: they can carry stderr or an untrusted provider payload.
    console.error(
      `Guard pilot stopped before dispatch: ${error.message.includes('pilot') || error.message.includes('budget') ? error.message : 'invalid preflight or source manifest'}`,
    );
    process.exitCode = 1;
  });
}

module.exports = { classifyFindings, loadFixtures };
