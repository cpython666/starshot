import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, rename, rm } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

// Build-time assets only. The Windows executable embeds these exact files;
// the installed app never downloads models or uploads images.
const models = [
  { name: 'text-detection.rten', bytes: 2510284, sha256: 'f15cfb56bd02c4bf478a20343986504a1f01e1665c2b3a0ad66340f054b1b5ca' },
  { name: 'text-recognition.rten', bytes: 9716568, sha256: 'e484866d4cce403175bd8d00b128feb08ab42e208de30e42cd9889d8f1735a6e' },
];
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
if (process.argv.length > 3) throw Error('Usage: node scripts/prepare-ocr-models.mjs [output-directory]');
const directory = resolve(process.argv[2] ?? join(root, 'src-tauri/models/ocrs'));
async function matches(path, model) {
  try {
    const data = await readFile(path);
    return data.byteLength === model.bytes && createHash('sha256').update(data).digest('hex') === model.sha256;
  } catch (error) { if (error.code === 'ENOENT') return false; throw error; }
}
await mkdir(directory, { recursive: true });
for (const model of models) {
  const destination = join(directory, model.name);
  if (await matches(destination, model)) { console.log(`Verified cached ${model.name}`); continue; }
  const temporary = `${destination}.download-${process.pid}`;
  try {
    await promisify(execFile)(process.platform === 'win32' ? 'curl.exe' : 'curl', [
      '--fail', '--location', '--retry', '2', '--connect-timeout', '20', '--max-time', '300',
      '--proto', '=https', '--proto-redir', '=https', '--max-filesize', String(model.bytes),
      '--output', temporary, `https://github.com/cpython666/starshot/releases/download/ocr-models-v1/${model.name}`,
    ], { timeout: 960000, maxBuffer: 1024 * 1024 });
    if (!await matches(temporary, model)) throw Error(`${model.name}: size or SHA-256 mismatch; refusing this model.`);
    await rename(temporary, destination);
    console.log(`Downloaded and verified ${model.name}`);
  } finally { await rm(temporary, { force: true }); }
}
