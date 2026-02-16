# Speaker Diarization Feature - Technical Specification

## Overview

Add multi-speaker recognition (speaker diarization) capability to MonoSub using FunASR ecosystem.

**Goal**: Enable subtitles to distinguish between speakers with labels like `[Speaker A]`, `[Speaker B]`.

---

## Architecture

### Two-Stage Pipeline

```
┌─────────────────────────────────────────────────────────────────────┐
│  Audio Input                                                         │
│      ↓                                                               │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ Stage 1: Speaker Diarization                                   │  │
│  │ Model: iic/speech_campplus_speaker-diarization_common          │  │
│  │ Output: [(start, end, speaker_id), ...]                        │  │
│  │ Example: [(0.0, 3.5, 'spk_0'), (3.8, 6.2, 'spk_1'), ...]       │  │
│  └───────────────────────────────────────────────────────────────┘  │
│      ↓                                                               │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ Stage 2: ASR (existing SenseVoice)                             │  │
│  │ Model: iic/SenseVoiceSmall                                     │  │
│  │ Output: [(start, end, text), ...]                              │  │
│  └───────────────────────────────────────────────────────────────┘  │
│      ↓                                                               │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ Stage 3: Merge by Timestamp Overlap                            │  │
│  │ Output: [(start, end, text, speaker), ...]                     │  │
│  │ Example: [(0.0, 1.5, '你好', 'Speaker A'), ...]                │  │
│  └───────────────────────────────────────────────────────────────┘  │
│      ↓                                                               │
│  Subtitle Output with Speaker Labels                                 │
└─────────────────────────────────────────────────────────────────────┘
```

### When Diarization is Disabled (Default)

```
Audio Input → SenseVoice ASR → Subtitle Output (no speaker labels)
```

---

## Implementation Details

### 1. Python Engine Changes

#### 1.1 New File: `core-engine/monosub_core/diarizer.py`

```python
"""Speaker Diarization using FunASR CAM++ model"""

from funasr import AutoModel
from typing import List, Tuple, Optional, Callable

class SpeakerDiarizer:
    """
    Speaker diarization using iic/speech_campplus_speaker-diarization_common
    """
    
    MODEL_ID = "iic/speech_campplus_speaker-diarization_common"
    
    def __init__(self, device: str = "auto"):
        self.device = device
        self.model = None
    
    def load_model(self, progress_callback: Optional[Callable] = None):
        """Load the CAM++ speaker diarization model"""
        if progress_callback:
            progress_callback(5, "Loading speaker diarization model...")
        
        self.model = AutoModel(model=self.MODEL_ID, device=self.device)
    
    def diarize(self, audio_path: str) -> List[Tuple[float, float, str]]:
        """
        Perform speaker diarization on audio file.
        
        Returns:
            List of (start_seconds, end_seconds, speaker_id) tuples
        """
        if not self.model:
            self.load_model()
        
        result = self.model.generate(audio=audio_path)
        
        # Parse result: [[start_sec, end_sec, speaker_id], ...]
        segments = []
        for item in result:
            if isinstance(item, (list, tuple)) and len(item) >= 3:
                start, end, spk = item[0], item[1], item[2]
                segments.append((float(start), float(end), str(spk)))
        
        return segments
    
    @staticmethod
    def assign_speakers(
        asr_segments: List[dict],
        diarization_segments: List[Tuple[float, float, str]]
    ) -> List[dict]:
        """
        Assign speakers to ASR segments based on timestamp overlap.
        
        Args:
            asr_segments: List of {start, end, text} from ASR
            diarization_segments: List of (start, end, speaker_id) from diarization
        
        Returns:
            List of {start, end, text, speaker} with speaker labels
        """
        # Map speaker IDs to readable labels
        speaker_map = {}
        speaker_counter = 0
        
        for seg in asr_segments:
            seg_start = seg['start']
            seg_end = seg['end']
            
            # Find best matching speaker by maximum overlap
            best_speaker = None
            best_overlap = 0
            
            for d_start, d_end, spk_id in diarization_segments:
                # Calculate overlap
                overlap_start = max(seg_start, d_start)
                overlap_end = min(seg_end, d_end)
                overlap = max(0, overlap_end - overlap_start)
                
                if overlap > best_overlap:
                    best_overlap = overlap
                    best_speaker = spk_id
            
            # Map to readable label
            if best_speaker and best_speaker not in speaker_map:
                speaker_map[best_speaker] = chr(ord('A') + speaker_counter)
                speaker_counter += 1
            
            seg['speaker'] = f"Speaker {speaker_map.get(best_speaker, '?')}" if best_speaker else None
        
        return asr_segments
```

