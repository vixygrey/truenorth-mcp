//! The pruning aspect: relevance-based context pruning over a log (jev-integration-eval R6).
//!
//! The aspect rates each log line's relevance to a task, then keeps the relevant lines and
//! drops the rest. The unit is one line (R6.1). The harness builds one Score question per
//! line, batches the questions into one request, and reads each line's Score answer by its
//! id. The harness owns the keep-or-drop step, so the decision is deterministic given the
//! model scores.
//!
//! The model is advisory. It returns one Score per line; the harness keeps a line when its
//! score is at or above the keep threshold and drops it otherwise (R6.2, R6.3). The kept
//! lines preserve the original order (R6.4).
//!
//! A whole-log request can exceed the token budget. The harness splits the lines into
//! ordered chunks, each chunk's request at or under the budget, and calls once per chunk in
//! order (R6.5). A single line that alone exceeds the budget goes in its own chunk as a best
//! effort, because the estimate is approximate and the line is not dropped.
//!
//! The aspect has no non-test caller until the benchmark bin lands (task 12). Each public
//! item carries a narrow non-test `allow` with this reason, per the repo dead-code policy
//! (main.rs). The task that wires the benchmark removes the attributes. The test build
//! exercises every item through the sibling `pruning_tests.rs` and `pruning_prop_tests.rs`.
//!
//! Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7. Design: jev-integration-eval, the
//! pruning aspect, Property 26 (line conservation), Property 27 (order preservation).

use std::collections::BTreeMap;

use super::{Answer, JevClient, JevError, JevRequest, Question, TOKEN_BUDGET};

/// The ordered relevance rubric levels. The Score answer returns an index on this scale.
///
/// The rubric has two levels, so the returned Score index is 0 (irrelevant) or 1 (relevant).
/// The keep threshold compares against this scale.
const RELEVANCE_LEVELS: [&str; 2] = ["irrelevant", "relevant"];

/// Build the question id for a line at the given zero-based index.
///
/// The ids are `line_0`, `line_1`, and so on, so each line's Score answer returns under a
/// stable id the harness reads back.
fn line_id(index: usize) -> String {
    format!("line_{index}")
}

/// The outcome of one pruning evaluation (jev-integration-eval R6).
///
/// The `kept` vector holds the kept lines in their original order (R6.4). The three counts
/// satisfy `kept_count + dropped_count == input_count` and `input_count` equals the number
/// of input lines (R6.6, R6.7).
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub struct PruningOutcome {
    /// The kept lines, in their original order.
    pub kept: Vec<String>,
    /// The number of kept lines.
    pub kept_count: usize,
    /// The number of dropped lines.
    pub dropped_count: usize,
    /// The number of input lines. Equals `kept_count + dropped_count`.
    pub input_count: usize,
}

/// Select the kept lines from the lines and their scores, in order (R6.2, R6.3, R6.4).
///
/// The function zips each line with its score in order, keeps a line when its score is at or
/// above the keep threshold, and drops it otherwise. The kept lines preserve the input order.
/// The counts satisfy `kept_count + dropped_count == input_count` and `input_count` equals
/// `lines.len()` (R6.6, R6.7).
///
/// The `scores` slice must match `lines` by index. A shorter `scores` slice treats a missing
/// score as a drop, so the pure helper never reads past its input.
#[cfg_attr(not(test), allow(dead_code))]
pub fn select_kept(lines: &[String], scores: &[f64], keep_threshold: f64) -> PruningOutcome {
    let mut kept = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        // A line is kept when a score is present at its index and is at or above the threshold.
        let keep = scores
            .get(index)
            .is_some_and(|score| *score >= keep_threshold);
        if keep {
            kept.push(line.clone());
        }
    }
    let input_count = lines.len();
    let kept_count = kept.len();
    let dropped_count = input_count - kept_count;
    PruningOutcome {
        kept,
        kept_count,
        dropped_count,
        input_count,
    }
}

