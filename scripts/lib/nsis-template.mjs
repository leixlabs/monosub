// The forked NSIS template, kept here so the patches have one home.
//
// `src-tauri/nsis/installer.nsi` is Tauri's own bundler template with three
// insertions; `tauri.conf.json` points `bundle.windows.nsis.template` at it.
// This module owns the patches, the file is generated from them by
// `scripts/release/sync-nsis-template.mjs`, and `installer.mjs` reads the header
// to warn when the checked-in copy was built for a different tauri-cli.
//
// ## Why a fork at all
//
// Upstream lets the *previous* version's uninstaller block an upgrade, and then
// dead-ends: "Unable to uninstall!" returns to the reinstall page, where the
// same call fails again. Observed on 2.0.2 -> 2.1.0 with nothing deleted at all
// and no file locked, which is what sent us looking:
//
//   * `uninstall.exe` is launched with this window hidden and without `/P`
//     unless the *parent* installer happens to be passive. Its
//     MUI_UNPAGE_CONFIRM then waits for a click nobody can see.
//   * Before that page, the uninstall section runs CheckIfAppIsRunning
//     (utils.nsh), which asks the Restart Manager who holds `monosub.exe` and,
//     when the app is still in the tray, shows a MessageBox. Cancel aborts the
//     whole section — no file deleted, non-zero exit, generic error here.
//
// Both are UI questions asked on behalf of a hidden window, so the fix is to
// always drive the old uninstaller passively (`/P`): its confirm page is skipped
// (`un.SkipIfPassive`) and it closes the app without asking. The second patch
// makes a remaining failure recoverable instead of terminal — every path this
// installer writes is one the old version already owned, so installing over it
// is safe; the user gets the choice instead of a dead end.
//
// ## Drift
//
// The template is version-matched to the CLI: variables the bundler substitutes
// (`{{product_name}}`, `{{#each resources}}`, ...) must stay as upstream has
// them. `bun run nsis:sync` re-downloads the template for the installed
// tauri-cli and re-applies these patches, failing loudly if an anchor moved.
// Do that first whenever `@tauri-apps/cli` is bumped.

import fs from 'node:fs';
import path from 'node:path';

import { projectRoot } from './paths.mjs';

export const TEMPLATE_PATH = path.join(projectRoot, 'src-tauri', 'nsis', 'installer.nsi');

/** Where the CLI's copy lives in the tauri repo, for a given release tag. */
export function upstreamUrl(tauriCliVersion) {
    return (
        `https://raw.githubusercontent.com/tauri-apps/tauri/tauri-cli-v${tauriCliVersion}` +
        '/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi'
    );
}

/** The tauri-cli actually installed, i.e. the template this build will expect. */
export function installedTauriCliVersion() {
    const manifest = path.join(projectRoot, 'node_modules', '@tauri-apps', 'cli', 'package.json');
    if (!fs.existsSync(manifest)) {
        throw new Error(`@tauri-apps/cli is not installed (${manifest} is missing) — run bun install`);
    }
    return JSON.parse(fs.readFileSync(manifest, 'utf8')).version;
}

/**
 * The patch set, applied in order to the upstream text.
 *
 * `find` strings are copied verbatim from upstream; each one must appear exactly
 * once, otherwise the source moved and the patch needs rewriting rather than
 * being skipped.
 */
