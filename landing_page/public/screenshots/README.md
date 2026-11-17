# Landing-page screenshots

Real captures of the current MonoSub macOS app (App Store **2.1.0** build) — the
light/green UI, in **both UI languages** and both themes:

| File | Shows |
|---|---|
| `main-en-light.webp` / `main-en-dark.webp` | English UI — empty main window |
| `main-zh-light.webp` / `main-zh-dark.webp` | 中文界面 — 空主界面 |
| `settings-en-light.webp` / `settings-en-dark.webp` | English UI — Settings |
| `settings-zh-light.webp` / `settings-zh-dark.webp` | 中文界面 — 设置 |

Source: captured at 2560×1600 from the running app (window-size 1280×800 on a 2×
display), then downscaled to 1600×1000 WebP (quality 84):

```bash
python3 - <<'PY'
from PIL import Image
Image.open('raw.png').convert('RGB').resize((1600,1000), Image.LANCZOS)\
  .save('public/screenshots/main-en-light.webp','WEBP',quality=84,method=6)
PY
```

The UI language follows the app's saved preference (`monosub-language` in the
WebView localStorage); theme likewise (`monosub-theme`). The homepage loads the
screenshot matching its own locale via `heroShot(lang)` in `src/lib/screenshots.ts`.
