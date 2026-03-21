globalThis.__SCCP_ROUTER_TEST_CONFIG = { localDomain: 1, otherEvmDomain: 2 };
await import('../../evm/shared/test-core/registerSccpRouterTests.mjs');
