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

/// Format a video report as human-readable markdown
pub fn format_report_readable(report: &VideoReport) -> String {
    let mut output = String::new();

    // Title
    output.push_str(&format!("# {}\n\n", report.title));

    // Meta info
    output.push_str(&format!(
        "**Duration:** {:.0} minutes | **Difficulty:** {} | **Language:** {}\n\n",
        report.duration_minutes, report.difficulty, report.language
    ));

    // Summary
    output.push_str("## Summary\n\n");
    output.push_str(&report.summary);
    output.push_str("\n\n");

    // Topics
    output.push_str("## Topics Covered\n\n");
    for topic in &report.topics {
        output.push_str(&format!("• {}\n", topic));
    }
    output.push('\n');

    // Key Takeaways
    output.push_str("## Key Takeaways\n\n");
    for (i, takeaway) in report.key_takeaways.iter().enumerate() {
        output.push_str(&format!("{}. {}\n", i + 1, takeaway));
    }
    output.push('\n');

    // Chapters
    output.push_str("## Chapters\n\n");
    for chapter in &report.chapters {
        let start = format_timestamp(chapter.start_seconds);
        let end = format_timestamp(chapter.end_seconds);
        output.push_str(&format!("### [{}–{}] {}\n\n", start, end, chapter.title));
        output.push_str(&format!("{}\n\n", chapter.summary));
    }

    // Prerequisites
    if !report.prerequisites.is_empty() {
        output.push_str("## Prerequisites\n\n");
        for prereq in &report.prerequisites {
            output.push_str(&format!("• {}\n", prereq));
        }
        output.push('\n');
    }

    // Target Audience
    output.push_str("## Target Audience\n\n");
    output.push_str(&report.target_audience);
    output.push('\n');

    output
}
