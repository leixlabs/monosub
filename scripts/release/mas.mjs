#!/usr/bin/env node
/**
 * Mac App Store 打包：构建 → 逐层签名 → 出 .pkg
 *
 * 为什么不用 `tauri build` 自带签名：
 *   1. 自带的 --deep 只保证"有签名"，不保证 Resources 里的 6 个命令行工具拿到
 *      `com.apple.security.inherit`，App Store 校验时会被打回；
 *   2. Tauri 不产出 .pkg，而 App Store 只接受 .pkg（不是 .dmg、不是 .zip）；
 *   3. 需要把 distribution provisioning profile 塞进 Contents/embedded.provisionprofile。
 *
 * 用法：
 *   日常沙盒自测：不需要 dev 包。正式包本机可直接跑，且照样进沙盒：
 *     open src-tauri/target/mas/MonoSub.app      # 容器在 ~/Library/Containers/leixlabs.monosub
 *
 *   可选：dev 签名（要求门户 Devices 里先注册这台 Mac，否则没有可用 profile）
 *     MAS_PROFILE=src-tauri/mas/dev.provisionprofile bun run release:mas --dev
 *
 *   正式上架（Mac App Distribution + Mac App Store Connect profile）
 *   注意：门户里选的是 "Mac App Distribution"，签发出来的 CN 仍是旧名
 *   "3rd Party Mac Developer Application"，这里必须用 security 打印的原字符串
 *     MAS_SIGN_IDENTITY="3rd Party Mac Developer Application: Lei Zhang (RF4CPKUXQ7)" \
 *     MAS_INSTALLER_IDENTITY="3rd Party Mac Developer Installer: Lei Zhang (RF4CPKUXQ7)" \
 *     MAS_PROFILE=src-tauri/mas/distribution.provisionprofile \
 *     bun run release:mas
 *
 * 可选参数：
 *   --dev         不加 --timestamp 以外的区分，仅用于打印提示与跳过 pkg
 *   --no-pkg      只签 .app，不出 pkg
 *   --skip-build  复用已有 .app（迭代签名问题时省一次 cargo build）
 *   --open        完成后在 Finder 中显示产物
 */

import { execFileSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const APP_NAME = 'MonoSub';
const APP_ID = 'leixlabs.monosub';

const args = new Set(process.argv.slice(2));
const isDev = args.has('--dev');
const wantPkg = !args.has('--no-pkg') && !isDev;

const log = (msg) => console.log(`\x1b[36m▸\x1b[0m ${msg}`);
const fail = (msg) => {
  console.error(`\x1b[31m✗\x1b[0m ${msg}`);
  process.exit(1);
};

// ---------------------------------------------------------------- 环境检查

const APP_ENTITLEMENTS = path.join(ROOT, 'src-tauri/entitlements/MonoSub.mas.entitlements');
const HELPER_ENTITLEMENTS = path.join(ROOT, 'src-tauri/entitlements/MonoSubHelper.mas.entitlements');
const PRIVACY_MANIFEST = path.join(ROOT, 'src-tauri/PrivacyInfo.xcprivacy');
const BUILD_DIR = path.join(ROOT, 'src-tauri/target/release/bundle/macos');
// dev 与正式包分开暂存：dev 自测不能覆盖已经签好的分发版
const STAGE_DIR = path.join(ROOT, 'src-tauri/target/mas', isDev ? 'dev' : '');
const STAGED_APP = path.join(STAGE_DIR, `${APP_NAME}.app`);
const PKG_PATH = path.join(STAGE_DIR, `${APP_NAME}.pkg`);

for (const required of [APP_ENTITLEMENTS, HELPER_ENTITLEMENTS, PRIVACY_MANIFEST]) {
  if (!fs.existsSync(required)) fail(`缺少文件：${path.relative(ROOT, required)}`);
}

function availableIdentities(policy = ['-p', 'codesigning']) {
  const out = execFileSync('security', ['find-identity', '-v', ...policy], { encoding: 'utf8' });
  return out
    .split('\n')
    .map((line) => line.match(/"(.+)"/)?.[1])
    .filter(Boolean);
}

const identities = availableIdentities();

// profile 路径按模式取默认值，MAS_PROFILE 可覆盖：
//   --dev → src-tauri/mas/dev.provisionprofile        （macOS App Development）
//   正式  → src-tauri/mas/distribution.provisionprofile（Mac App Store Connect）
const profile = path.resolve(
  ROOT,
  process.env.MAS_PROFILE ?? `src-tauri/mas/${isDev ? 'dev' : 'distribution'}.provisionprofile`,
);

if (!fs.existsSync(profile)) {
  fail(
    `缺少 provisioning profile：${path.relative(ROOT, profile)}\n` +
      (isDev
        ? '  developer.apple.com → Profiles → + → Development → macOS App Development\n' +
          '  选 App ID「MonoSub」+ Mac Development 证书 → Generate → 存为 src-tauri/mas/dev.provisionprofile\n' +
          '  注意：这条路径要求先在 Devices 里注册这台 Mac，否则会卡在 "No Devices are available"。\n' +
          '  日常自测其实不需要 dev 包——正式包本机可以直接跑，且照样进沙盒：\n' +
          '    open src-tauri/target/mas/MonoSub.app\n' +
          '  容器在 ~/Library/Containers/leixlabs.monosub。'
        : '  developer.apple.com → Profiles → + → Distribution → Mac App Store Connect\n' +
          '  选 App ID「MonoSub」+ Mac App Distribution 证书 → Generate → 存为 src-tauri/mas/distribution.provisionprofile'),
  );
}

// 从 profile 里读 Team ID：本机有三张 Apple Development 证书分属不同 team，
// 不按 team 过滤就会签成别人的身份。同时校验 App ID 与工程 identifier 一致。
const profileText = execFileSync('security', ['cms', '-D', '-i', profile], { encoding: 'utf8' });
const teamId = profileText.match(/<key>TeamIdentifier<\/key>\s*<array>\s*<string>([^<]+)<\/string>/)?.[1];
const profileAppId = profileText.match(
  /<key>com\.apple\.application-identifier<\/key>\s*<string>([^<]+)<\/string>/,
)?.[1];

if (!profileText.includes(APP_ID)) {
  fail(
    `profile 的 App ID 与 tauri.conf.json 的 identifier（${APP_ID}）不匹配。\n` +
      `  profile 里是：${profileAppId ?? '(读不出)'}`,
  );
}

const DEV_CERT = /^(Apple Development|Mac Development)/;
const DIST_CERT = /^(Apple Distribution|3rd Party Mac Developer Application)/;
const wantedCert = isDev ? DEV_CERT : DIST_CERT;

const identity =
  process.env.MAS_SIGN_IDENTITY ??
  identities.find((name) => wantedCert.test(name) && (!teamId || name.includes(`(${teamId})`)));

if (!identity) {
  fail(
    `找不到可用的签名身份（team ${teamId ?? '?'}，${isDev ? '开发' : '分发'}证书）。\n` +
      `  本机现有：\n${identities.map((n) => `    ${n}`).join('\n') || '    （无）'}\n` +
      (isDev
        ? '  需要 "Apple Development" 证书，且要与 profile 同属一个 team。'
        : '  需要 "Mac App Distribution" 签发的 "3rd Party Mac Developer Application" 证书。'),
  );
}

log(`签名身份：${identity}`);
log(`Profile：${path.relative(ROOT, profile)}`);

// ---------------------------------------------------------------- 构建

if (!args.has('--skip-build')) {
  log('构建 .app（tauri build --bundles app）…');
  execFileSync(
    path.join(ROOT, 'node_modules/.bin/tauri'),
    ['build', '--config', 'src-tauri/tauri.mas.conf.json', '--bundles', 'app'],
    { cwd: ROOT, stdio: 'inherit' },
  );
}

const builtApp = path.join(BUILD_DIR, `${APP_NAME}.app`);
if (!fs.existsSync(builtApp)) fail(`没有构建产物：${path.relative(ROOT, builtApp)}`);

// ---------------------------------------------------------------- 暂存

log('复制到暂存目录（避免反复污染构建产物）…');
fs.rmSync(STAGED_APP, { recursive: true, force: true });
fs.mkdirSync(STAGE_DIR, { recursive: true });
execFileSync('ditto', [builtApp, STAGED_APP], { stdio: 'inherit' });

// ---------------------------------------------------------------- 剥掉旧签名

// Tauri 建包时用的是 signingIdentity "-"（ad-hoc），嵌套代码的签名必须全部重做，
// 否则 keychain 里会出现"半签"状态，codesign --verify 才会暴露问题。
log('清除 Tauri 留下的 ad-hoc 签名…');
try {
  execFileSync('codesign', ['--remove-signature', STAGED_APP], { stdio: 'ignore' });
} catch {
  /* 没有旧签名时 codesign 会报错，忽略 */
}

// ---------------------------------------------------------------- 签名工具函数

function sign(target, entitlements = null) {
  const argv = ['--force', '--sign', identity, '--timestamp'];
  if (entitlements) argv.push('--entitlements', entitlements);
  argv.push(target);
  execFileSync('codesign', argv, { stdio: 'pipe' });
}

function isMachO(file) {
  try {
    const fd = fs.openSync(file, 'r');
    const head = Buffer.alloc(4);
    fs.readSync(fd, head, 0, 4, 0);
    fs.closeSync(fd);
    const magic = head.readUInt32BE(0);
    // 0xfeedface / 0xfeedfacf：thin（大端写法）；0xcffaedfe / 0xcefaedfe：little-endian thin；
    // 0xcafebabe / 0xbebafeca：fat / universal
    return [0xfeedface, 0xfeedfacf, 0xcffaedfe, 0xcefaedfe, 0xcafebabe, 0xbebafeca].includes(magic);
  } catch {
    return false;
  }
}

function walk(dir, visit) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, visit);
    else if (entry.isFile()) visit(full, entry.name);
  }
}

