import { readFileSync } from 'node:fs';
import path from 'node:path';
import { sha256 } from './delivery.mjs';
import { run } from './process.mjs';

export function buildEvidence(binary) {
  const executable_sha256 = sha256(binary);
  let version;
  try { version = run(binary, ['--version'], { capture: true }).trim().match(/^My Rivian Data (\d+\.\d+\.\d+)$/u)?.[1]; }
  catch { /* Never retain native diagnostics in an evidence artifact. */ }
  if (!version) throw new Error('The selected executable did not return a bounded application version.');
  const evidence = { executable_sha256, version, source_revision: 'unknown', provenance: 'unpackaged executable; source identity not established' };
  for (const manifest of [path.join(path.dirname(binary), 'BUILD.json'), path.resolve(path.dirname(binary), '../Resources/BUILD.json')]) {
    try {
      const build = JSON.parse(readFileSync(manifest, 'utf8'));
      if (build.sha256 === executable_sha256 && build.version === version && /^[a-f0-9]{40}$/u.test(build.revision)
        && build.source?.file === 'source.tar.gz' && /^[a-f0-9]{64}$/u.test(build.source.sha256)
        && sha256(path.join(path.dirname(manifest), 'source.tar.gz')) === build.source.sha256) {
        evidence.source_revision = build.revision;
        evidence.provenance = 'matching package BUILD.json; source archive included in package';
        break;
      }
    } catch { /* Absent package provenance is explicitly unknown. */ }
  }
  return evidence;
}
