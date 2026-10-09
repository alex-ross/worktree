use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::TempDir;
use worktree_core::Manager;

fn git(path: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

struct Fixture {
    _temporary: TempDir,
    repo: PathBuf,
    manager: Manager,
}
impl Fixture {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temporary.path()).unwrap();
        let repo = root.join("project with spaces");
        fs::create_dir(&repo).unwrap();
        git(&repo, &["init", "-b", "main"]);
        git(&repo, &["config", "user.name", "Test"]);
        git(&repo, &["config", "user.email", "test@example.invalid"]);
        fs::write(repo.join("file.txt"), "original\n").unwrap();
        fs::write(repo.join(".gitignore"), ".env\nsecrets/\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-m", "initial"]);
        let manager = Manager::new(root.join("home"), Some(root.join("trees")), None).unwrap();
        Self {
            _temporary: temporary,
            repo,
            manager,
        }
    }
}

#[test]
fn create_checks_out_new_branch_and_persists_discovery() {
    let f = Fixture::new();
    let tree = f.manager.create(&f.repo, "foobar").unwrap();
    assert_eq!(tree.path, f.manager.base.join("foobar/project with spaces"));
    assert_eq!(git(&tree.path, &["branch", "--show-current"]), b"foobar\n");
    assert_eq!(fs::read(tree.path.join("file.txt")).unwrap(), b"original\n");
    assert_eq!(
        f.manager.resolve(&f.repo, "foobar").unwrap().path,
        tree.path
    );
    assert!(f.manager.resolve(&f.repo, "missing").is_err());
    assert_eq!(f.manager.discover(&f.manager.home).unwrap().len(), 1);
}

#[test]
fn create_uses_existing_branch_and_rejects_checked_out_branch() {
    let f = Fixture::new();
    git(&f.repo, &["branch", "existing"]);
    assert_eq!(
        f.manager
            .create(&f.repo, "existing")
            .unwrap()
            .branch
            .as_deref(),
        Some("existing")
    );
    assert!(f.manager.create(&f.repo, "main").is_err());
    assert!(f.manager.create(&f.repo, "existing").is_err());
}

#[test]
fn nested_branch_names_and_creation_from_subdirectories_work() {
    let f = Fixture::new();
    fs::create_dir(f.repo.join("subdir")).unwrap();
    let tree = f
        .manager
        .create(&f.repo.join("subdir"), "feature/login")
        .unwrap();
    assert!(tree.path.ends_with("feature/login/project with spaces"));
    let second = f.manager.create(&tree.path, "second").unwrap();
    assert_eq!(tree.project, second.project);
    assert_eq!(second.repository, f.repo);
}

