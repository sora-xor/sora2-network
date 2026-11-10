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
  // Root's recovery latch. While it exists the bridge pallets refuse calls; only a
  // Root call removes it. A guardian halt does not set it.
  const HALT_KEY = u8aToHex(stringToU8a(':sora-nexus:recovery:bridge-halt:v1'));
  // How long after a guardian halt the console keeps showing it as current. The
  // chain does not record when devops restart the workers.
  const RECENT_HALT_MS = 24 * 60 * 60 * 1000;
  // BridgeHalt is pallet 122 in construct_runtime and halt is its call 0. The page
  // signs only these bytes, so a node serving forged metadata cannot swap the call.
  const HALT_CALL_PREFIX = new Uint8Array([122, 0]);
  const APP_NAME = 'SORA bridge halt';
  const DEV_ACCOUNTS = ['Alice', 'Bob', 'Charlie', 'Dave'];
  // Browser extension wallets, in Polkaswap's order and with its store links.
  // Fearless Wallet is SORA's own wallet. Other injected extensions follow.
  const WALLETS = [
    {
      id: 'fearless-wallet',
      title: 'Fearless Wallet',
      logo: 'img/fearless-wallet.svg',
      recommended: true,
      chrome: 'https://chrome.google.com/webstore/detail/fearless-wallet/nhlnehondigmgckngjomcpcefcdplmgc',
    },
    {
      id: 'polkadot-js',
      title: 'Polkadot{.js}',
      logo: 'img/polkadot-js.svg',
      chrome:
        'https://chrome.google.com/webstore/detail/polkadot%7Bjs%7D-extension/mopnmbcafieddcagagdcbnhejhlodfdd',
      firefox: 'https://addons.mozilla.org/firefox/addon/polkadot-js-extension',
    },
    {
      id: 'subwallet-js',
      title: 'SubWallet',
      logo: 'img/subwallet.svg',
      chrome: 'https://chrome.google.com/webstore/detail/subwallet/onhogfjeacnfoofkfgppdlbmlmnplgbn',
      firefox: 'https://addons.mozilla.org/firefox/addon/subwallet',
    },
    {
      id: 'talisman',
      title: 'Talisman',
      logo: 'img/talisman.svg',
      chrome: 'https://chrome.google.com/webstore/detail/talisman-wallet/fijngjgcjhjmmpcmkeiomlglpeiijkld',
      firefox: 'https://addons.mozilla.org/firefox/addon/talisman-wallet-extension',
    },
  ];
  const DEV_WALLET = { id: 'development', title: 'Development keys' };
  // The chosen wallet and account, so the next visit reconnects without asking.
  const STORAGE = { wallet: 'sora-bridge-halt:wallet', account: 'sora-bridge-halt:account' };

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
    halted: null, // Root's latch is set
    lastHalt: null, // { number, block, guardian, reason } of the newest guardian halt
    blockMs: 6000, // expected block time
    cooldown: 10, // blocks after a halt before the next one is accepted
    guardians: new Set(), // hex public keys
    wallet: null, // { id, title, logo? } of the connected wallet
    unsubAccounts: null, // stops the wallet's account subscription
    accounts: [], // the connected wallet's { address, name, source, key, signer?, pair? }
    selectedKey: null, // hex public key of the chosen account
    picker: null, // 'wallets' | 'accounts' while the chooser is open
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

  function setStatus(kind, text, detail = '') {
    const element = $('status');
    element.className = `status status-${kind}`;
    element.textContent = text;
    $('status-detail').textContent = detail;
    // The console styling (beacon, link diagram, alarm glow) follows this state.
    document.body.dataset.state = kind;
  }

  /// "4 min ago", from a block count and the chain's block time.
  function ago(blocks) {
    const minutes = Math.max(0, Math.round((blocks * state.blockMs) / 60000));
    if (minutes < 1) return 'just now';
    if (minutes < 60) return `${minutes} min ago`;
    const hours = Math.round(minutes / 60);
    if (hours < 48) return `${hours} h ago`;
    return `${Math.round(hours / 24)} days ago`;
  }

  /// Blocks until the chain accepts the next halt; 0 when it would accept one now.
  /// Pool validation checks against the next block.
  function cooldownLeft() {
    if (!state.lastHalt) return 0;
    return Math.max(0, state.lastHalt.block + state.cooldown - (state.lastBlock + 1));
  }

  function recentHalt() {
    return (
      Boolean(state.lastHalt) &&
      (state.lastBlock - state.lastHalt.block) * state.blockMs < RECENT_HALT_MS
    );
  }

  function log(text, kind) {
    const item = document.createElement('li');
    const time = new Date().toLocaleTimeString();
    item.textContent = `${time} — ${text}`;
    if (kind) item.className = kind;
    $('log').appendChild(item);
  }

  function selectedAccount() {
    return state.accounts.find((account) => account.key === state.selectedKey);
  }

  function remember(key, value) {
    try {
      if (value) window.localStorage.setItem(key, value);
      else window.localStorage.removeItem(key);
    } catch {
      // Site storage is off (private window, blocked site data); the page still works.
    }
  }

  function recall(key) {
    try {
      return window.localStorage.getItem(key);
    } catch {
      return null;
    }
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
      const seconds = Math.round((state.cooldown * state.blockMs) / 1000);
      return `Rejected by the chain: a guardian sent a halt less than ${seconds} seconds ago, and that halt stands. Nothing was written and no fee was charged.`;
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
    const waiting = state.supported && cooldownLeft() > 0;
    button.textContent = waiting
      ? 'HALT JUST SENT'
      : state.supported && recentHalt()
        ? 'HALT AGAIN'
        : 'HALT BRIDGE WORKERS';
    button.disabled =
      !state.api || !state.supported || !account || state.busy || tooLong || waiting;
  }

  function renderGuardians() {
    const list = $('guardians');
    list.replaceChildren();
    if (!state.supported) {
      const item = document.createElement('li');
      item.className = 'empty';
      item.textContent = '–';
      list.appendChild(item);
      return;
    }
    if (state.guardians.size === 0) {
      const item = document.createElement('li');
      item.className = 'empty';
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

  function badge(text, kind) {
    const element = document.createElement('span');
    element.className = kind ? `badge ${kind}` : 'badge';
    element.textContent = text;
    return element;
  }

  function walletIcon(wallet, small) {
    const size = small ? 'wallet-logo small' : 'wallet-logo';
    if (wallet.logo) {
      const image = document.createElement('img');
      image.src = wallet.logo;
      image.alt = '';
      image.className = size;
      return image;
    }
    const element = document.createElement('span');
    element.className = `${size} wallet-generic`;
    element.setAttribute('aria-hidden', 'true');
    element.textContent = (wallet.title || '?').charAt(0).toUpperCase();
    return element;
  }

  function avatar(address, size) {
    try {
      return window.bridgeHaltIdenticon(address, size);
    } catch {
      return document.createElement('span');
    }
  }

  function accountLabel(account) {
    const text = document.createElement('span');
    text.className = 'account-text';
    const name = document.createElement('span');
    name.className = 'account-name';
    name.textContent = account.name || 'Unnamed account';
    const address = document.createElement('span');
    address.className = 'mono account-address';
    address.textContent = account.address;
    text.append(name, address);
    return text;
  }

  const isGuardian = (account) => state.supported && state.guardians.has(account.key);

  /// Guardians first, otherwise in the wallet's order.
  function sortedAccounts() {
    return state.accounts
      .map((account, index) => ({ account, index }))
      .sort((a, b) => Number(isGuardian(b.account)) - Number(isGuardian(a.account)) || a.index - b.index)
      .map(({ account }) => account);
  }

  function renderAccount() {
    const account = selectedAccount();
    $('account-current').hidden = !account;
    if (account) {
      const wallet = state.wallet || {};
      const title = document.createElement('span');
      title.textContent = wallet.title || '';
      $('account-avatar').replaceChildren(avatar(account.address, 36));
      $('account-name').textContent = account.name || 'Unnamed account';
      $('account-address').textContent = account.address;
      $('account-wallet').replaceChildren(walletIcon(wallet, true), title);
    }
    $('connect').textContent = !state.wallet
      ? 'Connect account'
      : account
        ? 'Change account'
        : 'Choose account';
    $('disconnect').hidden = !state.wallet;
    render();
  }

  function renderWalletList() {
    const present = injected();
    const list = $('wallet-list');
    list.replaceChildren();
    for (const wallet of walletChoices()) {
      const installed = wallet.id === DEV_WALLET.id || Boolean(present[wallet.id]);
      const item = document.createElement('li');
      const choice = document.createElement(installed ? 'button' : 'div');
      choice.className = installed ? 'choice' : 'choice choice-missing';
      const title = document.createElement('span');
      title.className = 'choice-title';
      title.textContent = wallet.title;
      choice.append(walletIcon(wallet), title);
      if (wallet.recommended) choice.append(badge('Recommended', 'badge-accent'));
      if (state.wallet && state.wallet.id === wallet.id) choice.append(badge('Connected', 'badge-ok'));
      if (installed) {
        choice.type = 'button';
        choice.addEventListener('click', () => pickWallet(wallet));
      } else if (installUrl(wallet)) {
        const link = document.createElement('a');
        link.className = 'install';
        link.href = installUrl(wallet);
        link.target = '_blank';
        link.rel = 'noopener noreferrer';
        link.textContent = 'Install';
        choice.append(link);
      } else {
        choice.append(badge('Not installed'));
      }
      item.appendChild(choice);
      list.appendChild(item);
    }
  }

  function renderAccountList() {
    const list = $('account-list');
    list.replaceChildren();
    if (state.accounts.length === 0) {
      const item = document.createElement('li');
      item.className = 'hint';
      item.textContent =
        'The wallet shares no accounts with this site. Add an account in the wallet, or let this site see one.';
      list.appendChild(item);
      return;
    }
    for (const account of sortedAccounts()) {
      const item = document.createElement('li');
      const choice = document.createElement('button');
      choice.type = 'button';
      choice.className = account.key === state.selectedKey ? 'choice choice-selected' : 'choice';
      choice.append(avatar(account.address, 32), accountLabel(account));
      if (isGuardian(account)) choice.append(badge('Guardian', 'badge-ok'));
      choice.addEventListener('click', () => chooseAccount(account.key));
      item.appendChild(choice);
      list.appendChild(item);
    }
  }

  function renderPicker() {
    const step = state.picker;
    $('picker').hidden = !step;
    if (!step) return;
    const wallets = step === 'wallets';
    $('picker-title').textContent = wallets
      ? 'Choose a wallet'
      : `Choose an account${state.wallet ? ` in ${state.wallet.title}` : ''}`;
    $('wallet-step').hidden = !wallets;
    $('account-step').hidden = wallets;
    if (wallets) renderWalletList();
    else renderAccountList();
  }

  /// Redraw after the guardian list, the wallet or its accounts change.
  function renderAccountViews() {
    renderPicker();
    renderAccount();
  }

  function resetChainView() {
    state.lastBlock = 0;
    state.supported = false;
    state.halted = null;
    state.lastHalt = null;
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
    // Development keys exist only for a local node opened with ?dev.
    if (state.wallet && state.wallet.id === DEV_WALLET.id && !devMode()) forgetWallet();
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
      // A WebSocket failure arrives as a bare Event, which has no message.
      const reason = error && error.message ? ` (${error.message})` : '';
      if (current()) setStatus('unknown', `Cannot reach ${endpoint}${reason}. Pick another node.`);
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
    if (devMode() && !state.wallet) {
      useWallet(DEV_WALLET).then(() => {
        state.picker = 'accounts';
        renderAccountViews();
      });
    }
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
      if (record.isSome) {
        const value = record.unwrap();
        last = {
          number: value.number.toNumber(),
          block: value.block.toNumber(),
          guardian: value.guardian.toHex(),
          reason: u8aToString(value.reason.toU8a(true)),
        };
      }
      const consts = api.consts.bridgeHalt || {};
      if (consts.maxReasonLength) state.maxReason = consts.maxReasonLength.toNumber();
      if (consts.haltCooldown) state.cooldown = consts.haltCooldown.toNumber();
      if (api.consts.babe && api.consts.babe.expectedBlockTime) {
        state.blockMs = api.consts.babe.expectedBlockTime.toNumber();
      }
    }
    if (!current()) return;

    // Rebuild the lists only when they change, so an open dropdown is not reset every block.
    const listChanged =
      supported !== state.supported ||
      [...guardians].sort().join() !== [...state.guardians].sort().join();
    state.lastBlock = number;
    state.halted = latch !== null && latch !== undefined;
    state.lastHalt = last;
    state.supported = supported;
    state.guardians = guardians;
    const block = $('block');
    const blockLabel = `#${number.toLocaleString('en-US')}`;
    if (block.textContent !== blockLabel) {
      block.textContent = blockLabel;
      block.classList.remove('tick');
      void block.offsetWidth; // restart the tick animation
      block.classList.add('tick');
    }
    $('runtime').textContent = `${version.specName.toString()} ${version.specVersion.toNumber()}`;
    if (state.halted) {
      setStatus(
        'halted',
        'Bridges are HALTED',
        "Root's recovery latch is set: the bridge pallets refuse calls until a Root call removes it.",
      );
    } else if (last && recentHalt()) {
      setStatus(
        'halted',
        `Halt #${last.number} sent`,
        `${ago(number - last.block)}. Bridge workers stop on it and stay stopped until devops ` +
          `restart each node with --bridge-resume-after-halt ${last.number}.`,
      );
    } else if (last) {
      setStatus(
        'running',
        'No recent halt',
        `Last halt #${last.number}, ${ago(number - last.block)}. The chain does not show ` +
          'whether devops have restarted the workers since.',
      );
    } else {
      setStatus('running', 'Bridges are running', supported ? 'No guardian halt so far.' : '');
    }

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
      $('last-number').textContent = `#${last.number}`;
      $('last-guardian').textContent = sora(last.guardian);
      $('last-block').textContent =
        `#${last.block.toLocaleString('en-US')} · ${ago(number - last.block)}`;
      $('last-reason').textContent = last.reason || '(no message)';
    }
    if (listChanged) {
      renderGuardians();
      renderAccountViews();
    } else {
      render();
    }
  }

  // ---------- wallet ----------

  const injected = () => window.injectedWeb3 || {};

  /// Known wallets first, installed or not, then any other injected extension.
  function walletChoices() {
    const choices = [...WALLETS];
    for (const id of Object.keys(injected())) {
      if (!WALLETS.some((wallet) => wallet.id === id)) choices.push({ id, title: id });
    }
    if (devMode()) choices.push(DEV_WALLET);
    return choices;
  }

  function installUrl(wallet) {
    const firefox = /Firefox\//.test(navigator.userAgent);
    return (firefox && wallet.firefox) || wallet.chrome || wallet.firefox || '';
  }

  /// Extensions inject themselves shortly after the page loads.
  async function waitForInjection(id) {
    for (let attempt = 0; attempt < 20; attempt += 1) {
      const present = injected();
      if (id ? present[id] : Object.keys(present).length > 0) return;
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

  /// Substrate accounts from a wallet, in SORA format, without duplicates.
  function toAccounts(list, source, signer) {
    const accounts = [];
    for (const account of list) {
      if (account.type === 'ethereum') continue;
      let key;
      try {
        key = publicKey(account.address);
      } catch {
        continue;
      }
      if (accounts.some((known) => known.key === key)) continue;
      accounts.push({ address: sora(key), name: account.name || '', source, key, signer });
    }
    return accounts;
  }

  function stopAccounts() {
    const stop = state.unsubAccounts;
    state.unsubAccounts = null;
    if (stop) {
      try {
        stop();
      } catch {
        // Already stopped.
      }
    }
  }

  /// Connect `wallet` and load its accounts, following later changes in the
  /// wallet. Throws when the wallet is missing or the user declines.
  async function useWallet(wallet) {
    let accounts;
    let unsubscribe = null;
    if (wallet.id === DEV_WALLET.id) {
      accounts = await devAccounts();
    } else {
      const source = injected()[wallet.id];
      if (!source) throw new Error('not installed in this browser.');
      const extension = await source.enable(APP_NAME);
      accounts = toAccounts(await extension.accounts.get(), wallet.id, extension.signer);
      if (typeof extension.accounts.subscribe === 'function') {
        unsubscribe = extension.accounts.subscribe((list) => {
          if (!state.wallet || state.wallet.id !== wallet.id) return;
          state.accounts = toAccounts(list, wallet.id, extension.signer);
          if (!selectedAccount()) state.selectedKey = null;
          renderAccountViews();
        });
      }
    }
    stopAccounts();
    state.unsubAccounts = unsubscribe;
    state.wallet = { id: wallet.id, title: wallet.title, logo: wallet.logo };
    state.accounts = accounts;
    if (!selectedAccount()) state.selectedKey = null;
    remember(STORAGE.wallet, wallet.id);
  }

  async function pickWallet(wallet) {
    const note = $('picker-note');
    note.textContent =
      wallet.id === DEV_WALLET.id ? '' : `Waiting for ${wallet.title}. Allow this site if it asks.`;
    try {
      await useWallet(wallet);
    } catch (error) {
      note.textContent = `${wallet.title}: ${(error && error.message) || error}`;
      return;
    }
    note.textContent = '';
    if (state.accounts.length === 1) {
      chooseAccount(state.accounts[0].key);
      return;
    }
    state.picker = 'accounts';
    renderAccountViews();
  }

  function chooseAccount(key) {
    const account = state.accounts.find((item) => item.key === key);
    if (!account) return;
    state.selectedKey = key;
    remember(STORAGE.account, account.address);
    closePicker();
  }

  async function showWallets() {
    $('picker-note').textContent = '';
    state.picker = 'wallets';
    renderAccountViews();
    await waitForInjection();
    if (state.picker === 'wallets') renderPicker();
  }

  function openPicker() {
    if (!state.wallet) {
      showWallets();
      return;
    }
    $('picker-note').textContent = '';
    state.picker = 'accounts';
    renderAccountViews();
  }

  function closePicker() {
    const wasOpen = Boolean(state.picker);
    state.picker = null;
    $('picker-note').textContent = '';
    renderAccountViews();
    if (wasOpen) $('connect').focus();
  }

  /// Forget the wallet on this page. The site stays allowed in the wallet itself.
  function forgetWallet() {
    stopAccounts();
    state.wallet = null;
    state.accounts = [];
    state.selectedKey = null;
    state.picker = null;
    remember(STORAGE.wallet, null);
    remember(STORAGE.account, null);
    renderAccountViews();
  }

  /// Reconnect the wallet and account chosen on an earlier visit.
  async function restoreWallet() {
    const id = recall(STORAGE.wallet);
    if (!id || id === DEV_WALLET.id) return;
    const wallet = WALLETS.find((item) => item.id === id) || { id, title: id };
    await waitForInjection(id);
    if (!injected()[id] || state.wallet) return;
    try {
      await useWallet(wallet);
    } catch {
      return; // Declined or gone; the button still offers every wallet.
    }
    let key = null;
    try {
      key = publicKey(recall(STORAGE.account) || '');
    } catch {
      key = null;
    }
    if (key && state.accounts.some((account) => account.key === key)) state.selectedKey = key;
    else if (state.accounts.length === 1) state.selectedKey = state.accounts[0].key;
    renderAccountViews();
  }

  // ---------- halt ----------

  /// The first press lifts the safety cover; the emergency stop sends the halt.
  function askToConfirm() {
    document.body.classList.add('arming');
    $('confirm').hidden = false;
    $('confirm-yes').focus();
  }

  function disarm() {
    document.body.classList.remove('arming');
    $('confirm').hidden = true;
  }

  async function sendHalt() {
    disarm();
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
        log(
          `Included in block ${block}: the halt is on chain, and the bridge workers stop on it. Waiting for finality…`,
          'success',
        );
        idle(); // Keep watching for finality, but free the page.
      } else if (status.isRetracted) {
        log('That block was replaced by a fork. Waiting for the halt to be included again…');
      } else if (status.isFinalized) {
        log(`Final in block ${status.asFinalized.toHex()}.`, 'success');
        close();
      } else if (status.isFinalityTimeout) {
        log(
          'The including block has not been finalized; finality may be stalled. The status at the top shows whether the halt is on chain.',
          'error',
        );
        close();
      } else if (status.isDropped || status.isInvalid || status.isUsurped) {
        log(
          'The network dropped the transaction. If another guardian sent a halt in the last minute, that halt stands; otherwise press the button again.',
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
    $('connect').addEventListener('click', openPicker);
    $('disconnect').addEventListener('click', () => {
      forgetWallet();
      $('connect').focus();
    });
    $('picker-close').addEventListener('click', closePicker);
    $('picker-back').addEventListener('click', showWallets);
    $('picker').addEventListener('keydown', (event) => {
      if (event.key === 'Escape') closePicker();
    });
    $('reason').addEventListener('input', render);
    $('halt').addEventListener('click', askToConfirm);
    $('confirm-yes').addEventListener('click', sendHalt);
    $('confirm-no').addEventListener('click', () => {
      disarm();
      $('halt').focus();
    });
    renderAccount();
    if (select.value) connect(select.value);
    else setStatus('unknown', 'No RPC endpoint configured. Edit config.js.');
    restoreWallet();
  }

  start();
})();