#### 1.2 Modify: `core-engine/monosub_core/transcriber.py`

Add method to FunASRTranscriber:

```python
def transcribe_with_diarization(
    self,
    audio_path: str,
    progress_callback: ProgressCallback,
    language: str = "zh"
) -> List[Dict]:
    """
    Transcribe audio with speaker diarization.
    """
    from .diarizer import SpeakerDiarizer
    
    # Stage 1: Speaker diarization (0-30%)
    progress_callback.update(5, "Loading diarization model...")
    diarizer = SpeakerDiarizer(device=self.device)
    diarizer.load_model()
    
    progress_callback.update(15, "Analyzing speakers...")
    diarization_segments = diarizer.diarize(audio_path)
    
    progress_callback.update(30, "Speaker analysis complete")
    
    # Stage 2: ASR (30-85%)
    segments = self.transcribe(audio_path, progress_callback, language)
    
    # Stage 3: Merge (85-92%)
    progress_callback.update(88, "Assigning speakers...")
    segments = SpeakerDiarizer.assign_speakers(segments, diarization_segments)
    
    # If only one speaker detected, remove speaker labels for cleaner output
    unique_speakers = set(s.get('speaker') for s in segments if s.get('speaker'))
    if len(unique_speakers) <= 1:
        for seg in segments:
            seg['speaker'] = None
    
    return segments
```

#### 1.3 Modify: `core-engine/main.py`

Add CLI argument:

```python
parser.add_argument(
    "--diarization",
    action="store_true",
    help="Enable speaker diarization (multi-speaker recognition)",
)
```

Update main logic:

```python
if args.diarization:
    print("MSG:Multi-speaker mode enabled", flush=True)
    segments = transcriber.transcribe_with_diarization(audio_path, progress_callback, args.language)
else:
    segments = transcriber.transcribe(audio_path, progress_callback, args.language)
```

---

### 2. Subtitle Writer Changes

#### Modify: `core-engine/monosub_core/subtitle_writer.py`

Update `write_srt` method to support speaker labels:

```python
@staticmethod
def write_srt(
    segments: List[Dict],
    output_path: str,
    translation_segments: Optional[List[Dict]] = None
) -> None:
    with open(output_path, 'w', encoding='utf-8') as f:
        for idx, segment in enumerate(segments, 1):
            start = SubtitleWriter.format_timestamp_srt(segment['start'])
            end = SubtitleWriter.format_timestamp_srt(segment['end'])
            text = segment['text'].strip()
            speaker = segment.get('speaker')  # NEW
            
            f.write(f"{idx}\n")
            f.write(f"{start} --> {end}\n")
            
            # Add speaker label if present
            if speaker:
                text = f"[{speaker}]: {text}"
            
            # Handle bilingual...
            f.write(f"{text}\n\n")
```

Same pattern for `write_vtt` and `write_txt`.

---

### 3. Rust Backend Changes

#### Modify: `src-tauri/src/commands/transcribe.rs`

Update `TranscribeRequest`:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct TranscribeRequest {
    pub id: String,
    pub path: String,
    pub output_path: String,
    pub format: Option<String>,
    pub language: Option<String>,
    pub enable_diarization: Option<bool>,  // NEW
}
```

Update command argument passing in both `run_dev_mode` and `run_sidecar`:

```rust
let mut args = vec![
    video_path.to_string(),
    output_path.to_string(),
    "--format".to_string(),
    format.to_string(),
    "--language".to_string(),
    language.to_string(),
];

