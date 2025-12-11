use crate::types::{Transcript, VideoReport};

/// Format seconds as MM:SS timestamp
pub fn format_timestamp(seconds: f64) -> String {
    let mins = (seconds / 60.0) as u32;
    let secs = (seconds % 60.0) as u32;
    format!("{:02}:{:02}", mins, secs)
}

/// Format transcript segments with timestamps
pub fn format_transcript_with_timestamps(transcript: &Transcript) -> String {
    transcript
        .segments
        .iter()
        .map(|seg| format!("[{}] {}", format_timestamp(seg.start), seg.text.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn format_report_readable(report: &VideoReport) -> String {
    let mut output = String::new();
    output.push_str(&format!("# {}\n\n", report.title));
    output.push_str(&format!(
        "**Duration:** {:.0} minutes | **Difficulty:** {} | **Language:** {}\n\n",
        report.duration_minutes, report.difficulty, report.language
    ));

    output.push_str("## Key takeaways\n\n");
    for topic in &report.key_takeaways {
        output.push_str(&format!("• {}\n", topic));
    }
    output.push('\n');

    output.push_str("## Summary\n\n");
    output.push_str(&report.summary);
    output.push_str("\n\n");

    output.push_str("## Sections\n\n");
    for chapter in &report.sections {
        let start = format_timestamp(chapter.start_seconds);
        let end = format_timestamp(chapter.end_seconds);
        output.push_str(&format!("### [{}–{}] {}\n\n", start, end, chapter.title));
        output.push_str(&format!("{}\n\n", chapter.summary));
    }

    output.push('\n');

    output
}
