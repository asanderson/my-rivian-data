import { chmodSync, copyFileSync, cpSync, mkdirSync, readdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { root, run } from './process.mjs';
import { signMac, signWindows } from './signing.mjs';

// GitHub's macOS runners intermittently answer `hdiutil: create failed - Resource busy`:
// `create -srcfolder` mounts a scratch image to populate it, Spotlight starts indexing
// that volume, and the detach that follows is refused. -nospotlight keeps the indexer
// off the scratch volume; the bounded retry covers whatever else (XProtect scans of the
// freshly written tree) holds it, since nothing about the input changes between attempts
// and -ov overwrites a partial image.
export function createDiskImage(imageRoot, dmg, { attempts = 5, delayMs = 5000, runner = run } = {}) {
  for (let attempt = 1; ; attempt += 1) {
    try {
      return runner('hdiutil', ['create', '-ov', '-nospotlight', '-format', 'UDZO', '-volname', 'My Rivian Data', '-srcfolder', imageRoot, dmg]);
    } catch (error) {
      if (attempt >= attempts) throw error;
      console.log(`hdiutil create failed (attempt ${attempt} of ${attempts}): ${error.message}; retrying in ${delayMs / 1000}s`);
      Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, delayMs);
    }
  }
}

export function nativePackages({ staging, output, name, version, revision, signed }) {
  const artifacts = [];
  if (process.platform === 'win32') {
    const installer = path.join(output, `${name}-setup.exe`);
    const makensis = process.env.RIVIAN_MAKENSIS ?? 'C:\\Program Files (x86)\\NSIS\\makensis.exe';
    run(makensis, [`/DVERSION=${version}`, `/DREVISION=${revision.slice(0, 12)}`, `/DSOURCE=${staging}`, `/DOUTPUT=${installer}`,
      ...(signed ? [`/DNODE=${process.execPath}`, `/DSIGNER=${path.join(root, 'scripts/sign-windows.mjs')}`] : []), path.join(root, 'packaging/windows/installer.nsi')]);
    if (signed) signWindows(installer);
    artifacts.push(installer);
  } else if (process.platform === 'darwin') {
    const imageRoot = path.join(output, `${name}-dmg`);
    rmSync(imageRoot, { recursive: true, force: true });
    const app = path.join(imageRoot, 'My Rivian Data.app');
    mkdirSync(path.join(app, 'Contents/MacOS'), { recursive: true });
    cpSync(staging, path.join(app, 'Contents/Resources'), { recursive: true });
    copyFileSync(path.join(staging, 'my-rivian-data'), path.join(app, 'Contents/MacOS/my-rivian-data'));
    chmodSync(path.join(app, 'Contents/MacOS/my-rivian-data'), 0o755);
    // Resources contain the complete portable payload, source and legal notices.
    writeFileSync(path.join(app, 'Contents/Info.plist'), `<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>CFBundleName</key><string>My Rivian Data</string><key>CFBundleIdentifier</key><string>org.my-rivian-data.desktop</string><key>CFBundleExecutable</key><string>my-rivian-data</string><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleShortVersionString</key><string>${version}</string><key>CFBundleVersion</key><string>${version}</string><key>LSMinimumSystemVersion</key><string>13.0</string><key>NSHighResolutionCapable</key><true/></dict></plist>\n`);
    symlinkSync('/Applications', path.join(imageRoot, 'Applications'));
    const dmg = path.join(output, `${name}.dmg`);
    if (signed) signMac(app, 'app');
    createDiskImage(imageRoot, dmg);
    if (signed) signMac(dmg, 'dmg');
    artifacts.push(dmg);
  } else if (process.platform === 'linux') {
    if (process.arch !== 'x64') throw new Error('Linux native packages currently support x64 only.');
    const packageRoot = path.join(output, `${name}-linux-root`);
    rmSync(packageRoot, { recursive: true, force: true });
    mkdirSync(path.join(packageRoot, 'usr/bin'), { recursive: true });
    mkdirSync(path.join(packageRoot, 'usr/share/applications'), { recursive: true });
    cpSync(staging, path.join(packageRoot, 'usr/share/my-rivian-data'), { recursive: true });
    copyFileSync(path.join(staging, 'my-rivian-data'), path.join(packageRoot, 'usr/bin/my-rivian-data'));
    chmodSync(path.join(packageRoot, 'usr/bin/my-rivian-data'), 0o755);
    copyFileSync(path.join(root, 'packaging/linux/my-rivian-data.desktop'), path.join(packageRoot, 'usr/share/applications/my-rivian-data.desktop'));
    mkdirSync(path.join(packageRoot, 'DEBIAN'));
    writeFileSync(path.join(packageRoot, 'DEBIAN/control'), `Package: my-rivian-data\nVersion: ${version}+git${revision.slice(0, 12)}\nSection: utils\nPriority: optional\nArchitecture: amd64\nMaintainer: My Rivian Data contributors <maintainers@my-rivian-data.invalid>\nDepends: libc6 (>= 2.35), libgcc-s1, xdg-utils\nDescription: Local Rivian owner data application\n Unofficial application; opens an IPv4 loopback page in your browser.\n`);
    const deb = path.join(output, `${name}.deb`);
    run('dpkg-deb', ['--root-owner-group', '--build', packageRoot, deb]);
    rmSync(path.join(packageRoot, 'DEBIAN'), { recursive: true });
    const top = path.join(output, `${name}-rpmbuild`);
    rmSync(top, { recursive: true, force: true });
    for (const directory of ['BUILD', 'BUILDROOT', 'RPMS', 'SOURCES', 'SPECS', 'SRPMS']) mkdirSync(path.join(top, directory), { recursive: true });
    const spec = path.join(top, 'SPECS/my-rivian-data.spec');
    // rpm owns every installed path. No install/uninstall scriptlets execute code.
    const quotedSource = `'${path.join(packageRoot, 'usr').replaceAll("'", "'\\''").replaceAll('%', '%%')}'`;
    writeFileSync(spec, `%global __requires_exclude_from ^/usr/share/my-rivian-data/docs/\n%global __provides_exclude_from ^/usr/share/my-rivian-data/docs/\nName: my-rivian-data\nVersion: ${version}\nRelease: 1.git${revision.slice(0, 12)}\nSummary: Local Rivian owner data application\nLicense: GPL-3.0-only\nBuildArch: x86_64\nRequires: glibc >= 2.35\nRequires: xdg-utils\n%description\nUnofficial Rivian owner data application.\n%install\nmkdir -p "%{buildroot}"\ncp -a ${quotedSource} "%{buildroot}/"\n%files\n/usr/bin/my-rivian-data\n/usr/share/my-rivian-data\n/usr/share/applications/my-rivian-data.desktop\n`);
    run('rpmbuild', ['-bb', '--define', `_topdir ${top}`, '--define', '_build_id_links none', '--define', '__os_install_post %{nil}', spec]);
    const rpm = readdirSync(path.join(top, 'RPMS/x86_64')).find(file => file.endsWith('.rpm'));
    if (!rpm) throw new Error('RPM build produced no artifact.');
    const rpmOutput = path.join(output, `${name}.rpm`);
    copyFileSync(path.join(top, 'RPMS/x86_64', rpm), rpmOutput);
    artifacts.push(deb, rpmOutput);
  } else throw new Error('Unsupported desktop platform.');
  return artifacts;
}
