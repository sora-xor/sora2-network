// bundle-polkadot-api.js 16.5.4 names two UMD globals that no published bundle
// defines: `constants` (@polkadot/types/extrinsic/constants) and `util$1`
// (@polkadot/types/util). It reads exactly two members of them:
// - constants.LATEST_EXTRINSIC_VERSION, a fallback when metadata lists no
//   extrinsic version (4 in @polkadot/types 16.5.4);
// - util$1.unwrapStorageSi, used by every typed storage query.
// The definitions below are copied from @polkadot/types 16.5.4. This file must
// load before bundle-polkadot-api.js, which captures the globals when it loads.
window.constants = window.constants || { LATEST_EXTRINSIC_VERSION: 4 };
window.util$1 = window.util$1 || {
  unwrapStorageSi: (type) => (type.isPlain ? type.asPlain : type.asMap.value),
};