// ---------------------------------------------------------------- 由内向外签名

// 主可执行文件的名字取自 Info.plist 的 CFBundleExecutable——它是 Cargo 的包名
// （`monosub`），不等于 productName（`MonoSub`）。按 productName 猜路径会让主二进制
// 被误判成 helper，先签一遍 inherit 再被 app 封签覆盖。
const mainExecName = execFileSync(
  '/usr/libexec/PlistBuddy',
  ['-c', 'Print :CFBundleExecutable', path.join(STAGED_APP, 'Contents/Info.plist')],
  { encoding: 'utf8' },
).trim();

// 1) 动态库：只签名，不带 entitlement
const dylibs = [];
const helpers = [];

walk(path.join(STAGED_APP, 'Contents'), (full, name) => {
  if (!isMachO(full)) return;
  if (name.endsWith('.dylib') || name.endsWith('.so')) dylibs.push(full);
  else helpers.push(full);
});

// 主可执行文件由最后签 .app 时一并处理，这里排除，避免重复签
const mainExec = path.join(STAGED_APP, 'Contents/MacOS', mainExecName);
const nestedHelpers = helpers.filter((f) => f !== mainExec);

log(`签名 ${dylibs.length} 个动态库…`);
for (const lib of dylibs) sign(lib);

log(`签名 ${nestedHelpers.length} 个命令行工具（带 inherit）…`);
for (const tool of nestedHelpers) sign(tool, HELPER_ENTITLEMENTS);

// Frameworks（Tauri 目前没用到，留着以防以后加 WebKit 扩展/动态框架）
const frameworksDir = path.join(STAGED_APP, 'Contents/Frameworks');
if (fs.existsSync(frameworksDir)) {
  walk(frameworksDir, (full, name) => {
    if (isMachO(full) && !dylibs.includes(full) && !nestedHelpers.includes(full)) sign(full);
  });
}

// 2) 塞入 provisioning profile 与隐私清单（必须在封签之前）
log('写入 embedded.provisionprofile 与 PrivacyInfo.xcprivacy…');
fs.copyFileSync(profile, path.join(STAGED_APP, 'Contents/embedded.provisionprofile'));

// CFBundleVersion 每次上传都必须比上一个更大。Tauri 把它写成与 version 相同的
// 2.0.2，第二次上传同一版本会被 App Store Connect 判成"构建已存在"。
// 用 MAS_BUILD_NUMBER 覆盖（默认 3），营销版本仍是 2.0.2。
const buildNumber = process.env.MAS_BUILD_NUMBER ?? '3';
execFileSync('/usr/libexec/PlistBuddy', [
  '-c',
  `Set :CFBundleVersion ${buildNumber}`,
  path.join(STAGED_APP, 'Contents/Info.plist'),
]);
log(`构建号：CFBundleVersion = ${buildNumber}`);
fs.copyFileSync(PRIVACY_MANIFEST, path.join(STAGED_APP, 'Contents/Resources/PrivacyInfo.xcprivacy'));

