use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::json;
use tempfile::tempdir;
use truenorth_mcp::engine::workspace_upgrade::bundle::DesiredFile;
use truenorth_mcp::engine::workspace_upgrade::manifest::{ManagedFile, WorkspaceManifest, sha256};
use truenorth_mcp::engine::workspace_upgrade::plan::{PlanAction, RETIRED_GUARD_HOOK, build_plan};
use truenorth_mcp::engine::workspace_upgrade::transaction;

#[test]
fn check_is_read_only_and_apply_updates_an_unchanged_managed_file() {
    let package = tempdir().expect("package");
    write_package(package.path(), b"old skill\n", &["0.9.0"]);
    let repo = tempdir().expect("repo");

    assert_success(run(
        repo.path(),
        &[
            "init",
            "--profile",
            "generic",
            "--bundle-dir",
            package.path().to_str().expect("package path"),
        ],
    ));
    assert!(!repo.path().join(RETIRED_GUARD_HOOK).exists());
    let manifest_path = repo.path().join(".agent/workspace-manifest.yml");
    let manifest = fs::read_to_string(&manifest_path).expect("workspace manifest");
    fs::write(
        &manifest_path,
        manifest.replacen(
            &format!("bundle_version: {}", env!("CARGO_PKG_VERSION")),
            "bundle_version: 0.9.0",
            1,
        ),
    )
    .expect("downgrade fixture manifest");
    git(repo.path(), &["init"]);
    git(repo.path(), &["config", "user.email", "test@example.com"]);
    git(repo.path(), &["config", "user.name", "Test"]);
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "fixture"]);

    write_package(package.path(), b"new skill\n", &["0.9.0"]);
    let before = fs::read(&manifest_path).expect("manifest before check");
    let check = run(
        repo.path(),
        &[
            "upgrade",
            "--check",
            "--bundle-dir",
            package.path().to_str().expect("package path"),
        ],
    );
    assert_success(check);
    assert_eq!(
        fs::read(&manifest_path).expect("manifest after check"),
        before
    );
    assert_eq!(
        fs::read(repo.path().join("skills/using-truenorth/SKILL.md")).expect("skill after check"),
        b"old skill\n"
    );

    let apply = run(
        repo.path(),
        &[
            "upgrade",
            "--bundle-dir",
            package.path().to_str().expect("package path"),
        ],
    );
    assert_success(apply);
    assert_eq!(
        fs::read(repo.path().join("skills/using-truenorth/SKILL.md")).expect("upgraded skill"),
        b"new skill\n"
    );
    let upgraded_manifest = fs::read_to_string(manifest_path).expect("upgraded manifest");
    assert!(upgraded_manifest.contains(&format!("bundle_version: {}", env!("CARGO_PKG_VERSION"))));
    assert!(!repo.path().join(".agent/runtime/upgrade").exists());
}

#[test]
fn upgrade_rejects_invalid_configuration_before_mutation() {
    let package = tempdir().expect("package");
    write_package(package.path(), b"old skill\n", &[]);
    let repo = tempdir().expect("repo");
    assert_success(run(
        repo.path(),
        &[
            "init",
            "--profile",
            "generic",
            "--bundle-dir",
            package.path().to_str().expect("package path"),
        ],
    ));
    fs::write(
        repo.path().join(".agent/config/rules.yml"),
        "token_caps:\n  tool_payload_tokens: 0\n",
    )
    .expect("invalid token caps");
    git(repo.path(), &["init"]);
    git(repo.path(), &["config", "user.email", "test@example.com"]);
    git(repo.path(), &["config", "user.name", "Test"]);
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "invalid fixture"]);
    write_package(package.path(), b"new skill\n", &[]);

    let upgrade = run(
        repo.path(),
        &[
            "upgrade",
            "--bundle-dir",
            package.path().to_str().expect("package path"),
        ],
    );

    assert!(!upgrade.status.success());
    assert!(
        String::from_utf8_lossy(&upgrade.stderr).contains("token"),
        "stderr: {}",
        String::from_utf8_lossy(&upgrade.stderr)
    );
    assert_eq!(
        fs::read(repo.path().join("skills/using-truenorth/SKILL.md")).expect("skill"),
        b"old skill\n"
    );
    assert!(!repo.path().join(".agent/runtime/upgrade").exists());
}

