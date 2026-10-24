# Landing-page screenshots

These are real captures of the MonoSub app, not mock-ups. Each screen exists
twice: `-en` (English UI) and `-zh-CN` (Simplified Chinese UI). The landing page
picks the pair that matches its own locale — `/en/` loads `-en`, `/zh/` loads
`-zh-CN` — so an English page never shows a Chinese window.

Source: `~/Desktop/monosub-screenshots` (the app's full screenshot set, with a
README describing each frame). This folder holds only the subset the landing
page uses, converted with:

```bash
# full-window captures — 1600×1067
cwebp -q 80 -resize 1600 0 "<src>.png" -o "<name>-<locale>.webp"

# Settings captures — cropped to the modal (the window around it is empty),
# 1600×1352
ffmpeg -i "<src>.png" -vf "crop=1740:1470:330:110" /tmp/x.png
cwebp -q 82 -resize 1600 0 /tmp/x.png -o "<name>-<locale>.webp"
```

File names match the app screenshot set, so a re-capture can be mapped back by
name. If you replace one, update its intrinsic size in
`src/lib/screenshots.ts` — the page passes that to `<img>` to reserve layout.

| File | Shows |
|---|---|
| `02-queue` | A queue of files, each with its own speaker / translation switches |
| `08-batch-running` | A batch in flight: one row running, the rest queued |
| `07-row-states` | Skipped, waiting-for-slot, translated and partly-untranslated rows |
| `09-translated-row` | A finished row that also wrote its translated copy |
| `04-clipboard-pill` | The one-click import pill for a clipboard Bilibili link |
| `33-url-import-rows` | URL imports: downloading, fetched, needs a login, failed |
| `22-url-dialog-valid` | The "Import from a link" panel |
| `11-settings-models-top` | Model selection and the live plan preview |
| `27-models-installed` | The model catalogue (installed / not installed badges) |
| `12-settings-translate-top` | Translation settings and model downloads |
| `40-light-theme` | The light theme |

`og-image.jpg` is the 1200×630 social card, made from `02-queue`:

```bash
ffmpeg -i "<src>/02-queue-en.png" -vf "scale=1200:-2,crop=1200:630" -q:v 4 og-image.jpg
```
