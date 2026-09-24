//! End-to-end methodology profile fixtures.
//!
//! Each fixture drives the real MCP scaffold, task, and lifecycle tools, then executes the
//! emitted git hooks and validates both backlog ownership modes in one disposable repository.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use rmcp::model::{CallToolRequestParams, CallToolResult};
use serde_json::json;
use tempfile::TempDir;

use super::{Client, call_tool, connect};
use crate::engine::backlog;
use crate::engine::profile::{
    EPIC_BASED, GENERIC, ISSUE_PER_TASK, KANBAN, MILESTONE_BASED, Profile,
};

#[derive(Clone, Copy)]
struct ProfileCase {
    profile: Profile,
    valid_group_id: Option<&'static str>,
    valid_group_kind: Option<&'static str>,
    rejected_group_kind: &'static str,
    matching_branch: &'static str,
    nonmatching_branch: &'static str,
}

const EPIC_CASE: ProfileCase = ProfileCase {
    profile: EPIC_BASED,
    valid_group_id: Some("e390"),
    valid_group_kind: Some("epic"),
    rejected_group_kind: "milestone",
    matching_branch: "feat/e390-work",
    nonmatching_branch: "feat/m390-work",
};

const ISSUE_CASE: ProfileCase = ProfileCase {
    profile: ISSUE_PER_TASK,
    valid_group_id: Some("t390"),
    valid_group_kind: Some("ticket"),
    rejected_group_kind: "epic",
    matching_branch: "chore/work",
    nonmatching_branch: "docs/work",
};

const KANBAN_CASE: ProfileCase = ProfileCase {
    profile: KANBAN,
    valid_group_id: None,
    valid_group_kind: None,
    rejected_group_kind: "ticket",
    matching_branch: "feat/work",
    nonmatching_branch: "docs/work",
};

const MILESTONE_CASE: ProfileCase = ProfileCase {
    profile: MILESTONE_BASED,
    valid_group_id: Some("m390"),
    valid_group_kind: Some("milestone"),
    rejected_group_kind: "epic",
    matching_branch: "fix/m390-work",
    nonmatching_branch: "fix/e390-work",
};

const GENERIC_CASE: ProfileCase = ProfileCase {
    profile: GENERIC,
    valid_group_id: None,
    valid_group_kind: None,
    rejected_group_kind: "ticket",
    matching_branch: "chore/work",
    nonmatching_branch: "docs/work",
};

async fn run_profile_fixture(case: ProfileCase) -> anyhow::Result<()> {
    let repo = TempDir::new()?;
    let root = repo.path().to_path_buf();
    let (client, handle) = connect(root.clone()).await?;

    call_tool(
        &client,
        "truenorth_scaffold_project",
        json!({ "profile": case.profile.name }),
    )
    .await?;

    assert_scaffold(&root, case);
    assert_local_backlog(&root, case);
    exercise_task_recording(&client, &root, case).await?;
    exercise_lifecycle(&client, &root, case).await?;

    client.cancel().await?;
    handle.abort();

    exercise_commit_hook(&root, case);
    exercise_branch_hook(&root, case);
    exercise_external_backlog(&root, case);
    Ok(())
}