if enable_diarization {
    args.push("--diarization".to_string());
}
```

---

### 4. Frontend Changes

#### 4.1 Modify: `src/store/useStore.ts`

Add to state:

```typescript
interface AppState {
  // ... existing fields
  
  // Speaker diarization toggle
  enableSpeakerDiarization: boolean
  setEnableSpeakerDiarization: (enabled: boolean) => void
}
```

Add implementation:

```typescript
enableSpeakerDiarization: false,
setEnableSpeakerDiarization: (enabled) => set({ enableSpeakerDiarization: enabled }),
```

#### 4.2 Modify: `src/components/BottomBar.tsx`

Add switch component before the Stop button (right side):

```tsx
import { Users } from 'lucide-react';

// In component:
const enableSpeakerDiarization = useStore((state) => state.enableSpeakerDiarization);
const setEnableSpeakerDiarization = useStore((state) => state.setEnableSpeakerDiarization);

// In JSX, inside right-side div, before Stop button:
<div className="flex items-center gap-2 mr-3">
  <Users className="w-4 h-4 text-text-tertiary dark:text-gray-500" />
  <span className="text-xs text-text-tertiary dark:text-gray-500">
    {t('bottomBar.multiSpeaker')}
  </span>
  <button
    onClick={() => setEnableSpeakerDiarization(!enableSpeakerDiarization)}
    className={`relative w-10 h-5 rounded-full transition-colors ${
      enableSpeakerDiarization 
        ? 'bg-primary' 
        : 'bg-bg-secondary dark:bg-zinc-700'
    }`}
  >
    <span
      className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white shadow transition-transform ${
        enableSpeakerDiarization ? 'translate-x-5' : ''
      }`}
    />
  </button>
</div>
```

#### 4.3 Modify: `src/lib/transcription.ts`

Pass diarization flag:

```typescript
await invoke('start_transcription', {
  request: {
    id: file.id,
    path: file.path,
    output_path: outputPath,
    format: state.settings.outputFormat,
    language: state.settings.outputLanguage,
    enable_diarization: state.enableSpeakerDiarization,  // NEW
  },
});
```

#### 4.4 Add i18n keys

In `src/i18n/locales/en.json`:
```json
{
  "bottomBar": {
    "multiSpeaker": "Multi-speaker"
  }
}
```

In `src/i18n/locales/zh.json`:
```json
{
  "bottomBar": {
    "multiSpeaker": "多人识别"
  }
}
```

---

## Output Examples

### SRT with Speaker Labels

```
1
00:00:01,000 --> 00:00:03,500
[Speaker A]: 你好，今天我们来讨论一下项目进度。

2
00:00:03,800 --> 00:00:06,200
[Speaker B]: 好的，我先汇报一下开发情况。

3
00:00:06,500 --> 00:00:08,900
[Speaker A]: 请说。
```

### Single Speaker (auto-detected, no labels)

```
1
00:00:01,000 --> 00:00:03,500
你好，今天我们来讨论一下项目进度。

2
00:00:03,800 --> 00:00:06,200
首先说一下开发情况。
```

---

## Performance Considerations

| Aspect | Without Diarization | With Diarization |
|--------|--------------------|--------------------|
| Model Load | ~5s (SenseVoice) | ~8s (+CAM++) |
| Processing | ~0.1x realtime | ~0.12x realtime |
| Memory | ~2GB | ~3GB |
| First Download | ~500MB | ~700MB (+200MB) |

---

## Testing Checklist

- [ ] Single speaker audio → No speaker labels
- [ ] Two speakers audio → [Speaker A], [Speaker B] labels
- [ ] Three+ speakers audio → [Speaker A], [Speaker B], [Speaker C] labels
- [ ] Toggle OFF → Original fast logic
- [ ] Toggle ON → Two-stage pipeline
- [ ] All output formats (SRT, VTT, TXT)
- [ ] Chinese, English, and auto language modes
