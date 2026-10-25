#!/usr/bin/env node
// Build the distributable package for whichever platform this is.
//
//   node scripts/release/package.mjs [--skip-resources] [--sign]
//
//   macOS    → dist/MonoSub-<version>.dmg          (ad-hoc signed by Tauri)
//   Windows  → dist/monosub_<version>.0_x64.msix   (unsigned; the Store re-signs)
//
// One script, two branches. The two artifacts have nothing in common beyond
// "produced from the same source tree", which is why the platform-specific parts
// are separate functions rather than a shared pipeline with flags: the macOS flow
// ends in a Tauri-generated DMG that is already signed, the Windows flow builds
// its own MSIX payload because the Store package is not a Tauri bundle at all.
//
// Neither branch bumps the version. The tag must equal the version compiled into
// the app, so the bump happens after a successful publish (scripts/publish/
// publish.mjs) and never before a build.

import fs from 'node:fs';
import path from 'node:path';

import { log, die } from '../lib/log.mjs';
import { hostPlatform, targetTriple, bundledName } from '../lib/platform.mjs';
import {
    projectRoot,
    scriptsDir,
    srcTauriDir,
    distDir,
    resourcesDir,
    binDir,
    stagingDir,
    stagingAppDir,
    devCertPfx,
    devCertCer,
    tauriConfPath,
    iconPath,
    tauriCli,
} from '../lib/paths.mjs';
import { ensureDir, copyFile, copyDir, remove, exists, isFile } from '../lib/fs.mjs';
import { run, hasTool, requireTools } from '../lib/shell.mjs';
import { readTauriVersion, syncVersionsFromTauri } from '../lib/version.mjs';
import * as xml from '../lib/xml.mjs';

const platform = hostPlatform();
const args = process.argv.slice(2);
const skipResources = args.includes('--skip-resources') || args.includes('--skipresources');
const sign = args.includes('--sign');

if (args.includes('--help') || args.includes('-h')) {
    log.plain(`Usage: node scripts/release/package.mjs [--skip-resources] [--sign]

  (无参数)          打包当前平台：macOS → dist/MonoSub-<version>.dmg
                                 Windows → dist/monosub_<version>.0_x64.msix
  --skip-resources 复用已有的 resources/，不重新拉取运行时二进制
  --sign           额外产出一个 dev-signed 副本供本地安装（仅 Windows；
                   签名副本不能提交商店，未签名的原件仍会保留）
  -h, --help       显示这段说明

不打版本号、不联网发布。`);
    process.exit(0);
}

// `main` is synchronous here; Promise.resolve keeps the .catch valid
// whether it returns a promise or nothing at all.
Promise.resolve()
    .then(main)
    .catch((error) => {
        log.err(error.message);
        process.exit(1);
    });

function main() {
    if (platform === 'macos') return buildMacOs();
    if (platform === 'windows') return buildWindows();
    return die(`package runs on macOS or Windows (current: ${platform})`);
}

// ---------------------------------------------------------------------------
// macOS — DMG
// ---------------------------------------------------------------------------

