#!/usr/bin/env node

'use strict';

const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');

const DOMAIN_ORDER = ['sora', 'eth', 'bsc', 'sol', 'ton', 'tron', 'sora_kusama', 'sora_polkadot'];
const CORE_SORA_DOMAINS = ['eth', 'bsc', 'sol', 'ton', 'tron'];
const DOMAIN_TO_ID = {
  sora: 0,
  eth: 1,
  bsc: 2,
  sol: 3,
  ton: 4,
  tron: 5,
  sora_kusama: 6,
  sora_polkadot: 7,
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
    mode: null,
    maxMinutes: null,
    dryRun: false,
    skipPreflight: false,
    includeNegative: null,
    scenario: null,
    strictAdapters: false,
    matrix: null,
    artifactsDir: null,
    commandCacheEnabled: null,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const next = argv[i + 1];
    if ((arg === '--config' || arg === '-c') && next) {
      out.config = next;
      i += 1;
    } else if (arg === '--mode' && next) {
      out.mode = next;
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
    } else if (arg === '--disable-command-cache') {
      out.commandCacheEnabled = false;
    } else if (arg === '--enable-command-cache') {
      out.commandCacheEnabled = true;
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
      '  --mode <name>              Optional config mode (for overrides/presets)',
      '  --max-minutes <n>          Max wall clock budget in minutes',
      '  --dry-run                  Plan and validate commands without executing',
      '  --skip-preflight           Skip misc/sccp/run_all_tests.sh preflight',
      '  --include-negative         Force-enable negative checks',
      '  --exclude-negative         Disable negative checks',
      '  --scenario <src:dst>       Run a single scenario (example: eth:sol)',
      '  --strict-adapters          Require adapter scripts for all non-sora domains',
      '  --disable-command-cache    Disable per-command cross-scenario result cache',
      '  --enable-command-cache     Enable per-command cross-scenario result cache',
      '  --matrix <name>            Matrix key or mode (for example: full, sora-pairs, sora-core-pairs)',
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
  const defaultIrohaHub = path.resolve(repoRoot, '..', 'iroha');
  const vars = {
    harnessRoot,
    repoRoot,
    sora2Network: repoRoot,
    iroha: defaultIrohaHub,
    hub: defaultIrohaHub,
    sccpEth: path.resolve(repoRoot, 'sccp', 'chains', 'eth'),
    sccpBsc: path.resolve(repoRoot, 'sccp', 'chains', 'bsc'),
    sccpSol: path.resolve(repoRoot, 'sccp', 'chains', 'sol'),
    sccpTon: path.resolve(repoRoot, 'sccp', 'chains', 'ton'),
    sccpTron: path.resolve(repoRoot, 'sccp', 'chains', 'tron'),
    sora2Parachain: path.resolve(repoRoot, '..', 'sora2-parachain'),
    sccpSoraKusama: path.resolve(repoRoot, '..', 'sora2-parachain'),
    sccpSoraPolkadot: path.resolve(repoRoot, '..', 'sora2-parachain'),
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
  merged.modes = deepResolve(merged.modes || {});
  merged.matrixPresets = deepResolve(merged.matrixPresets || {});

  merged.harnessRoot = harnessRoot;
  merged.repoRoot = repoRoot;
  merged.vars = vars;
  return merged;
}

function resolveHubPath(config) {
  return config.paths.hub || config.paths.iroha || config.paths.destinationProofToolchain || config.paths.bridgeRelayer;
}

function resolveHubCommand(config) {
  return (
    config.commands?.hub?.publish_bundle ||
    config.commands?.iroha?.publish_bundle ||
    config.commands?.destinationProofToolchain?.proof_toolchain ||
    config.commands?.bridgeRelayer?.proof_toolchain
  );
}

function deepMerge(base, override) {
  if (Array.isArray(base) || Array.isArray(override)) {
    return Array.isArray(override) ? override.slice() : base;
  }
  if (!base || typeof base !== 'object') {
    return override;
  }
  if (!override || typeof override !== 'object') {
    return base;
  }

  const out = { ...base };
  for (const [k, v] of Object.entries(override)) {
    if (v && typeof v === 'object' && !Array.isArray(v) && out[k] && typeof out[k] === 'object' && !Array.isArray(out[k])) {
      out[k] = deepMerge(out[k], v);
    } else {
      out[k] = v;
    }
  }
  return out;
}

function applyModeConfig(config, modeName) {
  const modeConfig = config.modes?.[modeName];
  if (!modeConfig) {
    throw new Error(`Unknown mode '${modeName}'. Available modes: ${Object.keys(config.modes || {}).join(', ') || '(none)'}`);
  }

  const merged = {
    ...config,
    defaults: deepMerge(config.defaults || {}, modeConfig.defaults || {}),
    commands: deepMerge(config.commands || {}, modeConfig.commands || {}),
  };

  merged.activeMode = modeName;
  merged.activeModeConfig = modeConfig;
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
  if (mode === 'sora-core-pairs') {
    for (const domain of CORE_SORA_DOMAINS) {
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
    sora_kusama: config.paths.sccpSoraKusama,
    sora_polkadot: config.paths.sccpSoraPolkadot,
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
  if (step.kind === 'hub' || step.kind === 'destinationProofToolchain' || step.kind === 'bridgeRelayer') {
    return 'HUB_BUNDLE_PUBLICATION_FAILED';
  }
  if ((step.kind === 'domain' || step.kind === 'sora') && step.action === 'mint_verify') {
    return 'DEST_MINT_FAILED';
  }
  if (
    (step.kind === 'domain' && step.action === 'negative_verify') ||
    (step.kind === 'sora' && step.action === 'negative_verify')
  ) {
    return 'INVARIANT_FAILED';
  }
  if (step.kind === 'sora') {
    return 'SORA_SPOKE_STEP_FAILED';
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
    hub_bundle_kind: 'burn',
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

function writeScenarioContext(filePath, context) {
  fs.writeFileSync(filePath, `${JSON.stringify(context, null, 2)}\n`, 'utf8');
}

function applyStepOutputToScenarioContext(context, step, stepOutput) {
  if (!stepOutput || typeof stepOutput !== 'object') {
    return;
  }

  context.step_outputs[step.name] = stepOutput;
  if (typeof stepOutput.message_id === 'string' && stepOutput.message_id.length > 0) {
    context.message_id = stepOutput.message_id;
  }
  if (typeof stepOutput.payload_hex === 'string' && stepOutput.payload_hex.length > 0) {
    context.payload_hex = stepOutput.payload_hex;
  }

  if (step.kind === 'hub') {
    if (typeof stepOutput.norito_bundle_hex === 'string' && stepOutput.norito_bundle_hex.length > 0) {
      context.hub_bundle_norito_hex = stepOutput.norito_bundle_hex;
    } else if (typeof stepOutput.bundle_norito_hex === 'string' && stepOutput.bundle_norito_hex.length > 0) {
      context.hub_bundle_norito_hex = stepOutput.bundle_norito_hex;
    } else if (typeof stepOutput.scale_bundle_hex === 'string' && stepOutput.scale_bundle_hex.length > 0) {
      context.hub_bundle_norito_hex = stepOutput.scale_bundle_hex;
    } else if (typeof stepOutput.bundle_scale_hex === 'string' && stepOutput.bundle_scale_hex.length > 0) {
      context.hub_bundle_norito_hex = stepOutput.bundle_scale_hex;
    }

    if (stepOutput.json_bundle && typeof stepOutput.json_bundle === 'object') {
      context.hub_bundle_json = stepOutput.json_bundle;
    } else if (stepOutput.bundle_json && typeof stepOutput.bundle_json === 'object') {
      context.hub_bundle_json = stepOutput.bundle_json;
    }

    if (typeof stepOutput.bundle_json_path === 'string' && stepOutput.bundle_json_path.length > 0) {
      context.hub_bundle_json_path = stepOutput.bundle_json_path;
    }
    if (typeof stepOutput.bundle_norito_path === 'string' && stepOutput.bundle_norito_path.length > 0) {
      context.hub_bundle_norito_path = stepOutput.bundle_norito_path;
    } else if (typeof stepOutput.bundle_scale_path === 'string' && stepOutput.bundle_scale_path.length > 0) {
      context.hub_bundle_norito_path = stepOutput.bundle_scale_path;
    }
    if (typeof stepOutput.kind === 'string' && stepOutput.kind.length > 0) {
      context.hub_bundle_kind = stepOutput.kind;
    }
  }
}

async function runPreflight({ config, args, artifactsDir, timeoutMs, runBudget, commandEnv }) {
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
    env: commandEnv,
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

function checkRequiredPaths(config, scenarios) {
  const required = new Map();
  required.set('sora2Network', config.paths.sora2Network);
  if (scenarios.length > 0) {
    required.set('hub', resolveHubPath(config));
  }

  const neededDomains = new Set();
  for (const scenario of scenarios) {
    neededDomains.add(scenario.src);
    neededDomains.add(scenario.dst);
  }
  neededDomains.delete('sora');

  for (const domain of neededDomains) {
    const repoPath = resolveDomainRepo(config, domain);
    required.set(domain, repoPath);
  }

  const missing = [];
  for (const [name, value] of required.entries()) {
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
  commandCacheEnabled,
  commandEnv,
}) {
  const scenarioDir = path.join(artifactsDir, scenario.id);
  ensureDir(scenarioDir);
  const scenarioContextPath = path.join(scenarioDir, 'scenario-context.json');

  const steps = [];
  const scenarioContext = {
    ...buildScenarioPayload(scenario),
    step_outputs: {},
  };
  writeScenarioContext(scenarioContextPath, scenarioContext);

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

  const runHubCommand = () => {
    const cmd = resolveHubCommand(config);
    const cwd = resolveHubPath(config);
    if (!cmd) {
      return {
        ok: false,
        skipped: true,
        reason: 'Missing hub.publish_bundle command mapping',
      };
    }
    if (!cwd) {
      return {
        ok: false,
        skipped: true,
        reason: 'Missing hub path',
      };
    }
    return {
      type: 'command',
      cmd,
      cwd,
      cacheKey: `hub:${cwd}:${cmd}`,
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
      cmdBuilder: spec.cmdBuilder,
      cwd: spec.cwd,
      cacheMode: spec.mode,
    };
  };

  if (scenario.src === 'sora') {
    addStep('sora_burn', 'sora', 'sora', 'burn', runSoraCommand('burn'));
  } else {
    addStep('source_burn', 'domain', scenario.src, 'burn', runDomainAction(scenario.src, 'burn'));
  }

  addStep('hub_publish_bundle', 'hub', 'hub', 'publish_bundle', runHubCommand());

  if (scenario.dst === 'sora') {
    addStep('dest_mint_verify', 'sora', 'sora', 'mint_verify', runSoraCommand('mint_verify'));
  } else {
    addStep('dest_mint_verify', 'domain', scenario.dst, 'mint_verify', runDomainAction(scenario.dst, 'mint_verify'));
  }

  const includeNegative =
    args.includeNegative === null
      ? Boolean(config.defaults?.includeNegative)
      : Boolean(args.includeNegative);

  if (includeNegative) {
    if (scenario.dst === 'sora') {
      addStep('negative_sora', 'sora', 'sora', 'negative_verify', runSoraCommand('negative_verify'));
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
    const runnerPayload = JSON.parse(JSON.stringify(scenarioContext));
    const runnerCmd =
      typeof runner.cmdBuilder === 'function' ? runner.cmdBuilder(runnerPayload) : runner.cmd;
    const cacheKey = runner.cmdBuilder
      ? (
          runner.cacheMode === 'fallback'
            ? `fallback:${step.domain}:${runner.cwd}:${runnerCmd}`
            : `adapter:${step.domain}:${step.action}:${runner.cwd}:${runnerCmd}`
        )
      : runner.cacheKey;
    const stepEnv = {
      ...commandEnv,
      SCCP_SCENARIO_CONTEXT_FILE: scenarioContextPath,
      SCCP_SCENARIO_DIR: scenarioDir,
      SCCP_SCENARIO_ID: scenario.id,
      SCCP_SOURCE_DOMAIN: String(DOMAIN_TO_ID[scenario.src]),
      SCCP_DEST_DOMAIN: String(DOMAIN_TO_ID[scenario.dst]),
      SCCP_SOURCE_LABEL: domainLabel(scenario.src),
      SCCP_DEST_LABEL: domainLabel(scenario.dst),
      SCCP_MESSAGE_ID: scenarioContext.message_id || '',
      SCCP_PAYLOAD_HEX: scenarioContext.payload_hex || '',
      SCCP_HUB_BUNDLE_KIND: scenarioContext.hub_bundle_kind || 'burn',
      SCCP_HUB_BUNDLE_NORITO_HEX: scenarioContext.hub_bundle_norito_hex || '',
      SCCP_HUB_BUNDLE_JSON_PATH: scenarioContext.hub_bundle_json_path || '',
      SCCP_HUB_BUNDLE_NORITO_PATH: scenarioContext.hub_bundle_norito_path || '',
      SCCP_HUB_BUNDLE_SCALE_HEX: scenarioContext.hub_bundle_norito_hex || '',
      SCCP_HUB_BUNDLE_SCALE_PATH: scenarioContext.hub_bundle_norito_path || '',
    };
    const fromCache = commandCacheEnabled ? commandCache.get(cacheKey) : null;
    if (fromCache) {
      const cachedStepOutput = parseAdapterJson(fromCache.result?.stdout || '');
      const cached = {
        name: step.name,
        kind: step.kind,
        domain: step.domain,
        action: step.action,
        skipped: false,
        ok: fromCache.ok,
        cached: true,
        mode: runner.mode || 'command',
        cmd: runnerCmd,
        cwd: runner.cwd,
        log_file: fromCache.log_file,
        result: fromCache.result,
      };
      if (cachedStepOutput) {
        cached.step_output = cachedStepOutput;
        applyStepOutputToScenarioContext(scenarioContext, step, cachedStepOutput);
        writeScenarioContext(scenarioContextPath, scenarioContext);
      }
      stepResults.push(cached);
      if (!cached.ok) {
        scenarioOk = false;
        break;
      }
      continue;
    }

    const result = await execCommand({
      cmd: runnerCmd,
      cwd: runner.cwd,
      timeoutMs,
      env: stepEnv,
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
      cmd: runnerCmd,
      cwd: runner.cwd,
      log_file: logFile,
      result,
    };

    const parsedOutput = parseAdapterJson(result.stdout);
    if (parsedOutput) {
      normalized.step_output = parsedOutput;
      applyStepOutputToScenarioContext(scenarioContext, step, parsedOutput);
      writeScenarioContext(scenarioContextPath, scenarioContext);
    }

    if (normalized.mode === 'adapter') {
      if (args.dryRun) {
        normalized.adapter_output = { ok: true, dry_run: true };
      } else {
        normalized.adapter_output = normalized.step_output || null;
        const parsed = normalized.adapter_output;
        if (!parsed || parsed.ok !== true) {
          normalized.ok = false;
        }
      }
    }

    stepResults.push(normalized);

    if (commandCacheEnabled) {
      commandCache.set(cacheKey, {
        ok: normalized.ok,
        log_file: normalized.log_file,
        result: normalized.result,
      });
    }

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
  let config = resolveConfig(rawConfig, harnessRoot);
  if (args.mode) {
    config = applyModeConfig(config, args.mode);
  }

  const matrixPresets = config.matrixPresets || {};
  const defaultMatrixKey = config.activeModeConfig?.matrix || config.defaults?.matrix || 'full';
  const matrixKey = args.matrix || defaultMatrixKey;
  const matrixMode = matrixPresets[matrixKey] || matrixKey;
  if (!['full', 'sora-pairs', 'sora-core-pairs'].includes(matrixMode)) {
    throw new Error(`Invalid matrix mode '${matrixMode}' (resolved from key '${matrixKey}')`);
  }

  const defaultMaxMinutes = Number(config.defaults?.maxMinutes || 60);
  const maxMinutes = args.maxMinutes ?? defaultMaxMinutes;
  if (!Number.isFinite(maxMinutes) || maxMinutes <= 0) {
    throw new Error(`Invalid max-minutes: ${maxMinutes}`);
  }

  const defaultCommandCacheEnabled =
    typeof config.defaults?.commandCache === 'boolean'
      ? config.defaults.commandCache
      : true;
  const commandCacheEnabled =
    args.commandCacheEnabled === null ? defaultCommandCacheEnabled : args.commandCacheEnabled;

  const rustupToolchain =
    process.env.SCCP_RUSTUP_TOOLCHAIN ||
    process.env.RUSTUP_TOOLCHAIN ||
    'nightly-2025-05-08';
  const commandEnv = {
    ...process.env,
    RUSTUP_TOOLCHAIN: rustupToolchain,
  };

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

  const singleScenario = normalizeScenarioArg(args.scenario);
  let matrix = buildMatrix(matrixMode);
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

  const requiredMissing = checkRequiredPaths(config, scenarios);
  if (requiredMissing.length > 0) {
    const msg = requiredMissing
      .map((x) => `missing required path '${x.name}': ${x.path}`)
      .join('\n');
    throw new Error(msg);
  }

  const metadata = {
    run_id: runId,
    started_at: nowIso(),
    config_path: configPath,
    artifacts_dir: artifactsRoot,
    dry_run: args.dryRun,
    mode: args.mode || null,
    matrix_key: matrixKey,
    strict_adapters: args.strictAdapters,
    command_cache_enabled: commandCacheEnabled,
    matrix_mode: matrixMode,
    rustup_toolchain: rustupToolchain,
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
    commandEnv,
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
      commandCacheEnabled,
      commandEnv,
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
