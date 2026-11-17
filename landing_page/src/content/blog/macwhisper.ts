import type { Post } from './types';

/**
 * MacWhisper — the comparison MonoSub gets asked about most.
 *
 * Written honestly on purpose: MacWhisper wins on dictation, meeting capture,
 * integrations and export breadth, and the post says so. A comparison page that
 * pretends otherwise is worth nothing to a reader who has used both, and search
 * engines can tell thin affiliate copy from a real one.
 */
export const macwhisper: Post = {
  slug: 'macwhisper',
  category: 'comparison',
  competitor: 'MacWhisper',
  tagline: {
    zh: 'macOS 独占 · €64 买断',
    en: 'macOS only · €64 one-time',
  },
  teaser: {
    zh: '同为买断制本地转写，一个只做 macOS，一个覆盖 Windows。差别在平台，也在你要的是转写稿还是字幕。',
    en: 'Two one-time-purchase local transcribers: one is macOS-only, one covers Windows. The split is platform — and transcript versus subtitles.',
  },
  zh: {
    title: 'MonoSub 与 MacWhisper 对比：同为买断制本地转写，差在哪',
    metaTitle: 'MonoSub vs MacWhisper：本地字幕工具怎么选（2026）',
    description:
      'MacWhisper 是 macOS 上最成熟的本地转写应用，MonoSub 覆盖 Windows 与 macOS。价格、平台、说话人分离、字幕翻译逐项对比，包括 MonoSub 不如 MacWhisper 的地方。',
    updated: '2026-10-27',
    intro:
      '这两个工具解决同一个问题：不上传、不订阅，在你自己的电脑上把音视频变成字幕。选择的关键其实只有两条——你在什么系统上工作，以及你要的是"转写稿"还是"可以直接用的字幕文件"。',
    blocks: [
      { kind: 'h2', text: '一句话结论' },
      {
        kind: 'p',
        text: 'MacWhisper 是 macOS 上功能最全的转写应用，尤其是口述、会议录音和第三方服务集成，这些 MonoSub 都不做。MonoSub 的差异在两点：它同时支持 Windows，并且把"生成一份可以直接拖进剪辑软件的字幕"当作主线，而不是转写稿的副产品。',
      },
      { kind: 'h2', text: '关键差异一览' },
      {
        kind: 'table',
        headers: ['', 'MonoSub', 'MacWhisper'],
        rows: [
          ['Windows', '✓ 微软商店，签名安装', '✗ 不支持'],
          ['macOS', '✓ Apple Silicon，GitHub Releases', '✓ Intel 与 Apple Silicon'],
          ['价格', '免费 / Pro ¥48 一次性', '€64 买断（Pro）'],
          ['免费额度', '免费下载；单文件免费', '免费版可用，小模型'],
          ['离线处理', '✓ 全程本地', '✓ 全程本地'],
          ['说话人分离', '✓ 独立模型，队列中逐文件开关', '✓（标注 beta）'],
          ['字幕翻译', '✓ 本地翻译模型，离线', '✓（可接本地或云端模型）'],
          ['词级时间戳', '✓ 另写出词/静音文件', '✓'],
          ['批量队列', '✓ 每行独立开关，可设并发', '✓ 批量文件夹'],
          ['链接导入', '✓ 取平台字幕，无字幕则下音频转写', '✓ YouTube 等'],
          ['导出格式', 'SRT / VTT / TXT / MD / JSON', 'SRT / VTT / TXT / DOCX / PDF / HTML 等'],
          ['实时口述', '✗ 不做', '✓ 系统级口述'],
          ['会议自动录制', '✗ 不做', '✓ Zoom / Teams / Meet 等'],
          ['第三方集成', '✗ 不做', '✓ Notion / Zapier / Obsidian / webhook'],
          ['命令行', '✓ monosub-cli', '✓ CLI'],
          ['系统要求', 'Windows 10/11 x64；macOS 11+', 'macOS'],
        ],
        note: '价格核对于 2026-10-27。MacWhisper 另有 Mac App Store 版本（Whisper Transcription），采用订阅或较高价买断，与本文对比的官网直装版不是同一套定价。',
      },

      { kind: 'h2', text: 'MonoSub 在哪里更合适' },
      { kind: 'h3', text: '1. 你在 Windows 上' },
      {
        kind: 'p',
        text: '这是最直接的一条。MacWhisper 至今没有 Windows 版本，也没有公布过计划。如果你的主力机器是 Windows，剩下的选项基本是：跑在浏览器里的云服务（要上传）、开源脚本（要装 Python）、或者 MonoSub。',
      },
      { kind: 'h3', text: '2. 你要的是字幕，不是逐字稿' },
      {
        kind: 'p',
        text: 'MacWhisper 的出发点是转写：拿到文字，再决定要导出成什么。MonoSub 的出发点是字幕——队列里每一行代表一个待出字幕的视频，每行有自己的说话人分离与翻译开关，跑完直接产出 SRT。如果你一次要处理二十个视频，这两种心智模型的差别会很明显。',
      },
      { kind: 'h3', text: '3. 翻译要离线、不额外计费' },
      {
        kind: 'p',
        text: 'MonoSub 内置本地翻译模型，翻译这一步同样不联网、不按字符计费。MacWhisper 的 AI 功能主要走 BYOK（自带 API Key 接 OpenAI、Anthropic 等），也可以接 Ollama 本地模型——灵活，但要么产生 API 费用，要么需要你自己把本地模型配起来。',
      },
      { kind: 'h3', text: '4. 价格' },
      {
        kind: 'p',
        text: 'MonoSub 本体免费下载，单文件转换免费；批量用的 Pro 是一次性购买：Mac App Store 内购 $9.99 / ¥48，直营店离线授权 ¥36 / $8.60，Windows 商店版 ¥48 / $9.99 已含 Pro。MacWhisper Pro €64（约合 $69），同样是买断、没有订阅。价差本身不算决定性，但如果你的需求就是批量出字幕，多付的部分买的是 MonoSub 并不提供的那些功能。',
      },

      { kind: 'h2', text: 'MacWhisper 在哪里更合适' },
      {
        kind: 'callout',
        title: '这些是 MonoSub 不做的事',
        text: '下面四项是 MacWhisper 的强项，也是 MonoSub 明确不打算做的方向。如果你的工作流落在这里，应该直接选 MacWhisper。',
      },
      { kind: 'h3', text: '1. 系统级口述' },
      {
        kind: 'p',
        text: '在任意输入框里按住快捷键说话，文字直接落进去。这是 MacWhisper 最受欢迎的功能之一，和"给视频配字幕"是两个完全不同的场景。MonoSub 只处理文件，不做输入法。',
      },
      { kind: 'h3', text: '2. 会议录音' },
      {
        kind: 'p',
        text: 'MacWhisper 能检测并录制 Zoom / Teams / Meet 会议，自动开始结束。MonoSub 不会接管你的会议——它只在你有文件之后才开始工作。',
      },
      { kind: 'h3', text: '3. 与其它工具打通' },
      {
        kind: 'p',
        text: 'Notion、Obsidian、Zapier、n8n、自定义 webhook，转写完自动推送。MonoSub 的输出停在磁盘上，后续由你自己接。',
      },
      { kind: 'h3', text: '4. 导出格式更多' },
      {
        kind: 'p',
        text: '如果你的下游是 Word 或 PDF 文档，MacWhisper 的 DOCX / PDF 导出更顺手。MonoSub 的 MD 与 JSON 更偏工程用途。',
      },

      { kind: 'h2', text: '两者的共同点' },
      {
        kind: 'ul',
        items: [
          '都是买断制，没有强制订阅',
          '都在本地跑推理，媒体文件不出本机',
          '都支持说话人分离，且都把它当作附加能力而不是默认开启',
          '都提供命令行入口，可以嵌进脚本',
          '都依赖你下载模型文件（首次使用需要联网）',
        ],
      },

      { kind: 'h2', text: '性能怎么比' },
      {
        kind: 'p',
        text: '两个工具都可以跑 Whisper 系模型，也都能用上 Apple Silicon 的加速，所以"谁更快"很大程度上取决于你选了哪个模型、多大的模型。这里不做没有实测数据的排名——转写速度对模型大小、量化等级和音频时长的敏感程度，远超过两个应用之间封装层的差异。',
      },
      {
        kind: 'p',
        text: '可以确定的是：MonoSub 的模型方案是按需求求解的（要速度、要词级时间戳、要说话人分离、要特定语言，各选不同模型），Tongyi / FunASR 系在中文上的表现是这套方案的重点。MacWhisper 的选择面集中在 Whisper 家族的各档位。如果你主要处理中文内容，值得两边都用同一段素材试一次再做决定。',
      },

      { kind: 'h2', text: '怎么选' },
      {
        kind: 'table',
        headers: ['你的情况', '建议'],
        rows: [
          ['主力机是 Windows', 'MonoSub——MacWhisper 没有 Windows 版'],
          ['要系统级口述 / 会议录制', 'MacWhisper'],
          ['要批量出字幕文件给剪辑用', 'MonoSub'],
          ['要转写稿推送到 Notion 等', 'MacWhisper'],
          ['要离线翻译且不想配 API', 'MonoSub'],
          ['只用 macOS，且要最全功能', 'MacWhisper（Pro）'],
          ['预算敏感，只要字幕', 'MonoSub（免费 / Pro ¥48）'],
        ],
      },
    ],
    verdict:
      '如果你在 Windows 上、要的是字幕文件，MonoSub 更对路，也更便宜。如果你在 macOS 上、要的是一个能接管口述和会议的转写中枢，MacWhisper 的功能面明显更宽，那 €64 花得合理。两者不冲突——有人两个都装。',
  },

  en: {
    title: 'MonoSub vs MacWhisper: two one-time-purchase local transcribers, compared',
    metaTitle: 'MonoSub vs MacWhisper: which local subtitle tool to pick (2026)',
    description:
      'MacWhisper is the most mature local transcriber on macOS; MonoSub also runs on Windows. Price, diarization and translation compared, including its gaps.',
    updated: '2026-10-27',
    intro:
      'Both tools solve the same problem: turn audio and video into subtitles on your own machine, with no upload and no subscription. The decision comes down to two questions — which OS you work on, and whether you want a transcript or a finished subtitle file.',
    blocks: [
      { kind: 'h2', text: 'The short answer' },
      {
        kind: 'p',
        text: 'MacWhisper is the most complete transcription app on macOS, particularly for dictation, meeting capture and third-party integrations — none of which MonoSub does. MonoSub differs in two ways: it also runs on Windows, and it treats "produce a subtitle file you can drop into an editor" as the main job rather than a by-product of a transcript.',
      },
      { kind: 'h2', text: 'The differences that matter' },
      {
        kind: 'table',
        headers: ['', 'MonoSub', 'MacWhisper'],
        rows: [
          ['Windows', '✓ Microsoft Store, signed install', '✗ Not supported'],
          ['macOS', '✓ Apple Silicon, GitHub Releases', '✓ Intel and Apple Silicon'],
          ['Price', 'Free / Pro ¥48 one-time', '€64 one-time (Pro)'],
          ['Free tier', 'Free download; single file at a time', 'Free tier, smaller models'],
          ['Offline', '✓ Everything local', '✓ Everything local'],
          ['Speaker diarization', '✓ Dedicated model, per-row toggle in the queue', '✓ (labelled beta)'],
          ['Subtitle translation', '✓ Local translation model, offline', '✓ (local or cloud models)'],
          ['Word-level timestamps', '✓ Also writes word/silence files', '✓'],
          ['Batch queue', '✓ Per-row toggles, configurable concurrency', '✓ Batch folders'],
          ['URL import', '✓ Platform subtitles, else download audio and transcribe', '✓ YouTube and others'],
          ['Export formats', 'SRT / VTT / TXT / MD / JSON', 'SRT / VTT / TXT / DOCX / PDF / HTML and more'],
          ['Live dictation', '✗ Not a feature', '✓ System-wide dictation'],
          ['Automatic meeting capture', '✗ Not a feature', '✓ Zoom / Teams / Meet and more'],
          ['Third-party integrations', '✗ Not a feature', '✓ Notion / Zapier / Obsidian / webhooks'],
          ['Command line', '✓ monosub-cli', '✓ CLI'],
          ['Requirements', 'Windows 10/11 x64; macOS 11+', 'macOS'],
        ],
        note: 'Prices checked 2026-10-27. MacWhisper also ships on the Mac App Store as Whisper Transcription, which uses subscription or higher one-time pricing — a different SKU from the direct-download version compared here.',
      },

      { kind: 'h2', text: 'Where MonoSub fits better' },
      { kind: 'h3', text: '1. You work on Windows' },
      {
        kind: 'p',
        text: 'This is the decisive one. MacWhisper has no Windows build and has not announced one. If your main machine runs Windows, the remaining options are broadly: a browser-based cloud service (upload required), an open-source script (Python required), or MonoSub.',
      },
      { kind: 'h3', text: '2. You want subtitles, not a transcript' },
      {
        kind: 'p',
        text: 'MacWhisper starts from transcription: get the text, then decide what to export. MonoSub starts from subtitles — each row in the queue is a video awaiting subtitles, with its own diarization and translation toggles, and the run ends with an SRT. If you process twenty videos at a time, that difference in mental model is noticeable.',
      },
      { kind: 'h3', text: '3. Translation that stays offline and free at the margin' },
      {
        kind: 'p',
        text: 'MonoSub bundles a local translation model, so that step is also offline and not billed per character. MacWhisper’s AI features are mostly BYOK (bring your own key for OpenAI, Anthropic, and others), or Ollama for local models — flexible, but it either costs API money or requires you to stand up a local model yourself.',
      },
      { kind: 'h3', text: '4. Price' },
      {
        kind: 'p',
        text: 'MonoSub itself is a free download, and a single file at a time is free; Pro for batch work is a one-time purchase — $9.99 / ¥48 as a Mac App Store in-app purchase, or ¥36 / $8.60 as a direct-store offline licence, with the Windows Store build at ¥48 / $9.99 including Pro. MacWhisper Pro is €64 (roughly $69), also one-time with no subscription. The gap is not decisive on its own, but if your need is batch subtitles, the extra buys features MonoSub does not offer.',
      },

      { kind: 'h2', text: 'Where MacWhisper fits better' },
      {
        kind: 'callout',
        title: 'Things MonoSub does not do',
        text: 'These four are MacWhisper strengths and directions MonoSub deliberately stays out of. If your workflow lives here, pick MacWhisper.',
      },
      { kind: 'h3', text: '1. System-wide dictation' },
      {
        kind: 'p',
        text: 'Hold a hotkey in any text field, speak, and the words land there. It is one of MacWhisper’s most popular features and a completely different job from captioning video. MonoSub only processes files; it is not an input method.',
      },
      { kind: 'h3', text: '2. Meeting recording' },
      {
        kind: 'p',
        text: 'MacWhisper detects and records Zoom / Teams / Meet sessions, starting and stopping on its own. MonoSub does not sit in your meetings — it starts once you have a file.',
      },
      { kind: 'h3', text: '3. Pushing results into other tools' },
      {
        kind: 'p',
        text: 'Notion, Obsidian, Zapier, n8n, custom webhooks — a finished transcript goes wherever you point it. MonoSub’s output stops on disk and the plumbing is yours to build.',
      },
      { kind: 'h3', text: '4. Broader export' },
      {
        kind: 'p',
        text: 'If the downstream is a Word or PDF document, MacWhisper’s DOCX / PDF export is more convenient. MonoSub’s MD and JSON lean engineering-ward.',
      },

      { kind: 'h2', text: 'What they share' },
      {
        kind: 'ul',
        items: [
          'One-time purchase, no mandatory subscription',
          'Inference runs locally; media never leaves your machine',
          'Speaker diarization on both, treated as an add-on rather than a default',
          'A command-line entry point for scripting',
          'Model files are downloaded on first use (a connection is needed once)',
        ],
      },

      { kind: 'h2', text: 'On performance' },
      {
        kind: 'p',
        text: 'Both can run Whisper-family models and both can use Apple Silicon acceleration, so "which is faster" depends far more on the model you choose and its size than on the app wrapping it. This page will not publish a ranking without measured numbers — transcription speed is more sensitive to model size, quantization and audio length than to any difference between two shells around the same engine.',
      },
      {
        kind: 'p',
        text: 'What is fair to say: MonoSub solves for a model plan per requirement (speed, word-level timestamps, diarization, a specific language each pull toward different models), and the Tongyi / FunASR family is the emphasis for Chinese. MacWhisper’s choices sit within the Whisper family across sizes. If you mostly work with Chinese audio, run the same clip through both before deciding.',
      },

      { kind: 'h2', text: 'How to choose' },
      {
        kind: 'table',
        headers: ['Your situation', 'Pick'],
        rows: [
          ['Main machine is Windows', 'MonoSub — MacWhisper has no Windows build'],
          ['You want system-wide dictation or meeting capture', 'MacWhisper'],
          ['You need batches of subtitle files for editing', 'MonoSub'],
          ['You want transcripts pushed into Notion and friends', 'MacWhisper'],
          ['You want offline translation without configuring an API', 'MonoSub'],
          ['macOS only, and you want the fullest feature set', 'MacWhisper (Pro)'],
          ['Budget matters and you only need subtitles', 'MonoSub (free / Pro ¥48)'],
        ],
      },
    ],
    verdict:
      'On Windows, wanting subtitle files, MonoSub is the better fit and the cheaper one. On macOS, wanting a transcription hub that also handles dictation and meetings, MacWhisper is plainly broader — and worth the €64. They are not mutually exclusive; some people install both.',
  },
};