function buildMacOs() {
    const triple = targetTriple();
    const buildDir = path.join(srcTauriDir, 'target');

    log.step('1. 前置清理...');
    // Only the DMG caches: wiping all of target/ would force a full Rust rebuild.
    ensureDir(distDir);
    for (const file of fs.readdirSync(distDir)) {
        if (file.endsWith('.dmg')) remove(path.join(distDir, file));
    }
    for (const target of listDirs(buildDir)) {
        remove(path.join(buildDir, target, 'release', 'bundle', 'dmg'));
    }

    log.step('2. 同步版本号...');
    syncVersionsFromTauri();
    const version = readTauriVersion();
    const dmgName = `MonoSub-${version}.dmg`;
    log.success(`当前发布版本: v${version}`);

    log.step('3. 准备资源依赖...');
    if (!skipResources) {
        runNode(path.join(scriptsDir, 'release', 'prepare-resources.mjs'));
    } else {
        log.warn('--skip-resources：复用已有的 resources/');
    }

    log.step('4. 初始化前端环境及编译代码...');
    const pm = hasTool('bun') ? 'bun' : 'npm';
    run(pm, ['install']);
    run(pm, ['run', 'build']);
    log.success('前端 (Vite) 构建就绪');

    log.step('5. 编译命令行工具 (monosub-cli)...');
    // The CLI lands in resources/bin/ with the same `<name>-<triple>` naming every
    // other bundled binary uses, so `resources/**/*` carries it into the app,
    // sign-app's loop re-signs it and `engine::cli` resolves it — no externalBin,
    // no extra code.
    const cliSource = path.join(srcTauriDir, 'src', 'bin', 'monosub-cli.rs');
    const cliTarget = path.join(binDir, bundledName('monosub-cli'));
    if (isFile(cliSource)) {
        run('cargo', [
            'build',
            '--release',
            '--bin',
            'monosub-cli',
            '--manifest-path',
            path.join(srcTauriDir, 'Cargo.toml'),
            '--target',
            triple,
        ]);
        copyFile(path.join(buildDir, triple, 'release', 'monosub-cli'), cliTarget);
        log.success(`CLI 已就位: ${cliTarget}`);
    } else {
        // Not a hard failure: the CLI must not block packaging for everyone else.
        // Remove a stale copy so an outdated binary cannot be shipped.
        log.warn(`未找到 ${cliSource}，跳过 CLI 编译：本次构建不含 monosub-cli`);
        remove(cliTarget);
    }

    log.step('6. 打包 Tauri 原生客户端...');
    // Signing happens inside this step: tauri.conf.json sets
    // bundle.macOS.signingIdentity = "-", so Tauri ad-hoc signs before it builds
    // the .app and the DMG. The App inside the DMG is therefore already signed.
    run(tauriCli(), ['build', '--target', triple]);
    log.success('Rust / Tauri 二进制编译成功');

    log.step('7. 收集产物 DMG ...');
    const builtDmg = findDmg(buildDir);
    if (!builtDmg) {
        die('编译失败，未能找到 Tauri 自动生成的 DMG 包！');
    }
    const dmgPath = path.join(distDir, dmgName);
    copyFile(builtDmg, dmgPath);
    log.success(`产物 DMG 已收集: ${dmgPath}`);

    log.step('8. macOS 分发校验...');
    // Verify the artifact that will actually be published — the DMG, not the
    // .app on disk, which might be stale or might never have made it in.
    runNode(path.join(scriptsDir, 'release', 'verify.mjs'), dmgPath);

    log.success(`构建完成: ${dmgPath}`);
}

// ---------------------------------------------------------------------------
// Windows — MSIX
// ---------------------------------------------------------------------------

