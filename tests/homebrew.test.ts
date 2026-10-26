import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { generateHomebrewCask } from '../scripts/homebrew-cask.mjs';

test('Cask generation binds the current version to actual DMG bytes and preserves user data', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'starshot-cask-'));
  try {
    // Synthetic UDIF-shaped bytes test the generator only, not DMG mountability.
    const bytes = Buffer.alloc(1024); bytes.write('original-starshot-test-fixture'); bytes.write('koly', 512);
    const dmgPath = join(directory, 'Starshot_0.3.0_aarch64.dmg'), outputPath = join(directory, 'Casks/starshot.rb');
    await writeFile(dmgPath, bytes);
    const result = await generateHomebrewCask({ dmgPath, outputPath, version: '0.3.0' });
    assert.equal(result.sha256, createHash('sha256').update(bytes).digest('hex'));
    const cask = await readFile(outputPath, 'utf8');
    assert.ok(cask.includes(`sha256 "${result.sha256}"`));
    assert.ok(cask.includes('version "0.3.0"') && cask.includes('/releases/tag/v0.3.0'));
    assert.ok(cask.includes('auto_updates true') && cask.includes('depends_on arch: :arm64') && cask.includes('depends_on macos: :monterey'));
    assert.ok(cask.includes('app "Starshot.app"') && cask.includes('not Apple-notarized'));
    assert.doesNotMatch(cask, /:no_check|\bzap\b|\buninstall\b|xattr|quarantine|--force/);
    await assert.rejects(generateHomebrewCask({ dmgPath, outputPath, version: '0.3.0', tag: 'v0.3.1' }), /tag/);
    await assert.rejects(generateHomebrewCask({ dmgPath, outputPath, version: '0.3.1' }), /文件名/);
    await assert.rejects(generateHomebrewCask({ dmgPath, outputPath: join(directory, 'starclip.rb'), version: '0.3.0' }), /输出文件名/);
    assert.equal(await readFile(outputPath, 'utf8'), cask);
    bytes[512] = 0; await writeFile(dmgPath, bytes);
    await assert.rejects(generateHomebrewCask({ dmgPath, outputPath, version: '0.3.0' }), /UDIF/);
    assert.equal(await readFile(outputPath, 'utf8'), cask);
  } finally { await rm(directory, { recursive: true, force: true }); }
});
