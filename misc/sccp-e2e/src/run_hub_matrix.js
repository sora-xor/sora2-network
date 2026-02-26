#!/usr/bin/env node

'use strict';

const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');

const DOMAIN_ORDER = ['sora', 'eth', 'bsc', 'sol', 'ton', 'tron'];
const DOMAIN_TO_ID = {
  sora: 0,
  eth: 1,
  bsc: 2,
  sol: 3,
  ton: 4,
  tron: 5,
};

function nowIso() {
  return new Date().toISOString();
}

function tsForPath() {
  const d = new Date();
  const pad = (v) => String(v).padStart(2, '0');
  return [
    d.getUTCFullYear(),
    pad(d.getUTCMonth() + 1),
    pad(d.getUTCDate()),
    '-',
    pad(d.getUTCHours()),
    pad(d.getUTCMinutes()),
    pad(d.getUTCSeconds()),
  ].join('');
}

function xmlEscape(value) {
  return String(value)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;');
}

function parseArgs(argv) {
  const out = {
    config: null,
    maxMinutes: null,
    dryRun: false,
    skipPreflight: false,
    includeNegative: null,
    scenario: null,
    strictAdapters: false,
    matrix: 'full',
    artifactsDir: null,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const next = argv[i + 1];
    if ((arg === '--config' || arg === '-c') && next) {
      out.config = next;
      i += 1;
    } else if (arg === '--max-minutes' && next) {
      out.maxMinutes = Number(next);
      i += 1;
    } else if (arg === '--dry-run') {
      out.dryRun = true;
    } else if (arg === '--skip-preflight') {
      out.skipPreflight = true;
    } else if (arg === '--include-negative') {
      out.includeNegative = true;
    } else if (arg === '--exclude-negative') {
      out.includeNegative = false;
    } else if (arg === '--scenario' && next) {
      out.scenario = next;
      i += 1;
    } else if (arg === '--strict-adapters') {
      out.strictAdapters = true;
    } else if (arg === '--matrix' && next) {
      out.matrix = next;
      i += 1;
    } else if (arg === '--artifacts-dir' && next) {
      out.artifactsDir = next;
      i += 1;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return out;
}

function printHelp() {
  process.stdout.write(
    [
      'Usage: node misc/sccp-e2e/src/run_hub_matrix.js [options]',
      '',
      'Options:',
      '  -c, --config <path>        Config JSON path',
      '  --max-minutes <n>          Max wall clock budget in minutes',
      '  --dry-run                  Plan and validate commands without executing',
      '  --skip-preflight           Skip misc/sccp/run_all_tests.sh preflight',
      '  --include-negative         Force-enable negative checks',
      '  --exclude-negative         Disable negative checks',
      '  --scenario <src:dst>       Run a single scenario (example: eth:sol)',
      '  --strict-adapters          Require adapter scripts for all non-sora domains',
      '  --matrix <full|sora-pairs> Matrix mode (default: full)',
      '  --artifacts-dir <path>     Output directory for run artifacts',
      '  -h, --help                 Show help',
    ].join('\n') + '\n'
  );
}

function readJson(filePath) {
  const raw = fs.readFileSync(filePath, 'utf8');
  return JSON.parse(raw);
}

function ensureDir(dirPath) {
  fs.mkdirSync(dirPath, { recursive: true });
}

function resolveWithVars(input, vars) {
  if (typeof input !== 'string') {
    return input;
  }
  return input.replace(/\$\{([^}]+)\}/g, (_, name) => {
    if (!(name in vars)) {
      throw new Error(`Unknown variable '${name}' in '${input}'`);
    }
    return String(vars[name]);
  });
}

function resolveConfig(rawConfig, harnessRoot) {
  const repoRoot = path.resolve(harnessRoot, '..', '..');
  const vars = {
    harnessRoot,
    repoRoot,
    sora2Network: repoRoot,
    bridgeRelayer: path.resolve(repoRoot, '..', 'bridge-relayer'),
    sccpEth: path.resolve(repoRoot, '..', 'sccp-eth'),
    sccpBsc: path.resolve(repoRoot, '..', 'sccp-bsc'),
    sccpSol: path.resolve(repoRoot, '..', 'sccp-sol'),
    sccpTon: path.resolve(repoRoot, '..', 'sccp-ton'),
    sccpTron: path.resolve(repoRoot, '..', 'sccp-tron'),
  };

  const merged = JSON.parse(JSON.stringify(rawConfig));

  if (merged.paths) {
    for (const [k, v] of Object.entries(merged.paths)) {
      const resolved = resolveWithVars(v, vars);
      merged.paths[k] = path.isAbsolute(resolved)
        ? resolved
        : path.resolve(repoRoot, resolved);
      vars[k] = merged.paths[k];
    }
  }

  const deepResolve = (value) => {
    if (Array.isArray(value)) {
      return value.map((x) => deepResolve(x));
    }
    if (value && typeof value === 'object') {
      const out = {};
      for (const [k, v] of Object.entries(value)) {
        out[k] = deepResolve(v);
      }
      return out;
    }
    return resolveWithVars(value, vars);
  };

  merged.commands = deepResolve(merged.commands || {});
  merged.defaults = deepResolve(merged.defaults || {});

  merged.harnessRoot = harnessRoot;
  merged.repoRoot = repoRoot;
  merged.vars = vars;
  return merged;
}

function execCommand({
  cmd,
  cwd,
  timeoutMs,
  env,
  logFile,
  dryRun,
  runBudget,
}) {
  return new Promise((resolve) => {
    const startedAt = Date.now();

    if (dryRun) {
      const dry = `[dry-run] cwd=${cwd} cmd=${cmd}\n`;
      if (logFile) {
        fs.writeFileSync(logFile, dry, 'utf8');
      }
      resolve({
        ok: true,
        exitCode: 0,
        signal: null,
        timedOut: false,
        durationMs: 0,
        startedAt: new Date(startedAt).toISOString(),
        endedAt: new Date(startedAt).toISOString(),
        stdout: '',
        stderr: '',
      });
      return;
    }

    if (runBudget && runBudget.exceeded) {
      resolve({
        ok: false,
        exitCode: null,
        signal: null,
        timedOut: true,
        durationMs: 0,
        startedAt: new Date(startedAt).toISOString(),
        endedAt: new Date(startedAt).toISOString(),
        stdout: '',
        stderr: 'Global run budget exceeded before command execution.',
      });
      return;
    }

    const child = spawn(cmd, {
      cwd,
      env,
      shell: true,
      stdio: ['ignore', 'pipe', 'pipe'],
    });

    let stdout = '';
    let stderr = '';

    child.stdout.on('data', (chunk) => {
      stdout += chunk.toString('utf8');
    });

    child.stderr.on('data', (chunk) => {
      stderr += chunk.toString('utf8');
    });

    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill('SIGTERM');
    }, timeoutMs);

    child.on('close', (exitCode, signal) => {
      clearTimeout(timer);
      const endedAt = Date.now();
      const durationMs = endedAt - startedAt;

      if (logFile) {
        const lines = [
          `# command`,
          cmd,
          '',
          `# cwd`,
          cwd,
          '',
          `# started_at`,
          new Date(startedAt).toISOString(),
          '',
          `# ended_at`,
          new Date(endedAt).toISOString(),
          '',
          `# duration_ms`,
          String(durationMs),
          '',
          `# exit_code`,
          String(exitCode),
          '',
          `# signal`,
          String(signal),
          '',
          `# timed_out`,
          String(timedOut),
          '',
          '# stdout',
          stdout,
          '',
          '# stderr',
          stderr,
          '',
        ];
        fs.writeFileSync(logFile, lines.join('\n'), 'utf8');
      }

      resolve({
        ok: !timedOut && exitCode === 0,
        exitCode,
        signal,
        timedOut,
        durationMs,
        startedAt: new Date(startedAt).toISOString(),
        endedAt: new Date(endedAt).toISOString(),
        stdout,
        stderr,
      });
    });
  });
}