fn assert_scaffold(root: &Path, case: ProfileCase) {
    const COMMON_PATHS: [&str; 16] = [
        ".agent/layout.yml",
        ".agent/profile.yml",
        ".agent/config/rules.yml",
        ".agent/spec/requirements.md",
        ".agent/tasks/state.yml",
        ".agent/tasks/execution-status.yml",
        ".agent/memories/lessons.md",
        ".agent/memories/glossary.md",
        ".agent/product/scope.md",
        ".agent/telemetry/runs.yml",
        "AGENTS.md",
        "CONVENTIONS.md",
        ".githooks/commit-msg",
        ".githooks/post-merge",
        ".github/commit-template.md",
        ".github/pull-request-template.md",
    ];
    for relative in COMMON_PATHS {
        assert!(
            root.join(relative).is_file(),
            "profile={} rule=common-files missing={relative}",
            case.profile.name
        );
    }

    assert_eq!(
        fs::read_to_string(root.join(".agent/profile.yml")).expect("read profile"),
        format!("profile: {}\n", case.profile.name),
        "profile={} rule=profile-declaration",
        case.profile.name
    );
    assert_eq!(
        fs::read_to_string(root.join(".agent/tasks/state.yml")).expect("read state"),
        "phase: discover\n",
        "profile={} rule=initial-phase",
        case.profile.name
    );

    for relative in ["tasks/release-plan.yml", "tasks/backlog.yml"] {
        let expected = case.profile.starter_files.contains(&relative);
        assert_eq!(
            root.join(".agent").join(relative).is_file(),
            expected,
            "profile={} rule=starter-files path={relative}",
            case.profile.name
        );
    }
}

fn assert_local_backlog(root: &Path, case: ProfileCase) {
    if !case.profile.starter_files.contains(&"tasks/backlog.yml") {
        return;
    }
    let path = root.join(".agent/tasks/backlog.yml");
    let document: serde_yaml::Value =
        serde_yaml::from_str(&fs::read_to_string(&path).expect("read local backlog"))
            .expect("parse local backlog");
    assert_eq!(
        document["version"].as_str(),
        Some("1"),
        "profile={} rule=local-backlog-version",
        case.profile.name
    );
    assert_eq!(
        document["ownership"]["mode"].as_str(),
        Some("local"),
        "profile={} rule=local-backlog-mode",
        case.profile.name
    );
    assert_eq!(
        document["backlog"].as_sequence().map(Vec::len),
        Some(0),
        "profile={} rule=local-backlog-list",
        case.profile.name
    );
    backlog::validate_optional(root).unwrap_or_else(|error| {
        panic!(
            "profile={} rule=local-backlog-validation error={error}",
            case.profile.name
        )
    });
}

async fn exercise_task_recording(
    client: &Client,
    root: &Path,
    case: ProfileCase,
) -> anyhow::Result<()> {
    let task_name = format!("Exercise {} workflow", case.profile.name);
    let mut valid = json!({
        "task_name": task_name,
        "verify_command": "true",
    });
    if let Some(id) = case.valid_group_id {
        valid["group_id"] = json!(id);
    }
    if let Some(kind) = case.valid_group_kind {
        valid["group_kind"] = json!(kind);
    }
    call_tool(client, "truenorth_record_task", valid).await?;

    let plan_path = root.join(".agent/tasks/release-plan.yml");
    let plan: serde_yaml::Value =
        serde_yaml::from_str(&fs::read_to_string(&plan_path)?).expect("parse release plan");
    let task = &plan["tasks"][0];
    assert_eq!(
        task["task_name"].as_str(),
        Some(task_name.as_str()),
        "profile={} rule=task-name",
        case.profile.name
    );
    assert_eq!(
        task["verify_command"].as_str(),
        Some("true"),
        "profile={} rule=verify-command",
        case.profile.name
    );
    assert_eq!(
        task.get("group_id").and_then(serde_yaml::Value::as_str),
        case.valid_group_id,
        "profile={} rule=valid-group-id",
        case.profile.name
    );
    assert_eq!(
        task.get("group_kind").and_then(serde_yaml::Value::as_str),
        case.valid_group_kind,
        "profile={} rule=valid-group-kind",
        case.profile.name
    );

    if case.profile.name == ISSUE_PER_TASK.name {
        call_tool(
            client,
            "truenorth_record_task",
            json!({
                "task_name": "Exercise optional ticket grouping",
                "verify_command": "true",
            }),
        )
        .await?;
    }

    let before_reject = fs::read(&plan_path)?;
    let mismatch = rejected_tool_call(
        client,
        "truenorth_record_task",
        json!({
            "group_id": "wrong-390",
            "group_kind": case.rejected_group_kind,
            "task_name": "Reject incompatible grouping",
            "verify_command": "true",
        }),
    )
    .await;
    assert!(
        mismatch.contains(case.profile.name) && mismatch.contains("group_kind"),
        "profile={} rule=rejected-group-kind error={mismatch}",
        case.profile.name
    );
    assert_eq!(
        fs::read(&plan_path)?,
        before_reject,
        "profile={} rule=rejected-grouping-preserves-plan",
        case.profile.name
    );

    if case.profile.rule == crate::engine::profile::GroupingRule::Required {
        let missing = rejected_tool_call(
            client,
            "truenorth_record_task",
            json!({
                "task_name": "Reject missing grouping",
                "verify_command": "true",
            }),
        )
        .await;
        assert!(
            missing.contains(case.profile.name) && missing.contains("requires a grouping key"),
            "profile={} rule=required-grouping error={missing}",
            case.profile.name
        );
        assert_eq!(
            fs::read(&plan_path)?,
            before_reject,
            "profile={} rule=missing-grouping-preserves-plan",
            case.profile.name
        );
    }
    Ok(())
}