#[test]
fn resumed_upgrade_rolls_back_when_post_validation_fails() {
    let package = tempdir().expect("package");
    write_package(package.path(), b"skill\n", &[]);
    let repo = tempdir().expect("repo");
    assert_success(run(
        repo.path(),
        &[
            "init",
            "--profile",
            "generic",
            "--bundle-dir",
            package.path().to_str().expect("package path"),
        ],
    ));
    let installed = WorkspaceManifest::read_optional(repo.path())
        .expect("read manifest")
        .expect("workspace manifest");
    let manifest_before =
        fs::read(repo.path().join(".agent/workspace-manifest.yml")).expect("manifest");
    let rules_path = repo.path().join(".agent/config/rules.yml");
    let rules_before = fs::read(&rules_path).expect("rules");
    let mut desired: BTreeMap<String, DesiredFile> = installed
        .managed
        .iter()
        .map(|(path, record)| {
            (
                path.clone(),
                DesiredFile {
                    path: path.clone(),
                    bytes: fs::read(repo.path().join(path)).expect("managed file"),
                    mode: record.mode.clone(),
                },
            )
        })
        .collect();
    desired
        .get_mut(".agent/config/rules.yml")
        .expect("managed rules")
        .bytes = b"token_caps:\n  tool_payload_tokens: 0\n".to_vec();
    let plan = build_plan(
        repo.path(),
        &desired,
        Some(&installed),
        &installed.bundle_version,
        &installed.workspace_schema_version,
        &installed.profile,
        installed.skill_sets.clone(),
    )
    .expect("upgrade plan");
    transaction::prepare(repo.path(), &plan, &desired).expect("prepared transaction");

    let upgrade = run(
        repo.path(),
        &[
            "upgrade",
            "--bundle-dir",
            package.path().to_str().expect("package path"),
        ],
    );

    assert!(!upgrade.status.success());
    assert!(
        String::from_utf8_lossy(&upgrade.stderr).contains("token"),
        "stderr: {}",
        String::from_utf8_lossy(&upgrade.stderr)
    );
    assert_eq!(fs::read(rules_path).expect("restored rules"), rules_before);
    assert_eq!(
        fs::read(repo.path().join(".agent/workspace-manifest.yml")).expect("restored manifest"),
        manifest_before
    );
    assert!(!repo.path().join(".agent/runtime/upgrade").exists());
}
#[test]
fn upgrade_adds_and_removes_optional_skill_sets() {
    let package = tempdir().expect("package");
    write_package(package.path(), b"core skill\n", &[]);
    let repo = tempdir().expect("repo");
    assert_success(run(
        repo.path(),
        &[
            "init",
            "--profile",
            "generic",
            "--bundle-dir",
            package.path().to_str().expect("package path"),
        ],
    ));
    assert!(!repo.path().join("skills/visual-test").exists());
    git(repo.path(), &["init"]);
    git(repo.path(), &["config", "user.email", "test@example.com"]);
    git(repo.path(), &["config", "user.name", "Test"]);
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "core"]);

    assert_success(run(
        repo.path(),
        &[
            "upgrade",
            "--add-skill-set",
            "visual",
            "--bundle-dir",
            package.path().to_str().expect("package path"),
        ],
    ));
    assert!(repo.path().join("skills/visual-test/SKILL.md").is_file());
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "visual"]);

    assert_success(run(
        repo.path(),
        &[
            "upgrade",
            "--remove-skill-set",
            "visual",
            "--bundle-dir",
            package.path().to_str().expect("package path"),
        ],
    ));
    assert!(!repo.path().join("skills/visual-test/SKILL.md").exists());
    let manifest = fs::read_to_string(repo.path().join(".agent/workspace-manifest.yml"))
        .expect("workspace manifest");
    assert!(!manifest.contains("- visual"));
}

