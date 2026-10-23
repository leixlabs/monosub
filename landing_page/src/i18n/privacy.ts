export interface PrivacySection {
  heading: string;
  paragraphs: string[];
  list?: string[];
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
    'How MonoSub handles your data: everything is processed locally on your device. No cloud uploads, no accounts, no tracking.',
  title: 'Privacy Policy',
  last_updated_label: 'Last updated',
  last_updated: 'October 23, 2026',
  intro:
    'MonoSub is a desktop app that turns video and audio into subtitles using AI models that run entirely on your computer. This policy explains what data is handled, where it stays, and the limited cases in which information is sent to third parties.',
  back_home: '← Back to Home',
  sections: [
    {
      heading: 'Your Media Stays on Your Device',
      paragraphs: [
        'The video and audio files you process with MonoSub — as well as the transcripts, subtitles, and settings you generate — are handled locally on your computer. We do not upload your files to any server, and we have no infrastructure that receives them.',
        'You do not need an account, and we do not require your name, email address, or any other personal information to use the app.'
      ]
    },
    {
      heading: 'Network Activity',
      paragraphs: ['MonoSub works fully offline. The only optional network actions are:'],
      list: [
        'Update check: when you manually check for updates, the app asks GitHub for the latest release. It sends your app version number (and, like any web request, your IP address). No media or personal data is transmitted.',
        'URL import: if you import content by URL, the app downloads that content directly from the source you specified. The downloaded file is processed locally and is never sent to us.'
      ]
    },
    {
      heading: 'License Purchases',
      paragraphs: [
        'When you purchase a Pro license, payment is processed by third-party payment providers (such as Gumroad, Alipay, WeChat Pay, or card networks). We only receive the information needed to fulfill and support your license, such as your order reference and the email address you provided to the payment provider.',
        'Payment providers process your payment details under their own privacy policies, which we encourage you to review.'
      ]
    },
    {
      heading: 'Contact and Support',
      paragraphs: [
        'If you email us at contact@monosub.com, we receive and retain your email address and the content of your message solely to respond to your inquiry and provide support. We do not use this information for marketing unless you ask us to.'
      ]
    },
    {
      heading: 'Cookies and Local Storage',
      paragraphs: [
        'The MonoSub website uses your browser\u2019s local storage to remember your language preference. We do not use advertising cookies or track you across websites.'
      ]
    },
    {
      heading: 'Analytics and Telemetry',
      paragraphs: [
        'The current version of MonoSub does not include analytics, telemetry, or crash-reporting SDKs. If we ever add such features, we will update this policy before doing so.'
      ]
    },
    {
      heading: 'Data Retention',
      paragraphs: [
        'Support correspondence is kept only as long as needed to handle your request. License records are retained for legitimate business and legal purposes, including refund eligibility and accounting.'
      ]
    },
    {
      heading: 'Children\u2019s Privacy',
      paragraphs: [
        'MonoSub is not directed to children under 13 (or the minimum age set by law in your region), and we do not knowingly collect personal information from children.'
      ]
    },
    {
      heading: 'Your Rights',
      paragraphs: [
        'Depending on where you live, you may have rights to access, correct, or delete the limited personal information we hold about you (for example, under the GDPR, the CCPA/CPRA, or China\u2019s Personal Information Protection Law). To exercise these rights, contact us at contact@monosub.com.'
      ]
    },
    {
      heading: 'International Transfers',
      paragraphs: [
        'Because MonoSub does not receive your media files, there is nothing to transfer for the core product. The limited data we do handle (such as support emails) may be stored with providers located in countries other than your own, and we apply appropriate safeguards when this happens.'
      ]
    },
    {
      heading: 'Changes to This Policy',
      paragraphs: [
        'We may update this policy from time to time. The effective date at the top of this page reflects the latest version, and material changes will be announced on our website.'
      ]
    },
    {
      heading: 'Contact',
      paragraphs: ['Questions about this policy or your data? Email contact@monosub.com.']
    }
  ]
};

