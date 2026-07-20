//! Transcript loading and segmentation: plain text or timestamped JSONL from
//! the transcription stage, split into sentence spans with byte-offset
//! provenance.

use serde::Deserialize;

/// A transcript with optional time anchors (byte offset → seconds).
pub struct Transcript {
    pub text: String,
    anchors: Vec<(usize, f64)>,
}

#[derive(Deserialize)]
struct Segment {
    start: f64,
    text: String,
}

impl Transcript {
    /// Load from plain text (no time anchors) or JSONL segments
    /// (`{"start": seconds, "text": "..."}` per line, as produced by the
    /// transcription stage).
    pub fn load(raw: &str) -> Transcript {
        let trimmed = raw.trim_start();
        if trimmed.starts_with('{')
            && let Some(t) = Self::from_jsonl(raw)
        {
            return t;
        }
        Transcript { text: raw.to_string(), anchors: Vec::new() }
    }

    fn from_jsonl(raw: &str) -> Option<Transcript> {
        let mut text = String::new();
        let mut anchors = Vec::new();
        for line in raw.lines().filter(|l| !l.trim().is_empty()) {
            let seg: Segment = serde_json::from_str(line).ok()?;
            if !text.is_empty() {
                text.push(' ');
            }
            anchors.push((text.len(), seg.start));
            text.push_str(seg.text.trim());
        }
        Some(Transcript { text, anchors })
    }

    /// "HH:MM:SS" anchor for a byte offset, or empty when no timestamps exist.
    /// Anchors are built in ascending offset order, so binary search applies.
    pub fn time_at(&self, offset: usize) -> String {
        let idx = self.anchors.partition_point(|(o, _)| *o <= offset);
        match idx.checked_sub(1).map(|i| self.anchors[i].1) {
            Some(s) => {
                let s = s as u64;
                format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
            }
            None => String::new(),
        }
    }

    /// Sentence spans as byte offsets (split on ./!/? or blank lines).
    pub fn sentences(&self) -> Vec<(usize, usize)> {
        let bytes = self.text.as_bytes();
        let mut spans = Vec::new();
        let mut start = 0usize;
        let mut i = 0usize;
        while i < bytes.len() {
            let b = bytes[i];
            let is_end = matches!(b, b'.' | b'!' | b'?')
                && bytes.get(i + 1).is_none_or(|n| n.is_ascii_whitespace());
            let is_break = b == b'\n' && bytes.get(i + 1) == Some(&b'\n');
            if is_end || is_break {
                let end = if is_end { i + 1 } else { i };
                push_trimmed(&self.text, start, end, &mut spans);
                start = end;
                i = end + 1;
            } else {
                i += 1;
            }
        }
        push_trimmed(&self.text, start, bytes.len(), &mut spans);
        spans
    }
}

fn push_trimmed(text: &str, start: usize, end: usize, spans: &mut Vec<(usize, usize)>) {
    if start >= end || end > text.len() {
        return;
    }
    let slice = &text[start..end];
    let s = start + (slice.len() - slice.trim_start().len());
    let e = end - (slice.len() - slice.trim_end().len());
    if s < e {
        spans.push((s, e));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonl_transcript_carries_time_anchors() {
        let raw = "{\"start\": 0.0, \"text\": \"Hello there.\"}\n{\"start\": 75.5, \"text\": \"Your sun is in cancer.\"}\n";
        let t = Transcript::load(raw);
        assert_eq!(t.text, "Hello there. Your sun is in cancer.");
        assert_eq!(t.time_at(13), "00:01:15");
        assert_eq!(t.time_at(0), "00:00:00");
    }
}
