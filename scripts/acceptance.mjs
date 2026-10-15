import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { createInterface } from 'node:readline/promises';
import { acceptanceCatalogEvidence, acceptanceModels, acceptanceStatuses, sanitizeAcceptance } from './lib/acceptance.mjs';
import { defaultBinary, startLocalHost } from './lib/local-host.mjs';
import { main, root } from './lib/process.mjs';
import { buildEvidence } from './lib/build-evidence.mjs';
import { sha256 } from './lib/delivery.mjs';

main(async () => {
  const args = process.argv.slice(2);
  if (args.includes('--help') || args.includes('-h')) {
    console.log('Usage: node scripts/acceptance.mjs [--binary PATH]\nOpens the normal live app for manual owner sign-in and UI checks. Records only an approved model, numeric firmware version and catalog operation statuses. No credentials, vehicle IDs, response data or free text are collected.');
    return;
  }
  if (args.length && (args.length !== 2 || args[0] !== '--binary' || !args[1] || args[1].startsWith('--'))) {
    throw new Error('Use --binary PATH or no options for manual live acceptance.');
  }
  if (!process.stdin.isTTY || !process.stdout.isTTY) throw new Error('Manual owner acceptance requires an interactive terminal.');
  const binary = args.length ? path.resolve(args[1]) : defaultBinary;
  const build = buildEvidence(binary);
  const catalog = acceptanceCatalogEvidence(binary);
  const allowedIds = catalog.ids;
  console.log('The native app will open in your default browser. Sign in and complete verification only in that local app. Use its UI for checks.\nThis helper does not log in, call Rivian, or read account data. Enter only the requested model/version/status choices.');
  const host = await startLocalHost({ binary });
  const input = createInterface({ input: process.stdin, output: process.stdout });
  const choose = async (prompt, allowed, fallback) => {
    for (;;) {
      const value = (await input.question(prompt)).trim();
      if (!value && fallback) return fallback;
      if (allowed(value)) return value;
      console.log('Please enter one of the allowed choices; do not enter account information.');
    }
  };
  const interrupted = () => { input.close(); void host.stop(); };
  process.once('SIGINT', interrupted); process.once('SIGTERM', interrupted);
  try {
    await input.question('When you have finished the manual UI checks, press Enter here. ');
    const model = await choose(`Vehicle model (${acceptanceModels.join('/')}) only: `, value => acceptanceModels.includes(value));
    const firmware = await choose('Observed firmware version only (for example 2026.38.1): ', value => /^\d{4}(?:\.\d{1,6}){1,3}$/u.test(value));
    console.log(`Record each catalog operation as ${acceptanceStatuses.join('/')}. Enter leaves it not-tested. A blocked capability is not a passing live test.`);
    const operations = [];
    for (const id of allowedIds) operations.push({ id, status: await choose(`${id}: `, value => acceptanceStatuses.includes(value), 'not-tested') });
    const record = { approved: true, model, firmware, operations };
    if (sha256(binary) !== build.executable_sha256) throw new Error('The selected executable changed during acceptance; record discarded.');
    const sanitized = { ...sanitizeAcceptance(record, allowedIds), build: { ...build, catalog_sha256: catalog.sha256 } };
    console.log(`Review the complete record that would be saved:\n${JSON.stringify(sanitized, null, 2)}`);
    const approved = await choose('Save exactly this sanitized record? (yes/no): ', value => ['yes', 'no'].includes(value));
    if (approved !== 'yes') { console.log('Record discarded.'); return; }
    const directory = path.join(root, 'artifacts/acceptance');
    mkdirSync(directory, { recursive: true });
    const filename = path.join(directory, `${new Date().toISOString().replace(/[:.]/gu, '-')}.json`);
    writeFileSync(filename, `${JSON.stringify(sanitized, null, 2)}\n`);
    console.log(`Approved sanitized acceptance record: ${filename}`);
  } finally {
    input.close(); process.off('SIGINT', interrupted); process.off('SIGTERM', interrupted);
    await host.stop();
  }
});
