import type { Post } from './types';

/**
 * A tutorial, not a comparison.
 *
 * The blog needs a second shape of content: the comparison posts answer "which
 * tool", and this answers "how do I actually do the thing". It also gives the
 * category filter something real to filter, which a blog with one category
 * cannot demonstrate.
 *
 * The instructions here are written against MonoSub's actual UI, so every step
 * has to match a control that exists. Vague steps ("adjust settings as needed")
 * are the reason tutorial pages get bounced from.
 */
export const burnInWorkflow: Post = {
  slug: 'subtitle-to-video-workflow',
  category: 'tutorial',
  competitor: '工作流',
  tagline: {
    zh: '从视频到成片字幕',
    en: 'Video to finished subtitles',
  },
  teaser: {
    zh: '本地生成字幕后，怎么把它装进成片：不同剪辑软件要的格式、时间轴对齐，以及词级时间戳能做什么。',
    en: 'You have local subtitles. Here is how to get them into a finished video — formats, timing, and what word-level timestamps buy you.',
  },
  zh: {
    title: '本地生成字幕后，怎么把它变成成片里的字幕',
    metaTitle: '本地字幕怎么用：从 SRT 到成片的完整工作流（2026）',
    description:
      '拿到 SRT 之后要做什么。不同剪辑软件该导入哪种格式、时间轴为什么会偏、词级时间戳能做什么，以及烧录字幕的两种选择。',
    updated: '2026-10-24',
    intro:
      '生成字幕只是中间步骤，不是终点。多数人卡在下一步：文件拿到了，导入剪辑软件却发现时间对不上，或者格式不被接受。这篇按顺序讲清楚从字幕文件到成片之间发生的事。',
    blocks: [
      { kind: 'h2', text: '第一步：确认你的字幕文件是"干净"的' },
      {
        kind: 'p',
        text: '一个可用的 SRT 至少要满足三条：每段有递增的序号、时间码格式是 `00:00:01,000 --> 00:00:03,500`（注意是逗号不是句点）、文本不带多余的 HTML 标签。MonoSub 输出的文件默认满足这些，但如果你用别的工具生成，值得先打开确认——很多"导入失败"其实是格式问题，不是软件问题。',
      },
      {
        kind: 'ul',
        items: [
          '序号必须从 1 开始且连续，跳号会让部分播放器丢字幕',
          '时间码用逗号分隔毫秒（SRT），VTT 用句点',
          '单行超过约 42 个字符时，多数播放器会自动折行，但不一定折在语义处',
          '空行是段落分隔符，多一个空行可能让下一段被忽略',
        ],
      },

      { kind: 'h2', text: '第二步：选对格式' },
      {
        kind: 'p',
        text: '不同下游要的格式不一样。选错的典型症状是"导入后没有字幕轨道"，而不是报错。',
      },
      {
        kind: 'table',
        headers: ['下游', '格式', '说明'],
        rows: [
          ['Premiere Pro / DaVinci Resolve', 'SRT', '两者都原生支持，导入即成为字幕轨道'],
          ['Final Cut Pro', 'SRT', '需通过字幕工作流导入，或转成 .fcpxml'],
          ['YouTube / B 站上传', 'SRT 或 VTT', 'YouTube 两者都收，VTT 对样式控制更细'],
          ['网页播放器（video.js 等）', 'VTT', '浏览器 `<track>` 标签要 VTT，不收 SRT'],
          ['纯文本整理 / 二次编辑', 'TXT 或 MD', '丢掉时间轴，只留文本'],
          ['程序处理', 'JSON', '带结构化时间戳，方便脚本消费'],
        ],
        note: 'MonoSub 可导出 SRT / VTT / TXT / MD / JSON 五种。格式转换本身不改变时间戳，只改变容器写法。',
      },

      { kind: 'h2', text: '第三步：处理时间轴偏移' },
      {
        kind: 'p',
        text: '如果字幕整体比画面早或晚固定的几秒，这是最简单的偏移，所有剪辑软件都有"整体平移时间"的功能，输入 +2000ms 或 -1500ms 即可。',
      },
      {
        kind: 'p',
        text: '如果字幕开头对齐、越往后越偏，那是帧率不匹配——常见于素材是 23.976fps 而时间轴按 25fps 解释。这类问题不能靠平移解决，要按比例缩放时间轴，或者在生成字幕时就确认素材的真实帧率。',
      },
      {
        kind: 'callout',
        title: '一个容易被忽略的坑',
        text: '变量帧率（VFR）的视频——手机录屏、部分屏幕录制工具的输出——会让基于固定帧率的对齐整体失准。转成恒定帧率再处理，比事后修字幕省事得多。',
      },

      { kind: 'h2', text: '第四步：决定是外挂还是烧录' },
      {
        kind: 'p',
        text: '外挂字幕是独立文件，播放器加载；烧录（硬字幕）是把字幕画进画面像素里。两者的取舍很明确：',
      },
      {
        kind: 'table',
        headers: ['', '外挂字幕', '烧录字幕'],
        rows: [
          ['可编辑', '✓ 随时改文本和时间', '✗ 改字幕要重新导出'],
          ['可关闭', '✓ 观众自行选择', '✗ 永久在画面上'],
          ['平台兼容', '视平台而定，部分不支持上传', '✓ 任何平台都能播'],
          ['样式控制', '受播放器限制', '✓ 完全可控'],
          ['适合', '存档、多语言、需要无障碍字幕', '短视频、社交平台、投放素材'],
        ],
      },
      {
        kind: 'p',
        text: 'MonoSub 只出字幕文件，不烧录——这一步留给剪辑软件或 ffmpeg。如果你想直接用命令行，`ffmpeg -i in.mp4 -vf subtitles=sub.srt out.mp4` 就够了，代价是重新编码一次。',
      },

      { kind: 'h2', text: '词级时间戳能额外做什么' },
      {
        kind: 'p',
        text: '句级时间戳只能定位到"这句话什么时候开始"。词级时间戳记录每个词的起止，因此可以：',
      },
      {
        kind: 'ul',
        items: [
          '做卡拉 OK 式逐词高亮（短视频里最常见）',
          '按文字剪视频——删掉某个词，就剪掉对应的那段音频',
          '自动生成"删除静音"的剪辑点，不需要手动听',
          '重新断句：把一句话拆成两条字幕而不破坏时间对齐',
        ],
      },
      {
        kind: 'p',
        text: 'MonoSub 在生成字幕后会额外写出词级与静音信息，正是为了这几件事。如果你的工作流只是"上传视频配字幕"，句级就够了；如果要做逐词动效或按文本剪片，词级是必需的。',
      },

      { kind: 'h2', text: '校对：最花时间的一步' },
      {
        kind: 'p',
        text: '任何语音识别都会出错，重点不是消除错误，而是把校对成本压到最低。两个有效习惯：',
      },
      {
        kind: 'p',
        text: '一是先通读、只标错，不改。逐句改会让你在细节里丢掉整体判断。二是专有名词单独列一张表，第二次出现时批量替换——人名、产品名、术语的错误几乎总是重复出现的。',
      },
    ],
    verdict:
      '生成字幕是这条链上最自动化的一步。真正决定成片质量的是后面三件事：格式选对、时间轴对齐、以及你有没有认真校对。工具能省掉前两件的大部分麻烦，第三件没人能替你省。',
  },

  en: {
    title: 'You have local subtitles. Now get them into the finished video.',
    metaTitle: 'From SRT to finished video: a local subtitle workflow (2026)',
    description:
      'What to do once you have the SRT: which format each editor wants, why timing drifts, what word-level timestamps unlock, and burn-in versus sidecar.',
    updated: '2026-10-24',
    intro:
      'Generating subtitles is a middle step, not the end. Most people get stuck on the next one: the file exists, but the editor rejects it or the timing is off. This walks through what happens between a subtitle file and a finished video, in order.',
    blocks: [
      { kind: 'h2', text: '1. Make sure the file is actually clean' },
      {
        kind: 'p',
        text: 'A usable SRT satisfies three things: sequential numbering, timestamps in the form `00:00:01,000 --> 00:00:03,500` (a comma, not a period), and text free of stray HTML tags. MonoSub’s output meets this by default, but if another tool produced the file, open it and check — most "import failed" reports are format problems, not software problems.',
      },
      {
        kind: 'ul',
        items: [
          'Numbering starts at 1 and never skips; gaps make some players drop cues',
          'SRT separates milliseconds with a comma, VTT with a period',
          'Lines over ~42 characters get wrapped by most players, but not at sensible points',
          'A blank line separates cues; an extra one can make the next cue disappear',
        ],
      },

      { kind: 'h2', text: '2. Pick the right format' },
      {
        kind: 'p',
        text: 'Different downstream tools want different containers. The usual symptom of picking wrong is not an error — it is "the subtitles just are not there".',
      },
      {
        kind: 'table',
        headers: ['Downstream', 'Format', 'Note'],
        rows: [
          ['Premiere Pro / DaVinci Resolve', 'SRT', 'Both import it natively as a caption track'],
          ['Final Cut Pro', 'SRT', 'Via its caption workflow, or converted to .fcpxml'],
          ['YouTube / Bilibili upload', 'SRT or VTT', 'YouTube takes both; VTT gives finer style control'],
          ['Web players (video.js and friends)', 'VTT', 'A `<track>` element only accepts VTT'],
          ['Plain text / further editing', 'TXT or MD', 'Drops the timeline, keeps the words'],
          ['Scripted processing', 'JSON', 'Structured timestamps for code to consume'],
        ],
        note: 'MonoSub exports SRT / VTT / TXT / MD / JSON. Converting between them does not change timestamps, only the container.',
      },

      { kind: 'h2', text: '3. Deal with timing drift' },
      {
        kind: 'p',
        text: 'If the subtitles are uniformly early or late by a fixed amount, that is the easy case: every editor has a shift-all-times control — enter +2000 ms or −1500 ms and you are done.',
      },
      {
        kind: 'p',
        text: 'If the start is aligned but the drift grows, that is a frame-rate mismatch — classically 23.976 fps footage interpreted as 25 fps. Shifting cannot fix it; the timeline has to be scaled, or the true frame rate confirmed before generating subtitles at all.',
      },
      {
        kind: 'callout',
        title: 'An easy thing to miss',
        text: 'Variable frame rate (VFR) video — phone screen recordings, output from some capture tools — breaks any fixed-frame-rate alignment for the whole file. Convert to constant frame rate first; it is far less work than repairing subtitles afterwards.',
      },

      { kind: 'h2', text: '4. Sidecar or burned in' },
      {
        kind: 'p',
        text: 'A sidecar subtitle is a separate file a player loads; burning in draws the text into the pixels. The trade-off is straightforward:',
      },
      {
        kind: 'table',
        headers: ['', 'Sidecar', 'Burned in'],
        rows: [
          ['Editable', '✓ Change text and timing any time', '✗ Re-export to change a word'],
          ['Can be turned off', '✓ Viewer decides', '✗ Permanent'],
          ['Platform support', 'Varies; some reject uploads', '✓ Plays everywhere'],
          ['Style control', 'Limited by the player', '✓ Complete'],
          ['Best for', 'Archives, multiple languages, accessibility', 'Short-form, social, ad creative'],
        ],
      },
      {
        kind: 'p',
        text: 'MonoSub produces subtitle files and stops there; burn-in belongs to your editor or to ffmpeg. On the command line, `ffmpeg -i in.mp4 -vf subtitles=sub.srt out.mp4` is enough, at the cost of one re-encode.',
      },

      { kind: 'h2', text: 'What word-level timestamps buy you' },
      {
        kind: 'p',
        text: 'Cue-level timestamps only say when a line begins. Word-level timestamps record the start and end of every word, which makes this possible:',
      },
      {
        kind: 'ul',
        items: [
          'Karaoke-style highlighting, word by word — the most common short-form effect',
          'Editing video by editing text: delete a word and its audio goes with it',
          'Automatic cut points for silence removal, without listening through',
          'Re-segmenting a long line into two cues without breaking the timing',
        ],
      },
      {
        kind: 'p',
        text: 'MonoSub writes word and silence data alongside the subtitles precisely for these uses. If your workflow is "upload the video with captions", cue-level is enough; word-level is required for per-word animation or text-based editing.',
      },

      { kind: 'h2', text: 'Proofreading is the expensive part' },
      {
        kind: 'p',
        text: 'Every speech recogniser makes mistakes. The goal is not eliminating them but lowering the cost of fixing them. Two habits that work:',
      },
      {
        kind: 'p',
        text: 'First, read the whole thing once and only mark errors — do not fix as you go, or you will lose the overview in the details. Second, keep a list of proper nouns and replace them in bulk; name, product and jargon errors are almost always repeated.',
      },
    ],
    verdict:
      'Generating subtitles is the most automated step in the chain. What decides the finished result is the three things after it: the right format, a timeline that lines up, and whether you actually proofread. Software removes most of the pain from the first two. Nobody can remove the third for you.',
  },
};
