import type { Post } from './types';

/**
 * The cloud-subscription comparison, using the three services Chinese readers
 * actually compare against: 讯飞听见, 通义听悟 and 飞书妙记.
 *
 * These are one post rather than three because the argument is the same for all
 * of them — per-minute billing and an upload — and splitting it would produce
 * three thin pages competing with each other for the same query.
 */
export const cloudServices: Post = {
  slug: 'cloud-transcription',
  category: 'comparison',
  competitor: '讯飞听见 / 通义听悟 / 飞书妙记',
  tagline: {
    zh: '云端 · 按分钟计费',
    en: 'Cloud · billed per minute',
  },
  teaser: {
    zh: '免费额度够不够用，取决于你每月处理多少分钟。算一笔具体的账。',
    en: 'Whether the free tier is enough depends on your monthly minutes. Here is the arithmetic.',
  },
  zh: {
    title: 'MonoSub 与云端转写服务对比：讯飞听见、通义听悟、飞书妙记',
    metaTitle: '本地字幕 vs 云端转写：讯飞听见、通义听悟、飞书妙记怎么选（2026）',
    description:
      '云端转写按分钟计费、文件需上传。把它们和 MonoSub 放在一起算一笔账：免费额度、单次成本、隐私边界，以及什么情况下云端才是更好的选择。',
    updated: '2026-10-27',
    intro:
      '国内的转写服务已经相当成熟，免费额度也不小。所以真正该问的不是"本地是不是更先进"，而是：在你实际的处理量下，上传文件和按分钟付费，究竟是便利还是负担。这篇把三个最常见的服务放在一起，连同 MonoSub，算清楚这笔账。',
    blocks: [
      { kind: 'h2', text: '先给结论' },
      {
        kind: 'p',
        text: '每月只转写几十分钟、内容不敏感、又不想装软件，云端免费额度完全够用，没必要买任何东西。反过来，一旦你是长期、批量地处理视频，或者素材本身不能外传（未公开的课程、内部会议、客户访谈、法律与医疗录音），按分钟计费与上传这两件事就会同时变成问题。',
      },
      { kind: 'h2', text: '三个云端服务各自的样子' },
      {
        kind: 'table',
        headers: ['', '免费额度', '超出后', '特点'],
        rows: [
          ['讯飞听见', '新用户赠送时长，另有免费档', '按分钟计费，常见区间约 ¥0.3–0.5/分钟；套餐折算更低', '方言与嘈杂环境表现好，行业积累久，导出格式齐'],
          ['通义听悟', '个人每月约 5–10 小时（以官网为准）', '按分钟计费，常见区间约 ¥0.08/分钟起', '大模型总结能力强，适合课程与会议整理'],
          ['飞书妙记', '个人约 500 分钟/月', '按分钟计费', '与飞书文档、日历、任务打通，需绑定飞书账号'],
        ],
        note: '各家免费额度与单价调整频繁，以上为 2026-10-27 的公开信息区间，不构成报价。下单前请以官网为准——本页给的是量级，不是精确价格。',
      },
      {
        kind: 'p',
        text: '三者的共同点比差异更重要：都要上传文件、都按分钟或时长计量、免费额度按自然月刷新且不累积。这些是商业模式的必然结果，不是产品缺陷——云端要付算力账单，只能按用量收。',
      },

      { kind: 'h2', text: '把账算具体' },
      {
        kind: 'p',
        text: '假设你每周处理 10 个视频，平均每个 20 分钟，也就是每月约 800 分钟。按通义听悟与其同类服务的量级估算，超出免费额度后大约是每分钟 ¥0.08–0.12，一个月 800 分钟折算下来在 ¥64–96 之间。',
      },
      {
        kind: 'table',
        headers: ['每月处理量', '云端（估算，超出免费额度部分）', 'MonoSub', '说明'],
        rows: [
          ['≤ 2 小时', '通常落在免费额度内', '¥48 一次', '云端更划算，除非内容敏感'],
          ['5 小时', '约 ¥15–30 / 月', '¥48 一次', '三个月内基本打平'],
          ['13 小时（800 分钟）', '约 ¥64–96 / 月', '¥48 一次', '第一个月就已超过买断价'],
          ['40 小时', '约 ¥190–290 / 月', '¥48 一次', '差距随时间线性拉大'],
        ],
        note: '估算基于公开单价的量级，未计入各家套餐折扣与活动。真实成本请按你的实际用量和当期官网价格计算。',
      },
      {
        kind: 'p',
        text: '这张表解释了一个反直觉的现象：¥48 的买断价看起来比"免费额度"贵，但对每周都做视频的人来说，它其实比免费额度更便宜——因为你要用的部分正好在免费额度之外。',
      },

      { kind: 'h2', text: 'Cloud 更强的地方，也得说清楚' },
      {
        kind: 'callout',
        title: 'MonoSub 不做的事',
        text: '下面这些是云端服务真正的优势，不是"以后会补上"的功能。如果你的需求落在这里，云端是正确答案。',
      },
      { kind: 'h3', text: '1. 不需要你的电脑有任何算力' },
      {
        kind: 'p',
        text: '云端跑在服务器上。老笔记本、办公用的轻薄本、甚至只有一台平板，都能处理四小时的会议录音。MonoSub 的速度直接受你的 CPU/GPU 影响，弱机器上处理长音频会很慢。',
      },
      { kind: 'h3', text: '2. AI 总结与结构化整理' },
      {
        kind: 'p',
        text: '通义听悟和飞书妙记都能自动生成摘要、提取待办、划分议题，飞书还能把待办同步进飞书任务。MonoSub 输出的是字幕文件，不做内容理解——它连"这段话在讲什么"都不试图回答。',
      },
      { kind: 'h3', text: '3. 团队协作' },
      {
        kind: 'p',
        text: '共享、评论、按人分配，这些是 SaaS 的原生能力。MonoSub 是单机工具：文件在谁的电脑上，就在谁那儿。要协作就得自己传文件。',
      },
      { kind: 'h3', text: '4. 零安装' },
      {
        kind: 'p',
        text: '打开浏览器就能用。临时处理一次、又不打算长期用的场景，装一个桌面应用确实是没有必要的动作。',
      },

      { kind: 'h2', text: 'MonoSub 更强的地方' },
      { kind: 'h3', text: '1. 文件不出本机' },
      {
        kind: 'p',
        text: '这不是营销话术，而是一条可验证的性质：MonoSub 的转录与翻译都在本机完成，处理过程中没有上传步骤。对未公开的内容，这条性质本身就能决定工具选型——不是因为它更好，而是因为另一条路在合规上根本走不通。',
      },
      { kind: 'h3', text: '2. 没有计量，也没有额度焦虑' },
      {
        kind: 'p',
        text: '不用算还剩多少分钟，不用想这个月是不是超了，也不用为了省额度而降低处理质量。处理一个三小时的讲座和三十秒的短片，成本是一样的（都是电费和一点时间）。',
      },
      { kind: 'h3', text: '3. 离线可用' },
      {
        kind: 'p',
        text: '在飞机上、在断网的环境、在网速很差的现场，本地工具照常工作。云端服务在这一刻是不可用的。',
      },
      { kind: 'h3', text: '4. 批量队列而不是逐个上传' },
      {
        kind: 'p',
        text: '云端服务的交互范式是"选一个文件、等它处理完、看结果"。MonoSub 的范式是"把二十个文件丢进队列，去干别的事"。前者适合处理单个重要文件，后者适合清理积压素材。',
      },

      { kind: 'h2', text: '还有一种情况值得单独说' },
      {
        kind: 'p',
        text: '如果你每月处理量不大，但偶尔会遇到"这个绝对不能上传"的素材，那么最合理的方案往往不是二选一，而是两个都用：日常用免费额度，敏感素材用本地工具。¥48 在这种组合里买到的是一份保险，而不是替代品。',
      },

      { kind: 'h2', text: '怎么选' },
      {
        kind: 'table',
        headers: ['你的情况', '建议'],
        rows: [
          ['每月只转几十分钟，内容不敏感', '云端免费额度，不必花钱'],
          ['需要 AI 摘要与会议待办', '通义听悟 / 飞书妙记'],
          ['已经在用飞书办公', '飞书妙记（生态内几乎零成本）'],
          ['长期批量处理视频素材', 'MonoSub'],
          ['素材不能外传（未公开 / 内部 / 客户）', 'MonoSub'],
          ['经常在断网或弱网环境工作', 'MonoSub'],
          ['要方言、嘈杂环境的高准确率', '讯飞听见（其强项）'],
        ],
      },
    ],
    verdict:
      '云端转写不是"落后的方案"，它在需要 AI 总结、团队协作和弱设备时明显更好。但如果你每周都在批量处理视频，按分钟计费会在第一个月就超过 ¥48；如果你的素材不能外传，上传这条路直接不成立。这两种情况之外，免费额度是真的够用，不必买。',
  },

  en: {
    title: 'MonoSub vs cloud transcription: 讯飞听见, 通义听悟 and 飞书妙记',
    metaTitle: 'Local subtitles vs cloud transcription: which to pick (2026)',
    description:
      'Cloud transcription bills by the minute and needs an upload. Weigh it against MonoSub: free tiers, marginal cost, privacy — and when cloud really is better.',
    updated: '2026-10-27',
    intro:
      'Chinese cloud transcription services are mature and their free tiers are not stingy. So the real question is not whether local is more advanced. It is whether, at your actual volume, uploading files and paying by the minute is a convenience or a burden. This page puts the three services people compare most next to MonoSub and does the arithmetic.',
    blocks: [
      { kind: 'h2', text: 'The short answer' },
      {
        kind: 'p',
        text: 'If you transcribe a few dozen minutes a month, the content is not sensitive, and you would rather not install anything, the free tiers are plenty and there is nothing to buy. Once you are processing video regularly and in batches — or the material cannot leave the building (unreleased courses, internal meetings, client interviews, legal and medical recordings) — per-minute billing and the upload both become problems at the same time.',
      },
      { kind: 'h2', text: 'What the three cloud services look like' },
      {
        kind: 'table',
        headers: ['', 'Free tier', 'Beyond it', 'Character'],
        rows: [
          ['讯飞听见 (iFlytek)', 'Trial minutes for new accounts, plus a free tier', 'Per minute, commonly around ¥0.3–0.5/min; packages discount it', 'Strong on dialects and noisy audio; long track record; broad export'],
          ['通义听悟 (Alibaba)', 'Roughly 5–10 hours/month for individuals (check the site)', 'Per minute, commonly from about ¥0.08/min', 'Strong LLM summarisation; good for lectures and meetings'],
          ['飞书妙记 (Feishu)', 'About 500 minutes/month for individuals', 'Per minute', 'Wired into Feishu docs, calendar and tasks; requires a Feishu account'],
        ],
        note: 'Free tiers and unit prices change often. These are order-of-magnitude figures from public information as of 2026-10-27, not a quote. Check the vendor site before buying — this page gives magnitudes, not exact prices.',
      },
      {
        kind: 'p',
        text: 'What the three share matters more than how they differ: all require an upload, all meter by minute or duration, and free allowances reset monthly without rolling over. That is the business model, not a product flaw — cloud inference has a bill, so it has to be passed on by usage.',
      },

      { kind: 'h2', text: 'Making the arithmetic concrete' },
      {
        kind: 'p',
        text: 'Say you handle ten videos a week, twenty minutes each — about 800 minutes a month. At the order of ¥0.08–0.12 per minute beyond the free allowance, 800 minutes lands around ¥64–96 a month.',
      },
      {
        kind: 'table',
        headers: ['Monthly volume', 'Cloud (estimate, beyond free tier)', 'MonoSub', 'Note'],
        rows: [
          ['≤ 2 hours', 'Usually inside the free tier', '¥48 once', 'Cloud wins unless the content is sensitive'],
          ['5 hours', 'About ¥15–30/month', '¥48 once', 'Roughly breaks even within three months'],
          ['13 hours (800 min)', 'About ¥64–96/month', '¥48 once', 'Exceeds the purchase price in month one'],
          ['40 hours', 'About ¥190–290/month', '¥48 once', 'The gap widens linearly'],
        ],
        note: 'Estimates from published unit prices at order-of-magnitude precision; package discounts and promotions are excluded. Work out your real cost from your own volume and the vendor’s current pricing.',
      },
      {
        kind: 'p',
        text: 'This table explains a counter-intuitive result: ¥48 looks more expensive than "free tier", but for someone making videos every week it is cheaper than free — because the volume they need sits exactly beyond the free allowance.',
      },

      { kind: 'h2', text: 'Where the cloud is genuinely better' },
      {
        kind: 'callout',
        title: 'What MonoSub does not do',
        text: 'These are real cloud advantages, not "coming soon" gaps. If your need lives here, the cloud is the correct answer.',
      },
      { kind: 'h3', text: '1. It needs no compute from your machine' },
      {
        kind: 'p',
        text: 'Cloud inference runs on servers. An old laptop, a thin office ultrabook, even just a tablet can process a four-hour meeting recording. MonoSub’s speed is your CPU/GPU; a weak machine will be slow on long audio.',
      },
      { kind: 'h3', text: '2. AI summaries and structured notes' },
      {
        kind: 'p',
        text: '通义听悟 and 飞书妙记 generate summaries, extract action items and split topics; Feishu can sync those items into Feishu Tasks. MonoSub outputs subtitle files and does no content understanding — it does not attempt to answer what the passage was about.',
      },
      { kind: 'h3', text: '3. Team collaboration' },
      {
        kind: 'p',
        text: 'Sharing, commenting, assigning — native SaaS capabilities. MonoSub is a single-machine tool: the file lives on whoever’s computer has it. Collaboration means sending files around yourself.',
      },
      { kind: 'h3', text: '4. Nothing to install' },
      {
        kind: 'p',
        text: 'Open a browser and go. For a one-off job you do not plan to repeat, installing a desktop app is genuinely unnecessary work.',
      },

      { kind: 'h2', text: 'Where MonoSub is stronger' },
      { kind: 'h3', text: '1. The file never leaves the machine' },
      {
        kind: 'p',
        text: 'This is not marketing language but a verifiable property: transcription and translation both happen locally, and there is no upload step in the pipeline. For unreleased material that property alone decides the tool — not because it is better, but because the other path is not permissible at all.',
      },
      { kind: 'h3', text: '2. No metering, no allowance anxiety' },
      {
        kind: 'p',
        text: 'Nothing to count down, no wondering whether you have gone over this month, and no incentive to lower quality to save quota. A three-hour lecture and a thirty-second clip cost the same — electricity and a little time.',
      },
      { kind: 'h3', text: '3. Works offline' },
      {
        kind: 'p',
        text: 'On a plane, on a locked-down network, at a venue with terrible Wi-Fi, a local tool keeps working. A cloud service is simply unavailable at that moment.',
      },
      { kind: 'h3', text: '4. A batch queue rather than one upload at a time' },
      {
        kind: 'p',
        text: 'The cloud interaction model is "pick a file, wait, read the result". MonoSub’s is "drop twenty files in a queue and go do something else". The former suits one important file; the latter suits clearing a backlog.',
      },

      { kind: 'h2', text: 'One situation worth calling out' },
      {
        kind: 'p',
        text: 'If your monthly volume is small but you occasionally encounter material that absolutely cannot be uploaded, the sensible arrangement is usually not either/or: use the free allowance day to day, and a local tool for the sensitive items. In that combination the ¥48 buys insurance, not a replacement.',
      },

      { kind: 'h2', text: 'How to choose' },
      {
        kind: 'table',
        headers: ['Your situation', 'Pick'],
        rows: [
          ['A few dozen minutes a month, nothing sensitive', 'Cloud free tier; spend nothing'],
          ['You need AI summaries and meeting action items', '通义听悟 / 飞书妙记'],
          ['Your team already runs on Feishu', '飞书妙记 (near-zero marginal cost in-ecosystem)'],
          ['Regular batch processing of video', 'MonoSub'],
          ['Material cannot leave the machine', 'MonoSub'],
          ['You often work offline or on poor networks', 'MonoSub'],
          ['Dialects and noisy recordings at high accuracy', '讯飞听见 (its strength)'],
        ],
      },
    ],
    verdict:
      'Cloud transcription is not the backward option — it is clearly better when you need AI summaries, team collaboration, or weak hardware. But if you batch video every week, per-minute billing passes ¥48 in the first month; and if the material cannot leave the machine, the upload path does not exist at all. Outside those two cases, the free tiers really are enough, and there is nothing to buy.',
  },
};