// 3) 最后签 app 本体（这一步会用 app entitlements 签主可执行文件并封签）
log('签名 app 本体…');
sign(STAGED_APP, APP_ENTITLEMENTS);

// ---------------------------------------------------------------- 校验

log('校验签名…');
execFileSync('codesign', ['--verify', '--deep', '--strict', '--verbose=2', STAGED_APP], { stdio: 'inherit' });

/** codesign -d 把信息写到 stderr（有的版本写 stdout），两个流都收。 */
function entitlementsOf(target) {
  const result = spawnSync('codesign', ['-d', '--entitlements', ':-', target], { encoding: 'utf8' });
  return `${result.stdout ?? ''}${result.stderr ?? ''}`;
}

if (!entitlementsOf(STAGED_APP).includes('com.apple.security.app-sandbox')) {
  fail('app 的 entitlements 里没有 app-sandbox —— App Store 会以 ITMS-90296 拒绝。');
}

// ITMS-90886：profile 里有 application-identifier，签名里却没有（手工 codesign
// 不会像 Xcode 那样自动注入），上传会被退回。这里直接拿 profile 的值逐字核对，
// 以后改 bundle id / team 时不可能漏。
if (!profileAppId) {
  fail('读不出 profile 的 com.apple.application-identifier。');
}
if (!entitlementsOf(STAGED_APP).includes(profileAppId)) {
  fail(
    `app 的 entitlements 里缺少 application-identifier（${profileAppId}）。\n` +
      '  App Store 会以 ITMS-90886 拒绝：请检查 src-tauri/entitlements/MonoSub.mas.entitlements。',
  );
}

for (const tool of nestedHelpers) {
  if (!entitlementsOf(tool).includes('com.apple.security.inherit')) {
    fail(`${path.basename(tool)} 没有 inherit entitlement，App Store 会拒绝。`);
  }
}

// ---------------------------------------------------------------- 出包

if (wantPkg) {
  const installerIdentity =
    process.env.MAS_INSTALLER_IDENTITY ??
    // Installer 证书不参与 codesigning 策略，`-p codesigning` 的列表里没有它，
    // 必须查全量身份，否则自动识别永远落空。
    availableIdentities(['-p', 'basic']).find((n) =>
      /^(Mac Installer Distribution|3rd Party Mac Developer Installer)/.test(n),
    );
  if (!installerIdentity) {
    fail('缺少签名 pkg 的身份：需要 "Mac Installer Distribution" 证书（签发出的 CN 是 "3rd Party Mac Developer Installer"）。');
  }
  log('生成 .pkg…');
  fs.rmSync(PKG_PATH, { force: true });
  execFileSync(
    'productbuild',
    ['--component', STAGED_APP, '/Applications', '--sign', installerIdentity, '--timestamp', PKG_PATH],
    { stdio: 'inherit' },
  );
  log(`\x1b[32m✓\x1b[0m ${path.relative(ROOT, PKG_PATH)}`);
  console.log(`
下一步：
  1. 打开 Transporter（App Store 里免费下载）拖入上面这个 .pkg，或用：
     xcrun altool --upload-app -f "${PKG_PATH}" -t macos \\
       -u <你的 Apple ID> -p <App 专用密码>
  2. 等 App Store Connect 处理完（10–60 分钟），在"构建版本"里选中它，提交审核。
`);
} else {
  log(`\x1b[32m✓\x1b[0m ${path.relative(ROOT, STAGED_APP)}（未出 pkg）`);
  console.log(`
本地验证沙盒行为（务必做，很多问题只有真正跑起来才暴露）：
  open "${STAGED_APP}"
  log stream --predicate 'process == "sandboxd"' --info | grep ${APP_ID}
`);
}

if (args.has('--open')) {
  execFileSync('open', ['-R', wantPkg ? PKG_PATH : STAGED_APP]);
}
