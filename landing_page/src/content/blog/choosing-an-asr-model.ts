import type { Post } from './types';

/**
 * A "tips" post — the kind of thing that earns links because it solves a problem
 * rather than describing a product.
 *
 * Everything here is a decision MonoSub already makes, written up as advice: why
 * a model choice matters more than a GPU, why VAD changes output quality, why
 * "biggest model" is often the wrong default. No feature list, no pitch.
 */
export const pickingAsrModel: Post = {
  slug: 'choosing-an-asr-model',
  category: 'tips',
  competitor: '模型选择',
  tagline: {
    zh: '别默认选最大模型',
    en: 'Stop defaulting to the biggest model',
  },
  teaser: {
    zh: '中文识别、方言、噪声环境、长音频——不同场景该选不同模型。"最大的那个"经常不是答案。',
    en: 'Chinese, dialects, noisy rooms, long files — different jobs want different models. The biggest one is often the wrong default.',
  },
  zh: {
    title: '选语音识别模型时，最大的那个经常是错的',
    metaTitle: '怎么选语音识别模型：大小、语言、场景的取舍（2026）',
    description:
      '模型不是越大越好。语言覆盖、噪声鲁棒性、词级时间戳、显存与速度——按实际场景选择，附带为什么会选错的解释。',
    updated: '2026-10-21',
    intro:
      '"用最大的模型准没错"是本地转写里最常见的一笔冤枉账。大模型确实更准，但准确率的边际收益会很快变小，而时间、内存和散热代价是线性上升的。更关键的是：在中文、方言、强噪声这些具体场景里，模型家族的差异往往比参数量差异大得多。',
    blocks: [
      { kind: 'h2', text: '先看语言，再看大小' },
      {
        kind: 'p',
        text: '绝大多数量化对比是在英文上做的。如果你的素材主要是中文，这个排名基本不适用——中文的声调、同音字、以及口语里的省略，对模型的训练数据分布要求很高。一个以中文为主的模型（Tongyi / FunASR 系）即使参数量只有大模型的三分之一，在中文上的实际表现也经常更好。',
      },
      {
        kind: 'p',
        text: '粤语、闽南语、带口音的普通话是另一个台阶。这时候不是"准不准"的问题，而是"能不能识别"——语言覆盖不到，模型再大也是输出一段看起来像中文的胡话。',
      },
      {
        kind: 'callout',
        title: '一个可操作的判断顺序',
        text: '先按语言筛掉不支持你素材语言的模型，再按是否需要词级时间戳筛一轮，最后才在剩下的里面比大小。反过来做（先比大小）会让你在最强的模型上发现它根本不给词级时间戳。',
      },

      { kind: 'h2', text: '大模型的收益在哪、不在哪' },
      {
        kind: 'p',
        text: '参数量带来的提升主要体现在两处：罕见词与专有名词的识别，以及嘈杂环境下的鲁棒性。这两点确实是硬需求——如果你的素材是户外采访、会议现场，或者充满专业术语，大模型值得那个时间成本。',
      },
      {
        kind: 'p',
        text: '它不带来的东西也很明确：标点与断句的合理性（那更多取决于后处理）、时间戳精度（取决于对齐策略，不是参数量）、以及对口音的处理（取决于训练数据）。把大模型当成万能解，往往在换不来提升的地方付了钱。',
      },
      {
        kind: 'table',
        headers: ['你的素材', '优先看什么', '原因'],
        rows: [
          ['普通话口播、录课', '中等模型 + 词级时间戳', '清晰音频上大模型收益很小'],
          ['访谈、圆桌（多人）', '说话人分离能力', '谁说的比说了什么更难'],
          ['户外、现场、嘈杂', '噪声鲁棒性', '这是大模型真正赢的地方'],
          ['方言、口音重', '语言/方言覆盖', '覆盖不到时准确率无从谈起'],
          ['专有名词密集（医疗、法律）', '大模型 + 术语校对', '罕见词是参数量的强项'],
          ['长音频（数小时）', '速度与内存', '大模型在长音频上会撞内存墙'],
        ],
      },

      { kind: 'h2', text: 'VAD 为什么会影响你看得见的质量' },
      {
        kind: 'p',
        text: 'VAD（语音活动检测）负责判断"这段音频里有没有人说话"。它看起来是个内部细节，但直接决定输出：切分不准，模型就会在静音段上凭空生成句子，也就是所谓的幻觉字幕。',
      },
      {
        kind: 'p',
        text: '好的 VAD 会把长音频切成带自然停顿的片段，让每段都短到模型能稳定处理。坏的结果是字幕里出现整段没人在说的文字——这类错误比错别字更麻烦，因为校对时你很难判断"这句话到底有没有说过"。',
      },
      {
        kind: 'p',
        text: 'MonoSub 用 Silero VAD 做预切分，并把每段的边界记录下来。这既是准确率措施，也是可解释性措施：出问题时能定位到具体片段，而不是面对一整份可疑的转写稿。',
      },

      { kind: 'h2', text: '量化：省内存，代价是什么' },
      {
        kind: 'p',
        text: '同一个模型常有多种量化版本（q8_0、q4_k_m 等）。规律是：q8 相对原始精度的损失通常可以忽略，q4 开始能听出差别，更激进的量化在噪声环境下退化明显。',
      },
      {
        kind: 'p',
        text: '实用的做法是：内存够就上 q8，紧张就 q4，只有在跑不动时才继续往下压——而不是一开始就选最小的以换取速度。速度慢一点可以等，识别错了要重来。',
      },

      { kind: 'h2', text: '怎么快速试出适合你的模型' },
      {
        kind: 'p',
        text: '不要用别人的评测决定，用你自己的素材。一个十五分钟的流程：',
      },
      {
        kind: 'ul',
        items: [
          '挑一段你熟悉的、有代表性的素材（含你最头疼的那种情况：口音、噪声、术语）',
          '手动写下这段的真实文本，作为基准',
          '用两三个候选模型分别跑，把输出和基准逐句对比',
          '记录的不只是错字数，还有"错得有多难改"——错一个专有名词比错三个虚词更贵',
          '同时记下耗时，因为你的时间也是成本',
        ],
      },
      {
        kind: 'p',
        text: '这个流程花十五分钟，但能省掉之后几个月的反复试错。而且结论只对你这批素材有效——这也是为什么没有一份通用推荐能替你做决定。',
      },
    ],
    verdict:
      '模型选择是一次权衡，不是一次升级。先按语言和功能筛，再按素材特性在剩下的里面挑，最后才用大小和量化调性能。把顺序倒过来，最常见的结局是：跑得最慢的那个模型，在你最在意的指标上并不占优。',
  },

  en: {
    title: 'The biggest speech model is often the wrong choice',
    metaTitle: 'Choosing an ASR model: size, language and the real trade-offs (2026)',
    description:
      'Bigger is not better by default. Language coverage, noise robustness, word timestamps, memory and speed — pick by scenario, and why the usual instinct misfires.',
    updated: '2026-10-21',
    intro:
      '"Just use the largest model" is the most common wasted expense in local transcription. Large models are more accurate, but the returns fall off quickly while time, memory and heat rise linearly. More importantly: for Chinese, dialects and noisy rooms, the gap between model families dwarfs the gap between parameter counts.',
    blocks: [
      { kind: 'h2', text: 'Language first, size second' },
      {
        kind: 'p',
        text: 'Nearly every published comparison is measured on English. If your material is mainly Chinese, that ranking does not transfer — tone, homophones and the ellipsis of spoken Chinese lean heavily on the training distribution. A model built for Chinese (the Tongyi / FunASR family) frequently beats a larger general model on Chinese material despite a third of the parameters.',
      },
      {
        kind: 'p',
        text: 'Cantonese, Hokkien and heavily accented Mandarin are a different step up. There the question is not accuracy but capability: if the language is not covered, no amount of parameters helps — you get fluent-looking nonsense.',
      },
      {
        kind: 'callout',
        title: 'A usable order of operations',
        text: 'Filter by language support first, then by whether the model emits word-level timestamps, and only then compare sizes among what remains. Doing it the other way round gets you to the strongest model and then reveals it cannot give you timestamps at all.',
      },

      { kind: 'h2', text: 'Where size helps, and where it does not' },
      {
        kind: 'p',
        text: 'Parameters buy two things: rare words and proper nouns, and robustness in noise. Both are real needs — outdoor interviews, conference rooms, jargon-heavy material justify the time cost.',
      },
      {
        kind: 'p',
        text: 'What size does not buy is just as clear: sensible punctuation and segmentation (that is post-processing), timestamp precision (that is alignment strategy), and accent handling (that is training data). Treating a large model as a universal answer means paying for gains where they do not exist.',
      },
      {
        kind: 'table',
        headers: ['Your material', 'Optimise for', 'Why'],
        rows: [
          ['Clear Mandarin speech, lectures', 'Mid-size model + word timestamps', 'Large models add little on clean audio'],
          ['Interviews, panels (multiple speakers)', 'Diarization capability', 'Who spoke is harder than what was said'],
          ['Outdoor, live, noisy', 'Noise robustness', 'This is where size genuinely wins'],
          ['Dialects, strong accents', 'Language/dialect coverage', 'Without coverage, accuracy is moot'],
          ['Jargon-heavy (medical, legal)', 'Large model + glossary review', 'Rare words are the strong suit of size'],
          ['Long files (hours)', 'Speed and memory', 'Large models hit a memory wall'],
        ],
      },

      { kind: 'h2', text: 'Why VAD changes the quality you can see' },
      {
        kind: 'p',
        text: 'VAD decides whether there is speech in a stretch of audio. It sounds like an internal detail and is not: segment badly and the model will invent sentences over silence — the classic hallucinated subtitle.',
      },
      {
        kind: 'p',
        text: 'Good VAD cuts long audio at natural pauses so each piece is short enough to transcribe stably. Bad VAD leaves whole paragraphs of text nobody said, which is worse than a typo: while proofreading you have no easy way to tell whether the line was ever spoken.',
      },
      {
        kind: 'p',
        text: 'MonoSub segments with Silero VAD and records each chunk boundary. That is both an accuracy measure and a debuggability one: when something is wrong you can point at a specific chunk instead of distrusting an entire transcript.',
      },

      { kind: 'h2', text: 'Quantization: memory saved, and the price' },
      {
        kind: 'p',
        text: 'The same model usually ships at several quantizations (q8_0, q4_k_m, …). The pattern: q8 loses almost nothing against full precision, q4 starts to show, and more aggressive quantization degrades noticeably in noise.',
      },
      {
        kind: 'p',
        text: 'The practical rule: q8 if memory allows, q4 when it is tight, and go lower only when it will not run at all — rather than starting at the smallest to buy speed. Slow is something you can wait out; wrong is something you have to redo.',
      },

      { kind: 'h2', text: 'How to find your model in fifteen minutes' },
      {
        kind: 'p',
        text: 'Do not let someone else’s benchmark decide. Use your own material:',
      },
      {
        kind: 'ul',
        items: [
          'Pick one representative clip containing your worst case — accent, noise, jargon',
          'Transcribe it by hand as the ground truth',
          'Run two or three candidate models and compare line by line',
          'Record not just the error count but how expensive each error is to fix — one wrong proper noun costs more than three wrong particles',
          'Note the elapsed time too; your minutes are a cost',
        ],
      },
      {
        kind: 'p',
        text: 'Fifteen minutes here saves months of second-guessing. And the conclusion only holds for material like yours — which is exactly why no universal recommendation can make the decision for you.',
      },
    ],
    verdict:
      'Model choice is a trade-off, not an upgrade. Filter by language and capability, choose among what survives by how your material actually behaves, and use size and quantization last to tune performance. Reverse the order and the usual outcome is the slowest model, not winning on the thing you cared about.',
  },
};
