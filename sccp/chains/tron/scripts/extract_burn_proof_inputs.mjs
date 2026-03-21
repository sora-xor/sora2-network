#!/usr/bin/env node

import { pathToFileURL } from 'node:url';

export {
  buildOutput,
  decodeBurnPayloadV1,
  EXPORT_SCHEMA,
  LEGACY_SCHEMA,
  main,
  parseArgs,
  selectBurnLog,
} from './extract_burn_export.mjs';

import { main as extractBurnExportMain } from './extract_burn_export.mjs';

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  extractBurnExportMain();
}
