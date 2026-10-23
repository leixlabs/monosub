export interface PrivacySection {
  heading: string;
  paragraphs: string[];
  list?: string[];
  afterList?: string[];
}

export interface PrivacyContent {
  meta_title: string;
  meta_description: string;
  title: string;
  last_updated_label: string;
  last_updated: string;
  intro: string;
  back_home: string;
  sections: PrivacySection[];
}

const en: PrivacyContent = {
  meta_title: 'Privacy Policy - MonoSub',
  meta_description:
    'How MonoSub handles your data: everything is processed locally on your device. No accounts, no analytics, no cloud uploads.',
  title: 'Privacy Policy',
  last_updated_label: 'Last updated',
  last_updated: 'October 23, 2026',
  intro:
    'MonoSub ("the App") is a desktop transcription and subtitle-generation application that runs on your own computer. This policy describes what data the App processes, where it goes, and what it does not do. MonoSub has no account system, no analytics SDK, and no advertising.',
  back_home: '← Back to Home',
  sections: [
    {
      heading: 'Audio and Video Files Stay on Your Device',
      paragraphs: [
        'Files you add to the queue are processed locally by on-device models. The App extracts a 16 kHz mono WAV cache next to your source file, runs local speech recognition, optional speaker diarization, optional local translation, and writes subtitle formats (SRT/VTT/TXT/MD/JSON) to the location you choose.',
        'Your audio, video, and transcribed text are never uploaded to MonoSub\u2019s servers or to any third-party speech-recognition cloud service. No such servers exist.'
      ]
    },
    {
      heading: 'Network Connections Made by the App',
      paragraphs: ['The App only contacts the network in the following limited cases:'],
      list: [
        'Model download (on your action): when you choose to install or update a recognition, alignment, or translation model, the App downloads model weights from Hugging Face (huggingface.co) or its mirror (hf-mirror.com). No personal data is sent with this request.',
        'Manual update check (on your action): clicking "Check for Updates" sends one request to api.github.com/repos/leixlabs/monosub/releases/latest with a standard user-agent string to compare versions. It does not download or install anything, sends no identifiers, and runs only when you press the button.',
        'URL import (on your action): when you paste a video link and confirm import, the bundled yt-dlp contacts the target site (e.g., Bilibili) directly to fetch the selected subtitle track or audio. This traffic goes to the video platform under that platform\u2019s own terms and privacy policy; MonoSub does not forward your links or browsing data to any intermediate analytics service.'
      ],
      afterList: ['No telemetry, crash-reporting, usage-analytics, or advertising request is ever sent.']
    },
    {
      heading: 'Data Stored Locally on Your Device',
      paragraphs: ['All data the App stores lives on your own computer:'],
      list: [
        'settings.json: your preferences (output formats, language, model choices, translation settings, toggles), stored in the app config directory.',
        'Model weights: downloaded files kept in the models directory you choose.',
        'Log file: local runtime logs and frontend error messages, stored on disk for troubleshooting; they are never transmitted.',
        'Temporary cache: per-task WAV slices and intermediate files in a temp directory, removed when the task finishes or fails.',
        'Output subtitles: written only where you specify; the App does not copy them elsewhere.'
      ],
      afterList: [
        'You can inspect or delete all of the above at any time through your file manager; uninstalling the App removes the application, but you may delete the config, models, and logs manually.'
      ]
    },
    {
      heading: 'Clipboard Watching (Optional, User-Controlled)',
      paragraphs: [
        'When enabled (on by default; toggle in Settings), the App polls your clipboard locally at a low interval to detect a single supported video link and offer a one-click import. All matching happens on-device; clipboard contents are not uploaded, logged, or transmitted. The feature reads only the text currently on the clipboard and remembers only whether a suggestion was dismissed. You can disable it at any time, after which no clipboard polling occurs.'
      ]
    },
    {
      heading: 'Third-Party Services',
      paragraphs: ['MonoSub interacts with the following third-party services, and only in the ways described above:'],
      list: [
        'GitHub Releases / GitHub API: used solely for the manual version check.',
        'Hugging Face / hf-mirror: used solely to download model files you requested.',
        'The video platform you import from: subject to that platform\u2019s own privacy policy.',
        'Bundled open-source components (FFmpeg, audiocpp, llama.cpp, yt-dlp) run locally and make no independent phone-home.'
      ]
    },
    {
      heading: 'Security and Data Retention',
      paragraphs: [
        'All processing occurs on your device; there is no MonoSub database or backend that retains your content. Local files persist until you delete them.'
      ]
    },
    {
      heading: 'Children\u2019s Privacy',
      paragraphs: [
        'The App does not knowingly collect personal information from anyone, including children; all processing is local.'
      ]
    },
    {
      heading: 'Changes to This Policy',
      paragraphs: [
        'We may update this policy when functionality changes. The "Last updated" date will be revised accordingly.'
      ]
    },
    {
      heading: 'Contact',
      paragraphs: ['Questions about this policy or your data? Email kamalei.zhang@gmail.com.']
    }
  ]
};