function buildMatrix(mode) {
  const pairs = [];
  if (mode === 'sora-pairs') {
    for (const domain of DOMAIN_ORDER) {
      if (domain === 'sora') {
        continue;
      }
      pairs.push({ src: 'sora', dst: domain });
      pairs.push({ src: domain, dst: 'sora' });
    }
    return pairs;
  }

  for (const src of DOMAIN_ORDER) {
    for (const dst of DOMAIN_ORDER) {
      if (src === dst) {
        continue;
      }
      pairs.push({ src, dst });
    }
  }
  return pairs;
}

function domainLabel(domain) {
  return `${domain}(${DOMAIN_TO_ID[domain]})`;
}

function resolveDomainRepo(config, domain) {
  const map = {
    eth: config.paths.sccpEth,
    bsc: config.paths.sccpBsc,
    sol: config.paths.sccpSol,
    ton: config.paths.sccpTon,
    tron: config.paths.sccpTron,
  };
  return map[domain] || null;
}

function adapterScriptPath(config, domain) {
  const repo = resolveDomainRepo(config, domain);
  if (!repo) {
    return null;
  }
  return path.join(repo, 'scripts', 'sccp_e2e_adapter.sh');
}

function buildFallbackCommand(config, domain, action) {
  const dc = config.commands?.domains?.[domain];
  if (!dc) {
    return null;
  }
  const key = action;
  return dc[key] || null;
}