/// Split the line indices into ordered chunks that each stay at or under the budget (R6.5).
///
/// The function iterates the lines in order and greedily grows the current chunk. Before it
/// adds a line, it estimates the request for the current chunk plus that line. When the
/// estimate would exceed [`TOKEN_BUDGET`], it closes the current chunk and starts a new one.
/// A single line whose own chunk still exceeds the budget goes in its own chunk as a best
/// effort, because the estimate is approximate and the line is not dropped. The chunks stay
/// in order, and their concatenation is the original line order.
fn chunk_line_indices(task_description: &str, lines: &[String]) -> Vec<Vec<usize>> {
    let mut chunks: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    for index in 0..lines.len() {
        // A non-empty current chunk that would exceed the budget with this line is closed first.
        if !current.is_empty() {
            let mut candidate = current.clone();
            candidate.push(index);
            if over_budget(task_description, lines, &candidate) {
                chunks.push(std::mem::take(&mut current));
            }
        }
        current.push(index);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// Report whether the request for the given line indices estimates over the budget (R6.5).
///
/// The function builds the same request shape the call uses, then compares its token
/// estimate against [`TOKEN_BUDGET`]. The estimate is approximate, so a borderline request
/// estimates high.
fn over_budget(task_description: &str, lines: &[String], indices: &[usize]) -> bool {
    let request = build_chunk_request(task_description, lines, indices);
    super::estimate_tokens(&request) > TOKEN_BUDGET
}

/// Build one chunk request over the given line indices (R6.5).
///
/// The request state carries the task and the chunk's lines as JSON. The question map holds
/// one Score question per line, keyed by [`line_id`], that rates the line's relevance to the
/// task across the [`RELEVANCE_LEVELS`] rubric.
fn build_chunk_request(task_description: &str, lines: &[String], indices: &[usize]) -> JevRequest {
    let chunk_lines: Vec<&String> = indices.iter().map(|index| &lines[*index]).collect();
    let state = serde_json::json!({
        "task": task_description,
        "lines": chunk_lines,
    });

    let mut questions = BTreeMap::new();
    for index in indices {
        questions.insert(
            line_id(*index),
            Question::Score {
                instructions: "Rate how relevant this log line is to the task. Answer relevant \
                               when the line helps solve the task and irrelevant when it does not."
                    .to_string(),
                criteria: RELEVANCE_LEVELS.iter().map(|s| s.to_string()).collect(),
            },
        );
    }

    JevRequest::new(state, questions)
}

/// Read one line's Score answer by its id, or return a typed error (R6).
///
/// A present Score answer returns its value. A missing answer or a non-Score answer under the
/// line id returns [`JevError::MissingAnswer`] naming the id.
///
/// # Errors
///
/// Returns [`JevError::MissingAnswer`] when the line id is absent or its answer is not a Score.
fn read_line_score(answers: &BTreeMap<String, Answer>, id: &str) -> Result<f64, JevError> {
    match answers.get(id) {
        Some(Answer::Score { score, .. }) => Ok(*score),
        // A missing answer or a non-Score answer under the line id is a contract failure.
        _ => Err(JevError::MissingAnswer { id: id.to_string() }),
    }
}

/// Evaluate context pruning over a log against a task (jev-integration-eval R6, ADR-J3).
///
/// The aspect rates each log line's relevance to `task_description`, then keeps a line when
/// its score is at or above `keep_threshold` and drops it otherwise (R6.2, R6.3). The unit is
/// one line (R6.1). The harness builds one Score question per line and batches the questions
/// into one request. When the whole-log request would exceed the token budget, the harness
/// splits the lines into ordered chunks and calls once per chunk in order (R6.5). An empty
/// log returns an empty outcome and makes no call.
///
/// The kept lines preserve the original order (R6.4), and the counts satisfy
/// `kept_count + dropped_count == input_count` (R6.6, R6.7).
///
/// # Errors
///
/// Returns [`JevError::MissingAnswer`] when a chunk response omits a line's answer or returns
/// a non-Score answer under a line id. Propagates any [`JevError`] the client returns.
#[cfg_attr(not(test), allow(dead_code))]
pub async fn evaluate_pruning<C: JevClient>(
    client: &C,
    task_description: &str,
    log_lines: &[String],
    keep_threshold: f64,
) -> Result<PruningOutcome, JevError> {
    // An empty log needs no call. Return the empty outcome directly (R6.6, R6.7).
    if log_lines.is_empty() {
        return Ok(select_kept(log_lines, &[], keep_threshold));
    }

    // Split the lines into ordered chunks that each stay at or under the budget (R6.5).
    let chunks = chunk_line_indices(task_description, log_lines);

    // Gather one score per line, in order across the chunks.
    let mut scores: Vec<f64> = Vec::with_capacity(log_lines.len());
    for chunk in &chunks {
        let request = build_chunk_request(task_description, log_lines, chunk);
        let response = client.evaluate(request).await?;
        for index in chunk {
            let score = read_line_score(&response.answers, &line_id(*index))?;
            scores.push(score);
        }
    }

    Ok(select_kept(log_lines, &scores, keep_threshold))
}

// Example and property tests live in sibling files to hold this module under the size
// guidance. The `#[path]` include keeps them child modules of `pruning`.
#[cfg(test)]
#[path = "pruning_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "pruning_prop_tests.rs"]
mod prop_tests;