const zh: PrivacyContent = {
  meta_title: '隐私政策 - MonoSub',
  meta_description:
    '了解 MonoSub 如何处理你的数据：所有处理均在本地设备完成，无账号、无埋点、不上传云端。',
  title: '隐私政策',
  last_updated_label: '最后更新',
  last_updated: '2026年10月23日',
  intro:
    'MonoSub（"本 App"）是在你自己电脑上运行的本地转写与字幕生成工具。本政策说明 App 处理哪些数据、数据去向，以及它不会做什么。MonoSub 无账号系统、无埋点分析 SDK、无广告。',
  back_home: '← 返回首页',
  sections: [
    {
      heading: '音视频文件不出本机',
      paragraphs: [
        '你加入队列的文件全部由本地模型处理：本地提取 16kHz 单声道 WAV 缓存、本地语音识别、可选本地说话人分离、可选本地翻译，字幕（SRT/VTT/TXT/MD/JSON）写到你自己指定的位置。',
        '你的音频、视频和转写文本均不会上传到 MonoSub 服务器或任何第三方语音云服务（也不存在这样的服务器）。'
      ]
    },
    {
      heading: 'App 发起的网络连接仅限以下情况',
      paragraphs: ['App 仅在下述有限情况下连接网络：'],
      list: [
        '下载模型（你主动操作）：当你选择安装或更新识别、对齐或翻译模型时，App 从 Hugging Face（huggingface.co）或其镜像（hf-mirror.com）拉取模型权重，该请求不携带任何个人信息。',
        '手动检查更新（你点击按钮才触发）：点击"检查更新"仅向 api.github.com/repos/leixlabs/monosub/releases/latest 发送一次带标准 User-Agent 的版本比较请求，不下载、不安装、不带设备标识，只在按下按钮时运行。',
        '链接导入（你主动确认）：当你粘贴视频链接并确认导入时，内置 yt-dlp 直连目标站点（如 B 站）获取所选字幕轨或音频。该流量按视频平台自身的条款与隐私政策进行；MonoSub 不会把你的链接或浏览数据转发给任何中间分析服务。'
      ],
      afterList: ['App 从不发送任何遥测、崩溃上报、使用统计或广告请求。']
    },
    {
      heading: '仅存于本机的数据',
      paragraphs: ['App 存储的所有数据都在你自己的电脑上：'],
      list: [
        'settings.json：你的偏好设置（输出格式、语言、模型选择、翻译设置、开关），存储在应用配置目录。',
        '模型权重：下载的文件保存在你选择的模型目录。',
        '日志文件：本地运行日志和前端错误信息，仅用于排障，存于磁盘，从不外传。',
        '临时缓存：每个任务的 WAV 片段和中间文件位于临时目录，任务完成或失败后即被清理。',
        '字幕产物：只写入你指定的位置，App 不会复制到别处。'
      ],
      afterList: [
        '以上内容你都可以随时通过文件管理器查看或删除；卸载 App 只会移除应用程序，配置、模型和日志可手动删除。'
      ]
    },
    {
      heading: '剪贴板监听（可选、可关闭）',
      paragraphs: [
        '默认开启、可在设置中关闭。开启时，App 在本地以低频轮询剪贴板，检测是否为单个受支持的视频链接，以提供一键导入。所有匹配都在设备本地完成；剪贴板内容不会被上传、记录或转发。该功能只读取剪贴板上当前的文本，且只记住某条建议是否被忽略。你可以随时关闭，关闭后不再进行任何剪贴板轮询。'
      ]
    },
    {
      heading: '第三方服务',
      paragraphs: ['MonoSub 仅按上文所述方式与以下第三方服务交互：'],
      list: [
        'GitHub Releases / GitHub API：仅用于手动版本检查。',
        'Hugging Face / hf-mirror：仅用于下载你请求的模型文件。',
        '你导入的视频平台：适用该平台自身的隐私政策。',
        '内置开源组件（FFmpeg、audiocpp、llama.cpp、yt-dlp）均在本地运行，无独立外联。'
      ]
    },
    {
      heading: '安全与数据保留',
      paragraphs: [
        '所有处理都在你的设备上进行；不存在保存你内容的 MonoSub 数据库或后端。本地文件会保留到你手动删除为止。'
      ]
    },
    {
      heading: '儿童隐私',
      paragraphs: [
        'App 不会故意收集任何人（包括儿童）的个人信息；所有处理均在本地完成。'
      ]
    },
    {
      heading: '政策更新',
      paragraphs: [
        '功能发生变化时，我们可能更新本政策，并相应修订"最后更新"日期。'
      ]
    },
    {
      heading: '联系我们',
      paragraphs: ['对本政策或你的数据有疑问？请发送邮件至 kamalei.zhang@gmail.com。']
    }
  ]
};

export const privacyContent: Record<string, PrivacyContent> = {
  en,
  zh
};
