import { Translation } from '../index';

const en: Translation = {
  page_title: 'MonoSub - Local AI Subtitle Generator | Free',
  mobile_select_language: 'Select Language',

  // Navigation
  nav_features: 'Features',
  nav_screenshots: 'Screenshots',
  nav_faq: 'FAQ',

  // Hero
  hero_title_line1: 'Turn your videos & audio into text.',
  hero_title_line2: 'Locally.',
  hero_title_line3: ' For free.',
  hero_subtitle_line1: '100% local AI transcription, no cloud required',
  hero_subtitle_line2: 'All advanced features, completely free',
  hero_cta: 'Download Free Version',
  hero_learn_more: 'Learn More',
  hero_trust: 'No account required',
  hero_platform: 'macOS · Apple Silicon',

  // Hero carousel
  carousel_prev: 'Previous screenshot',
  carousel_next: 'Next screenshot',
  hero_shot_1: 'A queue of videos, each with its own speaker and translation switches',
  hero_shot_2: 'Batch processing — one file runs, the rest wait for a free slot',
  hero_shot_3: 'A finished file that also wrote its translated copy',
  hero_shot_4: 'Copy a Bilibili link and the app offers to import it',
  hero_shot_5: 'Light and dark themes',

  // Screenshot showcase
  showcase_title: 'See the app itself',
  showcase_subtitle: 'Every image below is a real screenshot of MonoSub, not a mock-up.',
  showcase_tab_queue: 'Queue & Batch',
  showcase_tab_translate: 'Translation',
  showcase_tab_speakers: 'Speakers & Models',
  showcase_tab_url: 'Import from a Link',
  showcase_queue_desc: 'Drop in as many files as you like. Each row carries its own speaker-separation and translation switch, the queue runs them up to your concurrency limit, and every step reports its own progress.',
  showcase_translate_desc: 'A local translation model rewrites the finished subtitles into a second language. It writes a separate file next to the original — the recognised subtitles are never touched, so a failed translation cannot lose your work.',
  showcase_speakers_desc: 'Speaker separation labels who said what, cue by cue, and the model picker answers four questions instead of asking you to choose a checkpoint: language scenario, speaker separation, word-level timing and memory budget.',
  showcase_url_desc: 'Paste a Bilibili link and MonoSub fetches the platform subtitle if the video has one. If it does not, it downloads the audio and transcribes it locally instead — the same pipeline as a file you dragged in.',
  shot_queue: 'The queue, with per-file switches',
  shot_batch: 'A batch in flight',
  shot_rowstates: 'Skipped, waiting, translated and partly untranslated rows',
  shot_translated: 'A translated subtitle written alongside the original',
  shot_translate_settings: 'Translation settings and model downloads',
  shot_models: 'Model selection and the live plan preview',
  shot_catalogue: 'The model catalogue — install and remove on demand',
  shot_url_rows: 'URL imports: downloading, fetched, needs a login, failed',
  shot_url_dialog: 'The import-from-a-link panel',
  shot_clipboard: 'One-click import when a link is on the clipboard',

  // Pain Points
  pain_title: 'Tired of Cloud Services?',
  pain_subtitle: 'Uploading private content to the cloud is risky and slow',
  pain_cloud_title: '❌ Cloud Services',
  pain_cloud_1: 'Upload videos to third-party servers',
  pain_cloud_2: 'Privacy risks for sensitive content',
  pain_cloud_3: 'Requires internet, slow processing',
  pain_cloud_4: 'Monthly fees or usage limits',
  pain_local_title: '✅ MonoSub Solution',
  pain_local_1: '100% local processing, data never leaves your computer',
  pain_local_2: 'Fully offline, blazing fast hardware acceleration',
  pain_local_3: 'Completely free, no payments, no limits',
  pain_local_4: 'All professional features included, no upsells',

  // Features
  features_title: 'Powerful Features, Simple Workflow',
  features_subtitle: 'Drag in a file, get a transcript. That\'s it.',
  feature_1_title: '🏠 100% Local Processing',
  feature_1_desc: 'Speech recognition and subtitle translation run entirely on your machine. No uploads, no cloud — your data never leaves your computer.',
  feature_2_title: '🌍 Local Subtitle Translation',
  feature_2_desc: 'Built-in local translation models. Generate translated subtitles in Chinese, English and more with one click — fully offline, no extra cost.',
  feature_3_title: '🗣️ Speaker Diarization',
  feature_3_desc: 'Automatically identifies who spoke when. Perfect for interviews, meetings, podcasts and multi-speaker recordings.',
  feature_4_title: '⚡ Hardware Acceleration',
  feature_4_desc: 'Apple Silicon Metal acceleration for faster local transcription and translation.',
  feature_5_title: '📝 Multiple Export Formats',
  feature_5_desc: 'Export to SRT, VTT, TXT, MD and JSON — compatible with all major video editors and players.',
  feature_6_title: '🎯 Batch Processing',
  feature_6_desc: 'Queue multiple files at once and process them automatically. Perfect for creators and researchers with large libraries.',
  feature_7_title: '🔗 Import from a Link',
  feature_7_desc: 'Paste a Bilibili link to take its platform subtitle, or let MonoSub download the audio and transcribe it locally.',
  feature_8_title: '⏱️ Word-Level Timestamps',
  feature_8_desc: 'Word-accurate cue boundaries plus a word/silence sidecar file, so a video can be cut on the transcript.',

  // FAQ
  faq_title: 'Frequently Asked Questions',
  faq_subtitle: 'Quick answers to your questions',
  faq_1_q: 'Which operating systems are supported?',
  faq_1_a: 'Currently MonoSub supports macOS (specially optimized for Apple Silicon M1/M2/M3 chips). Windows and Linux versions are in development and coming soon.',
  faq_2_q: 'Does it require internet?',
  faq_2_a: 'Not at all. MonoSub works completely offline after installation. All AI models and processing engines run locally. No video uploads to the cloud, no internet connection required.',
  faq_3_q: 'What are the advantages over cloud services?',
  faq_3_a: 'Main advantages: 1. Privacy: Data never leaves your machine, complete control; 2. Speed: Local hardware acceleration is faster; 3. Freedom: Works offline, unlimited usage; 4. Cost: Completely free, no subscriptions.',
  faq_4_q: 'Which video formats are supported?',
  faq_4_a: 'MonoSub supports almost all mainstream video formats including: MP4, MOV, AVI, MKV, FLV, WMV, etc. Any video file with an audio track can generate subtitles.',

  // Final CTA
  final_title: 'Ready to Get Started?',
  final_subtitle: 'Turn your videos into text, locally and for free',
  final_cta: 'Download Free Version',
  final_trust_1: 'No Account Required',
  final_trust_2: 'Free Forever',
  final_trust_3: 'Instant Download',

  // Footer
  footer_copyright: '© 2025 MonoSub. Built by Matrix Agent.',
  footer_terms: 'Terms of Service',
  footer_privacy: 'Privacy Policy',
  footer_contact: 'Contact Us'
};

export default en;
