#!/usr/bin/env node
// Regenerate src-tauri/nsis/installer.nsi from Tauri's own template.
//
//   node scripts/release/sync-nsis-template.mjs [--check] [--version <x.y.z>]
//
//   (no flags)        download the template for the installed tauri-cli, apply
//                     the patches from scripts/lib/nsis-template.mjs, write it
//   --check           download and compare, exit 1 on any difference; writes nothing
//   --version <v>     use another tauri-cli tag (e.g. to preview a CLI bump)
//
// Run this after every `@tauri-apps/cli` bump. The template is version-matched:
// the bundler substitutes `{{...}}` placeholders into whatever it finds at
// `bundle.windows.nsis.template`, so a template from an older CLI can reference
// variables the new one no longer fills in — and the patched upgrade path would
// be silently lost if someone "fixed" a build by copying the stock template.
//
// The patches themselves are documented in scripts/lib/nsis-template.mjs; this
// script only moves bytes and refuses to write anything if an anchor moved.

import fs from 'node:fs';
import path from 'node:path';

import { log, die, header } from '../lib/log.mjs';
import { projectRoot } from '../lib/paths.mjs';
import { download } from '../lib/net.mjs';
import { makeScratchDir, remove } from '../lib/fs.mjs';
import {
    TEMPLATE_PATH,
    upstreamUrl,
    installedTauriCliVersion,
    applyPatches,
} from '../lib/nsis-template.mjs';

const args = process.argv.slice(2);

if (args.includes('-h') || args.includes('--help')) {
    usage();
    process.exit(0);
}

const checkOnly = args.includes('--check');
const versionFlag = args.indexOf('--version');
const version = versionFlag === -1 ? null : args[versionFlag + 1];

if (versionFlag !== -1 && !version) die('--version 需要一个版本号，例如 2.12.3');

const unknown = args.filter((arg, index) => arg !== '--check' && arg !== '--version' && index !== versionFlag + 1);
if (unknown.length > 0) die(`未知参数: ${unknown.join(' ')}`);

Promise.resolve()
    .then(main)
    .catch((error) => {
        log.err(error.message);
        process.exit(1);
    });

async function main() {
    process.chdir(projectRoot);

    const cliVersion = version ?? installedTauriCliVersion();
    header(`NSIS 模板 — tauri-cli v${cliVersion}${checkOnly ? '（只校验）' : ''}`);

    const scratch = makeScratchDir('nsis-template-');
    try {
        const upstream = path.join(scratch, 'installer.nsi');
        log.step(`下载 ${upstreamUrl(cliVersion)}`);
        await download(upstreamUrl(cliVersion), upstream, { label: 'installer.nsi', retries: 3 });

        let generated;
        try {
            generated = applyPatches(fs.readFileSync(upstream, 'utf8'), cliVersion);
        } catch (error) {
            die(error.message);
        }

        const current = fs.existsSync(TEMPLATE_PATH) ? fs.readFileSync(TEMPLATE_PATH, 'utf8') : null;

        if (checkOnly) {
            if (current === generated) {
                log.ok('模板与 tauri-cli 版本一致，补丁齐全');
                return;
            }
            log.err('模板与当前 tauri-cli 不一致 —— 运行 bun run nsis:sync 重新生成');
            process.exit(1);
        }

        if (current === generated) {
            log.ok('模板已是最新，未改动');
            return;
        }

        fs.mkdirSync(path.dirname(TEMPLATE_PATH), { recursive: true });
        fs.writeFileSync(TEMPLATE_PATH, generated);
        log.success(`已写入 ${path.relative(projectRoot, TEMPLATE_PATH)}（${generated.split('\n').length - 1} 行）`);
        log.plain('  下次 `bun run release:exe` 的 NSIS 打包就会用这份模板。');
    } finally {
        remove(scratch);
    }
}

function usage() {
    console.log(`Usage: node scripts/release/sync-nsis-template.mjs [--check] [--version <x.y.z>]

  把 Tauri 官方 NSIS 模板拉下来，套上 scripts/lib/nsis-template.mjs 里的补丁，
  写成 src-tauri/nsis/installer.nsi —— tauri.conf.json 的
  bundle.windows.nsis.template 指向它。

  (无参数)          按已安装的 @tauri-apps/cli 版本重新生成
  --check           只比对，不改文件；不一致时以 1 退出（适合放进 CI）
  --version <v>     换成指定 tauri-cli tag，用来预览升级 CLI 后的模板差异
  -h, --help        这段说明

每次升级 @tauri-apps/cli 都要跑一次：模板与 CLI 版本是绑定的，模板落后会让
打包出来的安装程序引用新 CLI 不再填充的占位符。`);
}