#[test]
fn checked_in_historical_manifest_migrates_an_exact_legacy_workspace() {
    let package = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
        .join("npm");
    let repo = tempdir().expect("repo");
    assert_success(run(
        repo.path(),
        &[
            "init",
            "--profile",
            "generic",
            "--bundle-dir",
            package.to_str().expect("package path"),
        ],
    ));
    fs::remove_file(repo.path().join(".agent/workspace-manifest.yml"))
        .expect("remove current manifest");
    git(repo.path(), &["init"]);
    git(repo.path(), &["config", "user.email", "test@example.com"]);
    git(repo.path(), &["config", "user.name", "Test"]);
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "legacy fixture"]);

    let current: serde_json::Value = serde_json::from_slice(
        &fs::read(package.join("bundle/current.json")).expect("current bundle manifest"),
    )
    .expect("current bundle JSON");
    let historical: serde_json::Value = serde_json::from_slice(
        &fs::read(package.join("bundle/history/1.0.2.json")).expect("historical bundle manifest"),
    )
    .expect("historical bundle JSON");
    let current_hashes: BTreeMap<&str, &str> = current["files"]
        .as_array()
        .expect("current files")
        .iter()
        .map(|file| {
            (
                file["path"].as_str().expect("current path"),
                file["sha256"].as_str().expect("current hash"),
            )
        })
        .collect();
    let adopted_path = historical["files"]
        .as_array()
        .expect("historical files")
        .iter()
        .find_map(|file| {
            let path = file["path"].as_str()?;
            let hash = file["sha256"].as_str()?;
            (current_hashes.get(path) == Some(&hash)).then_some(path)
        })
        .expect("an unchanged historical bundle path");

    assert_success(run(
        repo.path(),
        &[
            "upgrade",
            "--bundle-dir",
            package.to_str().expect("package path"),
        ],
    ));

    let migrated = WorkspaceManifest::read_optional(repo.path())
        .expect("read migrated manifest")
        .expect("migrated manifest");
    assert_eq!(migrated.manifest_version, "3");
    assert_eq!(migrated.bundle_version, env!("CARGO_PKG_VERSION"));
    assert!(
        migrated.managed.contains_key(adopted_path),
        "historical exact match was not adopted: {adopted_path}"
    );
}

// Historical seed bytes are test evidence only; production retains solely their digest.
const HISTORICAL_HOOK: &str = concat!(
    "{\n",
    "  \"version\": \"v1\",\n",
    "  \"hooks\": [\n",
    "    {\n",
    "      \"name\": \"Jev active guardrail\",\n",
    "      \"description\": \"Runs the TrueNorth guardrail before a write tool. Automatic interception depends on the client honoring the PreToolUse hook. Where the client does not honor it, call the truenorth_guard_change tool before a write instead. TrueNorth cannot force-intercept a write an agent makes to a foreign tool.\",\n",
    "      \"trigger\": \"PreToolUse\",\n",
    "      \"matcher\": \"write_to_file|fs_write|str_replace\",\n",
    "      \"action\": { \"type\": \"command\", \"command\": \"truenorth-mcp guard\" }\n",
    "    }\n",
    "  ]\n",
    "}\n",
);

