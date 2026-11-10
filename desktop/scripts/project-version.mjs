import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const VERSION_FILES = Object.freeze([
  'desktop/package.json',
  'desktop/src-tauri/tauri.conf.json',
  'desktop/src-tauri/Cargo.toml',
  'desktop/src-tauri/Cargo.lock',
]);

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

export function parseStableVersion(version) {
  const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.exec(version);
  if (!match) {
    throw new Error(`Expected a stable X.Y.Z version, found '${version}'.`);
  }

  return { major: BigInt(match[1]), minor: BigInt(match[2]), patch: BigInt(match[3]) };
}

export function compareStableVersions(left, right) {
  const leftParts = parseStableVersion(left);
  const rightParts = parseStableVersion(right);
  for (const part of ['major', 'minor', 'patch']) {
    if (leftParts[part] < rightParts[part]) return -1;
    if (leftParts[part] > rightParts[part]) return 1;
  }
  return 0;
}

function readCargoPackageVersion(content, packageName) {
  const blocks = content.split(/(?=^\[)/m);
  const block = blocks.find((entry) => entry.split(/\r?\n/, 1)[0] === `[${packageName}]`);
  const match = block?.match(/^version\s*=\s*"([^"]+)"\s*$/m);
  if (!match) throw new Error(`Could not read [${packageName}] version.`);
  return match[1];
}

function readCargoLockPackageVersion(content, targetName) {
  const blocks = content.split(/(?=^\[\[package\]\]\s*$)/m);
  const matches = blocks.filter((block) => /^name\s*=\s*"([^"]+)"\s*$/m.exec(block)?.[1] === targetName);
  if (matches.length !== 1) {
    throw new Error(`Expected exactly one '${targetName}' package in Cargo.lock, found ${matches.length}.`);
  }

  const match = /^version\s*=\s*"([^"]+)"\s*$/m.exec(matches[0]);
  if (!match) throw new Error(`Could not read '${targetName}' version from Cargo.lock.`);
  return match[1];
}

export function readProjectVersion(root = repositoryRoot) {
  const packageJson = JSON.parse(readFileSync(resolve(root, VERSION_FILES[0]), 'utf8'));
  const tauriJson = JSON.parse(readFileSync(resolve(root, VERSION_FILES[1]), 'utf8'));
  const cargoToml = readFileSync(resolve(root, VERSION_FILES[2]), 'utf8');
  const cargoLock = readFileSync(resolve(root, VERSION_FILES[3]), 'utf8');
  const versions = [
    packageJson.version,
    tauriJson.version,
    readCargoPackageVersion(cargoToml, 'package'),
    readCargoLockPackageVersion(cargoLock, 'neo-airplay'),
  ];

  for (const version of versions) parseStableVersion(version);
  if (new Set(versions).size !== 1) {
    throw new Error(`Project version files do not match: ${versions.join(', ')}.`);
  }
  return versions[0];
}

function replaceCargoVersion(content, header, version) {
  const lines = content.split(/(?<=\n)/);
  const blocks = [];
  let current = [];
  for (const line of lines) {
    if (line.trim() === header && current.length > 0) {
      blocks.push(current);
      current = [];
    }
    current.push(line);
  }
  if (current.length > 0) blocks.push(current);

  const targets = blocks.filter((block) => header === '[package]'
    ? block[0]?.trim() === header
    : block[0]?.trim() === header && block.some((line) => line.trim() === 'name = "neo-airplay"'));
  if (targets.length !== 1) {
    throw new Error(`Expected exactly one ${header}${header === '[[package]]' ? ' named neo-airplay' : ''} block.`);
  }

  const target = targets[0];
  const index = target.findIndex((line) => /^version\s*=\s*"[^"]+"\s*$/.test(line.trim()));
  if (index < 0) throw new Error(`Could not find version in ${header} block.`);
  target[index] = target[index].replace(/^(version\s*=\s*")[^"]+("\s*)(\r?\n)?$/, (_line, prefix, suffix, newline = '') => `${prefix}${version}${suffix}${newline}`);
  return blocks.flat().join('');
}

export function writeProjectVersion(version, root = repositoryRoot) {
  parseStableVersion(version);

  const packagePath = resolve(root, VERSION_FILES[0]);
  const packageJson = JSON.parse(readFileSync(packagePath, 'utf8'));
  packageJson.version = version;
  writeFileSync(packagePath, `${JSON.stringify(packageJson, null, 2)}\n`);

  const tauriPath = resolve(root, VERSION_FILES[1]);
  const tauriJson = JSON.parse(readFileSync(tauriPath, 'utf8'));
  tauriJson.version = version;
  writeFileSync(tauriPath, `${JSON.stringify(tauriJson, null, 2)}\n`);

  const cargoTomlPath = resolve(root, VERSION_FILES[2]);
  writeFileSync(cargoTomlPath, replaceCargoVersion(readFileSync(cargoTomlPath, 'utf8'), '[package]', version));

  const cargoLockPath = resolve(root, VERSION_FILES[3]);
  writeFileSync(cargoLockPath, replaceCargoVersion(readFileSync(cargoLockPath, 'utf8'), '[[package]]', version));
}

export function findNextPatchVersion(baseVersion, tags) {
  const { major, minor, patch } = parseStableVersion(baseVersion);
  const existingTags = new Set(tags);
  let nextPatch = patch + 1n;
  let candidate = `${major}.${minor}.${nextPatch}`;
  while (existingTags.has(`v${candidate}`)) {
    nextPatch += 1n;
    candidate = `${major}.${minor}.${nextPatch}`;
  }
  return candidate;
}

export function isVersionOnlyChange(paths) {
  return paths.length > 0 && paths.every((path) => VERSION_FILES.includes(path));
}

function gitTags(root) {
  return execFileSync('git', ['tag', '--list'], { cwd: root, encoding: 'utf8' })
    .split(/\r?\n/)
    .filter(Boolean);
}

function runCli(args) {
  const [command, ...values] = args;
  if (command === 'read' && values.length === 0) {
    process.stdout.write(`${readProjectVersion()}\n`);
    return;
  }
  if (command === 'set' && values.length === 1) {
    writeProjectVersion(values[0]);
    process.stdout.write(`${values[0]}\n`);
    return;
  }
  if (command === 'next-patch' && values.length === 1) {
    process.stdout.write(`${findNextPatchVersion(values[0], gitTags(repositoryRoot))}\n`);
    return;
  }
  if (command === 'compare' && values.length === 2) {
    process.stdout.write(`${compareStableVersions(values[0], values[1])}\n`);
    return;
  }
  if (command === 'is-version-only') {
    process.stdout.write(`${isVersionOnlyChange(values) ? 'true' : 'false'}\n`);
    return;
  }
  throw new Error('Usage: project-version.mjs read | set <version> | next-patch <version> | compare <version> <version> | is-version-only <paths...>');
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    runCli(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  }
}
