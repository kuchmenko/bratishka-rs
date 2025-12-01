# bratishka

CLI tool to download YouTube videos, transcribe with Whisper, and generate AI-powered reports.

## Features

- Download videos from YouTube using yt-dlp
- Extract audio and transcribe with OpenAI Whisper
- Generate structured reports with AI (Grok, OpenAI, or Gemini)
- Smart caching - skip already-completed steps
- Multi-language report generation

## Requirements

- [yt-dlp](https://github.com/yt-dlp/yt-dlp) - Video downloader
- [ffmpeg](https://ffmpeg.org/) - Audio extraction
- [whisper](https://github.com/openai/whisper) - Speech-to-text
- One of: `XAI_API_KEY`, `OPENAI_API_KEY`, or `GEMINI_API_KEY`

### Install dependencies

```bash
# macOS
brew install yt-dlp ffmpeg
pip install openai-whisper

# Arch Linux
sudo pacman -S yt-dlp ffmpeg
pip install openai-whisper

# Ubuntu/Debian
sudo apt install ffmpeg
pip install yt-dlp openai-whisper
```

## Installation

### From source

```bash
cargo install --git https://github.com/kuchmenko/bratishka-rs
```

### From GitHub releases

Download the latest release for your platform from [Releases](https://github.com/kuchmenko/bratishka-rs/releases).

## Usage

```bash
# Basic usage (uses Grok by default)
export XAI_API_KEY=your-key
bratishka "https://youtube.com/watch?v=..."

# Use OpenAI
export OPENAI_API_KEY=your-key
bratishka "https://youtube.com/watch?v=..." -p openai

# Use Gemini
export GEMINI_API_KEY=your-key
bratishka "https://youtube.com/watch?v=..." -p gemini

# Force specific report language
bratishka "https://youtube.com/watch?v=..." -l en

# Force re-processing (ignore cache)
bratishka "https://youtube.com/watch?v=..." --force
```

### Options

```
Arguments:
  <URL>  Video URL

Options:
  -l, --lang <LANG>          Report language (defaults to video's detected language)
  -p, --provider <PROVIDER>  AI provider [default: grok] [possible values: grok, openai, gemini]
  -f, --force                Force re-processing even if cached files exist
  -h, --help                 Print help
```

## Output

Reports are cached in `~/.cache/bratishka/<url-hash>/` and include:

- `video.*` - Downloaded video
- `audio.wav` - Extracted audio
- `transcript.json` - Whisper transcription
- `report_<provider>_<lang>.json` - AI-generated report

### Report structure

```json
{
  "title": "Video title",
  "summary": "2-3 sentence summary",
  "duration_minutes": 45.5,
  "language": "en",
  "difficulty": "Intermediate",
  "topics": ["topic1", "topic2"],
  "key_takeaways": ["takeaway1", "takeaway2"],
  "chapters": [
    {
      "start_seconds": 0,
      "end_seconds": 180,
      "title": "Introduction",
      "summary": "Chapter summary"
    }
  ],
  "prerequisites": ["prerequisite1"],
  "target_audience": "Who this video is for"
}
```

## License

MIT
