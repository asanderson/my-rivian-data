import { readFileSync } from 'node:fs';
import path from 'node:path';
import { root, run } from './process.mjs';

export function bindgenVersion() {
  const lock = readFileSync(path.join(root, 'Cargo.lock'), 'utf8');
  const version = lock.match(/\[\[package\]\]\r?\nname = "wasm-bindgen"\r?\nversion = "([^"]+)"/u)?.[1];
  if (!version) throw new Error('Cannot locate wasm-bindgen in Cargo.lock.');
  return version;
}

export function hasMatchingBindgen() {
  try {
    return run('wasm-bindgen', ['--version'], { capture: true }).trim() === `wasm-bindgen ${bindgenVersion()}`;
  } catch {
    return false;
  }
}

export function requireToolchain() {
  if (Number(process.versions.node.split('.')[0]) < 24) throw new Error('Node.js 24 or newer is required to build.');
  if (!hasMatchingBindgen()) {
    throw new Error(`Run node scripts/setup.mjs, or install the matching CLI: cargo install wasm-bindgen-cli --version ${bindgenVersion()} --locked`);
  }
}
