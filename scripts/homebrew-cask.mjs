import { createReadStream } from 'node:fs';
import { mkdir, open, readFile, stat, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkVersion, projectRoot, stableVersion } from './version.mjs';
import { releaseNames } from './release-manifest.mjs';

/** Generate only from the actual release DMG; never accept a placeholder checksum. */
export async function generateHomebrewCask({ dmgPath, outputPath, version, tag = `v${version}` }) {
  stableVersion(version);
  if (tag !== `v${version}`) throw Error('Homebrew tag 与应用版本不一致。');
  if (basename(dmgPath) !== releaseNames(version).dmg) throw Error('DMG 文件名必须与当前版本及 Apple Silicon 架构一致。');
  if (basename(outputPath) !== 'starshot.rb') throw Error('Cask 输出文件名必须是 starshot.rb。');
  const artifact = await stat(dmgPath);
  if (!artifact.isFile() || artifact.size < 512) throw Error('DMG 文件为空或不完整。');
  const handle = await open(dmgPath, 'r');
  try {
    const footer = Buffer.alloc(4);
    await handle.read(footer, 0, footer.length, artifact.size - 512);
    if (footer.toString('ascii') !== 'koly') throw Error('文件缺少 DMG 的 UDIF 尾部标识。');
  } finally { await handle.close(); }
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(dmgPath)) hash.update(chunk);
  const sha256 = hash.digest('hex');
  const content = `cask "starshot" do
  version "${version}"
  sha256 "${sha256}"

  url "https://github.com/cpython666/starshot/releases/download/v#{version}/Starshot_#{version}_aarch64.dmg"
  name "Starshot"
  name "星镜"
  desc "Local-first screenshot, annotation and image pinning tool"
  homepage "https://github.com/cpython666/starshot"

  auto_updates true
  depends_on arch: :arm64
  depends_on macos: :monterey

  app "Starshot.app"

  caveats <<~EOS
    This is Starshot ${version} for Apple Silicon Macs.
    It is not Apple-notarized; macOS may block the first launch.
    See the release notes before installing: https://github.com/cpython666/starshot/releases/tag/${tag}
  EOS
end
`;
  await mkdir(dirname(resolve(outputPath)), { recursive: true });
  await writeFile(outputPath, content);
  return { version, tag, sha256, bytes: artifact.size, outputPath: resolve(outputPath) };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [dmgPath, outputPath, ...extra] = process.argv.slice(2);
    if (!dmgPath || !outputPath || extra.length) throw Error('用法：node scripts/homebrew-cask.mjs <Starshot_X.Y.Z_aarch64.dmg> <输出目录/starshot.rb>');
    const version = await checkVersion();
    const config = JSON.parse(await readFile(join(projectRoot, 'src-tauri/tauri.conf.json'), 'utf8'));
    if (config.bundle?.macOS?.minimumSystemVersion !== '12.0') throw Error('macOS 最低版本已变化，请同步 Cask 的 depends_on 后再生成。');
    console.log(JSON.stringify(await generateHomebrewCask({ dmgPath, outputPath, version }), null, 2));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
