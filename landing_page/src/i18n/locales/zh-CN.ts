import type { Translation } from '../index';

const zhCN: Translation = {
  lang_name: '中文（简体）',

  page_title: 'MonoSub — 本地 AI 字幕生成 · 视频转字幕不上传 · Mac 与 Windows',
  page_description:
    '在自己的 Mac 或 PC 上把视频和音频转成字幕：不上传、不需要账号、没有订阅。免费下载，批量转写用一次性 Pro 解锁。',
  og_alt: 'MonoSub 本地字幕应用在 Mac 上的界面',

  nav: {
    features: '能做什么',
    how: '怎么用',
    pricing: '价格',
    faq: '问题',
    blog: '博客',
    cta_primary: '免费下载',
    cta_secondary: '获取 Pro',
  },

  hero: {
    eyebrow: '本地优先的字幕工具',
    title: '把视频变成字幕。',
    title_accent: '不上传，就在本机。',
    subtitle:
      'MonoSub 的语音识别与翻译完全在你的 Mac 或 PC 上运行。素材从不上传——不需要账号，没有订阅，也没有按分钟计费。',
    cta_primary: '免费下载（Mac）',
    cta_secondary: '查看所有下载方式',
    note: '免费开始使用；批量转写用一次性 Pro 授权解锁。',
    trust: ['100% 本机运行', '免费下载', '一次性 Pro，永不订阅'],
    shot_caption: '主界面——拖入文件或粘贴链接。',
  },

  answer: {
    eyebrow: '先说结论',
    title: '可以——不上传文件，也能生成字幕。',
    body: 'MonoSub 是一个在你本机完成转录和翻译的桌面应用。拖入文件、选好模型，就能拿到 SRT、VTT、TXT、Markdown 或 JSON。语音引擎和翻译模型都在你自己的硬件上运行，所以文件不上传，模型下载完成后也无需联网。',
    stats: [
      { value: '0', label: '上传字节——文件始终留在你的磁盘上' },
      { value: '5', label: '种导出格式：SRT、VTT、TXT、MD、JSON' },
      { value: '1×', label: '一次性 Pro 解锁，没有月度账单' },
    ],
  },

  why: {
    eyebrow: '01 — 为什么本地',
    q: '为什么不直接用云端转写服务？',
    a: '云端工具在文件不重要时很方便；但一旦它重要起来——未公开的视频、客户访谈、内部会议——上传就意味着它离开你的电脑，停在别人的服务器上，而按分钟计费也会悄悄变成订阅。MonoSub 去掉了上传和计时器：活干在文件本来就在的地方。',
    cloud_title: '云端转写',
    local_title: 'MonoSub',
    cloud: [
      '文件要上传到第三方服务器',
      '按分钟计费，或一份你忘了的月付套餐',
      '依赖网络，还要排在别人后面',
      '无法确认上传的文件何时被删除',
    ],
    local: [
      '文件只在你磁盘上读取，绝不上传',
      '单文件免费；Pro 是一次性解锁',
      '模型下载完成后完全离线可用',
      'Apple Silicon 与 NVIDIA GPU 硬件加速',
    ],
  },

  how: {
    eyebrow: '02 — 怎么用',
    title: '三步，没有一步需要上传。',
    subtitle: 'macOS 和 Windows 走同一套流程：识别、可选翻译、导出，全部在本机完成。',
    steps: [
      {
        title: '下载安装',
        desc: '从 App Store 或微软商店获取，也可以直接用免费直装版。它就是普通桌面应用——不需要账号，不用登录。',
      },
      {
        title: '把文件拖进来',
        desc: '拖入一个视频，或一整个文件夹。免费版一次处理一个文件；Pro 解除这个限制，让队列并行跑完。',
      },
      {
        title: '把字幕导出去',
        desc: '导出 SRT、VTT、TXT、Markdown 或 JSON 词级文件——可选翻译成第二种语言，并与原文件并排写出。',
      },
    ],
    note: '首次运行会从「设置 → 模型」下载一个你选定的语音模型（约 250 MB）。之后转录完全离线。',
  },

  showcase: {
    eyebrow: '03 — 看界面',
    title: '直接看真实的界面。',
    subtitle: '下面每一张都是 MonoSub 的真实截图，不是效果图。',
    tours: [
      {
        id: 'main',
        label: '主界面',
        desc: '拖入文件，或粘贴一条链接。你看到的一切都在本机运行——不上传、不需要账号、没有订阅。',
        shots: ['浅色', '深色'],
      },
      {
        id: 'process',
        label: '处理队列',
        desc: '拖入一整个文件夹。每一行都有自己的说话人分离与翻译开关；Pro 让队列并行处理。',
        shots: ['浅色', '深色'],
      },
      {
        id: 'settings',
        label: '设置',
        desc: '文件与格式、模型、翻译、链接导入都在一处。只下载你真正需要的语音与翻译模型。',
        shots: ['浅色', '深色'],
      },
    ],
  },

  features: {
    eyebrow: '04 — 能做什么',
    title: '能力，没有小字条件。',
    subtitle: '下面每一项功能都在免费下载里。Pro 只改变你一次能处理几个文件。',
    items: [
      {
        title: '100% 本地处理',
        desc: '语音识别与字幕翻译全部在本机完成，数据绝不离开你的电脑，隐私完全由你掌控。',
      },
      {
        title: '字幕本地翻译',
        desc: '内置本地翻译模型，一键把字幕翻译为中文、英文等多语言，全程离线、无需额外付费。',
      },
      {
        title: '说话人分离',
        desc: '逐条字幕自动区分「谁在何时说话」，适合访谈、会议、播客等多人场景。',
      },
      {
        title: '硬件加速',
        desc: 'Apple Silicon 上走 Metal，Windows 上是同一套推理引擎，本地转写与翻译速度更快。',
      },
      {
        title: '链接导入',
        desc: '粘贴受支持的视频链接即可取用平台字幕；没有字幕时，会下载音频并在本地转写。',
      },
      {
        title: '词级时间戳',
        desc: '按词对齐生成字幕时间轴，并额外写出词/静音文件，可以直接按文字剪视频。',
      },
      {
        title: '全部导出格式',
        desc: '导出 SRT、VTT、TXT、MD、JSON，兼容主流视频编辑软件、播放器与 LLM 流程。',
      },
      {
        title: '批量处理',
        desc: '把整个素材库排进队列一次跑完。免费版一次一个文件；Pro 让它们并行处理。',
      },
    ],
  },

  pricing: {
    eyebrow: '05 — 价格',
    title: '下载免费，只有批量时才需要 Pro。',
    subtitle: '没有订阅、没有按分钟计费、不需要账号。需要同时处理多个文件时，一次性购买 Pro。',
    plans: [
      {
        id: 'free',
        name: '免费版',
        badge: '永久免费',
        price: '¥0',
        priceNote: '不用绑卡、不用账号',
        summary: '一次一个文件。',
        features: [
          '完整的转录与翻译引擎',
          '全部导出格式：SRT、VTT、TXT、MD、JSON',
          '说话人分离与词级时间戳',
          '队列中同时只处理一个文件',
        ],
        cta: '免费下载',
      },
      {
        id: 'pro',
        name: 'MonoSub Pro',
        badge: '一次性解锁',
        price: '¥48',
        priceNote: 'App Store 内购 ¥48 / $9.99 · 或直营店 ¥36 / $8.60',
        summary: '一次一个文件夹。',
        features: ['免费版的全部功能', '队列不限文件数量', '整个素材库并行批量处理', '免费的小版本更新'],
        cta: '解锁 Pro',
      },
    ],
    proNoteTitle: 'Pro 到底解锁了什么',
    proNote:
      '只有一件事：单文件限制。所有能力——说话人分离、词级时间戳、翻译、全部导出格式——免费版都有。Pro 是给「处理一整个文件夹」的人，而不是「处理一个文件」的人。',
    channelsTitle: '在哪儿获取',
    channelsNote:
      '价格以美元标示，App Store 与微软商店会按你所在地区结算。App Store 版已在美国及多数地区上架，中国大陆区仍在备案中。',
    columns: { platform: '平台', price: '价格', pro: 'Pro 怎么算' },
    channels: [
      {
        id: 'appstore',
        platform: 'macOS · App Store',
        price: '免费',
        pro: '内购 · ¥48 / $9.99',
        note: '中国区备案中',
      },
      {
        id: 'direct',
        platform: 'macOS / Windows · 直营店',
        price: '免费直装',
        pro: '离线授权 · ¥36 / $8.60',
        note: '授权不绑设备，两平台通用',
      },
      {
        id: 'windows',
        platform: 'Windows · 微软商店',
        price: '免费',
        pro: '内购 · ¥48 / $9.99',
        note: '签名、一键安装、自动更新',
      },
      {
        id: 'source',
        platform: '任意平台 · 源码构建',
        price: '免费',
        pro: '—',
        note: '源码可见许可',
      },
    ],
  },

  faq: {
    eyebrow: '问题',
    title: '大家真正会问的问题。',
    subtitle: '简短回答。没覆盖到的，博客里有更深的对比。',
    items: [
      {
        q: 'MonoSub 会上传我的文件吗？',
        a: '不会。语音识别与翻译都在你自己的机器上运行，音视频从不离开电脑。唯一的网络流量是你主动下载的模型文件，以及你粘贴的视频链接。',
      },
      {
        q: '真的免费吗？',
        a: '是。完整引擎免费下载、免费使用，一次一个文件——转录、翻译、说话人分离和全部导出格式都包含。Pro 是一次性购买，只用来解除单文件限制。',
      },
      {
        q: 'Pro 多少钱，是订阅吗？',
        a: 'Pro 是一次性购买：App Store 内购 ¥48 / $9.99，或直营店 checkout.leix.dev 的离线授权 ¥36 / $8.60。没有订阅，也没有按分钟计费。',
      },
      {
        q: '支持哪些平台？',
        a: 'macOS 12+（Apple Silicon）通过 App Store；Windows 10/11（x64）通过微软商店；macOS 另有一份免费构建在 GitHub Releases。',
      },
      {
        q: '需要联网吗？',
        a: '转录不需要。从「设置 → 模型」下载一次语音模型（约 250 MB），之后应用完全离线工作。',
      },
      {
        q: '支持哪些语言和文件格式？',
        a: '中文（普通话）、英文等，自动识别。视频音频都行——MP4、MOV、MKV、AVI、MP3、WAV、M4A、FLAC 等。导出 SRT、VTT、TXT、Markdown 或 JSON。',
      },
      {
        q: '能一次跑一整个文件夹吗？',
        a: '可以，用 Pro。免费版一次处理一个文件；Pro 解除限制，按你设定的并发数并行跑队列。批量处理是 Pro 唯一改变的东西。',
      },
      {
        q: 'App Store 版在中国能下载吗？',
        a: 'macOS 版已在美国及多数地区上架，中国大陆区仍在备案中。在此之前，直营店 checkout.leix.dev 的离线授权在中国可用，Windows 版在微软商店。',
      },
    ],
  },

  final: {
    title: '视频本来就在你电脑上，文本也留在那儿。',
    subtitle: '免费下载，几分钟就能转出第一份字幕。需要批量时再解锁 Pro。',
    cta_primary: '免费下载（Mac）',
    cta_secondary: '购买 Pro · ¥36 / $8.60',
    trust: ['免费下载', '无需账号', '一次性 Pro'],
  },

  footer: {
    tagline: '本地 AI 字幕，文件始终属于你。',
    blog: '博客',
    privacy: '隐私政策',
    contact: '联系我们',
    terms: '服务条款',
    copyright: '© 2026 MonoSub',
  },
};

export default zhCN;
