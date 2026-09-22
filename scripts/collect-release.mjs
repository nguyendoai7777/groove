// Copies the finished installers out of Cargo's target tree into ./release.
//
// Tauri writes bundles to src-tauri/target/<triple>/release/bundle/<format>/,
// which is four levels deep, differs per target triple, and sits inside a
// directory otherwise full of build intermediates. This lifts just the
// shippable files to one predictable place.
//
// Run automatically by the build scripts in package.json. Standalone:
//
//   node scripts/collect-release.mjs            # copy, keep what is there
//   node scripts/collect-release.mjs --clean    # empty ./release first

import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { copyFile, cp, mkdir, readdir, readFile, rm, stat } from 'node:fs/promises';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const targetDir = join(root, 'src-tauri', 'target');
const releaseDir = join(root, 'release');

// What a user actually installs from. The bundle directories also hold support
// files — bundle_dmg.sh, icon.icns — so this allow-list is what separates the
// artifacts from the machinery that built them.
const INSTALLER_EXTENSIONS = ['.dmg', '.exe', '.msi', '.deb', '.rpm', '.AppImage'];

/** Every `<triple>/release/bundle` directory Cargo has produced. */
async function findBundleDirs(dir, found = []) {
  let entries;
  try {
    entries = await readdir(dir, { withFileTypes: true });
  } catch {
    return found;
  }

  for (const entry of entries) {
    if (!entry.isDirectory()) continue;
    const path = join(dir, entry.name);

    if (entry.name === 'bundle') {
      // Only release bundles. A debug bundle is not something to hand out.
      if (basename(dir) === 'release') found.push(path);
      continue;
    }
    // Build intermediates are enormous and contain no bundles.
    if (entry.name === 'deps' || entry.name === 'build' || entry.name === 'incremental') continue;

    await findBundleDirs(path, found);
  }
  return found;
}

async function collectFiles(dir, found = []) {
  let entries;
  try {
    entries = await readdir(dir, { withFileTypes: true });
  } catch {
    return found;
  }

  for (const entry of entries) {
    const path = join(dir, entry.name);
    // A .app is a directory, and the .dmg beside it is the distributable form.
    if (entry.isDirectory()) {
      if (!entry.name.endsWith('.app')) await collectFiles(path, found);
      continue;
    }
    // `rw.*.dmg` are hdiutil's read-write scratch images. Their presence without
    // a matching final .dmg means the DMG step died partway.
    if (entry.name.startsWith('rw.')) {
      found.push({ path, scratch: true });
      continue;
    }
    if (INSTALLER_EXTENSIONS.some((ext) => entry.name.endsWith(ext))) found.push({ path, scratch: false });
  }
  return found;
}

const bundleDirs = await findBundleDirs(targetDir);
if (bundleDirs.length === 0) {
  console.error('No release bundles found under src-tauri/target — did the build finish?');
  process.exit(1);
}

const all = (await Promise.all(bundleDirs.map((d) => collectFiles(d)))).flat();
const installers = all.filter((f) => !f.scratch);
const scratch = all.filter((f) => f.scratch);

if (process.argv.includes('--clean') && existsSync(releaseDir)) {
  await rm(releaseDir, { recursive: true });
}
await mkdir(releaseDir, { recursive: true });

for (const { path } of installers) {
  const name = basename(path);
  await copyFile(path, join(releaseDir, name));
  const { size } = await stat(path);
  console.log(`release/${name}  ${(size / 1024 / 1024).toFixed(1)} MB`);
}

if (installers.length === 0) {
  console.warn('\nNo installers found, only build output.');
}

// Portable Windows build: the bare exe that Cargo leaves in <triple>/release/,
// plus the bundle resources Tauri copies beside it. Tauri resolves resources
// relative to the exe when it is not installed, so this folder runs as-is.
const tauriConf = JSON.parse(await readFile(join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'));
const { productName, version } = tauriConf;
const ARCH_NAMES = { x86_64: 'x64', aarch64: 'arm64', i686: 'x86' };

for (const bundleDir of bundleDirs) {
  const buildDir = dirname(bundleDir);
  const exe = join(buildDir, `${productName}.exe`);
  if (!existsSync(exe)) continue;

  const triple = basename(dirname(buildDir));
  const arch = ARCH_NAMES[triple.split('-')[0]] ?? 'x64';
  const name = `${productName}_${version}_${arch}-portable`;
  const outDir = join(releaseDir, name);

  await rm(outDir, { recursive: true, force: true });
  await mkdir(outDir, { recursive: true });
  await copyFile(exe, join(outDir, `${productName}.exe`));

  // `icons/taskbar/*` → copy `icons/taskbar`, the part before the first glob.
  for (const pattern of tauriConf.bundle?.resources ?? []) {
    const literal = pattern.split('/').filter((part) => !/[*?[{]/.test(part)).join('/');
    const src = join(buildDir, literal);
    if (literal && existsSync(src)) await cp(src, join(outDir, literal), { recursive: true });
  }

  // Windows 10+ ships bsdtar, which writes zip archives. Git Bash's GNU tar
  // shadows it on PATH and cannot, so call it by full path.
  const zip = join(releaseDir, `${name}.zip`);
  await rm(zip, { force: true });
  try {
    if (process.platform === 'win32') {
      const tar = join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'tar.exe');
      execFileSync(tar, ['-a', '-c', '-f', zip, '-C', releaseDir, name]);
    }
    else execFileSync('zip', ['-r', '-q', zip, name], { cwd: releaseDir });
    const { size } = await stat(zip);
    console.log(`release/${name}.zip  ${(size / 1024 / 1024).toFixed(1)} MB`);
  } catch (err) {
    console.warn(`Could not zip the portable build (${err.message}); folder is at release/${name}/`);
  }
}

if (scratch.length > 0) {
  console.warn(
    `\n${scratch.length} leftover hdiutil scratch image(s) in the bundle directory.\n` +
      'Those are written while a DMG is being assembled and removed when it succeeds,\n' +
      'so any that survive mean the DMG step failed. Delete them before rebuilding —\n' +
      'they are ~44 MB each and a stale one can make the next run fail too:\n' +
      scratch.map((f) => `  rm '${f.path}'`).join('\n'),
  );
}