function buildWindows() {
    requireTools(['cargo', 'node', 'npm', 'winapp', 'bun']);

    const version = readTauriVersion();
    if (!/^\d+\.\d+\.\d+$/.test(version)) {
        die(`tauri.conf.json version '${version}' is not x.y.z; MSIX needs four parts`);
    }
    // The manifest needs four parts, the app carries three. Both come from
    // tauri.conf.json so a release cannot ship a manifest that disagrees with the
    // binary.
    const manifestVersion = `${version}.0`;
    const packageName = process.env.MONOSUB_MSIX_NAME || 'monosub';
    const publisher = process.env.MONOSUB_MSIX_PUBLISHER || 'CN=leix';
    const displayName = 'MonoSub';
    const description = 'MonoSub - local voice to article';

    log.step(`Packaging MonoSub ${version} as ${manifestVersion}`);

    log.step('1. Preparing Windows runtime binaries ...');
    if (!skipResources) {
        runNode(path.join(scriptsDir, 'release', 'prepare-resources.mjs'));
    } else {
        log.warn('--skip-resources：复用已有的 resources\\');
    }

    log.step('2. Building frontend and the Tauri binary ...');
    // bun is not optional: tauri.conf.json's beforeBuildCommand is literally
    // `bun run build`, so a machine without bun fails inside `tauri build` with an
    // error that points at the frontend rather than at the missing tool.
    run('bun', ['install']);

    // --no-bundle: the Store package is the MSIX, not an NSIS/MSI installer, and
    // skipping the bundler also avoids downloading NSIS/WiX on a restricted network.
    run(tauriCli(), ['build', '--no-bundle']);

    const exePath = path.join(srcTauriDir, 'target', 'release', 'monosub.exe');
    if (!isFile(exePath)) die(`Expected ${exePath} after the build`);
    log.ok(`Built ${exePath}`);

    // `tauri build` compiles only the bundle's entry point, so the second binary
    // is built explicitly. It lands under resources\bin\ with the
    // `<name>-<triple>.exe` name every other bundled binary uses, which is what
    // `engine::cli` resolves and what the execution alias points at.
    log.step('2b. Building the command-line tool ...');
    const cliName = bundledName('monosub-cli');
    const cliBuilt = path.join(srcTauriDir, 'target', 'release', 'monosub-cli.exe');
    const cliSource = path.join(srcTauriDir, 'src', 'bin', 'monosub-cli.rs');
    if (!isFile(cliSource)) die('Missing src\\bin\\monosub-cli.rs - the Store package declares its alias');
    run('cargo', ['build', '--release', '--bin', 'monosub-cli', '--manifest-path', path.join(srcTauriDir, 'Cargo.toml')]);
    if (!isFile(cliBuilt)) die(`Expected ${cliBuilt} after building monosub-cli`);
    copyFile(cliBuilt, path.join(binDir, cliName));
    log.ok(`CLI staged as ${cliName}`);

    log.step('3. Assembling the package layout ...');
    // The payload is exactly what `AppPaths::resolve_resources_root` looks for next
    // to the executable: <install>\monosub.exe + <install>\resources\.
    remove(stagingDir);
    ensureDir(stagingAppDir);
    copyFile(exePath, path.join(stagingAppDir, 'monosub.exe'));
    copyDir(resourcesDir, path.join(stagingAppDir, 'resources'));

    log.step('4. Generating Package.appxmanifest and Store assets ...');
    // winapp manifest generate needs the executable to exist, hence after the
    // build. Its defaults are overridden so the identity is reproducible from this
    // script alone — no interactive `winapp init` state.
    run('winapp', [
        'manifest',
        'generate',
        '.',
        '--package-name',
        packageName,
        '--publisher-name',
        publisher,
        '--version',
        manifestVersion,
        '--description',
        description,
        '--executable',
        'monosub.exe',
        '--template',
        'Packaged',
        '--if-exists',
        'overwrite',
        '--quiet',
    ], { cwd: stagingAppDir });

    const manifest = path.join(stagingAppDir, 'Package.appxmanifest');
    // Store tiles come from the same source icon Tauri uses, so the Store listing
    // and the installed app cannot show different artwork.
    run('winapp', ['manifest', 'update-assets', iconPath, '--manifest', manifest, '--quiet']);

    patchManifest(manifest, { displayName, cliName });

    log.ok(`Manifest ready: ${manifest}`);

    log.step('5. Packing the MSIX ...');
    ensureDir(distDir);
    const msixName = `${packageName}_${manifestVersion}_x64.msix`;
    const msixPath = path.join(distDir, msixName);
    // --no-sign: the Store re-signs every package it ships, and a package carrying
    // a self-signed certificate is rejected at submission.
    run('winapp', ['package', stagingAppDir, '--manifest', manifest, '--output', msixPath, '--no-sign', '--quiet']);
    if (!isFile(msixPath)) die(`Packing produced no ${msixPath}`);
    log.ok(`Unsigned package: ${msixPath}`);

    if (sign) {
        log.step('6. Creating a dev-signed copy for local install ...');
        // The certificate lives outside staging (wiped every build) so that
        // `--if-exists skip` actually reuses it: a fresh certificate each run
        // would invalidate the .cer already imported into the Root store and
        // every later install would fail with 0x800B010A.
        ensureDir(path.dirname(devCertPfx));
        // --export-cer matters: an MSIX signature is validated against the ROOT
        // store, and `winapp cert install` only adds the certificate to
        // TrustedPeople. Without the .cer in Root, Add-AppxPackage fails with
        // 0x800B010A.
        run('winapp', ['cert', 'generate', '--manifest', manifest, '--output', devCertPfx, '--if-exists', 'skip', '--export-cer', '--quiet']);
        if (!isFile(devCertPfx)) die(`winapp produced no certificate at ${devCertPfx}`);
        run('winapp', ['sign', msixPath, devCertPfx, '--quiet']);

        const signedPath = path.join(distDir, `${packageName}_${manifestVersion}_x64.signed.msix`);
        fs.renameSync(msixPath, signedPath);
        // Keep an unsigned copy: the signed one cannot be submitted.
        run('winapp', ['package', stagingAppDir, '--manifest', manifest, '--output', msixPath, '--no-sign', '--quiet']);
        log.ok(`Dev-signed package: ${signedPath}`);
        log.plain('Install it (admin PowerShell) with:');
        log.plain(`    Import-Certificate -FilePath "${devCertCer}" -CertStoreLocation Cert:\\LocalMachine\\Root`);
        log.plain(`    Add-AppxPackage "${signedPath}"`);
        log.plain('The Root import is required - TrustedPeople alone is not enough for an MSIX.');
        log.plain('Only import it once: the certificate is reused across builds.');
    }

    log.step('7. Checking the package ...');
    runNode(path.join(scriptsDir, 'release', 'verify.mjs'), msixPath);

    log.success('MSIX build complete.');
    log.plain(`    ${msixPath}`);
}

/**
 * The five edits `winapp manifest generate` leaves to the caller.
 *
 *   DisplayName (Properties)      user-visible in Settings → Apps.
 *   DisplayName (VisualElements)  user-visible in the Start menu and on the tile —
 *                                 a DIFFERENT element, and the one a user reads.
 *                                 The bash version only patched the Properties
 *                                 one, so the Start menu shipped lowercase
 *                                 `monosub` (measured on the built package).
 *   PublisherDisplayName          what Settings shows as the publisher.
 *   Executable                    the `$targetnametoken$` placeholder is what the
 *                                 Tauri bundler would substitute; `winapp package`
 *                                 does not, and a manifest left with it fails
 *                                 validation.
 *   ProcessorArchitecture         the package is x64 only.
 *
 * Plus the execution alias, which see below.
 */
