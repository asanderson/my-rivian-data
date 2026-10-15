import { createHash } from 'node:crypto';

// The inventory covers all locked dependencies (including build/test and other
// target dependencies). Archive hashes are copied from the locks, never guessed.
export function createSboms({ version, revision, cargoLock, npmLock, notices }) {
  const components = [];
  for (const block of cargoLock.split('[[package]]').slice(1)) {
    const field = key => block.match(new RegExp(`^${key} = "([^"\\n]+)"`, 'm'))?.[1];
    const name = field('name'), packageVersion = field('version'), source = field('source');
    if (!source) continue; // Workspace source is described by the root component.
    if (source !== 'registry+https://github.com/rust-lang/crates.io-index') throw new Error(`Unsupported Cargo source for ${name}; supply an exact source digest.`);
    const checksum = field('checksum');
    if (!checksum || !/^[a-f0-9]{64}$/.test(checksum)) throw new Error(`Missing/invalid Cargo checksum for ${name}.`);
    const line = notices.split('\n').find(row => row.startsWith(`| ${name} | ${packageVersion} |`));
    const license = line?.split('|')[3]?.trim();
    components.push({ type: 'library', name, version: packageVersion,
      'bom-ref': `pkg:cargo/${name}@${packageVersion}`, purl: `pkg:cargo/${name}@${packageVersion}`,
      hashes: [{ alg: 'SHA-256', content: checksum }],
      ...(license ? { licenses: [{ license: { name: license } }] } : {}),
      externalReferences: [{ type: 'distribution', url: `https://crates.io/api/v1/crates/${name}/${packageVersion}/download` }],
      properties: [{ name: 'my-rivian-data:lock-source', value: source }, { name: 'my-rivian-data:inventory-scope', value: 'all locked Cargo targets and build dependencies' }],
    });
  }
  for (const [location, entry] of Object.entries(npmLock.packages ?? {})) {
    if (!location) continue;
    const name = entry.name ?? location.split('node_modules/').at(-1);
    const integrity = /^sha(256|384|512)-([A-Za-z0-9+/]+={0,2})$/.exec(entry.integrity ?? '');
    if (!integrity || !entry.resolved || !entry.version) throw new Error(`Missing/unsupported npm integrity for ${location}.`);
    const bytes = Buffer.from(integrity[2], 'base64');
    if (bytes.length !== Number(integrity[1]) / 8) throw new Error(`Invalid npm integrity for ${location}.`);
    const purl = `pkg:npm/${name.startsWith('@') ? name.replace('@', '%40') : name}@${entry.version}`;
    components.push({ type: 'library', name, version: entry.version, purl,
      'bom-ref': `${purl}?lock_path=${encodeURIComponent(location)}`,
      scope: entry.dev ? 'excluded' : 'required',
      hashes: [{ alg: `SHA-${integrity[1]}`, content: bytes.toString('hex') }],
      ...(entry.license ? { licenses: [{ license: { name: entry.license } }] } : {}),
      externalReferences: [{ type: 'distribution', url: entry.resolved }],
      properties: [{ name: 'my-rivian-data:lock-path', value: location }, { name: 'my-rivian-data:inventory-scope', value: entry.dev ? 'build/test dependency; not a runtime claim' : 'locked UI dependency' }],
    });
  }
  const rootRef = `my-rivian-data:${revision}`;
  const cyclonedx = { bomFormat: 'CycloneDX', specVersion: '1.6', version: 1,
    metadata: { component: { type: 'application', 'bom-ref': rootRef, name: 'My Rivian Data', version, licenses: [{ license: { id: 'GPL-3.0-only' } }], properties: [{ name: 'my-rivian-data:source-revision', value: revision }] } }, components,
    dependencies: [{ ref: rootRef, dependsOn: components.map(component => component['bom-ref']) }],
  };
  const digest = createHash('sha256').update(cargoLock).update(JSON.stringify(npmLock)).digest('hex');
  const id = index => `SPDXRef-Package-${index}`;
  const spdx = { spdxVersion: 'SPDX-2.3', dataLicense: 'CC0-1.0', SPDXID: 'SPDXRef-DOCUMENT', name: `my-rivian-data-${version}-locked-dependencies`,
    documentNamespace: `https://my-rivian-data.invalid/spdx/${revision}/${digest}`,
    creationInfo: { created: '1970-01-01T00:00:00Z', creators: ['Tool: my-rivian-data-locked-sbom'], comment: 'Deterministic lock inventory. Epoch is a reproducibility marker, not a release date. All locked targets/build dependencies included; not a deployed-only dependency graph.' },
    packages: [{ SPDXID: id(0), name: 'My Rivian Data', versionInfo: version, downloadLocation: 'NOASSERTION', filesAnalyzed: false, licenseConcluded: 'GPL-3.0-only', licenseDeclared: 'GPL-3.0-only', copyrightText: 'NOASSERTION', sourceInfo: `Git revision ${revision}` },
      ...components.map((component, index) => ({ SPDXID: id(index + 1), name: component.name, versionInfo: component.version, downloadLocation: component.externalReferences[0].url, filesAnalyzed: false,
        licenseConcluded: 'NOASSERTION', licenseDeclared: 'NOASSERTION', copyrightText: 'NOASSERTION',
        ...(component.licenses ? { licenseComments: component.licenses[0].license.name } : {}),
        checksums: component.hashes.map(hash => ({ algorithm: hash.alg.replace('-', ''), checksumValue: hash.content })),
        externalRefs: [{ referenceCategory: 'PACKAGE-MANAGER', referenceType: 'purl', referenceLocator: component.purl }],
      }))],
    relationships: [{ spdxElementId: 'SPDXRef-DOCUMENT', relationshipType: 'DESCRIBES', relatedSpdxElement: id(0) }, ...components.map((_, index) => ({ spdxElementId: id(0), relationshipType: 'DEPENDS_ON', relatedSpdxElement: id(index + 1) }))],
  };
  return { cyclonedx, spdx };
}
