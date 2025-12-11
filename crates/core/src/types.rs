use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Transcript {
    pub text: String,
    pub segments: Vec<Segment>,
    pub language: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoReport {
    pub title: String,
    pub summary: String,
    pub duration_minutes: f64,
    pub language: String,
    pub difficulty: String, // "Easy to understand" | "Moderate" | "Cognitively demanding"
    pub key_takeaways: Vec<String>,
    pub sections: Vec<Section>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Section {
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub title: String,
    pub summary: String,
}
