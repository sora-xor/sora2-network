// Deployment settings. Whoever hosts the page controls these; URL parameters
// cannot change them (`?rpc=` is accepted only for a node on this computer).
window.BRIDGE_HALT_CONFIG = {
  // RPC endpoints offered on the page, in order.
  endpoints: ['wss://ws.mof.sora.org', 'wss://mof2.sora.org'],
  // The page signs only on this chain. SORA mainnet genesis.
  genesisHash: '0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5',
};
