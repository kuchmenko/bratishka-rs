use std::{
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
};

use crate::provider::Provider;

/// Get the cache directory for a given URL
pub fn get_cache_dir(url: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let url_hash = hasher.finish();
    let cache_dir = get_root_cache_dir();

    cache_dir.join(url_hash.to_string())
}

pub fn get_root_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("bratishka")
}

pub fn get_model_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("models")
}

/// Find a video file in the cache directory
pub fn find_video_in_cache(cache_dir: &Path) -> Option<PathBuf> {
    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        return None;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            if matches!(ext.as_str(), "mp4" | "webm" | "mkv" | "mov" | "avi") {
                return Some(path);
            }
        }
    }
    None
}

/// Get the path for a cached audio file
pub fn get_audio_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join("audio.wav")
}

/// Get the path for a cached transcript file
pub fn get_transcript_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join("transcript.json")
}

/// Get the path for a cached report file (provider and language aware)
pub fn get_report_path(cache_dir: &Path, provider: &Provider, lang: &str) -> PathBuf {
    let provider_name = match provider {
        Provider::Grok => "grok",
        Provider::Openai => "openai",
        Provider::Gemini => "gemini",
    };
    cache_dir.join(format!("report_{}_{}.json", provider_name, lang))
}