export const PATCHES = [
    {
        name: 'drive the old uninstaller passively',
        find: `      ReadRegStr $4 SHCTX "\${MANUPRODUCTKEY}" ""
      ReadRegStr $R1 SHCTX "\${UNINSTKEY}" "UninstallString"
      \${IfThen} $UpdateMode = 1 \${|} StrCpy $R1 "$R1 /UPDATE" \${|} ; append /UPDATE
      \${IfThen} $PassiveMode = 1 \${|} StrCpy $R1 "$R1 /P" \${|} ; append /P
      StrCpy $R1 "$R1 _?=$4" ; append uninstall directory`,
        replace: `      ReadRegStr $4 SHCTX "\${MANUPRODUCTKEY}" ""
      ReadRegStr $R1 SHCTX "\${UNINSTKEY}" "UninstallString"
      \${IfThen} $UpdateMode = 1 \${|} StrCpy $R1 "$R1 /UPDATE" \${|} ; append /UPDATE
      ; Always passive, even when this installer is interactive (upstream only
      ; appended /P when the parent was already passive). The previous
      ; uninstaller is driven from here with this window hidden, and passive
      ; mode is what keeps the two things in it that need a human from
      ; dead-ending the upgrade:
      ;   * MUI_UNPAGE_CONFIRM is skipped (\`un.SkipIfPassive\`), instead of
      ;     waiting forever for a click nobody can see;
      ;   * the Restart Manager "app is running" prompt is skipped and the app
      ;     is closed without asking — answering it with Cancel aborts the whole
      ;     uninstall section, leaving every file in place and exiting non-zero,
      ;     which is exactly the "Unable to uninstall!" death below.
      StrCpy $R1 "$R1 /P"
      StrCpy $R1 "$R1 _?=$4" ; append uninstall directory`,
    },
    {
        name: 'offer to install over a failed uninstall',
        find: `      ; Other errors? show generic error message and return to select un/reinstall page
      MessageBox MB_ICONEXCLAMATION "$(unableToUninstall)"
      Abort`,
        replace: `      ; Other errors? Upstream only reported this and returned to the reinstall
      ; page, which is a dead end: the same call fails again on every retry, so
      ; the application can never be upgraded without uninstalling by hand
      ; first. Offer to install over the old tree instead — every path this
      ; installer writes is one the previous version already owned, so
      ; overwriting is safe, and the prompt keeps the leftover files an explicit
      ; choice rather than a silent one.
      MessageBox MB_YESNO|MB_ICONEXCLAMATION "$(unableToUninstall)$\\r$\\n$\\r$\\n$(reinstallFailedContinue)" IDYES reinst_continue
      Abort
    reinst_continue:`,
    },
    {
        name: 'define the string the second patch needs',
        find: `!include "StrFunc.nsh"
\${StrCase}
\${StrLoc}`,
        replace: `!include "StrFunc.nsh"
\${StrCase}
\${StrLoc}

; Only used by the patched reinstall path below. Upstream keeps every other
; string in the per-language files the bundler generates; this one has no
; upstream counterpart, so it lives here. \`languages\` in tauri.conf.json is the
; default English-only, hence the single LangString — adding a language there
; means adding one here too.
LangString reinstallFailedContinue \${LANG_ENGLISH} "The previous version could not be uninstalled. Continue anyway and install over it? Leftover files from the old version are overwritten where they overlap."`,
    },
];

/** Header written above the patched template; also what `installer.mjs` reads back. */
export function headerFor(tauriCliVersion) {
    return `; ---------------------------------------------------------------------------
; Forked from tauri-cli v${tauriCliVersion}
;   crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi
; Regenerate with: bun run nsis:sync
;
; Only two things differ from upstream, both in PageLeaveReinstall — upstream
; lets the *previous* uninstaller block the upgrade and then dead-ends with
; "Unable to uninstall!". See scripts/lib/nsis-template.mjs for the patches and
; the reasoning; that file is the source of truth, this header is regenerated.
; ---------------------------------------------------------------------------
`;
}

/**
 * Apply every patch to upstream's text.
 *
 * Throws — never warns — when an anchor is missing or ambiguous: a silently
 * unapplied patch would rebuild an installer that still dead-ends, which is the
 * failure this fork exists to fix.
 */
export function applyPatches(upstream, tauriCliVersion) {
    let source = upstream;
    for (const patch of PATCHES) {
        const occurrences = source.split(patch.find).length - 1;
        if (occurrences !== 1) {
            throw new Error(
                `NSIS template patch "${patch.name}" matched ${occurrences} times in the ` +
                    `tauri-cli v${tauriCliVersion} template (expected exactly 1).\n` +
                    '  Upstream changed; rewrite the patch in scripts/lib/nsis-template.mjs.'
            );
        }
        source = source.replace(patch.find, patch.replace);
    }
    return headerFor(tauriCliVersion) + source;
}

/** The tauri-cli version the checked-in template was generated from, or null. */
export function checkedInVersion() {
    if (!fs.existsSync(TEMPLATE_PATH)) return null;
    const match = fs.readFileSync(TEMPLATE_PATH, 'utf8').match(/^; Forked from tauri-cli v(\S+)$/m);
    return match ? match[1] : null;
}
