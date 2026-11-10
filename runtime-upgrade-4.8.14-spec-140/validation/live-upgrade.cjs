'use strict';

// Replay the published spec-140 package with the maintained validation tools.
const { runCli } = require('../../runtime-upgrade-4.8.14/validation/live-upgrade.cjs');
runCli(['--candidate-spec', '140', ...process.argv.slice(2)]);
