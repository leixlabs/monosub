#!/usr/bin/env node
// Bump the patch version in tauri.conf.json and sync Cargo.toml + package.json.
//
//   node scripts/lib/bump-version.mjs
//
// Called by scripts/publish/publish.mjs *after* a successful publish, never by a
// build: the release tag must equal the version compiled into the app, so bumping
// before a build ships an artifact whose name is one ahead of its tag (once
// produced `MonoSub-2.0.1.dmg` under the tag `v2.0.0`).
//
// Both platforms publish from the same tauri.conf.json, so this is shared rather
// than per-platform.

import { bumpPatchVersionAndSync } from './version.mjs';
import { log } from './log.mjs';

try {
    log.plain(bumpPatchVersionAndSync());
} catch (error) {
    log.err(error.message);
    process.exit(1);
}
