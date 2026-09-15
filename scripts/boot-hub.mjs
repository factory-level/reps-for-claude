// Verify the staged artifacts before executing the bundled hub. Paths come
// from the build manifest, not from requests or model output.
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { dirname, join, resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
const root = dirname(fileURLToPath(import.meta.url));
const manifest = JSON.parse(readFileSync(join(root, 'hub-manifest.json'), 'utf8'));
function verify(base, entries) {
  for (const [name, expected] of Object.entries(entries)) {
    const path = resolve(base, name);
    if (!path.startsWith(resolve(base) + sep)) throw new Error('Invalid artifact path');
    if (createHash('sha256').update(readFileSync(path)).digest('hex') !== expected) throw new Error(`Artifact checksum mismatch: ${name}`);
  }
}
verify(join(root, 'hub-bundle'), manifest.artifactSha256s);
verify(root, manifest.consumerSha256s);
await import(pathToFileURL(join(root, 'hub-bundle', 'hubd.mjs')).href);