function buildDomainCommand(config, domain, action, strictAdapters) {
  const script = adapterScriptPath(config, domain);
  if (script && fs.existsSync(script)) {
    return {
      cmdBuilder: (payload) => {
        const json = JSON.stringify(payload).replace(/'/g, "'\\''");
        return `${script} ${action} --json '${json}'`;
      },
      cwd: resolveDomainRepo(config, domain),
      mode: 'adapter',
    };
  }

  if (strictAdapters) {
    return null;
  }

  const fallback = buildFallbackCommand(config, domain, action);
  if (!fallback) {
    return null;
  }

  return {
    cmdBuilder: () => fallback,
    cwd: resolveDomainRepo(config, domain),
    mode: 'fallback',
  };
}

function normalizeScenarioArg(value) {
  if (!value) {
    return null;
  }
  const [src, dst] = value.split(':');
  if (!src || !dst) {
    throw new Error(`Invalid --scenario value '${value}'. Use <src:dst>, for example eth:sol.`);
  }
  if (!DOMAIN_ORDER.includes(src) || !DOMAIN_ORDER.includes(dst)) {
    throw new Error(`Unknown domain in --scenario '${value}'. Supported: ${DOMAIN_ORDER.join(', ')}.`);
  }
  if (src === dst) {
    throw new Error(`--scenario '${value}' is invalid; source and destination must differ.`);
  }
  return { src, dst };
}

function scenarioId(index, src, dst) {
  return `${String(index + 1).padStart(2, '0')}-${src}-to-${dst}`;
}

function classifyFailure(step) {
  if (!step) {
    return 'SCENARIO_FAILED';
  }
  if (step.name === 'budget_guard') {
    return 'BUDGET_EXCEEDED';
  }
  if (step.kind === 'domain' && step.action === 'burn') {
    return 'SOURCE_BURN_FAILED';
  }
  if (step.kind === 'bridgeRelayer') {
    return 'RELAYER_PROOF_BUILD_FAILED';
  }
  if (step.kind === 'domain' && step.action === 'mint_verify') {
    return 'DEST_MINT_FAILED';
  }
  if (
    (step.kind === 'domain' && step.action === 'negative_verify') ||
    (step.kind === 'sora' && step.action === 'negative')
  ) {
    return 'INVARIANT_FAILED';
  }
  if (step.kind === 'sora') {
    return 'SORA_ATTEST_OR_MINT_FAILED';
  }
  return 'SCENARIO_FAILED';
}

function buildScenarioPayload(scenario) {
  return {
    scenario_id: scenario.id,
    source_domain: DOMAIN_TO_ID[scenario.src],
    dest_domain: DOMAIN_TO_ID[scenario.dst],
    source_label: domainLabel(scenario.src),
    dest_label: domainLabel(scenario.dst),
  };
}

function parseAdapterJson(stdout) {
  if (typeof stdout !== 'string') {
    return null;
  }
  const trimmed = stdout.trim();
  if (!trimmed) {
    return null;
  }
  const lines = trimmed
    .split('\n')
    .map((x) => x.trim())
    .filter((x) => x.length > 0);
  for (let i = lines.length - 1; i >= 0; i -= 1) {
    try {
      const parsed = JSON.parse(lines[i]);
      if (parsed && typeof parsed === 'object') {
        return parsed;
      }
    } catch (_) {
      // keep scanning upward for the last JSON line
    }
  }
  return null;
}

async function runPreflight({ config, args, artifactsDir, timeoutMs, runBudget }) {
  const pf = config.commands?.preflight;
  if (!pf || !pf.enabled) {
    return {
      skipped: true,
      ok: true,
      reason: 'preflight disabled in config',
    };
  }

  if (args.skipPreflight) {
    return {
      skipped: true,
      ok: true,
      reason: '--skip-preflight was set',
    };
  }

  const logFile = path.join(artifactsDir, 'preflight.log');
  const result = await execCommand({
    cmd: pf.cmd,
    cwd: pf.cwd || config.repoRoot,
    timeoutMs,
    env: process.env,
    logFile,
    dryRun: args.dryRun,
    runBudget,
  });

  return {
    skipped: false,
    ok: result.ok,
    reason: result.ok ? 'ok' : 'preflight command failed',
    command: pf.cmd,
    cwd: pf.cwd || config.repoRoot,
    log_file: logFile,
    result,
  };
}

function checkRequiredPaths(config) {
  const required = {
    sora2Network: config.paths.sora2Network,
    bridgeRelayer: config.paths.bridgeRelayer,
    sccpEth: config.paths.sccpEth,
    sccpBsc: config.paths.sccpBsc,
    sccpSol: config.paths.sccpSol,
    sccpTon: config.paths.sccpTon,
    sccpTron: config.paths.sccpTron,
  };

  const missing = [];
  for (const [name, value] of Object.entries(required)) {
    if (!value || !fs.existsSync(value)) {
      missing.push({ name, path: value || '(undefined)' });
    }
  }
  return missing;
}

async function runScenario({
  config,
  scenario,
  args,
  timeoutMs,
  artifactsDir,
  runBudget,
  commandCache,
}) {
  const scenarioDir = path.join(artifactsDir, scenario.id);
  ensureDir(scenarioDir);

  const steps = [];
  const payload = buildScenarioPayload(scenario);

  const addStep = (name, kind, domain, action, runner) => {
    steps.push({ name, kind, domain, action, runner });
  };

  const runSoraCommand = (action) => {
    const cmd = config.commands?.sora?.[action];
    if (!cmd) {
      return {
        ok: false,
        skipped: true,
        reason: `Missing sora command mapping for action '${action}'`,
      };
    }
    return {
      type: 'command',
      cmd,
      cwd: config.paths.sora2Network,
      cacheKey: `sora:${config.paths.sora2Network}:${cmd}`,
    };
  };

  const runBridgeRelayerCommand = () => {
    const cmd = config.commands?.bridgeRelayer?.proof_toolchain;
    if (!cmd) {
      return {
        ok: false,
        skipped: true,
        reason: 'Missing bridgeRelayer.proof_toolchain command mapping',
      };
    }
    return {
      type: 'command',
      cmd,
      cwd: config.paths.bridgeRelayer,
      cacheKey: `bridgeRelayer:${config.paths.bridgeRelayer}:${cmd}`,
    };
  };

  const runDomainAction = (domain, action) => {
    const spec = buildDomainCommand(config, domain, action, args.strictAdapters);
    if (!spec) {
      return {
        ok: false,
        skipped: true,
        reason: `No command mapping for ${domain}:${action}`,
      };
    }
    return {
      type: 'domain',
      mode: spec.mode,
      cmd: spec.cmdBuilder(payload),
      cwd: spec.cwd,
      cacheKey:
        spec.mode === 'fallback'
          ? `fallback:${domain}:${spec.cwd}:${spec.cmdBuilder(payload)}`
          : `adapter:${domain}:${action}:${spec.cwd}`,
    };
  };

  if (scenario.src === 'sora') {
    addStep('sora_burn', 'sora', 'sora', 'burn', runSoraCommand('burn'));
  } else {
    addStep('source_burn', 'domain', scenario.src, 'burn', runDomainAction(scenario.src, 'burn'));
  }

  if (scenario.dst === 'sora') {
    if (scenario.src === 'sora') {
      addStep('sora_self_guard', 'sora', 'sora', 'noop', {
        ok: false,
        skipped: true,
        reason: 'source and destination are both sora (filtered upstream)',
      });
    } else {
      addStep(
        'sora_mint_from_source',
        'sora',
        'sora',
        'mint_from_source',
        runSoraCommand('mint_from_source')
      );
    }
  } else {
    if (scenario.src !== 'sora') {
      addStep('sora_attest', 'sora', 'sora', 'attest', runSoraCommand('attest'));
    }
    addStep('proof_toolchain', 'bridgeRelayer', 'bridgeRelayer', 'proof_toolchain', runBridgeRelayerCommand());
    addStep(
      'dest_mint_verify',
      'domain',
      scenario.dst,
      'mint_verify',
      runDomainAction(scenario.dst, 'mint_verify')
    );
  }

  const includeNegative =
    args.includeNegative === null
      ? Boolean(config.defaults?.includeNegative)
      : Boolean(args.includeNegative);

  if (includeNegative) {
    if (scenario.dst === 'sora') {
      addStep('negative_sora', 'sora', 'sora', 'negative', runSoraCommand('negative'));
    } else {
      addStep(
        'negative_dest',
        'domain',
        scenario.dst,
        'negative_verify',
        runDomainAction(scenario.dst, 'negative_verify')
      );
    }
  }

  const stepResults = [];
  let scenarioOk = true;

  for (const [stepIndex, step] of steps.entries()) {
    const logFile = path.join(scenarioDir, `${String(stepIndex + 1).padStart(2, '0')}-${step.name}.log`);

    if (step.runner && step.runner.skipped) {
      const skippedResult = {
        name: step.name,
        kind: step.kind,
        domain: step.domain,
        action: step.action,
        skipped: true,
        ok: false,
        reason: step.runner.reason,
        log_file: logFile,
      };
      fs.writeFileSync(logFile, `[skipped] ${step.runner.reason}\n`, 'utf8');
      stepResults.push(skippedResult);
      scenarioOk = false;
      break;
    }

    const runner = step.runner;
    const fromCache = commandCache.get(runner.cacheKey);
    if (fromCache) {
      const cached = {
        name: step.name,
        kind: step.kind,
        domain: step.domain,
        action: step.action,
        skipped: false,
        ok: fromCache.ok,
        cached: true,
        mode: runner.mode || 'command',
        cmd: runner.cmd,
        cwd: runner.cwd,
        log_file: fromCache.log_file,
        result: fromCache.result,
      };
      stepResults.push(cached);
      if (!cached.ok) {
        scenarioOk = false;
        break;
      }
      continue;
    }

    const result = await execCommand({
      cmd: runner.cmd,
      cwd: runner.cwd,
      timeoutMs,
      env: process.env,
      logFile,
      dryRun: args.dryRun,
      runBudget,
    });

    const normalized = {
      name: step.name,
      kind: step.kind,
      domain: step.domain,
      action: step.action,
      skipped: false,
      ok: result.ok,
      cached: false,
      mode: runner.mode || 'command',
      cmd: runner.cmd,
      cwd: runner.cwd,
      log_file: logFile,
      result,
    };

    if (normalized.mode === 'adapter') {
      if (args.dryRun) {
        normalized.adapter_output = { ok: true, dry_run: true };
      } else {
        const parsed = parseAdapterJson(result.stdout);
        normalized.adapter_output = parsed;
        if (!parsed || parsed.ok !== true) {
          normalized.ok = false;
        }
      }
    }

    stepResults.push(normalized);

    commandCache.set(runner.cacheKey, {
      ok: normalized.ok,
      log_file: normalized.log_file,
      result: normalized.result,
    });

    if (!normalized.ok) {
      scenarioOk = false;
      break;
    }
  }

  const failedStep = stepResults.find((s) => !s.ok);
  return {
    id: scenario.id,
    src: scenario.src,
    dst: scenario.dst,
    started_at: nowIso(),
    ok: scenarioOk,
    failure_code: scenarioOk ? null : classifyFailure(failedStep),
    repro_command: scenarioOk
      ? null
      : `misc/sccp-e2e/run_hub_matrix.sh --scenario ${scenario.src}:${scenario.dst} --skip-preflight`,
    steps: stepResults,
  };
}

function buildJUnit(results, suiteName) {
  const total = results.length;
  const failures = results.filter((r) => !r.ok).length;

  const cases = results.map((result) => {
    const className = `${result.src}_to_${result.dst}`;
    const name = result.id;
    if (result.ok) {
      return `    <testcase classname="${xmlEscape(className)}" name="${xmlEscape(name)}"/>`;
    }

    const failedStep = result.steps.find((s) => !s.ok) || result.steps[result.steps.length - 1];
    const failureCode = result.failure_code || classifyFailure(failedStep);
    const failureMessage = failedStep
      ? `${failureCode}: ${failedStep.name} (${failedStep.action}) failed`
      : 'scenario failed';
    const details = failedStep
      ? [
          `failure_code=${failureCode}`,
          `step=${failedStep.name}`,
          `domain=${failedStep.domain}`,
          `action=${failedStep.action}`,
          `cmd=${failedStep.cmd || ''}`,
          `log=${failedStep.log_file || ''}`,
        ].join('\n')
      : 'no details';

    return [
      `    <testcase classname="${xmlEscape(className)}" name="${xmlEscape(name)}">`,
      `      <failure message="${xmlEscape(failureMessage)}">${xmlEscape(details)}</failure>`,
      `    </testcase>`,
    ].join('\n');
  });

  return [
    '<?xml version="1.0" encoding="UTF-8"?>',
    `<testsuite name="${xmlEscape(suiteName)}" tests="${total}" failures="${failures}">`,
    ...cases,
    '</testsuite>',
    '',
  ].join('\n');
}

function summarize(results) {
  const total = results.length;
  const passed = results.filter((r) => r.ok).length;
  const failed = total - passed;

  const failedScenarios = results
    .filter((r) => !r.ok)
    .map((r) => {
      const failedStep = r.steps.find((s) => !s.ok);
      const code = classifyFailure(failedStep);
      if (!failedStep) {
        return `${r.id}: ${code} unknown failure`;
      }
      return `${r.id}: ${code} ${failedStep.name} (${failedStep.action}) [${failedStep.domain}]`;
    });

  return {
    total,
    passed,
    failed,
    failedScenarios,
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));

  const harnessRoot = path.resolve(__dirname, '..');
  const configPath = args.config
    ? path.resolve(process.cwd(), args.config)
    : path.join(harnessRoot, 'config.local.json');

  if (!fs.existsSync(configPath)) {
    throw new Error(`Config file not found: ${configPath}`);
  }

  const rawConfig = readJson(configPath);
  const config = resolveConfig(rawConfig, harnessRoot);

  const defaultMaxMinutes = Number(config.defaults?.maxMinutes || 60);
  const maxMinutes = args.maxMinutes || defaultMaxMinutes;
  if (!Number.isFinite(maxMinutes) || maxMinutes <= 0) {
    throw new Error(`Invalid max-minutes: ${maxMinutes}`);
  }

  const timeoutSeconds = Number(config.defaults?.perCommandTimeoutSeconds || 900);
  const timeoutMs = timeoutSeconds * 1000;

  const runId = `hub-matrix-${tsForPath()}`;
  const artifactsRoot = args.artifactsDir
    ? path.resolve(process.cwd(), args.artifactsDir)
    : path.join(harnessRoot, 'artifacts', runId);
  ensureDir(artifactsRoot);

  const runBudget = {
    startedAtMs: Date.now(),
    maxMs: maxMinutes * 60 * 1000,
    get exceeded() {
      return Date.now() - this.startedAtMs > this.maxMs;
    },
  };

  const requiredMissing = checkRequiredPaths(config);
  if (requiredMissing.length > 0) {
    const msg = requiredMissing
      .map((x) => `missing required path '${x.name}': ${x.path}`)
      .join('\n');
    throw new Error(msg);
  }

  const singleScenario = normalizeScenarioArg(args.scenario);
  let matrix = buildMatrix(args.matrix);
  if (singleScenario) {
    matrix = matrix.filter((x) => x.src === singleScenario.src && x.dst === singleScenario.dst);
  }

  if (matrix.length === 0) {
    throw new Error('Scenario matrix is empty after filtering.');
  }

  const scenarios = matrix.map((pair, index) => ({
    id: scenarioId(index, pair.src, pair.dst),
    src: pair.src,
    dst: pair.dst,
  }));

  const metadata = {
    run_id: runId,
    started_at: nowIso(),
    config_path: configPath,
    artifacts_dir: artifactsRoot,
    dry_run: args.dryRun,
    strict_adapters: args.strictAdapters,
    matrix_mode: args.matrix,
    scenario_filter: args.scenario || null,
    max_minutes: maxMinutes,
    total_scenarios: scenarios.length,
  };

  fs.writeFileSync(path.join(artifactsRoot, 'run-metadata.json'), JSON.stringify(metadata, null, 2));

  const preflight = await runPreflight({
    config,
    args,
    artifactsDir: artifactsRoot,
    timeoutMs,
    runBudget,
  });

  if (!preflight.ok) {
    const failure = {
      ...metadata,
      finished_at: nowIso(),
      preflight,
      scenarios: [],
      summary: {
        total: 0,
        passed: 0,
        failed: 0,
        failedScenarios: ['preflight failed'],
      },
    };
    fs.writeFileSync(path.join(artifactsRoot, 'report.json'), JSON.stringify(failure, null, 2));
    process.stdout.write(`Preflight failed. See ${path.join(artifactsRoot, 'preflight.log')}\n`);
    process.exit(1);
  }

  const commandCache = new Map();
  const results = [];

  for (const scenario of scenarios) {
    if (runBudget.exceeded) {
      results.push({
        id: scenario.id,
        src: scenario.src,
        dst: scenario.dst,
        ok: false,
        failure_code: 'BUDGET_EXCEEDED',
        repro_command: `misc/sccp-e2e/run_hub_matrix.sh --scenario ${scenario.src}:${scenario.dst} --skip-preflight`,
        timed_out_by_budget: true,
        steps: [
          {
            name: 'budget_guard',
            domain: 'harness',
            action: 'budget',
            ok: false,
            skipped: false,
            cmd: '',
            log_file: '',
            reason: 'Global run budget exceeded',
          },
        ],
      });
      continue;
    }

    const result = await runScenario({
      config,
      scenario,
      args,
      timeoutMs,
      artifactsDir: artifactsRoot,
      runBudget,
      commandCache,
    });
    results.push(result);

    process.stdout.write(
      `[${result.ok ? 'PASS' : 'FAIL'}] ${result.id} ${result.src}->${result.dst}\n`
    );
  }

  const summary = summarize(results);
  const report = {
    ...metadata,
    finished_at: nowIso(),
    preflight,
    summary,
    scenarios: results,
  };

  const reportPath = path.join(artifactsRoot, 'report.json');
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));

  const junitXml = buildJUnit(results, 'sccp-hub-matrix');
  const junitPath = path.join(artifactsRoot, 'junit.xml');
  fs.writeFileSync(junitPath, junitXml, 'utf8');

  const summaryLines = [
    `Run ID: ${runId}`,
    `Artifacts: ${artifactsRoot}`,
    `Scenarios: ${summary.total}`,
    `Passed: ${summary.passed}`,
    `Failed: ${summary.failed}`,
  ];
  if (summary.failedScenarios.length > 0) {
    summaryLines.push('Failed scenarios:');
    for (const line of summary.failedScenarios) {
      summaryLines.push(`- ${line}`);
    }
  }
  process.stdout.write(summaryLines.join('\n') + '\n');

  if (summary.failed > 0) {
    process.exit(1);
  }
}

main().catch((err) => {
  process.stderr.write(`${err.stack || err.message}\n`);
  process.exit(1);
});
