//! Tests for the skill phase map (task 8a).
//!
//! Included from `phase.rs` via `#[path]`, so `super` is the phase module.
//!
//! Requirements: 7.1.

use super::*;

#[test]
fn maps_known_skills_to_their_phases() {
    assert_eq!(phase_for_skill("survey-context"), "Discover");
    assert_eq!(phase_for_skill("model-domain"), "Design");
    assert_eq!(phase_for_skill("plan-work"), "Plan");
    assert_eq!(phase_for_skill("develop-tdd"), "Build");
    assert_eq!(phase_for_skill("verify-work"), "Verify");
    assert_eq!(phase_for_skill("release-branch"), "Release");
    assert_eq!(phase_for_skill("session-state"), "Sustain");
}

#[test]
fn unmapped_skill_is_other() {
    assert_eq!(phase_for_skill("nonexistent-skill"), "Other");
}

#[test]
fn late_added_skills_map_correctly() {
    // plan-tests, gate-trace, and security-review were appended in the legacy map.
    assert_eq!(phase_for_skill("plan-tests"), "Plan");
    assert_eq!(phase_for_skill("gate-trace"), "Verify");
    assert_eq!(phase_for_skill("security-review"), "Verify");
}
