import { Translation } from '../index';

const zhCN: Translation = {
  page_title: 'MonoSub - 本地AI字幕生成，完全免费',
  mobile_select_language: '选择语言',

  // Navigation
  nav_features: '功能',
  nav_screenshots: '界面',
  nav_faq: 'FAQ',

  // Hero
  hero_title_line1: '将视频和音频转换为文本。',
  hero_title_line2: '本地处理。',
  hero_title_line3: '免费使用。',
  hero_subtitle_line1: '100% 本地 AI 转录，无需云服务',
  hero_subtitle_line2: '所有功能完全免费，无需任何付费',
  hero_cta: '下载免费版',
  hero_learn_more: '了解更多',
  hero_trust: '无需账户',
  hero_platform: 'macOS · Apple Silicon',

  // Hero carousel
  carousel_prev: '上一张截图',
  carousel_next: '下一张截图',
  hero_shot_1: '文件队列，每一行都有自己的说话人分离与翻译开关',
  hero_shot_2: '批量处理：一个文件在跑，其余等待空闲位置',
  hero_shot_3: '完成的行同时写出了翻译版本',
  hero_shot_4: '复制一条 B 站链接，应用主动提示导入',
  hero_shot_5: '浅色与深色主题',

  // Screenshot showcase
  showcase_title: '直接看界面',
  showcase_subtitle: '下面每一张都是 MonoSub 的真实截图，不是效果图。',
  showcase_tab_queue: '队列与批量',
  showcase_tab_translate: '字幕翻译',
  showcase_tab_speakers: '说话人与模型',
  showcase_tab_url: '链接导入',
  showcase_queue_desc: '想拖入多少文件都可以。每一行都有独立的说话人分离与翻译开关，队列按你设定的并发数依次处理，每个步骤都有各自的进度。',
  showcase_translate_desc: '本地翻译模型把完成后的字幕改写为第二种语言，并单独写成一个文件——识别出的原始字幕不会被改动，所以翻译失败也不会弄丢你的成果。',
  showcase_speakers_desc: '说话人分离逐条字幕标注"谁在何时说话"。选模型时只需回答四个问题：语种场景、是否分离说话人、是否需要词级时间戳、显存预算。',
  showcase_url_desc: '粘贴一条 B 站链接，视频自带字幕时直接取用平台字幕；没有字幕时则下载音频在本地转写，走的仍是与本地文件完全相同的流程。',
  shot_queue: '队列，以及每一行的独立开关',
  shot_batch: '批量运行中',
  shot_rowstates: '跳过、等待空位、已翻译与部分未翻译的行',
  shot_translated: '翻译后的字幕与原始字幕并存',
  shot_translate_settings: '翻译设置与模型下载',
  shot_models: '模型选择与实时方案预览',
  shot_catalogue: '模型列表——按需下载与删除',
  shot_url_rows: '链接导入：下载中、已获取、需要登录、网络错误',
  shot_url_dialog: '「从链接导入」面板',
  shot_clipboard: '剪贴板里有链接时的一键导入',

  // Pain Points
  pain_title: '厌倦了云服务?',
  pain_subtitle: '云服务让你付出更多,获得更少',
  pain_cloud_title: '❌ 云端订阅服务',
  pain_cloud_1: '每月 ¥99-199 永无止境的订阅费',
  pain_cloud_2: '上传视频到云端,隐私风险高',
  pain_cloud_3: '依赖网络,速度受网速限制',
  pain_cloud_4: '使用时长限制,超时额外付费',
  pain_local_title: '✅ MonoSub 解决方案',
  pain_local_1: '完全免费，无任何付费项目',
  pain_local_2: '100% 本地处理,数据绝不离开你的电脑',
  pain_local_3: '完全离线可用,硬件加速极速处理',
  pain_local_4: '无限制使用,想用多少用多少',

  // Features
  features_title: '强大的功能,简单的工作流',
  features_subtitle: '专业级功能,无需专业级价格',
  feature_1_title: '🏠 100% 本地处理',
  feature_1_desc: '语音识别与字幕翻译全部在本机完成，数据绝不离开你的电脑，隐私完全由你掌控。',
  feature_2_title: '🌍 字幕本地翻译',
  feature_2_desc: '内置本地翻译模型，一键将字幕翻译为中文、英文等多语言，全程离线、无需额外付费。',
  feature_3_title: '🗣️ 说话人识别',
  feature_3_desc: '自动区分不同说话人，标注"谁在何时说话"，适合访谈、会议、播客等多人场景。',
  feature_4_title: '⚡ 硬件加速',
  feature_4_desc: 'Apple Silicon Metal 硬件加速，本地转写与翻译速度更快。',
  feature_5_title: '📝 多格式输出',
  feature_5_desc: '导出 SRT、VTT、TXT、MD、JSON 五种字幕格式，兼容主流视频编辑软件与播放器。',
  feature_6_title: '🎯 批量处理',
  feature_6_desc: '多文件队列批量转写，一次拖入多个视频或音频自动排队处理，适合大批量素材。',
  feature_7_title: '🔗 链接导入',
  feature_7_desc: '粘贴 B 站链接即可取用平台字幕；没有字幕时，MonoSub 会下载音频并在本地转写。',
  feature_8_title: '⏱️ 词级时间戳',
  feature_8_desc: '按词对齐生成字幕时间轴，并额外写出词/静音文件，可以直接按文字剪视频。',

  // FAQ
  faq_title: '常见问题',
  faq_subtitle: '快速解答你的疑问',
  faq_1_q: '支持哪些操作系统?',
  faq_1_a: '目前 MonoSub 支持 macOS(特别针对 Apple Silicon M1/M2/M3 芯片优化)。Windows 和 Linux 版本正在开发中,预计近期推出。',
  faq_2_q: '需要联网才能使用吗?',
  faq_2_a: '完全不需要。MonoSub 在安装后即可完全离线使用。所有 AI 模型和处理引擎都在本地运行,无需上传视频到云端,也不需要网络连接。',
  faq_3_q: '与云端服务相比有什么优势?',
  faq_3_a: '主要优势包括:1. 隐私:数据不出本机,完全掌控;2. 速度:本地硬件加速,处理更快;3. 自由:离线可用,无使用限制;4. 成本:完全免费,无任何订阅费用',
  faq_4_q: '支持哪些视频格式?',
  faq_4_a: 'MonoSub 支持几乎所有主流视频格式,包括:MP4, MOV, AVI, MKV, FLV, WMV 等。只要包含音频轨道的视频文件都可以生成字幕。',

  // Final CTA
  final_title: '准备好提升你的字幕工作流?',
  final_subtitle: '加入选择隐私和灵活的创作者行列',
  final_cta: '下载免费版',
  final_trust_1: '安全可靠',
  final_trust_2: '无需账户',
  final_trust_3: '即时下载',

  // Footer
  footer_copyright: '© 2025 MonoSub. Built by Matrix Agent.',
  footer_terms: '服务条款',
  footer_privacy: '隐私政策',
  footer_contact: '联系我们'
};

export default zhCN;