const zh: PrivacyContent = {
  meta_title: '隐私政策 - MonoSub',
  meta_description:
    '了解 MonoSub 如何处理你的数据：所有处理均在本地设备完成，不上传云端、无需账户、无追踪。',
  title: '隐私政策',
  last_updated_label: '最后更新',
  last_updated: '2026年10月23日',
  intro:
    'MonoSub 是一款桌面应用，使用完全在本地运行的 AI 模型将视频和音频转换为字幕。本政策说明我们会处理哪些数据、数据保存在哪里，以及在极少数情况下信息会被发送给第三方。',
  back_home: '← 返回首页',
  sections: [
    {
      heading: '你的媒体文件只保存在本地',
      paragraphs: [
        '你在 MonoSub 中处理的视频、音频文件，以及生成的转录文本、字幕和设置，都在你的电脑上本地完成处理。我们不会把你的文件上传到任何服务器，也没有接收这些文件的基础设施。',
        '使用应用无需注册账户，我们也不要求你提供姓名、邮箱或其他个人信息。'
      ]
    },
    {
      heading: '网络活动',
      paragraphs: ['MonoSub 完全支持离线使用。仅有的可选网络行为包括：'],
      list: [
        '更新检查：当你手动检查更新时，应用会向 GitHub 请求最新版本信息，并发送你的应用版本号（与任何网络请求一样，也会包含 IP 地址）。不会传输任何媒体或个人数据。',
        '链接导入：如果你通过 URL 导入内容，应用会直接从你指定的来源下载该内容。下载的文件在本地处理，绝不会发送给我们。'
      ]
    },
    {
      heading: '许可证购买',
      paragraphs: [
        '当你购买 Pro 许可证时，支付由第三方支付服务商（如 Gumroad、支付宝、微信支付或银行卡网络）处理。我们仅接收发放和支持许可证所需的信息，例如订单编号以及你提供给支付服务商的邮箱。',
        '支付服务商会根据其自身的隐私政策处理你的支付信息，建议你查看相应政策。'
      ]
    },
    {
      heading: '联系与支持',
      paragraphs: [
        '如果你通过 contact@monosub.com 给我们发送邮件，我们会收到并保存你的邮箱地址和邮件内容，仅用于回复你的问题并提供支持。除非你明确要求，我们不会将这些信息用于营销。'
      ]
    },
    {
      heading: 'Cookie 与本地存储',
      paragraphs: [
        'MonoSub 网站使用浏览器本地存储来记住你的语言偏好。我们不会使用广告 Cookie，也不会跨网站追踪你。'
      ]
    },
    {
      heading: '分析与遥测',
      paragraphs: [
        '当前版本的 MonoSub 不包含任何分析、遥测或崩溃上报 SDK。如果将来添加此类功能，我们会先更新本政策。'
      ]
    },
    {
      heading: '数据保留',
      paragraphs: [
        '支持往来邮件仅在处理你的请求所需的时间内保留。许可证记录会基于正当业务和法律目的（包括退款资格和财务记账）进行保留。'
      ]
    },
    {
      heading: '儿童隐私',
      paragraphs: [
        'MonoSub 不面向 13 岁以下儿童（或你所在地区法律规定的最低年龄），我们也不会故意收集儿童的个人信息。'
      ]
    },
    {
      heading: '你的权利',
      paragraphs: [
        '根据你所在地区的法律（例如 GDPR、CCPA/CPRA 或中国《个人信息保护法》），你可能有权访问、更正或删除我们持有的关于你的少量个人信息。如需行使这些权利，请通过 contact@monosub.com 与我们联系。'
      ]
    },
    {
      heading: '国际数据传输',
      paragraphs: [
        '由于 MonoSub 不会接收你的媒体文件，核心产品不存在数据传输。我们处理的少量数据（如支持邮件）可能存储在与你所在国家/地区不同的服务商处，发生这种情况时我们会采取适当的保护措施。'
      ]
    },
    {
      heading: '政策更新',
      paragraphs: [
        '我们可能不时更新本政策。本页顶部的生效日期反映最新版本，重大变更会在我们的网站上公布。'
      ]
    },
    {
      heading: '联系我们',
      paragraphs: ['对本政策或你的数据有疑问？请发送邮件至 contact@monosub.com。']
    }
  ]
};

export const privacyContent: Record<string, PrivacyContent> = {
  en,
  zh
};