async fn exercise_lifecycle(client: &Client, root: &Path, case: ProfileCase) -> anyhow::Result<()> {
    call_tool(
        client,
        "truenorth_advance_phase",
        json!({
            "from_phase": "discover",
            "to_phase": "design",
            "artifacts_summary": "Profile fixture designed.",
        }),
    )
    .await?;

    let state_path = root.join(".agent/tasks/state.yml");
    let designed = fs::read(&state_path)?;
    let state: serde_yaml::Value = serde_yaml::from_slice(&designed)?;
    assert_eq!(
        state["phase"].as_str(),
        Some("design"),
        "profile={} rule=adjacent-lifecycle",
        case.profile.name
    );

    let rejected = rejected_tool_call(
        client,
        "truenorth_advance_phase",
        json!({
            "from_phase": "design",
            "to_phase": "execute",
            "artifacts_summary": "Skipped planning.",
        }),
    )
    .await;
    assert!(
        rejected.contains("next phase must be `plan`"),
        "profile={} rule=nonadjacent-lifecycle error={rejected}",
        case.profile.name
    );
    assert_eq!(
        fs::read(&state_path)?,
        designed,
        "profile={} rule=rejected-lifecycle-preserves-state",
        case.profile.name
    );
    Ok(())
}

async fn rejected_tool_call(
    client: &Client,
    name: &'static str,
    args: serde_json::Value,
) -> String {
    let arguments = args.as_object().cloned().expect("tool args object");
    let outcome: Result<CallToolResult, _> = client
        .call_tool(CallToolRequestParams::new(name).with_arguments(arguments))
        .await;
    format!("{:?}", outcome.expect_err("tool call must be rejected"))
}

fn exercise_commit_hook(root: &Path, case: ProfileCase) {
    let valid_with_id = run_commit_hook(root, "feat(core): exercise workflow (#390)\n");
    assert!(
        valid_with_id.status.success(),
        "profile={} rule=commit-with-id stderr={}",
        case.profile.name,
        String::from_utf8_lossy(&valid_with_id.stderr)
    );

    let without_id = run_commit_hook(root, "feat(core): exercise workflow\n");
    assert_eq!(
        without_id.status.success(),
        !case.profile.require_issue_id,
        "profile={} rule=commit-id stderr={}",
        case.profile.name,
        String::from_utf8_lossy(&without_id.stderr)
    );
    if case.profile.require_issue_id {
        assert!(
            String::from_utf8_lossy(&without_id.stderr).contains("issue or ticket id"),
            "profile={} rule=commit-id-diagnostic",
            case.profile.name
        );
    }

    let malformed = run_commit_hook(root, "not a conventional commit\n");
    assert!(
        !malformed.status.success(),
        "profile={} rule=commit-format",
        case.profile.name
    );
    assert!(
        String::from_utf8_lossy(&malformed.stderr).contains("subject must be"),
        "profile={} rule=commit-format-diagnostic",
        case.profile.name
    );
}