#[test]
fn includes_copy_ignored_files_globs_directories_and_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    fs::write(
        f.repo.join(".worktreeinclude"),
        "# local config\n.env\nsecrets/\nmissing\n*.local\n",
    )
    .unwrap();
    fs::write(f.repo.join(".env"), "TOKEN=local").unwrap();
    fs::set_permissions(f.repo.join(".env"), fs::Permissions::from_mode(0o600)).unwrap();
    fs::create_dir(f.repo.join("secrets")).unwrap();
    fs::write(f.repo.join("secrets/config"), "private").unwrap();
    fs::write(f.repo.join("config.local"), "config").unwrap();
    let tree = f.manager.create(&f.repo, "included").unwrap();
    assert_eq!(fs::read(tree.path.join(".env")).unwrap(), b"TOKEN=local");
    assert_eq!(
        fs::read(tree.path.join("secrets/config")).unwrap(),
        b"private"
    );
    assert!(tree.path.join("config.local").is_file());
    assert!(tree.path.join(".worktreeinclude").is_file());
    assert_eq!(
        fs::metadata(tree.path.join(".env"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn unsafe_includes_are_rejected_before_any_worktree_is_created() {
    let f = Fixture::new();
    for pattern in [
        "../outside",
        "/etc/passwd",
        ".git/config",
        "secrets/../../outside",
    ] {
        fs::write(f.repo.join(".worktreeinclude"), pattern).unwrap();
        assert!(f.manager.create(&f.repo, "unsafe").is_err(), "{pattern}");
        assert!(!f.manager.base.join("unsafe").exists());
    }
}

#[test]
fn source_and_destination_symlinks_cannot_copy_outside_a_worktree() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    fs::write(f.repo.join(".worktreeinclude"), "secrets\n").unwrap();
    symlink(f.repo.join("file.txt"), f.repo.join("secrets")).unwrap();
    assert!(f.manager.create(&f.repo, "source-link").is_err());
    fs::remove_file(f.repo.join("secrets")).unwrap();
    fs::write(f.repo.join(".worktreeinclude"), "").unwrap();
    fs::create_dir(&f.manager.base).unwrap();
    symlink(&f.repo, f.manager.base.join("escape")).unwrap();
    assert!(f.manager.create(&f.repo, "escape").is_err());
    assert!(!f.repo.join("project with spaces").exists());
}

#[test]
fn unsafe_keys_are_rejected() {
    let f = Fixture::new();
    for key in ["../escape", "/absolute", "--force", "bad name", "@{-1}"] {
        assert!(f.manager.create(&f.repo, key).is_err(), "{key}");
    }
}

#[test]
fn fork_preserves_staged_unstaged_untracked_and_included_files() {
    let f = Fixture::new();
    let source = f.manager.create(&f.repo, "source").unwrap();
    fs::write(source.path.join("file.txt"), "staged\n").unwrap();
    git(&source.path, &["add", "file.txt"]);
    fs::write(source.path.join("file.txt"), "unstaged\n").unwrap();
    fs::write(source.path.join("untracked"), "untracked").unwrap();
    fs::write(source.path.join(".worktreeinclude"), ".env\n").unwrap();
    fs::write(source.path.join(".env"), "fork config").unwrap();
    let fork = f.manager.fork(&source.path, "forked").unwrap();
    assert_eq!(fs::read(fork.path.join("file.txt")).unwrap(), b"unstaged\n");
    assert_eq!(git(&fork.path, &["show", ":file.txt"]), b"staged\n");
    assert_eq!(fs::read(fork.path.join("untracked")).unwrap(), b"untracked");
    assert_eq!(fs::read(fork.path.join(".env")).unwrap(), b"fork config");
    assert_eq!(
        git(&fork.path, &["rev-parse", "HEAD"]),
        git(&source.path, &["rev-parse", "HEAD"])
    );
    assert!(f.manager.fork(&source.path, "forked").is_err());
}

#[test]
fn clean_removal_keeps_branch_and_dirty_removal_needs_force() {
    let f = Fixture::new();
    let tree = f.manager.create(&f.repo, "clean").unwrap();
    f.manager.remove(&tree, false).unwrap();
    assert!(!tree.path.exists());
    git(&f.repo, &["show-ref", "--verify", "refs/heads/clean"]);
    for filename in ["file.txt", "untracked", ".env"] {
        let tree = f.manager.create(&f.repo, "dirty").unwrap();
        fs::write(tree.path.join(filename), "do not lose this").unwrap();
        assert!(f.manager.remove(&tree, false).is_err());
        assert!(tree.path.is_dir());
        f.manager.remove(&tree, true).unwrap();
    }
}

#[test]
fn locked_and_main_worktrees_cannot_be_removed_even_with_force() {
    let f = Fixture::new();
    let main = f.manager.list_repository(&f.repo).unwrap().remove(0);
    assert!(f.manager.remove(&main, true).is_err());
    let tree = f.manager.create(&f.repo, "locked").unwrap();
    git(&f.repo, &["worktree", "lock", tree.path.to_str().unwrap()]);
    assert!(f.manager.remove(&tree, true).is_err());
    assert!(tree.path.is_dir());
}

#[test]
fn codex_and_external_worktrees_are_discovered_without_a_registry() {
    let f = Fixture::new();
    let codex = f
        .manager
        .codex_home
        .join("worktrees/abc/project with spaces");
    fs::create_dir_all(codex.parent().unwrap()).unwrap();
    git(
        &f.repo,
        &["worktree", "add", "--detach", codex.to_str().unwrap()],
    );
    let external = f.repo.parent().unwrap().join("external");
    git(
        &f.repo,
        &[
            "worktree",
            "add",
            "-b",
            "external",
            external.to_str().unwrap(),
        ],
    );
    let trees = f.manager.discover(f.repo.parent().unwrap()).unwrap();
    assert_eq!(trees.len(), 2);
    assert!(
        trees
            .iter()
            .any(|tree| tree.path == codex && tree.key == "abc" && tree.branch.is_none())
    );
    assert_eq!(f.manager.resolve(&f.repo, "abc").unwrap().path, codex);
}

#[test]
fn project_selection_disambiguates_the_same_branch_in_two_repositories() {
    let f = Fixture::new();
    let first = f.manager.create(&f.repo, "same").unwrap();
    let repo2 = f.repo.parent().unwrap().join("other");
    fs::create_dir(&repo2).unwrap();
    git(&repo2, &["clone", f.repo.to_str().unwrap(), "."]);
    let second = f.manager.create(&repo2, "same").unwrap();
    assert_ne!(first.path, second.path);
    assert_eq!(f.manager.resolve(&f.repo, "same").unwrap().path, first.path);
    assert_eq!(f.manager.resolve(&repo2, "same").unwrap().path, second.path);
    assert!(f.manager.resolve(&f.manager.home, "same").is_err());
}

#[test]
fn corrupt_registry_is_preserved() {
    let f = Fixture::new();
    fs::create_dir_all(f.manager.home.join(".worktree")).unwrap();
    let registry = f.manager.home.join(".worktree/repositories.json");
    fs::write(&registry, "broken registry").unwrap();
    assert!(f.manager.register(&f.repo).is_err());
    assert_eq!(fs::read_to_string(registry).unwrap(), "broken registry");
}

#[test]
fn concurrent_registration_preserves_all_projects() {
    let f = Fixture::new();
    let repositories: Vec<_> = (0..4)
        .map(|index| {
            let repo = f.repo.parent().unwrap().join(format!("repo-{index}"));
            fs::create_dir(&repo).unwrap();
            git(&repo, &["init", "-b", "main"]);
            repo
        })
        .collect();
    std::thread::scope(|scope| {
        for repo in &repositories {
            scope.spawn(|| f.manager.register(repo).unwrap());
        }
    });
    let bytes = fs::read(f.manager.home.join(".worktree/repositories.json")).unwrap();
    let stored: Vec<PathBuf> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(stored.len(), 4);
}

#[test]
fn ambiguous_detached_worktree_keys_need_a_full_path() {
    let f = Fixture::new();
    let first = f.manager.codex_home.join("worktrees/abc/first");
    let second = f.manager.codex_home.join("worktrees/abc/second");
    fs::create_dir_all(first.parent().unwrap()).unwrap();
    for path in [&first, &second] {
        git(
            &f.repo,
            &["worktree", "add", "--detach", path.to_str().unwrap()],
        );
    }
    assert!(f.manager.resolve(&f.repo, "abc").is_err());
    assert_eq!(
        f.manager
            .resolve(&f.repo, first.to_str().unwrap())
            .unwrap()
            .path,
        first
    );
}

#[test]
fn stale_registered_project_is_skipped_after_git_metadata_is_removed() {
    let f = Fixture::new();
    f.manager.register(&f.repo).unwrap();
    fs::remove_dir_all(f.repo.join(".git")).unwrap();
    assert!(f.manager.discover(&f.repo).unwrap().is_empty());
}