function patchManifest(manifest, { displayName, cliName }) {
    let text = fs.readFileSync(manifest, 'utf8');

    // Every edit re-reads the root: `setAttr` splices the string, so offsets
    // captured before an edit are stale after it.
    const root = () => xml.root(text, 'Package');

    const identity = xml.findChild(text, root(), 'Identity');
    if (!identity) die('Generated manifest has no <Identity>');
    text = xml.setAttr(text, identity, 'ProcessorArchitecture', 'x64');

    const properties = xml.findChild(text, root(), 'Properties');
    if (!properties) die('Generated manifest has no <Properties>');
    text = xml.setChildText(text, properties, 'DisplayName', displayName);
    text = xml.setChildText(text, xml.findChild(text, root(), 'Properties'), 'PublisherDisplayName', 'leix');

    const applications = xml.findChild(text, root(), 'Applications');
    if (!applications) die('Generated manifest has no <Applications>');
    const application = xml.findChild(text, applications, 'Application');
    if (!application) die('Generated manifest has no <Application>');

    text = xml.setAttr(text, application, 'Executable', 'monosub.exe');

    const visualElements = xml.findChild(text, xml.findChild(text, xml.findChild(text, root(), 'Applications'), 'Application'), 'VisualElements');
    if (!visualElements) {
        die('Generated manifest has no <VisualElements>; the Start-menu name cannot be set');
    }
    text = xml.setAttr(text, visualElements, 'DisplayName', displayName);

    // --- App execution alias -------------------------------------------
    // Declares `monosub-cli` as a command the shell resolves, without touching
    // PATH. Why an alias and not a PATH entry: MSIX installs under a
    // version-stamped WindowsApps folder, so a PATH entry naming it dies at the
    // next update. Windows puts aliases in
    // %LOCALAPPDATA%\Microsoft\WindowsApps, already on PATH, and re-points them
    // on every update.
    //
    // `winapp manifest add-alias` cannot do this one: it infers the target from
    // the Application's Executable and always writes EntryPoint-less uap5 aliases
    // aimed at the app itself. The uap3 form with an explicit Executable is
    // required, and it must sit inside <Application><Extensions> — as a direct
    // child of <Application>, makeappx rejects it with C00CE014.
    text = xml.setRootAttr(text, 'xmlns:uap3', 'http://schemas.microsoft.com/appx/manifest/uap/windows10/3');
    text = xml.setRootAttr(text, 'xmlns:desktop', 'http://schemas.microsoft.com/appx/manifest/desktop/windows10');

    const ignorable = (xml.getRootAttr(text, 'IgnorableNamespaces') || '')
        .split(/\s+/)
        .filter(Boolean);
    for (const prefix of ['uap3', 'desktop']) {
        if (!ignorable.includes(prefix)) ignorable.push(prefix);
    }
    text = xml.setRootAttr(text, 'IgnorableNamespaces', ignorable.join(' '));

    const app = xml.findChild(text, xml.findChild(text, root(), 'Applications'), 'Application');
    text = xml.appendChild(
        text,
        app,
        `<uap3:Extension Category="windows.appExecutionAlias" Executable="resources\\bin\\${cliName}" EntryPoint="Windows.FullTrustApplication">` +
            `<uap3:AppExecutionAlias><desktop:ExecutionAlias Alias="monosub-cli.exe" /></uap3:AppExecutionAlias>` +
            `</uap3:Extension>`,
        'Extensions'
    );

    // UTF-8 without a BOM: appx packaging rejects a BOM-prefixed manifest.
    fs.writeFileSync(manifest, text, 'utf8');
    log.ok(`Execution alias declared: monosub-cli.exe -> resources\\bin\\${cliName}`);

    // NOTE: no Microsoft.Win32WebView2 PackageDependency is declared. WebView2
    // ships as an Evergreen runtime, not an MSIX framework package, so declaring
    // the dependency makes Add-AppxPackage fail with 0x80073CF3 on machines that
    // do have WebView2.
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function runNode(script, ...args) {
    return run(process.execPath, [script, ...args], { cwd: projectRoot });
}

function listDirs(dir) {
    if (!exists(dir)) return [];
    return fs.readdirSync(dir, { withFileTypes: true }).filter((e) => e.isDirectory()).map((e) => e.name);
}

function findDmg(buildDir) {
    for (const target of listDirs(buildDir)) {
        const dmgDir = path.join(buildDir, target, 'release', 'bundle', 'dmg');
        if (!exists(dmgDir)) continue;
        const found = fs.readdirSync(dmgDir).find((file) => file.endsWith('.dmg'));
        if (found) return path.join(dmgDir, found);
    }
    return null;
}
