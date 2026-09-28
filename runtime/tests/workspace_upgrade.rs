use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::json;
use tempfile::tempdir;
use truenorth_mcp::engine::workspace_upgrade::bundle::DesiredFile;
use truenorth_mcp::engine::workspace_upgrade::manifest::{WorkspaceManifest, sha256};
use truenorth_mcp::engine::workspace_upgrade::plan::build_plan;
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
    assert!(manifest.contains("skill_sets:\n- core\n"));
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
    assert_eq!(migrated.manifest_version, "2");
    assert_eq!(migrated.bundle_version, env!("CARGO_PKG_VERSION"));
    assert!(
        migrated.managed.contains_key(adopted_path),
        "historical exact match was not adopted: {adopted_path}"
    );
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
        "schema_version": 2,
        "bundle_version": env!("CARGO_PKG_VERSION"),
        "workspace_schema_version": "2",
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
