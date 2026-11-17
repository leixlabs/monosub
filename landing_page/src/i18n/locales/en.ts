import type { Translation } from '../index';

const en: Translation = {
  lang_name: 'English',

  page_title: 'MonoSub — Local AI Subtitle Generator for Mac & Windows',
  page_description:
    'Turn video and audio into subtitles on your own Mac or PC. No cloud, no account, no subscription — download free, unlock unlimited batch once with Pro.',
  og_alt: 'MonoSub, a local subtitle app, running on a Mac',

  nav: {
    features: 'What it does',
    how: 'How it works',
    pricing: 'Pricing',
    faq: 'Questions',
    blog: 'Blog',
    cta_primary: 'Download free',
    cta_secondary: 'Get Pro',
  },

  hero: {
    eyebrow: 'Local-first subtitle studio',
    title: 'Turn video into subtitles.',
    title_accent: 'On your machine, not the cloud.',
    subtitle:
      'MonoSub runs speech recognition and translation entirely on your Mac or PC. Your recordings are never uploaded — no account, no subscription, no per-minute meter.',
    cta_primary: 'Download free for Mac',
    cta_secondary: 'All download options',
    note: 'Free to start. Unlimited batch unlocks with a one-time Pro license.',
    trust: ['100% on-device', 'Free to download', 'One-time Pro — never a subscription'],
    shot_caption: 'The queue, with per-file speaker and translation switches.',
  },

  answer: {
    eyebrow: 'The short answer',
    title: 'Yes — you can generate subtitles without sending the file anywhere.',
    body: 'MonoSub is a desktop app that transcribes and translates your audio locally. Drop in a file, choose a model, and get SRT, VTT, TXT, Markdown or JSON back. The speech engine and the translation model run on your own hardware, so nothing is uploaded and nothing needs a connection after the model download.',
    stats: [
      { value: '0', label: 'bytes uploaded — files stay on your disk' },
      { value: '5', label: 'export formats: SRT, VTT, TXT, MD, JSON' },
      { value: '1×', label: 'one-time Pro unlock; never a monthly bill' },
    ],
  },

  why: {
    eyebrow: '01 — Why local',
    q: 'Why not just use a cloud transcription service?',
    a: 'Cloud tools are convenient until the file matters — an unreleased video, a client interview, a private meeting. Uploading it means it leaves your machine and sits on someone else’s server, and per-minute billing quietly becomes a subscription. MonoSub removes the upload and the meter: the work happens where the file already is.',
    cloud_title: 'Cloud transcription',
    local_title: 'MonoSub',
    cloud: [
      'Your file is uploaded to a third-party server',
      'Billed per minute, or a monthly plan you forget',
      'Needs a connection; queues behind other users',
      'You cannot verify when the upload is deleted',
    ],
    local: [
      'Files are read on your disk and never uploaded',
      'Free for single files; Pro is a one-time unlock',
      'Works fully offline after the model download',
      'Hardware-accelerated on Apple Silicon and NVIDIA GPUs',
    ],
  },

  how: {
    eyebrow: '02 — How it works',
    title: 'Three steps, and none of them involve an upload.',
    subtitle:
      'The same pipeline on macOS and Windows: recognition, optional translation and export all run locally.',
    steps: [
      {
        title: 'Download and install',
        desc: 'Get MonoSub from the App Store or the Microsoft Store, or use the free direct build. It is a normal desktop app — no account, no sign-in.',
      },
      {
        title: 'Drop in your files',
        desc: 'Drag in one video or a whole folder. The free version runs one file at a time; Pro lifts that limit and runs the queue in parallel.',
      },
      {
        title: 'Take the subtitles out',
        desc: 'Export SRT, VTT, TXT, Markdown or a JSON word sidecar — optionally translated into a second language and written next to the original.',
      },
    ],
    note: 'The first run downloads a speech model of your choice (~250 MB) from Settings → Models. After that, transcription is fully offline.',
  },

  showcase: {
    eyebrow: '03 — The app',
    title: 'Look at the actual app.',
    subtitle: 'Every image below is a real screenshot of MonoSub, not a mock-up.',
    tours: [
      {
        id: 'queue',
        label: 'Queue & batch',
        desc: 'Drop in as many files as you like. Each row carries its own speaker-separation and translation switches, the queue runs them up to your concurrency limit, and every step reports its own progress.',
        shots: [
          'The queue, with per-file switches',
          'A batch in flight',
          'Skipped, waiting, translated and partly untranslated rows',
        ],
      },
      {
        id: 'translate',
        label: 'Translation',
        desc: 'A local translation model rewrites the finished subtitles into a second language. It writes a separate file next to the original — the recognised subtitles are never touched, so a failed translation cannot lose your work.',
        shots: [
          'A translated subtitle written alongside the original',
          'Translation settings and model downloads',
        ],
      },
      {
        id: 'speakers',
        label: 'Speakers & models',
        desc: 'Speaker separation labels who said what, cue by cue. The model picker answers four questions instead of making you choose a checkpoint: language scenario, speaker separation, word-level timing and memory budget.',
        shots: [
          'Model selection and the live plan preview',
          'The model catalogue — install and remove on demand',
        ],
      },
      {
        id: 'url',
        label: 'Import from a link',
        desc: 'Paste a supported video link and MonoSub takes the platform subtitle if there is one. If not, it downloads the audio and transcribes it locally — the same pipeline as a file you dragged in.',
        shots: [
          'URL imports: downloading, fetched, needs a login, failed',
          'The import-from-a-link panel',
          'One-click import when a link is on the clipboard',
        ],
      },
    ],
  },

  features: {
    eyebrow: '04 — What it does',
    title: 'Capability without the asterisks.',
    subtitle:
      'Every feature below is in the free download. Pro only changes how many files you can run at once.',
    items: [
      {
        title: '100% local processing',
        desc: 'Speech recognition and translation run entirely on your machine. No uploads, no cloud — your data never leaves your computer.',
      },
      {
        title: 'Local subtitle translation',
        desc: 'Built-in local translation models. Generate translated subtitles in Chinese, English and more with one click — fully offline, no extra cost.',
      },
      {
        title: 'Speaker separation',
        desc: 'Automatically identifies who spoke when, cue by cue. Built for interviews, meetings, podcasts and multi-speaker recordings.',
      },
      {
        title: 'Hardware acceleration',
        desc: 'Metal on Apple Silicon, and the same inference engines on Windows — faster local transcription and translation.',
      },
      {
        title: 'Import from a link',
        desc: 'Paste a supported video link to take its platform subtitle, or let MonoSub download the audio and transcribe it locally.',
      },
      {
        title: 'Word-level timestamps',
        desc: 'Word-accurate cue boundaries plus a word/silence sidecar file, so a video can be cut on the transcript.',
      },
      {
        title: 'Every export format',
        desc: 'Export to SRT, VTT, TXT, MD and JSON — compatible with all major video editors, players and LLM pipelines.',
      },
      {
        title: 'Batch processing',
        desc: 'Queue a whole library and let it run. The free version does one file at a time; Pro runs them in parallel.',
      },
    ],
  },

  pricing: {
    eyebrow: '05 — Pricing',
    title: 'Free to download. Pro only when you batch.',
    subtitle:
      'No subscription, no per-minute meter, no account. Pay once for Pro if you need to run more than one file at a time.',
    plans: [
      {
        id: 'free',
        name: 'Free',
        badge: 'Always free',
        price: '$0',
        priceNote: 'No card, no account',
        summary: 'For a file at a time.',
        features: [
          'Full transcription & translation engine',
          'All export formats: SRT, VTT, TXT, MD, JSON',
          'Speaker separation & word timestamps',
          'One file in the queue at a time',
        ],
        cta: 'Download free',
      },
      {
        id: 'pro',
        name: 'MonoSub Pro',
        badge: 'One-time unlock',
        price: '$9.99',
        priceNote: '$9.99 / ¥48 via the App Store · or ¥36 / $8.60 direct',
        summary: 'For a folder at a time.',
        features: [
          'Everything in Free',
          'Unlimited files in the queue',
          'Batch your whole library in parallel',
          'Free minor updates',
        ],
        cta: 'Unlock Pro',
      },
    ],
    proNoteTitle: 'What Pro actually unlocks',
    proNote:
      'Exactly one thing: the single-file limit. Every capability — speaker separation, word timestamps, translation, every export format — is in the free version. Pro is for people who process a folder, not a file.',
    channelsTitle: 'Where to get it',
    channelsNote:
      'Prices in USD; the App Store and Microsoft Store charge in your local currency. The App Store listing is live in the US and most regions — mainland China is pending filing.',
    columns: { platform: 'Platform', price: 'Price', pro: 'How Pro works' },
    channels: [
      {
        id: 'appstore',
        platform: 'macOS · App Store',
        price: 'Free',
        pro: 'In-app purchase · $9.99 / ¥48',
        note: 'China availability pending filing',
      },
      {
        id: 'direct',
        platform: 'macOS / Windows · Direct',
        price: 'Free build',
        pro: 'Offline license · ¥36 / $8.60',
        note: 'License works on both, not device-bound',
      },
      {
        id: 'windows',
        platform: 'Windows · Microsoft Store',
        price: 'Free',
        pro: 'In-app purchase · $9.99 / ¥48',
        note: 'Signed, one-click install, auto-updates',
      },
      {
        id: 'source',
        platform: 'Any · Build from source',
        price: 'Free',
        pro: '—',
        note: 'Source-available licence',
      },
    ],
  },

  faq: {
    eyebrow: 'Questions',
    title: 'The questions people actually ask.',
    subtitle: 'Short answers. If yours is missing, the blog goes deeper.',
    items: [
      {
        q: 'Does MonoSub upload my files?',
        a: 'No. Speech recognition and translation run on your own machine, so your audio and video never leave the computer. The only network traffic is the model files you choose to download and any video link you paste.',
      },
      {
        q: 'Is it really free?',
        a: 'Yes. The full engine is free to download and free to use one file at a time — transcription, translation, speaker separation and every export format included. Pro is a one-time purchase that removes the single-file limit.',
      },
      {
        q: 'What does Pro cost, and is it a subscription?',
        a: 'Pro is a one-time purchase: $9.99 / ¥48 through the App Store in-app purchase, or ¥36 / $8.60 for the direct offline license at checkout.leix.dev. There is no subscription and no per-minute billing.',
      },
      {
        q: 'Which platforms are supported?',
        a: 'macOS 12+ (Apple Silicon) through the App Store, and Windows 10/11 (x64) through the Microsoft Store. A free macOS build is also available on GitHub Releases.',
      },
      {
        q: 'Do I need an internet connection?',
        a: 'Not for transcription. Download a speech model once (~250 MB) from Settings → Models, and after that the app works completely offline.',
      },
      {
        q: 'Which languages and file formats work?',
        a: 'Chinese (Mandarin), English and more, detected automatically. Video or audio — MP4, MOV, MKV, AVI, MP3, WAV, M4A, FLAC and others. Export SRT, VTT, TXT, Markdown or JSON.',
      },
      {
        q: 'Can I process a whole folder at once?',
        a: 'Yes, with Pro. The free version does one file at a time; Pro removes that limit and runs the queue in parallel up to your concurrency setting. Batch processing is the only thing Pro changes.',
      },
      {
        q: 'Is the App Store version available in China?',
        a: 'The macOS App Store listing is live in the US and most regions; the mainland China store is pending filing. Until then, the direct license at checkout.leix.dev works in China, and the Windows build is on the Microsoft Store.',
      },
    ],
  },

  final: {
    title: 'Your videos are already on your machine. Keep the text there too.',
    subtitle: 'Download free and transcribe your first file in minutes. Unlock Pro only if you batch.',
    cta_primary: 'Download free for Mac',
    cta_secondary: 'Buy Pro — ¥36 / $8.60',
    trust: ['Free download', 'No account', 'One-time Pro'],
  },

  footer: {
    tagline: 'Local AI subtitles. Your files stay yours.',
    blog: 'Blog',
    privacy: 'Privacy',
    contact: 'Contact',
    terms: 'Terms',
    copyright: '© 2026 MonoSub',
  },
};

export default en;
