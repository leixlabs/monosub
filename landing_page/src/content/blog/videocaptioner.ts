import type { Post } from './types';

/**
 * VideoCaptioner (卡卡字幕助手) — the hardest comparison to write honestly.
 *
 * It is free, open source, GPL-3.0, and overlaps MonoSub's core job almost
 * exactly: transcribe, segment, translate, and hand back SRT. For a Chinese
 * creator the honest answer is often "VideoCaptioner costs nothing, start there".
 * Saying so is the point of the page — the alternatives a reader will find on
 * their own anyway, so pretending otherwise just loses their trust.
 */
export const videocaptioner: Post = {
  slug: 'videocaptioner',
  category: 'comparison',
  competitor: 'VideoCaptioner',
  tagline: {
    zh: '免费开源 · 需配置流水线',
    en: 'Free & open source · pipeline setup required',
  },
  teaser: {
    zh: '它免费、开源，能力还很强。所以这篇主要在讲：免费的那部分代价落在谁身上。',
    en: 'It is free, open source and genuinely capable — so this post is about where the free part costs you.',
  },
  zh: {
    title: 'MonoSub 与 VideoCaptioner（卡卡字幕助手）对比：免费开源与买断桌面版',
    metaTitle: 'MonoSub vs VideoCaptioner：免费开源字幕工具够用吗（2026）',
    description:
      'VideoCaptioner 免费开源、功能惊人地全；MonoSub 是买断的桌面应用。两者都能本地出字幕，差别在配置成本、依赖边界和稳定性由谁负责。逐项对比。',
    updated: '2026-10-27',
    intro:
      '这是最难写的一篇对比，因为 VideoCaptioner 是一个很强的免费工具：它把语音识别、语义断句、LLM 润色、翻译、字幕烧录串成一条完整流水线，GPL-3.0 开源，GitHub 上万级 star。如果你的第一诉求是"零成本"，它就是答案，本文也不会绕弯子。这篇真正要讨论的是：当它免费的时候，代价落在哪里。',
    blocks: [
      { kind: 'h2', text: '先给结论' },
      {
        kind: 'p',
        text: '如果你愿意动手、能接受"自己配置、自己排错"，VideoCaptioner 的价值远超它的价格——它是免费的，且能力密度很高。MonoSub 卖的不是功能更多，而是把这些能力打包成一个不用配置、不用装运行时、出问题有人管（且有可追溯的版本）的桌面应用。对一部分人这值 ¥48，对另一部分人不值，两种判断都合理。',
      },
      { kind: 'h2', text: '关键差异一览' },
      {
        kind: 'table',
        headers: ['', 'MonoSub', 'VideoCaptioner'],
        rows: [
          ['价格', '¥48 / $9.99 买断（Windows）', '免费'],
          ['许可', '源码可见（PolyForm Small Business）', 'GPL-3.0 开源'],
          ['形态', '原生桌面应用（Tauri）', 'Python 应用，含 GUI 与 CLI'],
          ['运行环境', '无需 Python，内置推理引擎', '需要 Python 环境（或用打包安装包）'],
          ['默认离线', '✓ 全程本地，含翻译', '✓ 识别可本地；LLM 功能走云端 API'],
          ['语音识别', '46 个 ASR 模型，含 FunASR / 通义系', 'faster-whisper、whisper.cpp、必剪、剪映等'],
          ['字幕翻译', '本地翻译模型（离线、不计费）', 'LLM 翻译（需 API Key）或必应/谷歌（免费）'],
          ['说话人分离', '✓ 独立模型', '— 非主线能力'],
          ['词级时间戳', '✓ 另写出词/静音文件', '✓'],
          ['批量处理', '✓ 队列 + 并发控制', '✓ 并发处理'],
          ['字幕烧录进视频', '✗ 只出字幕文件', '✓ 软/硬字幕合成'],
          ['配音生成', '✗', '✓ 按字幕生成配音轨'],
          ['链接导入', '✓ 取平台字幕或下音频', '✓ YouTube / B站等'],
          ['导出格式', 'SRT / VTT / TXT / MD / JSON', 'SRT / ASS / VTT'],
          ['安装方式', '微软商店一键（签名）', '安装包 / pip / 源码'],
          ['自动更新', '✓（商店版）', '—'],
          ['支持渠道', '商店购买、有维护方', 'GitHub Issues，社区维护'],
        ],
        note: 'VideoCaptioner 的能力与许可核对于 2026-10-27，依据其 GitHub 仓库与文档。它是快速迭代的开源项目，具体功能请以仓库当前版本为准。',
      },

      { kind: 'h2', text: 'VideoCaptioner 更强的地方' },
      {
        kind: 'callout',
        title: '这几项 MonoSub 做不到',
        text: '如果你的需求落在下面任何一条，直接去用 VideoCaptioner，没有理由为 MonoSub 付费。',
      },
      { kind: 'h3', text: '1. 它是免费的，而且真的不弱' },
      {
        kind: 'p',
        text: '这一点必须放在最前面。免费不等于简陋：词级时间戳、VAD 语音活动检测、语义断句、上下文翻译、批量并发，这些它都有，而且默认免费路径（必剪识别 + 必应翻译）不需要任何 API Key。对预算敏感的个人创作者，这是非常实在的选择。',
      },
      { kind: 'h3', text: '2. 一条龙到成片' },
      {
        kind: 'p',
        text: 'MonoSub 到 SRT 就结束了，压制和配音是你的剪辑软件的事。VideoCaptioner 能把字幕烧进视频、也能按字幕生成配音音轨——如果你的产出直接就是"带字幕的成片"，少一个环节就少一次折腾。',
      },
      { kind: 'h3', text: '3. LLM 润色字幕文本' },
      {
        kind: 'p',
        text: '它用大模型做断句和文案润色，还能配术语表。MonoSub 不做这一步：它输出的是识别结果本身。如果你在意字幕的"可读性"而不只是"准确率"，这个差异是实质性的——当然代价是需要一个 LLM API。',
      },
      { kind: 'h3', text: '4. 开源可审计' },
      {
        kind: 'p',
        text: 'GPL-3.0 意味着每一行都能看、能改、能自己编译。对需要确认"这个工具到底把数据发去哪了"的人，这比任何隐私声明都有说服力。MonoSub 是源码可见许可，可以读，但分发和商用有限制，不是 OSI 意义上的开源。',
      },

      { kind: 'h2', text: 'MonoSub 更强的地方' },
      { kind: 'h3', text: '1. 没有环境要配' },
      {
        kind: 'p',
        text: 'VideoCaptioner 是 Python 应用。虽然提供打包好的安装包，但一旦涉及本地模型、CUDA 加速、字幕合成的 ffmpeg 参数，出问题时的排查对象是 Python 生态。MonoSub 把推理引擎作为预编译二进制内置，装完即用，不需要 Python，也不需要自己解决依赖冲突。',
      },
      { kind: 'h3', text: '2. 翻译不产生 API 费用，也不需要联网' },
      {
        kind: 'p',
        text: 'MonoSub 的翻译用随包的本地翻译模型，跑完不产生任何按字符计费，处理敏感内容时也不需要把字幕发到任何地方。VideoCaptioner 的免费翻译走必应/谷歌（联网、有速率与稳定性问题），要质量就得上 LLM API（按 token 计费）。长期大量处理时，这笔账会变得具体。',
      },
      { kind: 'h3', text: '3. 签名安装与自动更新' },
      {
        kind: 'p',
        text: 'Windows 版通过微软商店分发：商店签名、一键安装、后台自动更新，不需要处理 SmartScreen 警告，也不需要每次手动下载新版本。这条路本身就是 ¥48 里的一部分。',
      },
      { kind: 'h3', text: '4. 队列是为一件事设计的' },
      {
        kind: 'p',
        text: 'MonoSub 的界面只有一件事：让一堆视频按顺序变成字幕。每行独立开关说话人分离与翻译、按设定并发跑、每步各自进度。VideoCaptioner 的 GUI 覆盖的流程更多，也因此操作面更宽；如果你只想批量出字幕，窄的界面反而更快。',
      },

      { kind: 'h2', text: '一个诚实的成本对比' },
      {
        kind: 'p',
        text: '把"免费"和"¥48"放在一起比，容易忽略时间成本。一个具体的算法：如果配置 Python 环境、调通本地模型、处理一次字幕合成报错合计花掉你两小时，而你的时间按 ¥100/小时计，这一次就已经超过 MonoSub 的价格。反过来，如果你本来就熟 Python，也享受调工具的过程，那这个成本接近于零，VideoCaptioner 显然是更优解。',
      },
      {
        kind: 'p',
        text: '另一侧的成本是 API：如果每月用 LLM 翻译十万字，按主流价格算是几元到几十元不等，取决于模型与用量。这个数字 Monthly 都会发生，而 MonoSub 的 ¥48 只发生一次。处理量越大，本地翻译模型的价值越明显——前提是你能接受它在质量上不如大模型。',
      },

      { kind: 'h2', text: '怎么选' },
      {
        kind: 'table',
        headers: ['你的情况', '建议'],
        rows: [
          ['预算为零，能接受折腾', 'VideoCaptioner'],
          ['需要把字幕烧进成片或生成配音', 'VideoCaptioner'],
          ['要用 LLM 润色字幕、维护术语表', 'VideoCaptioner'],
          ['不想装 Python、不想调依赖', 'MonoSub'],
          ['处理敏感内容，翻译也不能联网', 'MonoSub'],
          ['要长期批量跑，不想累积 API 账单', 'MonoSub'],
          ['希望有签名安装和自动更新', 'MonoSub'],
          ['要说话人分离（访谈、会议、播客）', 'MonoSub'],
        ],
      },
    ],
    verdict:
      'VideoCaptioner 是一个值得尊重的项目，免费、开源、能力密度高，很多人的正确答案就是它。MonoSub 的 ¥48 买的不是更多功能，而是"不用配置、不用维护、不用管依赖，且翻译不产生持续账单"。愿意动手就用前者，愿意付钱省事就用后者——两者都不是错的。',
  },

  en: {
    title: 'MonoSub vs VideoCaptioner: free and open source, or a paid desktop app',
    metaTitle: 'MonoSub vs VideoCaptioner: is the free open-source tool enough? (2026)',
    description:
      'VideoCaptioner is free, open source and remarkably complete; MonoSub is a paid desktop app. Both make local subtitles — the difference is who owns setup.',
    updated: '2026-10-27',
    intro:
      'This is the hardest comparison on the site to write honestly, because VideoCaptioner is a genuinely strong free tool: speech recognition, semantic segmentation, LLM cleanup, translation and subtitle burn-in in one pipeline, GPL-3.0, tens of thousands of GitHub stars. If your first requirement is "costs nothing", it is the answer, and this page will not dance around that. What is worth discussing is where the cost lands when the software is free.',
    blocks: [
      { kind: 'h2', text: 'The short answer' },
      {
        kind: 'p',
        text: 'If you are willing to tinker — to configure, to debug, to own the setup — VideoCaptioner is worth far more than its price, because its price is zero and its capability density is high. MonoSub is not selling more features. It sells those capabilities packaged as a desktop app with nothing to configure, no runtime to install, a support channel, and traceable versions. For some people that is worth ¥48; for others it is not, and both judgments are reasonable.',
      },
      { kind: 'h2', text: 'The differences that matter' },
      {
        kind: 'table',
        headers: ['', 'MonoSub', 'VideoCaptioner'],
        rows: [
          ['Price', '¥48 / $9.99 one-time (Windows)', 'Free'],
          ['Licence', 'Source-available (PolyForm Small Business)', 'GPL-3.0 open source'],
          ['Form', 'Native desktop app (Tauri)', 'Python application, GUI and CLI'],
          ['Runtime', 'No Python; inference engines bundled', 'Python environment (or a packaged installer)'],
          ['Offline by default', '✓ Including translation', '✓ Local recognition; LLM features call a cloud API'],
          ['Speech recognition', '46 ASR models, incl. FunASR / Tongyi family', 'faster-whisper, whisper.cpp, Bijian, Jianying and more'],
          ['Subtitle translation', 'Local translation model (offline, unmetered)', 'LLM translation (API key) or Bing/Google (free)'],
          ['Speaker diarization', '✓ Dedicated model', '— Not a core capability'],
          ['Word-level timestamps', '✓ Also writes word/silence files', '✓'],
          ['Batch processing', '✓ Queue with concurrency control', '✓ Concurrent processing'],
          ['Burn subtitles into video', '✗ Subtitle files only', '✓ Soft and hard subtitles'],
          ['Dubbing generation', '✗', '✓ Builds a voice track from subtitles'],
          ['URL import', '✓ Platform subtitles, else download audio', '✓ YouTube, Bilibili and more'],
          ['Export formats', 'SRT / VTT / TXT / MD / JSON', 'SRT / ASS / VTT'],
          ['Install', 'One click via Microsoft Store (signed)', 'Installer / pip / from source'],
          ['Auto-update', '✓ (Store build)', '—'],
          ['Support', 'Store purchase, maintained vendor', 'GitHub Issues, community maintained'],
        ],
        note: 'VideoCaptioner capabilities and licence checked 2026-10-27 against its GitHub repository and docs. It is a fast-moving open-source project — check the current release for specifics.',
      },

      { kind: 'h2', text: 'Where VideoCaptioner is stronger' },
      {
        kind: 'callout',
        title: 'Things MonoSub cannot do',
        text: 'If your need falls under any of these, go use VideoCaptioner. There is no reason to pay for MonoSub.',
      },
      { kind: 'h3', text: '1. It is free, and genuinely capable' },
      {
        kind: 'p',
        text: 'This belongs first. Free does not mean thin: word-level timestamps, VAD, semantic segmentation, context-aware translation and batch concurrency are all there, and the default free path (Bijian recognition + Bing translation) needs no API key at all. For a budget-conscious individual creator this is a very real option.',
      },
      { kind: 'h3', text: '2. It goes all the way to a finished video' },
      {
        kind: 'p',
        text: 'MonoSub stops at the SRT; burning in and dubbing belong to your editor. VideoCaptioner can burn subtitles into the video and generate a dubbing track from the subtitles. If your deliverable is directly "a captioned video", one fewer hop is one fewer thing to go wrong.',
      },
      { kind: 'h3', text: '3. LLM cleanup of the subtitle text' },
      {
        kind: 'p',
        text: 'It uses a large model for sentence segmentation and copy polish, with glossary support. MonoSub does not do this step — it outputs the recognition result itself. If you care about how readable the subtitles are, not just how accurate, the difference is substantive. The trade-off is that you need an LLM API.',
      },
      { kind: 'h3', text: '4. Open source, auditable' },
      {
        kind: 'p',
        text: 'GPL-3.0 means every line can be read, changed and compiled. For anyone who wants to verify where a tool actually sends data, that beats any privacy statement. MonoSub is source-available: readable, but with restrictions on redistribution and commercial use — not open source in the OSI sense.',
      },

      { kind: 'h2', text: 'Where MonoSub is stronger' },
      { kind: 'h3', text: '1. Nothing to set up' },
      {
        kind: 'p',
        text: 'VideoCaptioner is a Python application. Packaged installers exist, but the moment local models, CUDA acceleration or ffmpeg arguments for burn-in are involved, the debugging surface is the Python ecosystem. MonoSub ships its inference engines as prebuilt binaries — install and go, no Python, no dependency conflicts to resolve.',
      },
      { kind: 'h3', text: '2. Translation with no API bill and no connection' },
      {
        kind: 'p',
        text: 'MonoSub translates with a bundled local model: no per-character billing, and sensitive subtitles never leave the machine. VideoCaptioner’s free translation goes through Bing/Google (online, with rate limits and reliability caveats), and quality means an LLM API billed per token. At sustained volume that becomes a concrete number.',
      },
      { kind: 'h3', text: '3. Signed install and automatic updates' },
      {
        kind: 'p',
        text: 'The Windows build ships through the Microsoft Store: signed, one-click, updated in the background. No SmartScreen warnings to talk a user through, no manual re-download each release. That distribution path is part of what the ¥48 buys.',
      },
      { kind: 'h3', text: '4. The queue is designed for exactly one job' },
      {
        kind: 'p',
        text: 'MonoSub’s interface does one thing: turn a stack of videos into subtitles, in order. Per-row diarization and translation toggles, a concurrency setting, separate progress per step. VideoCaptioner’s GUI covers more of the pipeline and therefore presents more surface. If all you want is batch subtitles, the narrower interface is faster.',
      },

      { kind: 'h2', text: 'An honest cost comparison' },
      {
        kind: 'p',
        text: 'Comparing "free" with "¥48" invites ignoring time. A concrete way to count it: if setting up Python, getting a local model working and clearing one burn-in error costs you two hours, and your time is worth ¥100/hour, that single session already exceeds MonoSub’s price. Conversely, if you already know Python and enjoy tuning tools, that cost is near zero and VideoCaptioner is plainly the better choice.',
      },
      {
        kind: 'p',
        text: 'The other side is API spend: translating a hundred thousand characters a month through an LLM costs somewhere between a few and a few dozen yuan depending on model and volume. That recurs monthly; MonoSub’s ¥48 happens once. The heavier your throughput, the more the local translation model is worth — assuming you accept it is not as good as a large model.',
      },

      { kind: 'h2', text: 'How to choose' },
      {
        kind: 'table',
        headers: ['Your situation', 'Pick'],
        rows: [
          ['Zero budget, happy to tinker', 'VideoCaptioner'],
          ['You need burned-in subtitles or dubbing', 'VideoCaptioner'],
          ['You want LLM polish and a glossary', 'VideoCaptioner'],
          ['You will not install Python or chase dependencies', 'MonoSub'],
          ['Sensitive material — translation must stay offline', 'MonoSub'],
          ['Sustained batch volume without an accumulating API bill', 'MonoSub'],
          ['You want a signed install and auto-updates', 'MonoSub'],
          ['You need speaker diarization (interviews, meetings, podcasts)', 'MonoSub'],
        ],
      },
    ],
    verdict:
      'VideoCaptioner is a project worth respecting: free, open source, dense with capability, and for many people the correct answer. MonoSub’s ¥48 does not buy more features — it buys not having to configure, maintain or babysit dependencies, plus translation that generates no recurring bill. Tinker and use the former; pay to skip it and use the latter. Neither is wrong.',
  },
};