// The later supported seed changed only its input-contract description.
const LATER_HISTORICAL_HOOK: &str = concat!(
    "{\n",
    "  \"version\": \"v1\",\n",
    "  \"hooks\": [\n",
    "    {\n",
    "      \"name\": \"Jev active guardrail\",\n",
    "      \"description\": \"Runs the TrueNorth guardrail before a write tool. The client must normalize stdin to JSON containing paths, proposed content, and optional task, including replacement-tool proposals. Raw client tool envelopes are not accepted. Automatic interception depends on the client honoring the PreToolUse hook and supplying this input contract. Otherwise call truenorth_guard_change before a write. TrueNorth cannot force-intercept writes to foreign tools.\",\n",
    "      \"trigger\": \"PreToolUse\",\n",
    "      \"matcher\": \"write_to_file|fs_write|str_replace\",\n",
    "      \"action\": { \"type\": \"command\", \"command\": \"truenorth-mcp guard\" }\n",
    "    }\n",
    "  ]\n",
    "}\n",
);

fn hook_action(output: Output) -> serde_json::Value {
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("plan JSON");
    report["actions"]
        .as_array()
        .expect("actions")
        .iter()
        .find(|entry| entry["path"] == RETIRED_GUARD_HOOK)
        .cloned()
        .expect("retired hook appears in plan")
}

fn hook_upgrade(repo: &Path, package: &Path, check: bool) -> Output {
    let mut args = vec!["upgrade"];
    if check {
        args.push("--check");
    }
    args.extend(["--bundle-dir", package.to_str().expect("package path")]);
    run(repo, &args)
}

fn commit_fixture(repo: &Path) {
    git(repo, &["init"]);
    git(repo, &["config", "user.email", "test@example.com"]);
    git(repo, &["config", "user.name", "Test"]);
    git(repo, &["add", "."]);
    git(repo, &["commit", "-m", "fixture"]);
}

#[test]
fn owned_hook_is_removed_without_recreation_and_absent_ownership_is_dropped() {
    assert_eq!(
        sha256(HISTORICAL_HOOK.as_bytes()),
        "961deba14bc14be3ff69c5e730c2b192ee6d3b929744686377fa04650e64c18b"
    );
    for present in [true, false] {
        let package = tempdir().expect("package");
        write_package(package.path(), b"skill\n", &[]);
        let repo = tempdir().expect("repo");
        assert_success(run(
            repo.path(),
            &[
                "init",
                "--profile",
                "generic",
                "--bundle-dir",
                package.path().to_str().unwrap(),
            ],
        ));
        let hook = repo.path().join(RETIRED_GUARD_HOOK);
        if present {
            fs::create_dir_all(hook.parent().unwrap()).expect("hook directory");
            fs::write(&hook, HISTORICAL_HOOK).expect("historical hook");
        }
        let mut manifest = WorkspaceManifest::read_optional(repo.path())
            .unwrap()
            .unwrap();
        manifest.managed.insert(
            RETIRED_GUARD_HOOK.to_string(),
            ManagedFile {
                source_sha256: sha256(HISTORICAL_HOOK.as_bytes()),
                mode: "0644".to_string(),
            },
        );
        fs::write(
            repo.path().join(".agent/workspace-manifest.yml"),
            manifest.to_yaml().unwrap(),
        )
        .unwrap();
        commit_fixture(repo.path());
        let check = hook_action(hook_upgrade(repo.path(), package.path(), true));
        assert_eq!(check["action"], "remove");
        assert_eq!(
            check["reason"],
            if present {
                "removed_unchanged_managed_file"
            } else {
                "already_absent"
            }
        );
        assert_eq!(hook.exists(), present, "check must not write");
        assert_eq!(
            hook_action(hook_upgrade(repo.path(), package.path(), false))["action"],
            "remove"
        );
        assert!(!hook.exists());
        let next = WorkspaceManifest::read_optional(repo.path())
            .unwrap()
            .unwrap();
        assert!(!next.managed.contains_key(RETIRED_GUARD_HOOK));
        assert_success(hook_upgrade(repo.path(), package.path(), true));
    }
}

