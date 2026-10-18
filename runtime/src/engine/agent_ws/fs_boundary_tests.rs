use std::fs;
use std::io::{self, Read};
use std::os::unix::fs::symlink;
use std::path::Path;

use proptest::prelude::*;
use tempfile::tempdir;

use super::RepositoryFs;

#[test]
fn regular_reads_and_directory_names_survive_a_symlinked_repository_root() {
    let root = tempdir().expect("root");
    let repo = root.path().join("repo");
    fs::create_dir_all(repo.join("specs/adr")).expect("adr");
    fs::write(repo.join("specs/adr/0001.md"), "decision").expect("adr file");
    let alias = root.path().join("alias");
    symlink(&repo, &alias).expect("root alias");
    let filesystem = RepositoryFs::open(&alias).expect("anchor");
    assert_eq!(
        filesystem
            .read(Path::new("specs/adr/0001.md"))
            .expect("read"),
        b"decision"
    );
    assert_eq!(
        filesystem
            .read_dir_names(Path::new("specs/adr"))
            .expect("names"),
        ["0001.md"]
    );
}

#[test]
fn protected_paths_are_rejected_before_opening() {
    for path in [
        "",
        ".",
        "../outside",
        "/outside",
        "a/../outside",
        "a\\outside",
        ".env",
        ".env/data",
        "key.pem",
        "credentials/data",
        ".agent/telemetry/log.json",
        "telemetry/log.json",
    ] {
        assert!(
            RepositoryFs::validate_path(Path::new(path)).is_err(),
            "{path}"
        );
    }
}

#[test]
fn symlink_files_and_parents_cannot_be_read_or_enumerated() {
    let root = tempdir().expect("root");
    let outside = tempdir().expect("outside");
    fs::write(outside.path().join("canary"), "private canary").expect("canary");
    symlink(outside.path(), root.path().join("alias")).expect("directory alias");
    symlink(outside.path().join("canary"), root.path().join("note.md")).expect("file alias");
    let filesystem = RepositoryFs::open(root.path()).expect("anchor");
    assert!(filesystem.open_read(Path::new("note.md")).is_err());
    assert!(filesystem.open_read(Path::new("alias/canary")).is_err());
    assert!(filesystem.read_dir_names(Path::new("alias")).is_err());
    assert!(filesystem.validate_directory(Path::new("alias")).is_err());
}

#[test]
fn opened_handle_does_not_follow_a_later_symlink_substitution() {
    let root = tempdir().expect("root");
    let outside = tempdir().expect("outside");
    fs::write(root.path().join("note.md"), "original").expect("note");
    fs::write(outside.path().join("canary"), "private").expect("canary");
    let filesystem = RepositoryFs::open(root.path()).expect("anchor");
    let mut file = filesystem
        .open_read(Path::new("note.md"))
        .expect("safe handle");
    fs::remove_file(root.path().join("note.md")).expect("remove alias");
    symlink(outside.path().join("canary"), root.path().join("note.md")).expect("replacement");
    let mut content = String::new();
    file.read_to_string(&mut content).expect("read handle");
    assert_eq!(content, "original");
    assert!(filesystem.read(Path::new("note.md")).is_err());
}

#[test]
fn nonregular_files_are_rejected_without_opening_a_blocking_reader() {
    let root = tempdir().expect("root");
    let filesystem = RepositoryFs::open(root.path()).expect("anchor");
    fs::create_dir(root.path().join("directory")).expect("directory");
    assert!(filesystem.open_read(Path::new("directory")).is_err());
    let fifo = std::ffi::CString::new(root.path().join("pipe").as_os_str().as_encoded_bytes())
        .expect("fifo name");
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert!(filesystem.open_read(Path::new("pipe")).is_err());
}

#[test]
fn atomic_replacement_and_no_clobber_creation_preserve_existing_bytes() {
    let root = tempdir().expect("root");
    let filesystem = RepositoryFs::open(root.path()).expect("anchor");
    filesystem
        .create_dir_all(Path::new(".agent/tasks"))
        .expect("directories");
    let path = Path::new(".agent/tasks/state.json");
    filesystem.write_new(path, b"before").expect("new file");
    assert_eq!(
        filesystem
            .write_new(path, b"clobber")
            .expect_err("must not replace")
            .kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(filesystem.read(path).expect("unchanged"), b"before");
    filesystem.write_atomic(path, b"after").expect("replace");
    assert_eq!(filesystem.read(path).expect("updated"), b"after");
    assert_eq!(
        filesystem
            .read_dir_names(Path::new(".agent/tasks"))
            .expect("no temporaries"),
        ["state.json"]
    );
    filesystem.remove_file(path).expect("remove");
    assert_eq!(
        filesystem.read(path).expect_err("absent").kind(),
        io::ErrorKind::NotFound
    );
}

#[test]
fn mutations_cannot_follow_symlinks_or_leave_agent_scope() {
    let root = tempdir().expect("root");
    let outside = tempdir().expect("outside");
    fs::write(outside.path().join("canary"), "private").expect("canary");
    fs::create_dir(root.path().join(".agent")).expect("agent");
    symlink(outside.path(), root.path().join(".agent/tasks")).expect("parent alias");
    symlink(
        outside.path().join("canary"),
        root.path().join(".agent/note"),
    )
    .expect("file alias");
    let filesystem = RepositoryFs::open(root.path()).expect("anchor");
    for path in [
        ".agent/tasks/canary",
        ".agent/note",
        "specs/adr.md",
        "../canary",
    ] {
        assert!(
            filesystem
                .write_atomic(Path::new(path), b"changed")
                .is_err()
        );
        assert!(filesystem.remove_file(Path::new(path)).is_err());
    }
    assert!(
        filesystem
            .create_dir_all(Path::new(".agent/tasks/new"))
            .is_err()
    );
    assert!(!outside.path().join("new").exists());
    assert_eq!(
        fs::read(outside.path().join("canary")).expect("canary"),
        b"private"
    );
}

proptest! {
    #[test]
    fn traversal_never_changes_an_outside_file(name in "[a-z]{1,16}", bytes in prop::collection::vec(any::<u8>(), 0..128)) {
        let outside = tempdir().expect("outside");
        let root = outside.path().join("repo");
        fs::create_dir(&root).expect("repo");
        fs::write(outside.path().join(&name), b"canary").expect("canary");
        let filesystem = RepositoryFs::open(&root).expect("anchor");
        let path = format!("../{name}");
        prop_assert!(filesystem.read(Path::new(&path)).is_err());
        prop_assert!(filesystem.write_atomic(Path::new(&path), &bytes).is_err());
        prop_assert!(filesystem.remove_file(Path::new(&path)).is_err());
        prop_assert_eq!(fs::read(outside.path().join(&name)).expect("canary"), b"canary");
    }
}
