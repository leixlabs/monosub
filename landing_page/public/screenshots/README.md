# Landing-page screenshots

Real captures of the MonoSub macOS app (App Store **2.1.0** build) — the current
light/green UI, not the old dark/purple one. Six states, each in light and dark:

| File | Shows |
|---|---|
| `main-light.webp` / `main-dark.webp` | The empty main window (drag-in / import-from-link) |
| `working-light.webp` / `working-dark.webp` | A batch queue being processed |
| `setting-light.webp` / `setting-dark.webp` | The Settings window (files & format) |

Source: the App Store Connect `APP_DESKTOP` screenshots for version 2.1.0
(2560×1600), downloaded with `asc screenshots download`, then downscaled to
1600×1000 WebP:

```bash
asc screenshots download --version-localization <id> --output-dir shots/
python3 - <<'PY'
from PIL import Image
Image.open('shots/main-light.png').convert('RGB').resize((1600,1000), Image.LANCZOS)\
  .save('public/screenshots/main-light.webp','WEBP',quality=84,method=6)
PY
```

A single set serves both locales: the App Store's Chinese listing was uploaded
with the English-UI captures, and interface language is a user setting rather than
a property of the product shot. `src/lib/screenshots.ts` owns the name list and
each image's intrinsic size (passed to `<img>` to reserve layout, preventing CLS).
