# MonoSub 安装指南

欢迎使用 MonoSub！这是一个本地语音转字幕的桌面应用。

## 系统要求

- **操作系统**: macOS 11.0 或更高版本
- **处理器**: Apple Silicon (M1/M2/M3) 或 Intel x86_64
- **内存**: 至少 4GB RAM (推荐 8GB)
- **磁盘空间**: 至少 5GB 可用空间

## 安装步骤

### 1. 安装应用

将 **MonoSub.app** 拖动到 **Applications** 文件夹。

### 2. 安装依赖

MonoSub 需要 Python 3.9+ 和一些 Python 库才能工作。

#### 选项 A: 自动安装 (推荐)

打开终端，运行以下命令：

```bash
# 下载并运行依赖安装脚本
curl -fsSL https://raw.githubusercontent.com/your-repo/monosub/main/scripts/install-deps.sh | bash
```

#### 选项 B: 手动安装

1. **安装 Python** (如果尚未安装)

```bash
# 使用 Homebrew
brew install python@3.11

# 或从官网下载: https://www.python.org/downloads/
```

2. **安装 Python 依赖**

```bash
pip3 install funasr modelscope ffmpeg-python torch torchaudio --user
```

### 3. 首次运行

1. 在 Applications 文件夹中双击 **MonoSub.app**
2. 如果看到"无法打开"警告，请到 **系统偏好设置 > 安全性与隐私** 中允许
3. 应用会自动检测依赖，如有缺失会提示您安装

## 使用说明

1. **选择文件**: 点击"选择文件"按钮，选择视频或音频文件
2. **选择输出格式**: 支持 SRT、VTT、TXT 格式
3. **开始转录**: 点击"开始转录"按钮
4. **查看结果**: 转录完成后，字幕文件会保存在与源文件相同的目录

## 支持的文件格式

### 视频格式
- MP4, MOV, AVI, MKV, FLV, WMV

### 音频格式
- MP3, WAV, FLAC, AAC, M4A, OGG, WMA

## 常见问题

### 应用无法打开

**问题**: 双击应用后没有反应或提示"已损坏"

**解决方案**:
```bash
# 移除隔离属性
xattr -cr /Applications/MonoSub.app

# 允许从任何来源打开应用
sudo spctl --master-disable
```

### 选择文件后没有显示

**问题**: 选择文件后，文件列表不显示

**解决方案**: 这通常是因为依赖未安装

1. 检查 Python: `python3 --version`
2. 检查 FFmpeg: `ffmpeg -version`
3. 重新运行依赖安装脚本

### 转录失败

**问题**: 点击开始转录后失败或卡住

**可能原因**:
1. Python 依赖包未安装或版本不兼容
2. 文件格式不支持
3. 磁盘空间不足

**解决方案**:
1. 重新安装 Python 依赖
2. 转换文件格式为 MP4/MP3
3. 清理磁盘空间

### 模型下载慢

**问题**: 首次使用时模型下载很慢

**解决方案**:
- FunASR 模型会在首次使用时自动下载（约 1-2GB）
- 请保持网络连接并耐心等待
- 模型会缓存在本地，后续使用无需重复下载

## 卸载

如需卸载 MonoSub:

1. 删除应用:
```bash
rm -rf /Applications/MonoSub.app
```

2. 删除用户数据 (可选):
```bash
rm -rf ~/Library/Application\ Support/com.monosub.client
rm -rf ~/Library/Caches/com.monosub.client
```

3. 卸载 Python 依赖 (可选):
```bash
pip3 uninstall funasr modelscope ffmpeg-python torch torchaudio -y
```

## 获取帮助

如果遇到问题，请:

1. 查看 [GitHub Issues](https://github.com/your-repo/monosub/issues)
2. 提交新的 Issue 并附上错误日志
3. 发送邮件至: support@monosub.app

## 隐私说明

MonoSub 完全在本地运行，不会上传任何数据到云端:

- ✅ 所有语音识别在本地进行
- ✅ 文件和字幕保存在您的设备上
- ✅ 不收集任何用户数据
- ✅ 不需要网络连接 (除了首次下载模型)

---

感谢使用 MonoSub! 🎉
