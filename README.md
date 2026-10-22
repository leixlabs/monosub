<div align="center">

# MonoSub

**Turn your videos & audio into text. Locally. For free.**

[中文文档](README_CN.md) · [Download](#-download) · [FAQ](#-faq)

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

- **Content creators** turning YouTube videos into blog posts
- **Researchers** transcribing interviews for analysis
- **Students** converting lecture recordings to searchable notes
- **Developers** feeding video content to LLMs for processing

---

## 📥 Download

**Latest Release:** [GitHub Releases](https://github.com/leixlabs/monosub/releases/latest)

### System Requirements

- **macOS** 11.0+ (Apple Silicon M1/M2/M3 optimized)
- **Windows** (Coming Soon)
- **Linux** (Coming Soon)

---

## 🚀 Quick Start

1. **Download** MonoSub from [Releases](https://github.com/leixlabs/monosub/releases/latest)
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

The CLI does not pick its own model — it reuses the plan you already chose in the App (persisted in the settings file), so both run the same model. It only takes the per-run choices: output directory, format, language, and whether to translate (translation is off unless you ask for it with `--translate --target <lang>`).

```bash
monosub-cli video.mp4
monosub-cli video.mp4 --output-dir ~/subs --format srt,vtt
monosub-cli video.mp4 --translate --target en
monosub-cli *.mp4 --json
```

`--json` sends JSON Lines to stdout for scripts; progress goes to stderr. Files are processed in order, and one failure does not stop the rest. With `--translate --target en` the translated subtitle is written alongside the original as `video.en.srt`, leaving `video.srt` untouched.

---

## 💎 Pro License

**MonoSub is completely free** — unlimited transcription for personal use.

| Feature | Free | Pro |
|---------|------|-----|
| Transcription | ✅ Unlimited | ✅ Unlimited |
| Export to SRT, TXT, VTT, MD | ✅ | ✅ |
| Batch processing | ✅ | ✅ |
| **Speaker diarization** (who spoke when) | ❌ | ⏳ *temporarily unavailable* |
| **Priority support** | ❌ | ✅ |

> **Multi-speaker recognition is temporarily unavailable in v2.0.** The v2.0 engine swap changed the
> speech backend, and diarization is being re-implemented on the new engine. It will return in a later
> release; existing Pro licenses stay valid and Pro will cover it again once it ships.

**Pro License: $20** — one-time purchase, lifetime valid  
🎉 **First 1,000 customers: 50% off with code `F1N1MSVJ`**

[**Upgrade to Pro**](https://polar.leix.dev/v1/checkout-links/polar_cl_JgYlmsUnlbej0vpnuI197nQltlEsUxg1hZd0r3unixT/redirect)

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

### Is there a refund policy?

Yes. 7-day money-back guarantee on Pro licenses.

---

<div align="center">

Made for people who just want their videos in text format.

[Download Now](https://github.com/leixlabs/monosub/releases/latest)

</div>
