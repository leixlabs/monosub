#!/usr/bin/env node
// Verifies that every locale exposes exactly the same key set as en.json
// (docs/tech-spec/tasks.md P1-20). Exits non-zero and prints the offending keys
// per file when a locale is missing or carrying extra keys.
//
// Usage: node scripts/utils/check-i18n-keys.mjs

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const localeDir = path.resolve(here, '../../src/i18n/locales');
const referenceFile = 'en.json';

/** Flatten a nested locale object into dotted keys. */
const flattenKeys = (value, prefix = '') =>
  Object.entries(value).flatMap(([key, child]) => {
    const dotted = prefix ? `${prefix}.${key}` : key;
    return typeof child === 'object' && child !== null ? flattenKeys(child, dotted) : [dotted];
  });

const readLocale = (file) => {
  const raw = fs.readFileSync(path.join(localeDir, file), 'utf8');
  try {
    return JSON.parse(raw);
  } catch (error) {
    throw new Error(`${file} is not valid JSON: ${error.message}`);
  }
};

const files = fs
  .readdirSync(localeDir)
  .filter((file) => file.endsWith('.json'))
  .sort();

if (!files.includes(referenceFile)) {
  console.error(`[check-i18n-keys] missing reference locale ${referenceFile} in ${localeDir}`);
  process.exit(1);
}

const referenceKeys = new Set(flattenKeys(readLocale(referenceFile)));
let failures = 0;

for (const file of files) {
  if (file === referenceFile) continue;

  const keys = new Set(flattenKeys(readLocale(file)));
  const missing = [...referenceKeys].filter((key) => !keys.has(key)).sort();
  const extra = [...keys].filter((key) => !referenceKeys.has(key)).sort();

  if (missing.length === 0 && extra.length === 0) continue;

  failures += 1;
  console.error(`${file}:`);
  if (missing.length > 0) {
    console.error(`  missing ${missing.length}: ${missing.join(', ')}`);
  }
  if (extra.length > 0) {
    console.error(`  extra ${extra.length}: ${extra.join(', ')}`);
  }
}

if (failures > 0) {
  console.error(`\n[check-i18n-keys] ${failures} of ${files.length} locale file(s) differ from ${referenceFile}`);
  process.exit(1);
}

console.log(`[check-i18n-keys] OK — ${files.length} locales match ${referenceFile} (${referenceKeys.size} keys each)`);
