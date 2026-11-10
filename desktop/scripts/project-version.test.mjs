import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import {
  findNextPatchVersion,
  isVersionOnlyChange,
  compareStableVersions,
  parseStableVersion,
  readProjectVersion,
  VERSION_FILES,
  writeProjectVersion,
} from './project-version.mjs';

function projectFixture() {
  const root = mkdtempSync(join(tmpdir(), 'neo-airplay-version-'));
  mkdirSync(join(root, 'desktop/src-tauri'), { recursive: true });
  writeFileSync(join(root, VERSION_FILES[0]), '{\n  "name": "neo-airplay-desktop",\n  "version": "0.2.0"\n}\n');
  writeFileSync(join(root, VERSION_FILES[1]), '{\n  "version": "0.2.0"\n}\n');
  writeFileSync(join(root, VERSION_FILES[2]), '[package]\nname = "neo-airplay"\nversion = "0.2.0"\n');
  writeFileSync(join(root, VERSION_FILES[3]), [
    '[[package]]',
    'name = "another-crate"',
    'version = "0.2.0"',
    '',
    '[[package]]',
    'name = "neo-airplay"',
    'version = "0.2.0"',
    'dependencies = ["another-crate"]',
    '',
  ].join('\n'));
  return root;
}

test('accepts only stable X.Y.Z versions without leading zeroes', () => {
  assert.deepEqual(parseStableVersion('0.2.10'), { major: 0n, minor: 2n, patch: 10n });
  for (const version of ['v0.2.0', '0.2', '0.2.0-rc.1', '01.2.0', '0.02.0', '0.2.00']) {
    assert.throws(() => parseStableVersion(version), /stable X\.Y\.Z/);
  }
});

test('reads the version only when all four project files agree', () => {
  const root = projectFixture();
  try {
    assert.equal(readProjectVersion(root), '0.2.0');
    writeFileSync(join(root, VERSION_FILES[1]), '{\n  "version": "0.2.1"\n}\n');
    assert.throws(() => readProjectVersion(root), /do not match/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('increments patch versions across digit boundaries and skips tagged versions', () => {
  assert.equal(findNextPatchVersion('0.2.9', []), '0.2.10');
  assert.equal(findNextPatchVersion('0.2.0', ['v0.2.1', 'v0.2.2']), '0.2.3');
  assert.equal(findNextPatchVersion('2.8.4', ['v2.8.4', 'v2.8.5']), '2.8.6');
});

test('compares numeric version components', () => {
  assert.equal(compareStableVersions('0.2.9', '0.2.10'), -1);
  assert.equal(compareStableVersions('0.3.0', '0.2.99'), 1);
  assert.equal(compareStableVersions('0.2.0', '0.2.0'), 0);
});

test('updates only the project package in Cargo.lock', () => {
  const root = projectFixture();
  try {
    writeProjectVersion('0.2.3', root);
    assert.equal(readProjectVersion(root), '0.2.3');
    const cargoLock = readFileSync(join(root, VERSION_FILES[3]), 'utf8');
    assert.match(cargoLock, /name = "another-crate"\nversion = "0\.2\.0"/);
    assert.match(cargoLock, /name = "neo-airplay"\nversion = "0\.2\.3"/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('recognizes non-empty changes containing only version source files', () => {
  assert.equal(isVersionOnlyChange([VERSION_FILES[0], VERSION_FILES[3]]), true);
  assert.equal(isVersionOnlyChange([]), false);
  assert.equal(isVersionOnlyChange([VERSION_FILES[0], 'desktop/src/App.vue']), false);
});
