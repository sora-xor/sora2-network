// SORA bridge emergency halt. Plain script, no build step.
// Libraries: the pinned polkadot.js browser bundles in vendor/ (see README.md).
(() => {
  'use strict';

  const { ApiPromise, WsProvider } = window.polkadotApi;
  const { u8aToHex, hexToU8a, stringToU8a, u8aToString, u8aConcat, u8aEq, compactToU8a } =
    window.polkadotUtil;
  const { decodeAddress, encodeAddress, cryptoWaitReady } = window.polkadotUtilCrypto;

  const CONFIG = window.BRIDGE_HALT_CONFIG || {};
  const SS58_PREFIX = 69; // SORA addresses start with "cn".
  const HALT_KEY = u8aToHex(stringToU8a(':sora-nexus:recovery:bridge-halt:v1'));
  // BridgeHalt is pallet 122 in construct_runtime and halt is its call 0. The page
  // signs only these bytes, so a node serving forged metadata cannot swap the call.
  const HALT_CALL_PREFIX = new Uint8Array([122, 0]);
  const APP_NAME = 'SORA bridge halt';
  // Fearless Wallet's extension shipped a key collector (October 2026).
  const BLOCKED_EXTENSIONS = new Set(['fearless-wallet']);
  const DEV_ACCOUNTS = ['Alice', 'Bob', 'Charlie', 'Dave'];

  const $ = (id) => document.getElementById(id);
  const params = new URLSearchParams(window.location.search);
  const encoder = new TextEncoder();

  const state = {
    api: null,
    provider: null,
    unsubHeads: null,
    endpoint: null,
    generation: 0,
    supported: false,
    halted: null,
    guardians: new Set(), // hex public keys
    accounts: [], // { address, name, source, key, signer?, pair? }
    busy: false,
    maxReason: 256,
    lastBlock: 0,
    stopWatch: null, // stops status reporting for the latest halt attempt
  };

  // ---------- helpers ----------

  const sora = (key) => encodeAddress(hexToU8a(key), SS58_PREFIX);
  const publicKey = (address) => u8aToHex(decodeAddress(address));

  function isLocal(endpoint) {
    try {
      const { hostname } = new URL(endpoint);
      return hostname === '127.0.0.1' || hostname === 'localhost' || hostname === '[::1]';
    } catch {
      return false;
    }
  }

  const devMode = () => params.has('dev') && isLocal(state.endpoint);

  function endpoints() {
    const list = [...(CONFIG.endpoints || [])];
    // A link must not be able to point guardians at someone else's node.
    const extra = params.get('rpc');
    if (extra && /^wss?:\/\//.test(extra) && isLocal(extra) && !list.includes(extra)) {
      list.unshift(extra);
    }
    return list;
  }

  /// Signing is allowed on the configured chain, or on a local node in dev mode.
  function trustedChain(api) {
    return devMode() || api.genesisHash.toHex() === CONFIG.genesisHash;
  }

  function expectedHaltCall(reason) {
    return u8aConcat(HALT_CALL_PREFIX, compactToU8a(reason.length), reason);
  }

  function setStatus(kind, text) {
    const element = $('status');
    element.className = `status status-${kind}`;
    element.textContent = text;
  }

  function log(text, kind) {
    const item = document.createElement('li');
    const time = new Date().toLocaleTimeString();
    item.textContent = `${time} — ${text}`;
    if (kind) item.className = kind;
    $('log').appendChild(item);
  }

  function selectedAccount() {
    const index = Number($('account').value);
    return Number.isInteger(index) ? state.accounts[index] : undefined;
  }

  function reasonBytes() {
    return encoder.encode($('reason').value.trim());
  }

  /// Plain-language reasons for the transaction-pool rejections this call can hit.
  function explain(error) {
    const text = String((error && error.message) || error);
    if (/Invalid signing address|BadSigner/i.test(text)) {
      return 'Rejected by the chain: this account is not a guardian. Nothing was written on chain and no fee was charged.';
    }
    if (/outdated|Stale/i.test(text)) {
      return 'Rejected by the chain: the bridges are already halted. Nothing was written and no fee was charged.';
    }
    if (/Inability to pay|Payment/i.test(text)) {
      return 'Rejected by the chain: the account holds no XOR, so it does not exist on chain, or the wallet added a tip. Send the account a little XOR and sign without a tip; the halt itself is free.';
    }
    if (/cancel|rejected by user|user rejected|rejected the request/i.test(text)) {
      return 'Signing was cancelled. Nothing was sent.';
    }
    return `Failed: ${text}`;
  }

  function describeDispatchError(api, dispatchError) {
    if (dispatchError.isModule) {
      const { section, name, docs } = api.registry.findMetaError(dispatchError.asModule);
      return `${section}.${name}: ${docs.join(' ')}`;
    }
    return dispatchError.toString();
  }

  // ---------- rendering ----------

  function render() {
    const account = selectedAccount();
    const bytes = reasonBytes().length;
    const tooLong = bytes > state.maxReason;
    $('reason-count').textContent = `${bytes} / ${state.maxReason} bytes`;
    $('reason-count').className = tooLong ? 'hint role-other' : 'hint';

    const role = $('account-role');
    if (!account) {
      role.textContent = '';
    } else if (!state.supported) {
      role.textContent = '';
    } else if (state.guardians.has(account.key)) {
      role.textContent = '✓ Guardian. The chain accepts a halt from this account, and it costs no fee.';
      role.className = 'hint role-guardian';
    } else {
      role.textContent =
        '✗ Not a guardian. The chain will reject a halt from this account: nothing is written and no fee is charged.';
      role.className = 'hint role-other';
    }

    const button = $('halt');
    button.textContent = state.halted ? 'BRIDGES ALREADY HALTED' : 'HALT ALL BRIDGES';
    button.disabled = !state.api || !state.supported || !account || state.busy || tooLong;
  }

  function renderGuardians() {
    const list = $('guardians');
    list.replaceChildren();
    if (!state.supported) {
      const item = document.createElement('li');
      item.textContent = '–';
      list.appendChild(item);
      return;
    }
    if (state.guardians.size === 0) {
      const item = document.createElement('li');
      item.textContent = 'No guardians yet. Governance adds them with bridgeHalt.addGuardian.';
      list.appendChild(item);
      return;
    }
    for (const key of state.guardians) {
      const item = document.createElement('li');
      item.textContent = sora(key);
      list.appendChild(item);
    }
  }

  function renderAccounts() {
    const select = $('account');
    const previous = selectedAccount();
    select.replaceChildren();
    if (state.accounts.length === 0) {
      const option = document.createElement('option');
      option.textContent = 'No accounts found';
      select.appendChild(option);
      select.disabled = true;
      render();
      return;
    }
    state.accounts.forEach((account, index) => {
      const option = document.createElement('option');
      option.value = String(index);
      const mark = state.guardians.has(account.key) ? '✓ ' : '';
      const name = account.name ? `${account.name} — ` : '';
      option.textContent = `${mark}${name}${account.address}`;
      select.appendChild(option);
    });
    select.disabled = false;
    const keep = previous && state.accounts.findIndex((account) => account.key === previous.key);
    const guardian = state.accounts.findIndex((account) => state.guardians.has(account.key));
    select.value = String(keep >= 0 ? keep : Math.max(guardian, 0));
    render();
  }

  function resetChainView() {
    state.lastBlock = 0;
    state.supported = false;
    state.halted = null;
    state.guardians = new Set();
    $('chain').textContent = '–';
    $('block').textContent = '–';
    $('runtime').textContent = '–';
    $('last-halt').hidden = true;
    $('unsupported').hidden = true;
    renderGuardians();
    render();
  }

  // ---------- chain ----------

  async function disconnect() {
    if (state.stopWatch) state.stopWatch();
    const { api, provider, unsubHeads } = state;
    state.api = null;
    state.provider = null;
    state.unsubHeads = null;
    try {
      if (unsubHeads) unsubHeads();
      if (api) await api.disconnect();
      else if (provider) await provider.disconnect();
    } catch {
      // Already closed.
    }
  }

  async function connect(endpoint) {
    const generation = ++state.generation;
    await disconnect();
    state.endpoint = endpoint;
    resetChainView();
    setStatus('unknown', `Connecting to ${endpoint}…`);

    const provider = new WsProvider(endpoint, 3000);
    state.provider = provider;
    const current = () => generation === state.generation;
    provider.on('error', () => {
      if (current() && !state.api) setStatus('unknown', `Cannot reach ${endpoint}. Retrying…`);
    });
    provider.on('disconnected', () => {
      if (current() && state.api) setStatus('unknown', 'Connection lost. Reconnecting…');
    });

    let api;
    try {
      api = new ApiPromise({ provider, noInitWarn: true });
      await api.isReadyOrError;
    } catch (error) {
      if (current()) setStatus('unknown', `Could not load the chain: ${error.message || error}`);
      return;
    }
    if (!current()) {
      await api.disconnect();
      return;
    }
    state.api = api;
    const chain = await api.rpc.system.chain();
    if (!current()) return;
    $('chain').textContent = chain.toString();
    state.unsubHeads = await api.rpc.chain.subscribeNewHeads((header) => {
      refresh(api, provider, header, generation).catch((error) => {
        if (current()) setStatus('unknown', `Could not read the chain: ${error.message || error}`);
      });
    });
    if (devMode() && state.accounts.length === 0) connectWallet();
  }

  async function refresh(api, provider, header, generation) {
    const number = header.number.toNumber();
    // Reads for consecutive heads can finish out of order; keep the newest.
    const current = () => generation === state.generation && number >= state.lastBlock;
    const hash = header.hash.toHex();
    // Raw read: null means absent. Any value, even empty, halts the bridges.
    const latch = await provider.send('state_getStorage', [HALT_KEY, hash]);
    const version = api.runtimeVersion;
    const supported = Boolean(
      trustedChain(api) && api.tx.bridgeHalt && api.tx.bridgeHalt.halt && api.query.bridgeHalt,
    );
    let guardians = new Set();
    let last = null;
    if (supported) {
      const at = await api.at(hash);
      const [set, record] = await Promise.all([
        at.query.bridgeHalt.guardians(),
        at.query.bridgeHalt.lastHalt(),
      ]);
      guardians = new Set([...set].map((account) => account.toHex()));
      last = record.isSome ? record.unwrap() : null;
      if (api.consts.bridgeHalt && api.consts.bridgeHalt.maxReasonLength) {
        state.maxReason = api.consts.bridgeHalt.maxReasonLength.toNumber();
      }
    }
    if (!current()) return;

    // Rebuild the lists only when they change, so an open dropdown is not reset every block.
    const listChanged =
      supported !== state.supported ||
      [...guardians].sort().join() !== [...state.guardians].sort().join();
    state.lastBlock = number;
    state.halted = latch !== null && latch !== undefined;
    state.supported = supported;
    state.guardians = guardians;
    $('block').textContent = `#${number.toLocaleString('en-US')}`;
    $('runtime').textContent = `${version.specName.toString()} ${version.specVersion.toNumber()}`;
    setStatus(
      state.halted ? 'halted' : 'running',
      state.halted ? 'Bridges are HALTED' : 'Bridges are running',
    );

    const notice = $('unsupported');
    notice.hidden = supported;
    if (!trustedChain(api)) {
      notice.textContent =
        `This node serves a different chain (genesis ${api.genesisHash.toHex()}) from the one ` +
        'configured for this page, so the button is off. Check the node list in config.js.';
    } else if (!supported) {
      notice.textContent =
        `This node runs runtime ${version.specVersion.toNumber()}, which has no halt call yet. ` +
        'The button works once the BridgeHalt runtime upgrade is enacted.';
    }

    $('last-halt').hidden = !last;
    if (last) {
      $('last-guardian').textContent = sora(last.guardian.toHex());
      $('last-block').textContent = `#${last.block.toNumber().toLocaleString('en-US')}`;
      const message = u8aToString(last.reason.toU8a(true));
      $('last-reason').textContent = message || '(no message)';
    }
    if (listChanged) {
      renderGuardians();
      renderAccounts();
    } else {
      render();
    }
  }

  // ---------- wallet ----------

  async function waitForInjection() {
    for (let attempt = 0; attempt < 15; attempt += 1) {
      if (window.injectedWeb3 && Object.keys(window.injectedWeb3).length > 0) return;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
  }

  async function devAccounts() {
    await cryptoWaitReady();
    const keyring = new window.polkadotKeyring.Keyring({ type: 'sr25519', ss58Format: SS58_PREFIX });
    return DEV_ACCOUNTS.map((name) => {
      const pair = keyring.addFromUri(`//${name}`);
      return {
        address: pair.address,
        name: `${name} (local dev key)`,
        source: 'development',
        key: u8aToHex(pair.publicKey),
        pair,
      };
    });
  }

  async function connectWallet() {
    $('connect').disabled = true;
    const accounts = [];
    try {
      await waitForInjection();
      const injected = window.injectedWeb3 || {};
      const names = Object.keys(injected);
      if (names.some((name) => BLOCKED_EXTENSIONS.has(name))) {
        log('Ignoring the Fearless Wallet extension. Use another wallet for guardian keys.', 'error');
      }
      for (const name of names.filter((item) => !BLOCKED_EXTENSIONS.has(item))) {
        try {
          const extension = await injected[name].enable(APP_NAME);
          for (const account of await extension.accounts.get()) {
            if (account.type === 'ethereum') continue;
            const key = publicKey(account.address);
            if (accounts.some((known) => known.key === key)) continue;
            accounts.push({
              address: sora(key),
              name: account.name || '',
              source: name,
              key,
              signer: extension.signer,
            });
          }
        } catch (error) {
          log(`${name}: ${error.message || error}`, 'error');
        }
      }
      if (devMode()) accounts.push(...(await devAccounts()));
      if (accounts.length === 0) {
        log(
          names.length === 0
            ? 'No wallet extension found. Install polkadot{.js}, SubWallet or Talisman and allow this site.'
            : 'The wallet shared no accounts with this site.',
          'error',
        );
      }
    } finally {
      state.accounts = accounts;
      $('connect').disabled = false;
      $('connect').textContent = accounts.length ? 'Reload accounts' : 'Connect wallet';
      renderAccounts();
    }
  }

  // ---------- halt ----------

  function askToConfirm() {
    $('confirm').hidden = false;
    $('confirm-yes').focus();
  }

  async function sendHalt() {
    $('confirm').hidden = true;
    const api = state.api;
    const account = selectedAccount();
    if (!api || !account || state.busy || !api.tx.bridgeHalt || !trustedChain(api)) return;
    const reason = reasonBytes();
    if (reason.length > state.maxReason) return;

    // Stop reporting on any earlier attempt before this one writes to the log.
    if (state.stopWatch) state.stopWatch();
    $('log').replaceChildren();
    state.busy = true;
    render();
    const idle = () => {
      state.busy = false;
      render();
    };

    let tx;
    try {
      // polkadot.js reads a Uint8Array Bytes argument as already length-prefixed
      // SCALE; a hex string is taken as the raw message.
      tx = api.tx.bridgeHalt.halt(u8aToHex(reason));
    } catch (error) {
      log(`Could not build the transaction: ${error.message || error}`, 'error');
      idle();
      return;
    }
    if (!u8aEq(tx.method.toU8a(), expectedHaltCall(reason))) {
      log(
        'Refusing to sign: this node encodes the halt differently from BridgeHalt.halt (pallet 122, call 0). Use another node.',
        'error',
      );
      idle();
      return;
    }
    if (!state.guardians.has(account.key)) {
      log('This account is not on the guardian list, so the chain is expected to reject the request.');
    }
    log(account.pair ? 'Signing with the local development key…' : 'Waiting for your wallet to sign…');

    const options = account.pair ? { nonce: -1, tip: 0 } : { signer: account.signer, nonce: -1, tip: 0 };
    let unsubscribe = null;
    let closed = false;
    const close = () => {
      if (closed) return;
      closed = true;
      if (unsubscribe) unsubscribe();
      if (state.stopWatch === close) state.stopWatch = null;
      idle();
    };
    state.stopWatch = close;
    const onStatus = ({ status, dispatchError }) => {
      if (closed) return;
      if (status.isReady) {
        log('Sent to the network. Waiting for a block…');
      } else if (status.isInBlock) {
        const block = status.asInBlock.toHex();
        if (dispatchError) {
          log(`Included in block ${block} but failed: ${describeDispatchError(api, dispatchError)}`, 'error');
          close();
          return;
        }
        log(`Included in block ${block}: the bridges are halted. Waiting for finality…`, 'success');
        idle(); // Keep watching for finality, but free the page.
      } else if (status.isRetracted) {
        log('That block was replaced by a fork. Waiting for the halt to be included again…');
      } else if (status.isFinalized) {
        log(`Final in block ${status.asFinalized.toHex()}.`, 'success');
        close();
      } else if (status.isFinalityTimeout) {
        log(
          'The including block has not been finalized; finality may be stalled. The status at the top shows whether the bridges are halted.',
          'error',
        );
        close();
      } else if (status.isDropped || status.isInvalid || status.isUsurped) {
        log(
          'The network dropped the transaction. If another guardian halted first, the bridges are already halted; otherwise press the button again.',
          'error',
        );
        close();
      }
    };
    try {
      unsubscribe = await tx.signAndSend(account.pair || account.address, options, onStatus);
      if (closed) unsubscribe();
    } catch (error) {
      log(explain(error), 'error');
      close();
    }
  }

  // ---------- start ----------

  function start() {
    // Clickjacking guard; hosts should also send frame-ancestors 'none'.
    if (window.top !== window.self) {
      document.body.textContent = 'This page does not run inside a frame. Open it directly.';
      return;
    }
    const select = $('endpoint');
    for (const endpoint of endpoints()) {
      const option = document.createElement('option');
      option.value = endpoint;
      option.textContent = endpoint;
      select.appendChild(option);
    }
    select.addEventListener('change', () => connect(select.value));
    $('connect').addEventListener('click', connectWallet);
    $('account').addEventListener('change', render);
    $('reason').addEventListener('input', render);
    $('halt').addEventListener('click', askToConfirm);
    $('confirm-yes').addEventListener('click', sendHalt);
    $('confirm-no').addEventListener('click', () => {
      $('confirm').hidden = true;
      $('halt').focus();
    });
    render();
    if (select.value) connect(select.value);
    else setStatus('unknown', 'No RPC endpoint configured. Edit config.js.');
  }

  start();
})();