#[test]
fn modified_and_unowned_hooks_remain_visible_after_upgrade() {
    for owned in [true, false] {
        let package = tempdir().expect("package");
        write_package(package.path(), b"skill\n", &[]);
        let repo = tempdir().expect("repo");
        assert_success(run(
            repo.path(),
            &[
                "init",
                "--profile",
                "generic",
                "--bundle-dir",
                package.path().to_str().unwrap(),
            ],
        ));
        let hook = repo.path().join(RETIRED_GUARD_HOOK);
        fs::create_dir_all(hook.parent().unwrap()).unwrap();
        fs::write(&hook, b"local hook\n").unwrap();
        if owned {
            let mut manifest = WorkspaceManifest::read_optional(repo.path())
                .unwrap()
                .unwrap();
            manifest.managed.insert(
                RETIRED_GUARD_HOOK.to_string(),
                ManagedFile {
                    source_sha256: sha256(HISTORICAL_HOOK.as_bytes()),
                    mode: "0644".to_string(),
                },
            );
            fs::write(
                repo.path().join(".agent/workspace-manifest.yml"),
                manifest.to_yaml().unwrap(),
            )
            .unwrap();
        }
        commit_fixture(repo.path());
        let first = hook_action(hook_upgrade(repo.path(), package.path(), true));
        assert_eq!(first["action"], if owned { "conflict" } else { "preserve" });
        assert_eq!(
            first["reason"],
            if owned {
                "removed_locally_modified_file"
            } else {
                "retired_unowned_hook_requires_manual_removal"
            }
        );
        assert_success(hook_upgrade(repo.path(), package.path(), false));
        assert_eq!(fs::read(&hook).unwrap(), b"local hook\n");
        let next = WorkspaceManifest::read_optional(repo.path())
            .unwrap()
            .unwrap();
        assert!(!next.managed.contains_key(RETIRED_GUARD_HOOK));
        let again = hook_action(hook_upgrade(repo.path(), package.path(), true));
        assert_eq!(
            again["reason"],
            "retired_unowned_hook_requires_manual_removal"
        );
    }
}

#[test]
fn manifestless_exact_legacy_hook_is_adopted_and_retired() {
    for (hook_bytes, digest) in [
        (
            HISTORICAL_HOOK,
            "961deba14bc14be3ff69c5e730c2b192ee6d3b929744686377fa04650e64c18b",
        ),
        (
            LATER_HISTORICAL_HOOK,
            "70801cbf33a523a9d529fef20099dd7b87c26b08e63d27970536e02c6dcb1e9b",
        ),
    ] {
        assert_eq!(sha256(hook_bytes.as_bytes()), digest);
        assert_manifestless_hook_retired(hook_bytes);
    }
}

fn assert_manifestless_hook_retired(hook_bytes: &str) {
    let package = tempdir().expect("package");
    write_package(package.path(), b"skill\n", &["0.9.0"]);
    let mut current: serde_json::Value =
        serde_json::from_slice(&fs::read(package.path().join("bundle/current.json")).unwrap())
            .unwrap();
    current["bundle_version"] = json!("0.9.0");
    fs::create_dir_all(package.path().join("bundle/history")).unwrap();
    fs::write(
        package.path().join("bundle/history/0.9.0.json"),
        format!("{current}\n"),
    )
    .unwrap();
    let repo = tempdir().expect("repo");
    assert_success(run(
        repo.path(),
        &[
            "init",
            "--profile",
            "generic",
            "--bundle-dir",
            package.path().to_str().unwrap(),
        ],
    ));
    fs::remove_file(repo.path().join(".agent/workspace-manifest.yml")).unwrap();
    let hook = repo.path().join(RETIRED_GUARD_HOOK);
    fs::create_dir_all(hook.parent().unwrap()).unwrap();
    fs::write(&hook, hook_bytes).unwrap();
    commit_fixture(repo.path());
    let check = hook_action(hook_upgrade(repo.path(), package.path(), true));
    assert_eq!(check["action"], "remove");
    assert_eq!(check["reason"], "removed_unchanged_managed_file");
    assert!(hook.exists());
    assert_success(hook_upgrade(repo.path(), package.path(), false));
    assert!(!hook.exists());
    assert!(
        !WorkspaceManifest::read_optional(repo.path())
            .unwrap()
            .unwrap()
            .managed
            .contains_key(RETIRED_GUARD_HOOK)
    );
}

