'use strict';

const path = require('node:path');
const { main } = require('../../runtime-upgrade-4.8.15/validation/prepare-governance.cjs');
main(['--candidate-spec', '140', '--rehearsal', path.join(__dirname, '../live-fork-rehearsal.json'), ...process.argv.slice(2)])
  .catch((error) => { console.error(error); process.exitCode = 1; });
