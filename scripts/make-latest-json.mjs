#!/usr/bin/env node
/**
 * Build the updater manifest (`latest.json`) from the artifacts CI just built.
 *
 * The Tauri updater fetches this one file, compares `version` against the
 * running app, and if it is newer downloads `url` and verifies it against
 * `signature`. The signature is the *content* of the .sig file, not a path.
 *
 * Tauri validates the whole manifest before it even looks at the version, so a
 * single malformed platform entry breaks updates for every platform. This
 * script therefore fails loudly rather than emitting a partial file.
 *
 * Usage: node scripts/make-latest-json.mjs <installers-dir> <version> <out.json>
 */
import { readFileSync, writeFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const [, , dir, rawVersion, out] = process.argv;
if (!dir || !rawVersion || !out) {
  console.error('usage: make-latest-json.mjs <dir> <version> <out.json>');
  process.exit(1);
}
const version = rawVersion.replace(/^v/, '');
const REPO = process.env.GITHUB_REPOSITORY ?? 'vafc21/llamachat';
const base = `https://github.com/${REPO}/releases/download/v${version}`;

/** Every file under dir, recursively. */
function walk(d) {
  return readdirSync(d).flatMap((n) => {
    const p = join(d, n);
    return statSync(p).isDirectory() ? walk(p) : [p];
  });
}

const files = walk(dir);
const sigs = files.filter((f) => f.endsWith('.sig'));

/**
 * Match a .sig to its platform key.
 *
 * The updater consumes a specific artifact per platform, which is NOT always
 * the installer a human downloads:
 *   - macOS  -> the .app.tar.gz, never the .dmg
 *   - Linux  -> the .AppImage
 *   - Windows-> the NSIS .exe (preferred over .msi; both are signed)
 */
function platformFor(sigPath) {
  const f = sigPath.replace(/\.sig$/, '');
  const name = f.split('/').pop();
  if (name.endsWith('.tar.gz')) {
    // CI renames these to carry the target triple, because both macOS jobs
    // otherwise emit an identically-named LlamaChat.app.tar.gz that would
    // collide in the flat release and ship one arch's build to the other.
    if (name.includes('aarch64-apple-darwin')) return 'darwin-aarch64';
    if (name.includes('x86_64-apple-darwin')) return 'darwin-x86_64';
    throw new Error(
      `macOS updater archive without a target triple: ${name}\n` +
        'Both macOS jobs produce LlamaChat.app.tar.gz; the rename step in ' +
        'build.yml must run, or the two architectures overwrite each other.',
    );
  }
  if (name.endsWith('.AppImage')) return 'linux-x86_64';
  if (name.endsWith('-setup.exe')) return 'windows-x86_64';
  return null; // .msi, .deb, .rpm — signed, but not what the updater pulls.
}

const platforms = {};
for (const sig of sigs) {
  const key = platformFor(sig);
  if (!key) continue;
  const asset = sig.replace(/\.sig$/, '').split('/').pop();
  const signature = readFileSync(sig, 'utf8').trim();
  if (!signature) throw new Error(`empty signature file: ${sig}`);
  // First match wins; both .msi and .exe can map to Windows.
  if (!platforms[key]) {
    platforms[key] = { signature, url: `${base}/${encodeURIComponent(asset)}` };
  }
}

const found = Object.keys(platforms);
if (found.length === 0) {
  console.error('No updater signatures found. Artifacts present:');
  for (const f of files) console.error('  ' + f);
  console.error('\nIs TAURI_SIGNING_PRIVATE_KEY set, and createUpdaterArtifacts=true?');
  process.exit(1);
}

// A missing platform means those users silently never get the update, so say
// so in the log even though it is not fatal.
for (const want of ['darwin-aarch64', 'darwin-x86_64', 'linux-x86_64', 'windows-x86_64']) {
  if (!found.includes(want)) console.warn(`WARNING: no updater artifact for ${want}`);
}

writeFileSync(
  out,
  JSON.stringify(
    {
      version,
      notes: `See https://github.com/${REPO}/releases/tag/v${version}`,
      pub_date: new Date().toISOString(),
      platforms,
    },
    null,
    2,
  ),
);
console.log(`latest.json written for ${version}: ${found.join(', ')}`);
