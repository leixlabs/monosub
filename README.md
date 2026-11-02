<div align="center">

# MonoSub

**Turn your videos & audio into text. Locally. No cloud.**

[中文文档](README_CN.md) · [Microsoft Store](https://apps.microsoft.com/detail/9P6WTCCRQTVW) · [Download](#-download) · [FAQ](#-faq)

</div>

---

## The Problem

You have hours of video content. Podcasts, interviews, lectures, meetings. But LLMs can't watch videos or listen to audio — they only understand text.

So you need transcripts. But existing solutions either:

- Force you to upload private content to the cloud
- Charge monthly fees for something that should be simple
- Take forever because everything has to upload first

**MonoSub fixes this.** Run it on your computer, process files locally, get text fast.

---

## What MonoSub Does

Drag in a video or audio file. Get a transcript. That's it.

- **100% local** — your files never leave your computer
- **Works offline** — no internet needed after installation
- **Multiple formats** — MP4, MOV, MP3, WAV, and more
- **Export to SRT, VTT, TXT** — use with any LLM or video editor
- **Fast** — hardware accelerated on Apple Silicon and NVIDIA GPUs

### Perfect For

- **Content creators** turning online videos into blog posts
- **Researchers** transcribing interviews for analysis
- **Students** converting lecture recordings to searchable notes
- **Developers** feeding video content to LLMs for processing

---

## 📥 Download

| Platform | Where |
|---|---|
| **Windows** 10 / 11 (x64) | [**Microsoft Store**](https://apps.microsoft.com/detail/9P6WTCCRQTVW) — **$9.99 / ¥48, one-time** |
| **macOS** 11.0+ (Apple Silicon) | [GitHub Releases](https://github.com/leixlabs/monosub/releases/latest) |
| **Linux** | Coming soon |

<a href="https://apps.microsoft.com/detail/9P6WTCCRQTVW">
  <img src="https://get.microsoft.com/images/en-us%20dark.svg" alt="Get it from Microsoft Store" height="52" />
</a>

**Windows** is a paid app: **$9.99 / ¥48 once**, no subscription, no in-app purchase.
The Store build is signed and validated by Microsoft, installs in one click, and updates
itself. What you pay for is packaging and upkeep, not capability — the same program
builds from this repository's source (see [Pricing](#-pricing)).

**macOS** is distributed through [GitHub Releases](https://github.com/leixlabs/monosub/releases/latest).

---

## 🚀 Quick Start

1. **Get** MonoSub from the [Microsoft Store](https://apps.microsoft.com/detail/9P6WTCCRQTVW)
   ($9.99 / ¥48), or from [Releases](https://github.com/leixlabs/monosub/releases/latest) on macOS
2. **Open Settings → Models** and download the speech model (~250 MB, one time)
3. **Drag** video/audio files into the app
4. **Click** "Start"
5. **Get** your transcript

That's it. No account. No subscription. No internet needed after the model download.

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

**Windows: $9.99 / ¥48, once, through the Microsoft Store.** No subscription, no
in-app purchase, no upsell. The transaction is over when you buy it.

| | Microsoft Store (Windows) | macOS · GitHub Releases | Build from source |
| --- | --- | --- | --- |
| Price | **$9.99 / ¥48, one-time** | See Releases | Free |
| Transcription | ✅ Unlimited | ✅ Unlimited | ✅ Unlimited |
| Export SRT, TXT, VTT, MD | ✅ | ✅ | ✅ |
| Batch processing | ✅ | ✅ | ✅ |
| Subtitle translation | ✅ | ✅ | ✅ |
| Speaker diarization (who spoke when) | ✅ | ✅ | ✅ |
| Word-level timestamps | ✅ | ✅ | ✅ |
| Signed & verified by the platform | ✅ Microsoft | ❌ (xattr workaround, see FAQ) | ❌ |
| **Automatic updates** | **✅** | **❌** | **❌** |
| Setup effort | One click | One drag | Toolchain + a build |

**What the $9.99 buys.** Microsoft's signing, one-click install, and updates that
arrive without you thinking about them. Not features — every feature listed above is
in the free source build too.

**Paying is optional.** MonoSub's source is public under a source-available licence
(see below). Clone it, build it, and you get the exact same program: no licence key,
no account, no feature gates, no telemetry. The Store listing sells convenience.

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

This is Apple's Gatekeeper being overprotective. Run this in Terminal:

```bash
xattr -cr /Applications/MonoSub.app
```

Then open the app again.

### Do I need internet to use MonoSub?

**Not for transcription.** The first time you use it, download the speech model from  
Settings → Models (~250 MB, one time). After that MonoSub works completely offline.

### What file formats are supported?

Video: MP4, MOV, AVI, MKV, FLV, WMV  
Audio: MP3, WAV, AAC, FLAC, M4A  
If it has audio, MonoSub can transcribe it.

### What languages can it transcribe?

Currently supports Chinese (Mandarin), English, and more languages are being added. The AI automatically detects the language.

### How much does it cost?

On Windows, **$9.99 / ¥48 once**, through the Microsoft Store — no subscription, no
in-app purchases. On macOS, see
[Releases](https://github.com/leixlabs/monosub/releases/latest). Building from source
costs nothing on either platform.

### Is there a refund policy?

Microsoft Store purchases are refunded by Microsoft under
[their own policy](https://support.microsoft.com/help/2884681) — MonoSub never handles
the payment, so the refund comes from the Store.

### Does the paid build have more features?

No. Same engine, same models, same features, same output formats. The Store build adds
Microsoft's signing, one-click install, and automatic updates.

---

<div align="center">

Made for people who just want their videos in text format.

[**Get it for Windows**](https://apps.microsoft.com/detail/9P6WTCCRQTVW) · [Download for macOS](https://github.com/leixlabs/monosub/releases/latest)

</div>
