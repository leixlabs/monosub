#!/usr/bin/env node
/**
 * 本机可运行的安装包：构建 → 逐层签名 → 出 .pkg，两种模式。
 *
 *   bun run release:pkg            非沙盒版（日常自用 / 内测分发）
 *   bun run release:pkg:sandbox    沙盒版（行为等同 MAS 包，用来本地验证沙盒）
 *
 * 为什么 release:mas 的 pkg 装了打不开 —— 两件事叠加，缺一不可：
 *   1. 证书：`3rd Party Mac Developer Application`（Mac App Distribution）只在
 *      "由 App Store 安装"时才被放行，双击本地装好的 App 会被 launchd 拒掉；
 *   2. 内置的 distribution.provisionprofile：系统据此按 App Store 安装态校验收据，
 *      本地没有收据，于是报 "The application can't be opened."
 *
 * 所以两个模式都做了同一件事：用开发证书签、不塞分发 profile。区别只在沙盒：
 *
 *   | 项目     | release:pkg              | release:pkg:sandbox              | release:mas        |
 *   |----------|--------------------------|----------------------------------|--------------------|
 *   | 沙盒      | 关                        | 开（与 MAS 逐条一致）             | 开                  |
 *   | 证书      | Apple Development / DevID | Apple Development / DevID        | 3rd Party Mac Dev  |
 *   | profile   | 不塞                      | 不塞                             | 塞 distribution     |
 *   | 本地启动   | ✅                        | ✅（行为＝上架包）                 | ❌                  |
 *   | 产物用途  | 自用 / 内测                | 沙盒自测、审核前预演               | 上传 App Store      |
 *
 * 沙盒版为什么不声明 application-identifier：那是"受 profile 授权"的 restricted
 * entitlement，没有 profile 还声明它，进程会在 exec 阶段因无效授权被杀。
 * 见 src-tauri/entitlements/MonoSub.sandbox.local.entitlements 的注释。
 *
 * 用法：
 *   bun run release:pkg -- --skip-build   # 复用已有 .app，只重签 + 出包
 *   bun run release:pkg -- --install      # 出包后弹管理员授权装到 /Applications
 *
 * 证书优先级（LOCAL_SIGN_IDENTITY 可覆盖）：
 *   Developer ID Application → 可分发给任何人（配合 --notarize）
 *   Apple Development        → 仅本机与同 Team 机器可开（当前账号的默认）
 *   Mac Developer            → 同上，老命名
 *   都没有                    → ad-hoc，仅本机可跑，打警告
 *
 * 可选参数：
 *   --sandbox           构建沙盒版（等同 release:pkg:sandbox）
 *   --skip-build        复用 src-tauri/target/release/bundle/macos/MonoSub.app
 *   --no-pkg            只签 .app，不出 pkg
 *   --no-pkg-sign       出未签名的 pkg（Installer 拒签时回退用）
 *   --ad-hoc            强制 ad-hoc 签名
 *   --identity <name>   同 LOCAL_SIGN_IDENTITY
 *   --notarize          公证 + staple（需要 Developer ID）
 *   --install           装到 /Applications 并启动
 *   --open              完成后在 Finder 中显示产物
 */

import { execFileSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

import { log, die } from '../lib/log.mjs';
import { hostPlatform } from '../lib/platform.mjs';
import { projectRoot, srcTauriDir, tauriCli } from '../lib/paths.mjs';
import { run } from '../lib/shell.mjs';
import { readTauriVersion } from '../lib/version.mjs';

// ---------------------------------------------------------------- 参数

const APP_NAME = 'MonoSub';
const APP_ID = 'leixlabs.monosub';

const args = process.argv.slice(2);
const has = (flag) => args.includes(flag);
const flagValue = (name) => {
    const i = args.indexOf(name);
    return i >= 0 ? args[i + 1] : undefined;
};

if (has('--help') || has('-h')) {
    log.plain(`用法: node scripts/release/local-pkg.mjs [选项]

  (无参数)        非沙盒版：构建 → 签 .app → build/pkg/${APP_NAME}-<version>-local.pkg
  --sandbox       沙盒版：行为等同 MAS 包，本地可启动
                  → build/pkg/${APP_NAME}-<version>-local-sandbox.pkg
  --skip-build    复用已有 .app
  --no-pkg        只签 .app，不出 pkg
  --no-pkg-sign   pkg 不签名（本地安装无影响）
  --ad-hoc        强制 ad-hoc 签名
  --identity X    指定签名身份（同 LOCAL_SIGN_IDENTITY）
  --notarize      公证 + staple（需要 Developer ID）
  --install       装到 /Applications 并启动
  --open          在 Finder 中显示产物`);
    process.exit(0);
}

if (hostPlatform() !== 'macos') die('release:pkg 只产出 macOS 安装包');

// 不能用 sudo：codesign 要用的私钥在自己的登录钥匙串里，root 的钥匙串没有它，
// 表现是第一个 dylib 就 "codesign exited with code 1"（底层 errSecInternalComponent）。
// 需要管理员权限的只有 installer 那一步，--install 会自己弹系统授权面板。
if (process.getuid?.() === 0) {
    die(
        '不要用 sudo 跑这个脚本。\n' +
            '  签名用的私钥在你的登录钥匙串里，root 读不到；安装步骤本身会弹管理员授权。\n' +
            '  正确用法：bun run release:pkg:sandbox -- --install',
    );
}

const sandbox = has('--sandbox');
const skipBuild = has('--skip-build');
const wantPkg = !has('--no-pkg');
const wantPkgSign = !has('--no-pkg-sign');
const wantInstall = has('--install');
const wantNotarize = has('--notarize');

const ENTITLEMENTS_DIR = path.join(srcTauriDir, 'entitlements');
// 沙盒模式用"去掉 restricted entitlement 的 MAS 版本"，其余逐条一致
const MODE = sandbox
    ? {
          label: '沙盒',
          config: 'src-tauri/tauri.sandbox-local.conf.json',
          appEntitlements: path.join(ENTITLEMENTS_DIR, 'MonoSub.sandbox.local.entitlements'),
          helperEntitlements: path.join(ENTITLEMENTS_DIR, 'MonoSubHelper.mas.entitlements'),
          stageDir: path.join(srcTauriDir, 'target/local-sandbox'),
          suffix: 'local-sandbox',
          expectSandbox: true,
      }
    : {
          label: '非沙盒',
          config: null,
          appEntitlements: path.join(ENTITLEMENTS_DIR, 'MonoSub.local.entitlements'),
          helperEntitlements: null,
          stageDir: path.join(srcTauriDir, 'target/local'),
          suffix: 'local',
          expectSandbox: false,
      };

const BUILT_APP = path.join(srcTauriDir, 'target/release/bundle/macos', `${APP_NAME}.app`);
const STAGED_APP = path.join(MODE.stageDir, `${APP_NAME}.app`);
const STAGED_PKG = path.join(MODE.stageDir, `${APP_NAME}.pkg`);

for (const required of [MODE.appEntitlements, MODE.helperEntitlements].filter(Boolean)) {
    if (!fs.existsSync(required)) die(`缺少文件：${path.relative(projectRoot, required)}`);
}
if (MODE.config && !fs.existsSync(path.join(projectRoot, MODE.config))) {
    die(`缺少配置：${MODE.config}`);
}

/**
 * 删构建产物一律走 /bin/rm，不用 fs.rmSync。
 *
 * 某些运行环境（例如把 IDE 的 node 包装成"安全删除"的那种）会把 fs.rm 系列
 * 改写成"移到废纸篓"：对 target/ 下的产物一旦遇到占用或同名条目就报
 * `FSMoveObjectToTrashSync failed (status -5000)`，本脚本会在复制暂存目录那一步
 * 直接崩掉，而目标其实只是一个可以重建的构建产物。
 */
function hardRemove(target) {
    const result = spawnSync('/bin/rm', ['-rf', target], { encoding: 'utf8' });
    if (result.status !== 0) {
        throw new Error(`rm -rf ${target} 失败：${(result.stderr ?? '').trim() || `退出码 ${result.status}`}`);
    }
}

log.info(`模式：${MODE.label}${sandbox ? '（与 MAS 包同一套 entitlements，本地可启动）' : ''}`);

// ---------------------------------------------------------------- 证书

/** `security find-identity` 的 CN 列表（括号里是证书 ID，不是 Team ID）。 */
function listIdentities(policy = ['-p', 'codesigning']) {
    const out = execFileSync('security', ['find-identity', '-v', ...policy], { encoding: 'utf8' });
    return out
        .split('\n')
        .map((line) => line.match(/"(.+)"/)?.[1])
        .filter(Boolean);
}

/**
 * 能在本机/内测机器上直接执行的证书。
 * 刻意排除 "3rd Party Mac Developer*"：App Store 分发证书签出来的东西本地跑不了。
 */
const CANDIDATES = [
    { match: /^Developer ID Application:/ },
    { match: /^Apple Development:/ },
    { match: /^Mac Developer:/ },
];

const identities = listIdentities();

let identity;
if (has('--ad-hoc')) {
    identity = '-';
} else {
    const requested = flagValue('--identity') ?? process.env.LOCAL_SIGN_IDENTITY;
    if (requested) {
        if (/3rd Party Mac Developer|Mac App Distribution/.test(requested)) {
            die(
                `"${requested}" 是 App Store 分发证书，签出来的 App 本地无法启动。\n` +
                    '  请改用 "Apple Development" 或 "Developer ID Application"。',
            );
        }
        identity = requested;
    } else {
        for (const candidate of CANDIDATES) {
            identity = identities.find((name) => candidate.match.test(name));
            if (identity) break;
        }
    }
}

if (!identity) {
    identity = '-';
    log.warn('没有可用的开发/分发证书，退回 ad-hoc 签名（仅本机可跑）。');
} else if (identity === '-') {
    log.warn('使用 ad-hoc 签名（仅本机可跑）。');
}

const isDeveloperId = /^Developer ID Application:/.test(identity);
// Developer ID 必须带安全时间戳才能公证；开发证书带上就要联网，不必要
const withTimestamp = isDeveloperId;
const hardened = isDeveloperId;

log.info(`签名身份：${identity}`);
log.info(`Hardened Runtime：${hardened ? '开启' : '关闭'}`);

if (isDeveloperId) {
    log.ok('检测到 Developer ID：产物可分发（未公证的包在别人机器上仍需右键打开）。');
} else {
    log.warn(
        '当前是开发证书：只在能信任这张证书的机器上双击可开。\n' +
            '  要给任何人用，需要另签 "Developer ID Application" + "Developer ID Installer"，\n' +
            '  然后 bun run release:pkg --notarize。',
    );
}

// ---------------------------------------------------------------- 构建

if (!skipBuild) {
    log.step(`构建 .app（${MODE.config ?? '默认配置'}）…`);
    const buildArgs = ['build', '--bundles', 'app'];
    if (MODE.config) buildArgs.push('--config', MODE.config);
    run(tauriCli(), buildArgs, { cwd: projectRoot });
}

if (!fs.existsSync(BUILT_APP)) {
    die(`没有构建产物：${path.relative(projectRoot, BUILT_APP)}`);
}

// ---------------------------------------------------------------- 暂存

log.step('复制到暂存目录…');
// 之前用 sudo 跑过一次的话，暂存目录会变成 root 属主，删不掉也写不进。
// 直接给出修法，而不是抛一句 EACCES。
try {
    hardRemove(STAGED_APP);
    fs.mkdirSync(MODE.stageDir, { recursive: true });
} catch (error) {
    die(
        `暂存目录清不掉：${path.relative(projectRoot, MODE.stageDir)}\n` +
            `  ${error.message}\n` +
            '  多半是之前用 sudo 跑过，或 Installer 把 payload 写进了这里（root 属主）。修法：\n' +
            `  sudo chown -R ${process.getuid()}:${process.getgid()} ${path.relative(projectRoot, MODE.stageDir)}`,
    );
}
run('ditto', [BUILT_APP, STAGED_APP]);

// ---------------------------------------------------------------- 清掉 App Store 分发痕迹

// distribution/dev profile 会触发"App Store 安装态"校验收据，本地版一律不留
const profilePath = path.join(STAGED_APP, 'Contents/embedded.provisionprofile');
if (fs.existsSync(profilePath)) {
    hardRemove(profilePath);
    log.warn('已移除 Contents/embedded.provisionprofile（分发凭证，本地版必须没有）。');
}

log.step('清除 Tauri 留下的 ad-hoc 签名…');
try {
    run('codesign', ['--remove-signature', STAGED_APP], { stdio: 'pipe' });
} catch {
    /* 没有旧签名时 codesign 报错，忽略 */
}

// ---------------------------------------------------------------- 构建号

// 营销版本保持 2.0.2 不变，但 CFBundleVersion 用编译时刻（yyyyMMddHHmm），保证
// 每次都比已安装的那个大。否则 Installer 认为"同版本已安装"而**跳过整个 payload**：
// 屏幕上显示"安装完成"，/Applications 里躺着的还是旧包（踩过一次：旧的 CFBundleVersion=3，
// 新的还是 2.0.2，于是 MAS 那份被原样留着，启动照样被拒）。
const buildNumber = process.env.LOCAL_BUILD_NUMBER ?? new Date()
    .toISOString()
    .replace(/[-:T]/g, '')
    .slice(0, 14);
execFileSync('/usr/libexec/PlistBuddy', [
    '-c',
    `Set :CFBundleVersion ${buildNumber}`,
    path.join(STAGED_APP, 'Contents/Info.plist'),
]);
log.info(`构建号：CFBundleVersion = ${buildNumber}（保证覆盖同版本的已安装 App）`);

// ---------------------------------------------------------------- 签名

function sign(target, { entitlements = null } = {}) {
    const argv = ['--force', '--sign', identity];
    if (hardened) argv.push('--options', 'runtime');
    if (withTimestamp) argv.push('--timestamp');
    if (entitlements) argv.push('--entitlements', entitlements);
    argv.push(target);

    // 自己收 stderr：`run()` 抛的只有退出码，codesign 的真正原因（钥匙串锁了、
    // 私钥 ACL 没授权等）全在 stderr 里，被吞掉就变成无从下手的 "code 1"。
    const result = spawnSync('codesign', argv, { encoding: 'utf8' });
    if (result.status !== 0) {
        const detail = `${result.stdout ?? ''}${result.stderr ?? ''}`.trim();
        const hint = /errSecInternalComponent|The specified item could not be found in the keychain/i.test(detail)
            ? '\n  私钥读不到：先用钥匙串访问解锁 login，或 `security unlock-keychain`；\n' +
              '  另外别用 sudo 跑（root 的钥匙串里没有这张证书）。'
            : '';
        die(`签名失败：${path.relative(projectRoot, target)}\n  ${detail || '(codesign 无输出)'}${hint}`);
    }
}

function isMachO(file) {
    try {
        const fd = fs.openSync(file, 'r');
        const head = Buffer.alloc(4);
        fs.readSync(fd, head, 0, 4, 0);
        fs.closeSync(fd);
        const magic = head.readUInt32BE(0);
        // thin（大小端）/ fat
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

// 主可执行文件名取自 Info.plist：它是 Cargo 包名（monosub），不是 productName
const mainExecName = execFileSync(
    '/usr/libexec/PlistBuddy',
    ['-c', 'Print :CFBundleExecutable', path.join(STAGED_APP, 'Contents/Info.plist')],
    { encoding: 'utf8' },
).trim();
const mainExec = path.join(STAGED_APP, 'Contents/MacOS', mainExecName);

const dylibs = [];
const tools = [];
walk(path.join(STAGED_APP, 'Contents'), (full, name) => {
    if (full === mainExec || !isMachO(full)) return;
    if (name.endsWith('.dylib') || name.endsWith('.so')) dylibs.push(full);
    else tools.push(full);
});

// 由内向外：动态库 → 随包工具 → app 本体
log.step(`签名 ${dylibs.length} 个动态库…`);
for (const lib of dylibs) sign(lib);

// 沙盒模式下工具必须带 inherit，否则子进程无法继承父沙盒；沙盒外则不带
log.step(`签名 ${tools.length} 个随包可执行${MODE.helperEntitlements ? '（带 inherit）' : ''}…`);
for (const tool of tools) sign(tool, { entitlements: MODE.helperEntitlements });

const frameworksDir = path.join(STAGED_APP, 'Contents/Frameworks');
if (fs.existsSync(frameworksDir)) {
    log.step('签名 Frameworks…');
    walk(frameworksDir, (full) => {
        if (isMachO(full) && !dylibs.includes(full) && !tools.includes(full)) sign(full);
    });
}

log.step('签名 app 本体…');
sign(STAGED_APP, { entitlements: MODE.appEntitlements });

// ---------------------------------------------------------------- 校验

log.step('校验签名…');
run('codesign', ['--verify', '--deep', '--strict', '--verbose=2', STAGED_APP]);

/** codesign -d 的信息写到 stderr（部分版本写 stdout），两个流都收。 */
function entitlementsOf(target) {
    const result = spawnSync('codesign', ['-d', '--entitlements', ':-', target], { encoding: 'utf8' });
    return `${result.stdout ?? ''}${result.stderr ?? ''}`;
}

const appEntitlements = entitlementsOf(STAGED_APP);
const hasSandbox = appEntitlements.includes('com.apple.security.app-sandbox');

if (MODE.expectSandbox && !hasSandbox) {
    die('沙盒版签出来却没有 app-sandbox —— 检查 tauri.sandbox-local.conf.json 是否被覆盖。');
}
if (!MODE.expectSandbox && hasSandbox) {
    die('非沙盒版签出来带上了 app-sandbox —— 检查 entitlements 是否被改成了 MAS 那份。');
}
if (fs.existsSync(profilePath)) {
    die('签名后仍存在 embedded.provisionprofile —— 这样的包本地启动会被拒。');
}

// 只做提示：未公证的包 spctl 一定 reject，本机无 quarantine 不影响启动
const assessment = spawnSync('spctl', ['--assess', '--type', 'exec', '--verbose=2', STAGED_APP], {
    encoding: 'utf8',
});
const assessmentText = `${assessment.stdout ?? ''}${assessment.stderr ?? ''}`.trim();
if (assessment.status === 0) {
    log.ok('Gatekeeper 评估通过。');
} else {
    log.dim(`Gatekeeper：${assessmentText.split('\n')[0] || 'rejected'}（未公证，属预期）`);
}

// ---------------------------------------------------------------- 出包

function resolveInstallerIdentity() {
    const requested = process.env.LOCAL_INSTALLER_IDENTITY;
    if (requested) return requested;
    // Installer 证书不参与 codesigning 策略，-p codesigning 里查不到，必须查全量
    const all = listIdentities(['-p', 'basic']);
    const order = [/^Developer ID Installer:/, /^Mac Installer Distribution:/, /^3rd Party Mac Developer Installer:/];
    for (const pattern of order) {
        const found = all.find((name) => pattern.test(name));
        if (found) return found;
    }
    return null;
}

/**
 * 去掉 PackageInfo 里的 <upgrade-bundle> 与 <strict-identifier>。
 *
 * 这两条是 pkgbuild 自动写进去的"认亲"线索：安装时 PackageKit 会按 bundle id
 * 在磁盘上找另一份 MonoSub.app（就是本仓库 target/ 下的构建产物），把安装目标
 * **relocated** 过去 —— 于是"装"到了构建目录自己身上，/Applications 依旧为空，
 * 而 installer 的退出码是 0，脚本看起来一切正常：
 *
 *   PackageKit: Applications/MonoSub.app relocated to
 *     Users/lei/.../src-tauri/target/local-sandbox/MonoSub.app
 *
 * 去掉之后，payload 只会落到 --install-location 指定的 /Applications。
 * 扩容 → 改 PackageInfo → 重新 flatten，是 pkg 层面唯一可靠的关法（没有命令行开关）。
 */
function stripRelocationHints(pkg) {
    const dir = path.join(MODE.stageDir, 'expand');
    hardRemove(dir);
    run('pkgutil', ['--expand', pkg, dir], { stdio: 'pipe' });

    let patched = 0;
    // 布局有两种：productbuild 出来的 product archive 是「子目录 + 组件包」，
    // pkgbuild 直接产出的组件包则是**平铺**的（Bom/PackageInfo/Payload 在根目录）。
    // 所以递归找 PackageInfo，不假设层级。
    const infos = [];
    (function collect(current) {
        for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
            const full = path.join(current, entry.name);
            if (entry.isDirectory()) collect(full);
            else if (entry.name === 'PackageInfo') infos.push(full);
        }
    })(dir);

    for (const info of infos) {
        const text = fs.readFileSync(info, 'utf8');
        const next = text
            // <relocate> 才是真正让 PackageKit 去磁盘上"找另一份同名 bundle"的开关，
            // 另外三条是它的配套线索（升级/原子替换/严格 bundle 匹配），一并去掉。
            .replace(/<relocate>[\s\S]*?<\/relocate>/g, '')
            .replace(/<upgrade-bundle>[\s\S]*?<\/upgrade-bundle>/g, '')
            .replace(/<update-bundle\s*\/>/g, '')
            .replace(/<atomic-update-bundle\s*\/>/g, '')
            .replace(/<strict-identifier>[\s\S]*?<\/strict-identifier>/g, '');
        if (next !== text) {
            fs.writeFileSync(info, next, 'utf8');
            patched += 1;
        }
    }
    if (patched === 0) log.warn('PackageInfo 里没有可去掉的 relocation 提示，结构可能变了。');

    hardRemove(pkg);
    run('pkgutil', ['--flatten', dir, pkg], { stdio: 'pipe' });
    hardRemove(dir);
}

const version = readTauriVersion();
// 产物放 build/ 而不是 dist/：dist/ 是 Vite 的输出目录，每次 `tauri build` 前
// 的 `bun run build` 都会清空它 —— 两个模式先后跑一次就会互相抹掉对方的 pkg。
const outputDir = path.join(projectRoot, 'build', 'pkg');
const pkgPath = path.join(outputDir, `${APP_NAME}-${version}-${MODE.suffix}.pkg`);

if (wantPkg) {
    log.step('生成 .pkg…');
    hardRemove(STAGED_PKG);

    // 不再用 `productbuild --component`：它把包版本取自 App 的
    // CFBundleShortVersionString（2.0.2），与上次安装留下的收据同版本，
    // Installer 会认为"已安装"而静默跳过 payload（退出码仍是 0）。
    // 改成 pkgbuild，显式给一个单调递增的版本号（编译时刻）。
    const componentPkg = path.join(MODE.stageDir, `${APP_NAME}-component.pkg`);
    hardRemove(componentPkg);
    run('pkgbuild', [
        '--component',
        STAGED_APP,
        '--identifier',
        APP_ID,
        '--version',
        buildNumber,
        '--install-location',
        '/Applications',
        componentPkg,
    ]);

    // pkgbuild 会在 PackageInfo 里写 <upgrade-bundle> 与 <strict-identifier>。
    // 这两条是给 PackageKit 的"认亲"线索：安装时它会按 bundle id 在磁盘上找到
    // 另一份 MonoSub.app（就是本仓库 target/ 下那份构建产物），把安装目标
    // **relocated** 过去，于是"装"到了构建目录自己身上，/Applications 依旧为空：
    //
    //   PackageKit: Applications/MonoSub.app relocated to
    //     Users/lei/.../src-tauri/target/local-sandbox/MonoSub.app
    //
    // 去掉这两条后，payload 只会落到 --install-location 指定的位置。
    stripRelocationHints(componentPkg);

    let pkgSigned = false;
    if (wantPkgSign) {
        const installerIdentity = resolveInstallerIdentity();
        if (installerIdentity) {
            log.info(`pkg 签名身份：${installerIdentity}`);
            // productsign 直接签扁平包；productbuild 只用来做 product archive，
            // 这里不需要那一层（改了 PackageInfo 后重签也不会被覆盖）
            run('productsign', ['--sign', installerIdentity, componentPkg, STAGED_PKG]);
            pkgSigned = true;
        } else {
            if (isDeveloperId) log.warn('缺少 Developer ID Installer 证书，pkg 未签名（公证会被拒）。');
            else log.info('没有 installer 类证书，pkg 以未签名形式产出（本地安装不受影响）。');
            fs.copyFileSync(componentPkg, STAGED_PKG);
        }
    } else {
        fs.copyFileSync(componentPkg, STAGED_PKG);
    }

    if (pkgSigned) {
        const check = spawnSync('pkgutil', ['--check-signature', STAGED_PKG], { encoding: 'utf8' });
        const text = `${check.stdout ?? ''}${check.stderr ?? ''}`;
        // pkgutil 对开发证书签的包只写 "issued by Apple (Development)"，不写 verified
        const ok = /successfully verified|issued by Apple \((Development|Mac Developer|Developer ID)\)/i.test(text);
        if (ok) log.ok('pkg 签名校验通过。');
        else log.warn(`pkg 签名状态异常：\n${text.trim()}`);
    }

    try {
        fs.mkdirSync(outputDir, { recursive: true });
        hardRemove(pkgPath);
        fs.copyFileSync(STAGED_PKG, pkgPath);
    } catch (error) {
        if (error.code === 'EACCES' || error.code === 'EPERM') {
            die(
                `产物目录写不进去：${path.relative(projectRoot, outputDir)}\n` +
                    `  sudo chown -R ${process.getuid()}:${process.getgid()} ${path.relative(projectRoot, outputDir)}`,
            );
        }
        throw error;
    }
    log.success(`安装包：${path.relative(projectRoot, pkgPath)}`);
}

// ---------------------------------------------------------------- 公证

if (wantNotarize) {
    if (!wantPkg) die('--notarize 需要 pkg，不能和 --no-pkg 同时用');
    if (!isDeveloperId) die('公证要求 Developer ID Application 签名');

    log.step('上传公证…');
    const profile = process.env.NOTARY_PROFILE;
    const notaryArgs = ['notarytool', 'submit', pkgPath, '--wait'];
    if (profile) {
        notaryArgs.push('--keychain-profile', profile);
    } else {
        const { APPLE_ID: appleId, APPLE_TEAM_ID: teamId, APPLE_APP_PASSWORD: password } = process.env;
        if (!appleId || !teamId || !password) {
            die(
                '缺少公证凭据：设置 NOTARY_PROFILE（推荐，先 xcrun notarytool store-credentials）\n' +
                    '  或 APPLE_ID / APPLE_TEAM_ID / APPLE_APP_PASSWORD。',
            );
        }
        notaryArgs.push('--apple-id', appleId, '--team-id', teamId, '--password', password);
    }
    run('xcrun', notaryArgs);
    log.step('staple 票据…');
    run('xcrun', ['stapler', 'staple', pkgPath]);
    log.ok('公证完成，产物可直接分发。');
}

// ---------------------------------------------------------------- 安装

const TARGET_APP = path.join('/Applications', `${APP_NAME}.app`);

/** 包成 shell 单引号字面量：路径里的空格、中文、括号都不用再操心。 */
function shQuote(value) {
    return `'${String(value).replace(/'/g, `'\\''`)}'`;
}

function bundleIdOf(appPath) {
    const out = spawnSync(
        '/usr/libexec/PlistBuddy',
        ['-c', 'Print :CFBundleIdentifier', path.join(appPath, 'Contents/Info.plist')],
        { encoding: 'utf8' },
    );
    return out.status === 0 ? out.stdout.trim() : null;
}

if (wantInstall) {
    if (!wantPkg) die('--install 需要 pkg，不能和 --no-pkg 同时用');
    log.step('安装到 /Applications（会弹出管理员授权）…');

    // 不再先 rm 掉旧 App：包版本已单调递增，Installer 会走正常升级路径覆盖。
    // 先删反而危险——万一 installer 因别的原因失败，用户会连旧版本都没了。
    const existingId = fs.existsSync(TARGET_APP) ? bundleIdOf(TARGET_APP) : null;
    if (existingId && existingId !== APP_ID) {
        die(
            `${TARGET_APP} 已存在，但 bundle id 是 ${existingId}，不是本项目。\n` +
                '  请先手动处理它，再重试。',
        );
    }

    // 整个命令拼进 AppleScript 的字符串里：内层不能出现裸双引号，
    // 否则会把字符串截断（"A real number can't go after this real number. (-2740)"）。
    const command = `installer -pkg ${shQuote(pkgPath)} -target /`;
    const script = `do shell script "${command}" with administrator privileges`;
    const result = spawnSync('osascript', ['-e', script], { encoding: 'utf8' });

    if (result.status !== 0) {
        // 用户在授权面板点取消也算失败（osascript 返回 -128），不该抛栈
        const detail = `${result.stdout ?? ''}${result.stderr ?? ''}`.trim();
        log.warn(`自动安装未完成：${detail.split('\n')[0] || `osascript 退出码 ${result.status}`}`);
        log.plain('  手动安装二选一：');
        log.plain(`    sudo installer -pkg "${pkgPath}" -target /`);
        log.plain(`    open -W "${pkgPath}"   # 走系统安装器界面`);
    } else {
        // 装完核对：/Applications 里那份必须是本次签出来的，而不是被跳过的旧包
        const installedId = bundleIdOf(TARGET_APP);
        // 必须 -dvv：默认详细级别只打 Executable/Identifier/TeamIdentifier，
        // 不打 Authority 行，用 -dv 判断"是不是本次的身份"会恒为假。
        const installedSign = spawnSync('codesign', ['-dv', '--verbose=2', TARGET_APP], { encoding: 'utf8' });
        const signText = `${installedSign.stdout ?? ''}${installedSign.stderr ?? ''}`;

        if (installedId !== APP_ID) {
            log.warn(
                `装完没找到 ${TARGET_APP}（bundle id 读到 ${installedId ?? '无'}）。\n` +
                    '  Installer 很可能跳过了 payload —— 清掉收据后重装：\n' +
                    `    sudo pkgutil --forget ${APP_ID} && sudo rm -rf ${TARGET_APP}`,
            );
        } else if (identity !== '-' && !signText.includes(identity)) {
            log.warn(
                '装进去的 App 签名身份与本次不一致 —— Installer 很可能跳过了 payload。\n' +
                    '  核对：codesign -dv /Applications/MonoSub.app | grep Authority\n' +
                    `  强制重装：sudo pkgutil --forget ${APP_ID}`,
            );
        } else {
            log.ok('安装完成，签名身份已核对。');
            run('open', ['-a', TARGET_APP]);
        }
    }
}

if (has('--open')) {
    execFileSync('open', ['-R', wantPkg ? pkgPath : STAGED_APP]);
}

// ---------------------------------------------------------------- 收尾提示

log.blank();
log.plain('下一步：');
if (wantPkg) {
    log.plain(`  双击安装：${pkgPath}`);
    log.plain(`  命令行安装：sudo installer -pkg "${pkgPath}" -target /`);
    log.plain(`  启动：open -a "${APP_NAME}"`);
} else {
    log.plain(`  直接运行：open "${STAGED_APP}"`);
}

log.plain('');
log.plain('说明：');
log.plain('  · 别用 sudo 跑本脚本：签名私钥在你的登录钥匙串里，root 读不到；');
log.plain('    需要管理员权限的只有安装那一步，--install 会自己弹授权面板。');
if (sandbox) {
    log.plain(`  · 沙盒版：容器在 ~/Library/Containers/${APP_ID}，与上架包同一套权限`);
    log.plain('    自测重点＝沙盒相关失败：写回源文件目录、自定义模型目录重启后失效、');
    log.plain('    llama-server 的 127.0.0.1 监听。看拒绝日志：');
    log.plain(`    log stream --predicate 'process == "sandboxd"' --info | grep ${APP_ID}`);
    log.plain('  · 清容器重来：rm -rf ~/Library/Containers/' + APP_ID);
} else {
    log.plain(`  · 非沙盒版：数据目录 ~/Library/Application Support/${APP_ID}`);
    log.plain(`    （沙盒版的模型在 ~/Library/Containers/${APP_ID}/ 下，两者不共用）`);
}
if (!isDeveloperId) {
    log.plain('  · 换到别的机器：xattr -dr com.apple.quarantine <pkg> 或右键 → 打开；');
    log.plain('    要彻底免这一步，需要 Developer ID + 公证。');
}