fn run_commit_hook(root: &Path, message: &str) -> Output {
    let message_path = root.join("COMMIT_EDITMSG.fixture");
    fs::write(&message_path, message).expect("write commit message");
    Command::new("/bin/sh")
        .arg(root.join(".githooks/commit-msg"))
        .arg(message_path)
        .current_dir(root)
        .output()
        .expect("run emitted commit-msg hook")
}

fn exercise_branch_hook(root: &Path, case: ProfileCase) {
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "fixture@example.com"]);
    git(root, &["config", "user.name", "Profile Fixture"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "chore: scaffold fixture"]);

    merge_topic(root, case.matching_branch, "matching.txt");
    merge_topic(root, case.nonmatching_branch, "nonmatching.txt");

    let output = Command::new("/bin/sh")
        .arg(root.join(".githooks/post-merge"))
        .current_dir(root)
        .output()
        .expect("run emitted post-merge hook");
    assert!(
        output.status.success(),
        "profile={} rule=branch-hook stderr={}",
        case.profile.name,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !branch_exists(root, case.matching_branch),
        "profile={} rule=matching-branch",
        case.profile.name
    );
    assert!(
        branch_exists(root, case.nonmatching_branch),
        "profile={} rule=nonmatching-branch",
        case.profile.name
    );
    assert!(
        branch_exists(root, "main"),
        "profile={} rule=trunk-preservation",
        case.profile.name
    );
}

fn merge_topic(root: &Path, branch: &str, file: &str) {
    git(root, &["checkout", "-q", "-b", branch]);
    fs::write(root.join(file), format!("{branch}\n")).expect("write branch fixture");
    git(root, &["add", file]);
    git(root, &["commit", "-q", "-m", "chore: add branch fixture"]);
    git(root, &["checkout", "-q", "main"]);
    git(
        root,
        &["merge", "-q", "--no-ff", branch, "-m", "merge fixture"],
    );
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn branch_exists(root: &Path, branch: &str) -> bool {
    Command::new("git")
        .args(["rev-parse", "--verify", &format!("refs/heads/{branch}")])
        .current_dir(root)
        .output()
        .expect("inspect branch")
        .status
        .success()
}

fn exercise_external_backlog(root: &Path, case: ProfileCase) {
    let path = root.join(".agent/tasks/backlog.yml");
    fs::write(
        &path,
        "version: '1'\nownership:\n  mode: external\n  provider: fixture\n  url: https://example.invalid/issues\n",
    )
    .expect("write external backlog");
    backlog::validate_optional(root).unwrap_or_else(|error| {
        panic!(
            "profile={} rule=external-backlog error={error}",
            case.profile.name
        )
    });
    let document: serde_yaml::Value =
        serde_yaml::from_str(&fs::read_to_string(&path).expect("read external backlog"))
            .expect("parse external backlog");
    assert!(
        document.get("backlog").is_none(),
        "profile={} rule=external-backlog-no-cache",
        case.profile.name
    );

    fs::write(
        &path,
        "version: '1'\nownership:\n  mode: external\n  provider: fixture\n  url: https://example.invalid/issues\nbacklog: []\n",
    )
    .expect("write cached external backlog");
    let error = backlog::validate_optional(root).expect_err("cached external backlog must fail");
    assert!(
        error
            .to_string()
            .contains("must not contain a cached `backlog`"),
        "profile={} rule=external-backlog-cache error={error}",
        case.profile.name
    );
}

macro_rules! profile_fixture {
    ($name:ident, $case:expr) => {
        #[tokio::test]
        async fn $name() -> anyhow::Result<()> {
            run_profile_fixture($case).await
        }
    };
}

profile_fixture!(epic_based_profile_workflow, EPIC_CASE);
profile_fixture!(issue_per_task_profile_workflow, ISSUE_CASE);
profile_fixture!(kanban_profile_workflow, KANBAN_CASE);
profile_fixture!(milestone_based_profile_workflow, MILESTONE_CASE);
profile_fixture!(generic_profile_workflow, GENERIC_CASE);
