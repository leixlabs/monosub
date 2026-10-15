#!/usr/bin/env python3
"""Regenerate the subtitle writer goldens from `segments.json`.

The goldens in this directory are produced by the *original* Python writer
(`core-engine/monosub_core/subtitle_writer.py`) so that `subtitle::writer`
(P1-11) can be proven byte-for-byte identical for the same segment list.

Usage (from the repo root):

    core-engine/.venv/bin/python src-tauri/tests/fixtures/generate_goldens.py

Notes
-----
`SubtitleWriter.write_json()` only accepts a raw word-level dict (`--raw`), so
it cannot emit the segment array used by v2 (`spec §5.4`). For the JSON golden
we therefore call `json.dumps(segments, ensure_ascii=False, indent=2)`
explicitly; the resulting `segments.json.golden` is the same array the v2 Rust
writer must reproduce.
"""

import json
import sys
from pathlib import Path

FIXTURES = Path(__file__).resolve().parent
REPO_ROOT = FIXTURES.parents[2]
sys.path.insert(0, str(REPO_ROOT / "core-engine"))

from monosub_core.subtitle_writer import SubtitleWriter  # noqa: E402


def main() -> None:
    segments = json.loads((FIXTURES / "segments.json").read_text(encoding="utf-8"))

    # JSON: segment array (spec §5.4), emitted directly because write_json()
    # requires a raw word-level dict. No trailing newline — this is exactly
    # `json.dumps(segments, ensure_ascii=False, indent=2)`.
    (FIXTURES / "segments.json.golden").write_text(
        json.dumps(segments, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )

    for fmt in ("srt", "vtt", "txt", "md"):
        out = FIXTURES / f"segments.{fmt}"
        SubtitleWriter.write(segments, str(out), fmt)
        print(f"wrote {out.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()
