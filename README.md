<div align="center">

# MonoSub

**Turn your videos & audio into subtitles. On your machine, not the cloud.**

[中文文档](README_CN.md) · [Download](#-download) · [Pricing](#-pricing) · [FAQ](#-faq)

Local transcription & translation · macOS & Windows · free to start

</div>

---

## The Problem

You have hours of video content. Podcasts, interviews, lectures, meetings. But LLMs can't watch videos or listen to audio — they only understand text.

So you need transcripts. But the tools that exist either:

- Force you to upload private content to the cloud
- Charge monthly fees — or meter you by the minute — for something that should be simple
- Make you wait while everything uploads first

**MonoSub fixes this.** Run it on your computer, process files locally, get text fast. And it starts free: you only pay once if you need to run a whole folder at a time.

---

## What MonoSub Does

Drag in a video or audio file. Get a transcript, or a finished subtitle file. That's it.

- **100% local** — your files never leave your computer; nothing is uploaded
- **Works offline** — no internet needed after the one-time model download
- **Transcribe & translate** — local speech recognition plus a local translation model, no per-minute bill
- **Subtitles, not just text** — speaker diarization, word-level timestamps, and SRT, VTT, TXT, MD or JSON export
- **Import from a link** — paste a supported video URL to take its platform subtitle or transcribe the audio locally
- **Fast** — hardware-accelerated on Apple Silicon and NVIDIA GPUs
- **Batch when you need it** — the queue runs one file at a time for free; Pro lifts the limit and runs your whole library in parallel

### Perfect For

- **Content creators** turning online videos into blog posts or subtitles
- **Researchers** transcribing interviews for analysis
- **Students** converting lecture recordings to searchable notes
- **Developers** feeding video content to LLMs for processing
- **Anyone with sensitive footage** — client interviews, internal meetings, unreleased work — that must not be uploaded

---

## 📥 Download

| Platform | Where |
|---|---|
| **macOS** 12+ (Apple Silicon) | [**Mac App Store**](https://apps.apple.com/us/app/monosub/id6829689694) — **free download**, Pro is a one-time in-app purchase |
| **Windows** 10 / 11 (x64) | [**Microsoft Store**](https://apps.microsoft.com/detail/9P6WTCCRQTVW) — **free download**, Pro is a one-time in-app purchase |
| **macOS / Windows · direct** | [**checkout.leix.dev/p/monosub-pro**](https://checkout.leix.dev/p/monosub-pro) — Pro offline licence **¥36 / $8.60**; free build on [Releases](https://github.com/leixlabs/monosub/releases/latest) |
| **Linux** | Coming soon |

<a href="https://apps.apple.com/us/app/monosub/id6829689694">
  <img src="https://tools.applemediaservices.com/api/badges/download-on-the-app-store/black/en-us?size=250x83" alt="Download on the Mac App Store" height="52" />
</a>
&nbsp;
<a href="https://apps.microsoft.com/detail/9P6WTCCRQTVW">
  <img src="https://get.microsoft.com/images/en-us%20dark.svg" alt="Get it from Microsoft Store" height="52" />
</a>

**MonoSub is free to download and free to use one file at a time** — transcription, translation, speaker diarization and every export format are included. A one-time **Pro** licence removes the single-file limit so you can batch a whole folder. There is no subscription and no per-minute billing, ever.

- **Mac App Store**: free download + Pro as a one-time in-app purchase ($9.99 / ¥48). The listing is live in the US and most regions; **mainland China is pending filing**.
- **Windows / Microsoft Store**: free download + Pro as a one-time in-app purchase ($9.99 / ¥48) — signed, one-click install, automatic updates.
- **Direct**: buy the **Pro offline licence** at [checkout.leix.dev](https://checkout.leix.dev/p/monosub-pro) for ¥36 / $8.60. It is not tied to a device and works on both macOS and Windows; the free build itself is on [Releases](https://github.com/leixlabs/monosub/releases/latest).
- **Build from source**: the source is public (source-available licence below), so building it yourself is always free.

---

## 🚀 Quick Start

1. **Get** MonoSub — [Mac App Store](https://apps.apple.com/us/app/monosub/id6829689694) (free), [Microsoft Store](https://apps.microsoft.com/detail/9P6WTCCRQTVW) (Windows), or the direct build from [Releases](https://github.com/leixlabs/monosub/releases/latest)
2. **Open Settings → Models** and download the speech model (~250 MB, one time)
3. **Drag** video or audio files into the app
4. **Click** "Start"
5. **Get** your transcript — export SRT, VTT, TXT, MD or JSON

That's it. No account. No subscription. No internet needed after the model download. The free version processes one file at a time; Pro unlocks the queue.

> **v2.0 note:** speech recognition now runs on a bundled [audio.cpp](https://github.com/0xShug0/audio.cpp)  
> engine instead of Python + PyTorch. The app is much smaller and uses far less memory, but the  
> recognition model is no longer shipped inside the installer — you download it once from  
> Settings → Models. See the release notes for details.

---

## 💻 Command Line

MonoSub ships a command line entry point, `monosub-cli`, for the cases a window does not serve: batch scripts, CI, and agents — running a folder of files from a shell is the whole point of it. It runs the same pipeline as the app, and writes the same formats next to (or instead of) the files you point it at.

The CLI does not pick its own model — it reuses the plan you already chose in the App (persisted in the settings file), so both run the same model. It only takes the per-run choices: output directory, format, language, whether to translate (translation is off unless you ask for it with `--translate --target <lang>`), and whether you want word-level timing. `--model <id>` can substitute one model for one run — `--list-models` prints the ids — and it is never written back to your settings.

```bash
monosub-cli video.mp4
monosub-cli video.mp4 --output-dir ~/subs --format srt,vtt
monosub-cli video.mp4 --translate --target en
monosub-cli video.mp4 --word-timestamps      # + video.words.json
monosub-cli video.mp4 --model qwen3_asr_0_6b_q8_0 --word-timestamps
monosub-cli --list-models
monosub-cli *.mp4 --json
```

`--json` sends JSON Lines to stdout for scripts; progress goes to stderr. Files are processed in order, and one failure does not stop the rest. With `--translate --target en` the translated subtitle is written alongside the original as `video.en.srt`, leaving `video.srt` untouched.

`--word-timestamps` adds `video.words.json` next to the subtitle: every word with its own start and end, plus the silences between them — the input a karaoke highlight or any animation timeline is built from. It needs a plan solved for word timing (`qwen3_asr` + the forced aligner), so if the saved plan cannot align the CLI says so before it runs and exits with code 6 rather than quietly estimating.

---

## 💎 Pricing

**Free to download. Free to use one file at a time. Pro is a one-time unlock for batch — never a subscription.**

| | Free | Pro |
| --- | --- | --- |
| Price | **$0** | **$9.99 / ¥48** via the App Store in-app purchase — or **¥36 / $8.60** direct |
| Transcription & translation | ✅ | ✅ |
| Export SRT, VTT, TXT, MD, JSON | ✅ | ✅ |
| Speaker diarization & word timestamps | ✅ | ✅ |
| Files in the queue at once | **One** | **Unlimited** |
| Batch a whole library in parallel | — | ✅ |
| Subscriptions / per-minute billing | Never | Never |

### Where to get it

| Platform | Download | Price / how Pro works |
| --- | --- | --- |
| **macOS · App Store** | Free | Pro in-app purchase **$9.99 / ¥48** · China store pending filing |
| **macOS / Windows · Direct** | Free build | Pro offline licence **¥36 / $8.60** — not device-bound, works on both |
| **Windows · Microsoft Store** | Free | Pro in-app purchase **$9.99 / ¥48** |
| **Any · Build from source** | Free | — |

Prices shown in USD; the App Store and Microsoft Store charge in your local currency.

**What Pro actually unlocks.** Exactly one thing: the single-file limit. Every capability — speaker diarization, word timestamps, translation, every export format — is in the free version. Pro is for people who process a folder, not a file.

**Building from source is still free.** MonoSub's source is public under a source-available licence (see below). Clone it, build it, and you get the exact same program: no licence key, no account, no feature gates, no telemetry.

---

## 📄 License

MonoSub's source is released under the **[PolyForm Small Business License 1.0.0](https://polyformproject.org/licenses/small-business/1.0.0)** —
source-available, not open source.

| Who | Commercial use |
|---|---|
| Individuals, hobby projects, anything non-commercial | ✅ Free |
| Companies with **fewer than 100 people** and **under $1M revenue** | ✅ Free |
| Companies above that threshold | 💬 Separate commercial licence required |

**Model weights are not covered by this licence and are not distributed with MonoSub.** Every model is fetched from its publisher's own host, and each carries the licence its publisher chose — 16 of the 129 catalogue entries are non-commercial. The app shows each model's licence, with a link to the publisher's terms, before a download starts.

Third-party components (FFmpeg, audio.cpp, llama.cpp, yt-dlp) are disclosed in
[`src-tauri/resources/licenses/THIRD-PARTY-NOTICES.md`](src-tauri/resources/licenses/THIRD-PARTY-NOTICES.md).

---

## FAQ

### macOS says "MonoSub is damaged and can't be opened"

This only affects the **GitHub Releases** build, which is ad-hoc signed. The Mac App Store version is signed and notarized by Apple and opens normally. For the Releases build, run this in Terminal:

```bash
xattr -cr /Applications/MonoSub.app
```

Then open the app again.

### Is MonoSub really free?

Yes. The full engine is free to download and free to use one file at a time — transcription, translation, speaker diarization and every export format included. Pro is a one-time purchase that only removes the single-file limit.

### How much does Pro cost, and is it a subscription?

No subscription. Pro is one-time: **$9.99 / ¥48** as a Mac App Store in-app purchase, or **¥36 / $8.60** as a direct offline licence at [checkout.leix.dev](https://checkout.leix.dev/p/monosub-pro). The Windows Microsoft Store version is also a free download with Pro as a one-time **$9.99 / ¥48** in-app purchase. Building from source is free on every platform.

### Is the Mac App Store version available in China?

The macOS App Store listing is live in the US and most regions; the **mainland China store is pending filing**. Until then, the direct offline licence at [checkout.leix.dev](https://checkout.leix.dev/p/monosub-pro) works in China, and the Windows build is on the Microsoft Store.

### Do I need internet to use MonoSub?

**Not for transcription.** The first time you use it, download the speech model from Settings → Models (~250 MB, one time). After that MonoSub works completely offline.

### What file formats are supported?

Video: MP4, MOV, AVI, MKV, FLV, WMV  
Audio: MP3, WAV, AAC, FLAC, M4A  
If it has audio, MonoSub can transcribe it.

### What languages can it transcribe?

Currently supports Chinese (Mandarin), English, and more languages are being added. The AI automatically detects the language.

### Is there a refund policy?

Refunds follow the store you bought from: Apple handles Mac App Store refunds, and Microsoft handles Microsoft Store refunds, under their own policies. For the direct licence, contact support from your order page at [checkout.leix.dev](https://checkout.leix.dev/p/monosub-pro).

### Does Pro have more features?

No. Same engine, same models, same features, same output formats. Pro only removes the single-file queue limit so you can batch. Windows and macOS work the same way: free to use one file at a time, Pro to batch.

---

<div align="center">

Made for people who just want their videos in text format — without uploading them.

[**Download for Mac**](https://apps.apple.com/us/app/monosub/id6829689694) · [**Get it for Windows**](https://apps.microsoft.com/detail/9P6WTCCRQTVW) · [**Buy Pro directly**](https://checkout.leix.dev/p/monosub-pro)

</div>