#[cfg(unix)]
#[test]
fn changed_mode_and_symlink_hook_are_never_deleted() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    for symlinked in [true, false] {
        let package = tempdir().expect("package");
        write_package(package.path(), b"skill\n", &[]);
        let repo = tempdir().expect("repo");
        assert_success(run(
            repo.path(),
            &[
                "init",
                "--profile",
                "generic",
                "--bundle-dir",
                package.path().to_str().unwrap(),
            ],
        ));
        let hook = repo.path().join(RETIRED_GUARD_HOOK);
        fs::create_dir_all(hook.parent().unwrap()).unwrap();
        let outside = tempdir().expect("outside");
        let target = outside.path().join("canary");
        fs::write(&target, HISTORICAL_HOOK).unwrap();
        if symlinked {
            symlink(&target, &hook).unwrap();
        } else {
            fs::write(&hook, HISTORICAL_HOOK).unwrap();
            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut installed = WorkspaceManifest::read_optional(repo.path())
            .unwrap()
            .unwrap();
        installed.managed.insert(
            RETIRED_GUARD_HOOK.to_string(),
            ManagedFile {
                source_sha256: sha256(HISTORICAL_HOOK.as_bytes()),
                mode: "0644".to_string(),
            },
        );
        fs::write(
            repo.path().join(".agent/workspace-manifest.yml"),
            installed.to_yaml().unwrap(),
        )
        .unwrap();
        commit_fixture(repo.path());
        let check = hook_action(hook_upgrade(repo.path(), package.path(), true));
        assert_eq!(check["action"], "conflict");
        assert_success(hook_upgrade(repo.path(), package.path(), false));
        assert_eq!(fs::read(&target).unwrap(), HISTORICAL_HOOK.as_bytes());
        if symlinked {
            assert!(
                fs::symlink_metadata(&hook)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
        } else {
            assert_eq!(
                fs::metadata(&hook).unwrap().permissions().mode() & 0o111,
                0o111
            );
        }
        assert_eq!(
            hook_action(hook_upgrade(repo.path(), package.path(), true))["reason"],
            "retired_unowned_hook_requires_manual_removal"
        );
    }
}

#[cfg(unix)]
#[test]
fn symlink_parent_never_reads_or_removes_hook_outside_worktree() {
    use std::os::unix::fs::symlink;
    let package = tempdir().unwrap();
    write_package(package.path(), b"skill\n", &[]);
    let repo = tempdir().unwrap();
    assert_success(run(
        repo.path(),
        &[
            "init",
            "--profile",
            "generic",
            "--bundle-dir",
            package.path().to_str().unwrap(),
        ],
    ));
    let outside = tempdir().unwrap();
    fs::create_dir_all(outside.path().join("hooks")).unwrap();
    fs::write(outside.path().join("hooks/jev-guard.json"), HISTORICAL_HOOK).unwrap();
    symlink(outside.path(), repo.path().join(".kiro")).unwrap();
    let check = hook_upgrade(repo.path(), package.path(), true);
    assert!(!check.status.success(), "unsafe parent must fail planning");
    assert_eq!(
        fs::read(outside.path().join("hooks/jev-guard.json")).unwrap(),
        HISTORICAL_HOOK.as_bytes()
    );
}

#[test]
fn retired_hook_transaction_refuses_external_edit_and_restores_on_rollback() {
    let package = tempdir().unwrap();
    write_package(package.path(), b"skill\n", &[]);
    let repo = tempdir().unwrap();
    assert_success(run(
        repo.path(),
        &[
            "init",
            "--profile",
            "generic",
            "--bundle-dir",
            package.path().to_str().unwrap(),
        ],
    ));
    let hook = repo.path().join(RETIRED_GUARD_HOOK);
    fs::create_dir_all(hook.parent().unwrap()).unwrap();
    fs::write(&hook, HISTORICAL_HOOK).unwrap();
    let mut installed = WorkspaceManifest::read_optional(repo.path())
        .unwrap()
        .unwrap();
    installed.managed.insert(
        RETIRED_GUARD_HOOK.to_string(),
        ManagedFile {
            source_sha256: sha256(HISTORICAL_HOOK.as_bytes()),
            mode: "0644".to_string(),
        },
    );
    let desired: BTreeMap<String, DesiredFile> = installed
        .managed
        .iter()
        .filter(|(path, _)| path.as_str() != RETIRED_GUARD_HOOK)
        .map(|(path, record)| {
            (
                path.clone(),
                DesiredFile {
                    path: path.clone(),
                    bytes: fs::read(repo.path().join(path)).unwrap(),
                    mode: record.mode.clone(),
                },
            )
        })
        .collect();
    let plan = build_plan(
        repo.path(),
        &desired,
        Some(&installed),
        &installed.bundle_version,
        &installed.workspace_schema_version,
        &installed.profile,
        installed.skill_sets.clone(),
    )
    .unwrap();
    assert_eq!(
        plan.actions
            .iter()
            .find(|entry| entry.path == RETIRED_GUARD_HOOK)
            .unwrap()
            .action,
        PlanAction::Remove
    );
    transaction::prepare(repo.path(), &plan, &desired).unwrap();
    fs::write(&hook, b"external edit\n").unwrap();
    assert!(transaction::resume(repo.path()).is_err());
    assert_eq!(fs::read(&hook).unwrap(), b"external edit\n");
    fs::write(&hook, HISTORICAL_HOOK).unwrap();
    transaction::resume(repo.path()).unwrap();
    assert!(!hook.exists());
    transaction::rollback(repo.path()).unwrap();
    assert_eq!(fs::read(&hook).unwrap(), HISTORICAL_HOOK.as_bytes());
}

fn write_package(root: &Path, skill: &[u8], supported_from: &[&str]) {
    let skill_path = root.join("skills/using-truenorth/SKILL.md");
    fs::create_dir_all(skill_path.parent().expect("skill parent")).expect("skills directory");
    fs::write(&skill_path, skill).expect("skill");
    let visual = b"visual skill\n";
    let visual_path = root.join("skills/visual-test/SKILL.md");
    fs::create_dir_all(visual_path.parent().expect("visual skill parent"))
        .expect("visual skill directory");
    fs::write(&visual_path, visual).expect("visual skill");
    let manifest = json!({
        "schema_version": 3,
        "bundle_version": env!("CARGO_PKG_VERSION"),
        "workspace_schema_version": "3",
        "supported_from": supported_from,
        "skill_sets": [
            {
                "name": "core",
                "support": "supported",
                "description": "Core test skills.",
                "prerequisites": "none",
                "skills": ["using-truenorth"]
            },
            {
                "name": "visual",
                "support": "optional",
                "description": "Visual test skills.",
                "prerequisites": "none",
                "skills": ["visual-test"]
            }
        ],
        "files": [
            {
                "path": "skills/using-truenorth/SKILL.md",
                "sha256": sha256(skill),
                "mode": "0644"
            },
            {
                "path": "skills/visual-test/SKILL.md",
                "sha256": sha256(visual),
                "mode": "0644"
            }
        ]
    });
    let manifest_path = root.join("bundle/current.json");
    fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
        .expect("manifest directory");
    fs::write(manifest_path, format!("{manifest}\n")).expect("manifest");
}

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_truenorth-mcp"))
        .current_dir(root)
        .args(args)
        .output()
        .expect("run binary")
}

fn assert_success(output: Output) {
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {}: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}
