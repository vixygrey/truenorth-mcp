//! Bounded, line-aware diagnostic excerpts for gate failures.
//!
//! The executor retains an actionable error and final summary without returning a raw byte slice
//! that can start inside a long compiler invocation. Excerpts never persist command output.

/// Maximum bytes returned in a gate diagnostic excerpt.
pub const MAX_DIAGNOSTIC_EXCERPT_BYTES: usize = 2 * 1024;
const MAX_DIAGNOSTIC_LINE_BYTES: usize = 512;
const TRUNCATION_MARKER: &str =
    "[stderr excerpt truncated; rerun the configured gate locally for full output]\n";
const LINE_TRUNCATION_MARKER: &str = " [line truncated]\n";

/// Bounded, redacted stderr selected for an MCP gate failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticExcerpt {
    /// The returned diagnostic text.
    pub text: String,
    /// Whether the original stderr could not be represented in full.
    pub truncated: bool,
    /// Number of stderr bytes captured after redaction.
    pub captured_bytes: usize,
    /// Number of bytes returned in [`Self::text`].
    pub returned_bytes: usize,
}

/// Build a bounded excerpt that keeps the last actionable diagnostic and final summary.
pub fn diagnostic_excerpt(stderr: &[u8]) -> DiagnosticExcerpt {
    let captured_bytes = stderr.len();
    let stderr = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = stderr.split_inclusive('\n').collect();
    let start = lines
        .iter()
        .rposition(is_actionable_diagnostic)
        .unwrap_or(0);
    let selected = &lines[start..];
    let (lines, line_truncated) = truncate_lines(selected);
    let complete = lines.concat();

    if !line_truncated && complete.len() <= MAX_DIAGNOSTIC_EXCERPT_BYTES {
        return DiagnosticExcerpt {
            returned_bytes: complete.len(),
            text: complete,
            truncated: false,
            captured_bytes,
        };
    }

    let text = bounded_excerpt(&lines);
    DiagnosticExcerpt {
        returned_bytes: text.len(),
        text,
        truncated: true,
        captured_bytes,
    }
}

fn is_actionable_diagnostic(line: &&str) -> bool {
    let line = line.trim_start();
    line.starts_with("error[")
        || (line.starts_with("error:")
            && !line.starts_with("error: could not compile")
            && !line.starts_with("error: aborting due to"))
        || line.starts_with("fatal error:")
}

fn truncate_lines(lines: &[&str]) -> (Vec<String>, bool) {
    let mut truncated = false;
    let lines = lines
        .iter()
        .map(|line| {
            let (line, did_truncate) = truncate_line(line);
            truncated |= did_truncate;
            line
        })
        .collect();
    (lines, truncated)
}

fn truncate_line(line: &str) -> (String, bool) {
    if line.len() <= MAX_DIAGNOSTIC_LINE_BYTES {
        return (line.to_string(), false);
    }

    let limit = MAX_DIAGNOSTIC_LINE_BYTES - LINE_TRUNCATION_MARKER.len();
    (
        format!(
            "{}{}",
            truncate_to_bytes(line, limit),
            LINE_TRUNCATION_MARKER
        ),
        true,
    )
}

fn bounded_excerpt(lines: &[String]) -> String {
    let mut excerpt = lines.first().cloned().unwrap_or_default();
    let mut suffix = Vec::new();
    let mut index = lines.len();

    while index > 1 {
        index -= 1;
        let line = &lines[index];
        if excerpt.len() + TRUNCATION_MARKER.len() + line.len() > MAX_DIAGNOSTIC_EXCERPT_BYTES {
            break;
        }
        suffix.push(line.as_str());
    }

    suffix.reverse();
    excerpt.push_str(TRUNCATION_MARKER);
    for line in suffix {
        excerpt.push_str(line);
    }
    excerpt
}

fn truncate_to_bytes(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }

    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
