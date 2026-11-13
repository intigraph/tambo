// arming: workflow_dispatch only; never invoke this bootstrap on a crew host.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const https = require('node:https');
const crypto = require('node:crypto');
const {spawnSync} = require('node:child_process');
const sha = data => crypto.createHash('sha256').update(data).digest('hex');
function verify(data, expected, size) {
  if (data.length !== size || sha(data) !== expected) throw Error('frozen input mismatch');
}
function releaseEntry(release, entry) {
  const section = release.toString().split('\nSHA256:\n')[1]?.split(/\n[^ \n]/)[0];
  if (!section?.split('\n').some(line => {
    const fields = line.trim().split(/\s+/);
    return fields[0] === entry.sha256 && Number(fields[1]) === entry.bytes && fields[2] === entry.name;
  })) throw Error('index missing from pinned Release SHA256 section');
}
function download(url, maximum, redirects = 0) {
  if (!['snapshot.debian.org', 'snapshot-cloudflare.debian.org'].includes(new URL(url).hostname)
      || !url.startsWith('https://') || redirects > 5) throw Error('snapshot destination refused');
  return new Promise((resolve, reject) => {
    const request = https.get(url, response => {
      if ([301, 302, 303, 307, 308].includes(response.statusCode)) {
        response.resume();
        download(new URL(response.headers.location, url).href, maximum, redirects + 1).then(resolve, reject);
      } else if (response.statusCode !== 200) {
        response.resume(); reject(Error('snapshot HTTP ' + response.statusCode));
      } else {
        const chunks = []; let size = 0;
        response.on('data', chunk => {
          size += chunk.length;
          if (size > maximum) response.destroy(Error('snapshot size exceeded'));
          else chunks.push(chunk);
        });
        response.on('error', reject);
        response.on('end', () => resolve(Buffer.concat(chunks)));
      }
    });
    request.setTimeout(60000, () => request.destroy(Error('snapshot timeout')));
    request.on('error', reject);
  });
}
async function main() {
  const os = fs.readFileSync('/etc/os-release', 'utf8');
  if (process.env.GITHUB_ACTIONS !== 'true' || process.arch !== 'x64'
      || !fs.existsSync('/.dockerenv') || !/^ID=debian$/m.test(os)
      || !/^VERSION_CODENAME=trixie$/m.test(os)) throw Error('disposable trixie job required');
  const workspace = process.env.GITHUB_WORKSPACE;
  process.chdir(workspace);
  const manifest = JSON.parse(fs.readFileSync('scripts/trixie-qualification.json'));
  const output = path.join(workspace, 'qualification-results'); fs.mkdirSync(output, {recursive: true});
  const log = fs.openSync(path.join(output, 'bootstrap.log'), 'w');
  const run = (command, args, timeout = 600000) => {
    const child = spawnSync(command, args, {stdio: ['ignore', log, log], timeout,
      env: {...process.env, DEBIAN_FRONTEND: 'noninteractive'}});
    if (child.error || child.status !== 0) throw Error(command + ' failed');
  };
  fs.writeFileSync(path.join(output, 'dependency-license-closure.json'), JSON.stringify(manifest.closure, null, 2));
  for (const [name, digest] of Object.entries(manifest.source_files)) {
    if (sha(fs.readFileSync(path.join('qualification-source', name))) !== digest) throw Error('source pin mismatch');
  }
  const sources = [];
  for (const snapshot of manifest.snapshots) {
    const base = `https://snapshot.debian.org/archive/${snapshot.archive}/${manifest.snapshot}`;
    const release = await download(`${base}/dists/${snapshot.suite}/InRelease`, snapshot.release_bytes);
    verify(release, snapshot.release_sha256, snapshot.release_bytes);
    for (const entry of snapshot.indexes) {
      releaseEntry(release, entry);
      const data = await download(`${base}/dists/${snapshot.suite}/${entry.name}`, entry.bytes);
      verify(data, entry.sha256, entry.bytes);
    }
    // Historical snapshot mode, not freshness or unauthenticated archive mode.
    sources.push(`deb [check-valid-until=no] ${base}/ ${snapshot.suite} main`);
    sources.push(`deb-src [check-valid-until=no] ${base}/ ${snapshot.suite} main`);
  }
  fs.writeFileSync(path.join(output, 'frozen-inputs-verified.json'), JSON.stringify(manifest.snapshots, null, 2));
  // The action runtime has its own TLS roots; bootstrap APT with the SAME roots
  // rather than disable HTTPS checks or install prerequisites from a live repo.
  const roots = path.join(process.env.RUNNER_TEMP, 'trixie-bootstrap-ca.pem');
  fs.writeFileSync(roots, require('node:tls').rootCertificates.join('\n'));
  for (const name of fs.readdirSync('/etc/apt/sources.list.d')) fs.unlinkSync('/etc/apt/sources.list.d/' + name);
  fs.writeFileSync('/etc/apt/sources.list', sources.join('\n') + '\n');
  fs.writeFileSync('/etc/apt/apt.conf.d/99qualification-ca', `Acquire::https::CaInfo "${roots}";\n`);
  run('apt-get', ['update']); // APT must also verify Debian's signatures.
  const cached = fs.readdirSync('/var/lib/apt/lists');
  for (const snapshot of manifest.snapshots) {
    const names = cached.filter(name => name.endsWith(`_dists_${snapshot.suite}_InRelease`));
    if (names.length !== 1 || sha(fs.readFileSync('/var/lib/apt/lists/' + names[0])) !== snapshot.release_sha256)
      throw Error('APT Release differs from verified snapshot');
  }
  run('apt-get', ['install', '-y', '--no-install-recommends', ...manifest.apt_build_dependencies]);
  run('python3', ['scripts/qualify-trixie-package.py', '--source', 'qualification-source', '--output', 'qualification-results'], 1200000);
  fs.closeSync(log);
}
module.exports = {verify, releaseEntry, download};
if (require.main === module) main().catch(error => {
  console.error(error.message);
  const output = path.join(process.env.GITHUB_WORKSPACE || '.', 'qualification-results');
  if (fs.existsSync(output)) {
    fs.appendFileSync(path.join(output, 'bootstrap.log'), error.message + '\n');
    const receipt = path.join(output, 'result.json');
    if (!fs.existsSync(receipt)) fs.writeFileSync(receipt, JSON.stringify({status: 'INCOMPLETE', reason: error.message}));
  }
  process.exitCode = 1;
});
