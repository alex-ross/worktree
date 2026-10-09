use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tempfile::TempDir;

const BINARY: &str = env!("CARGO_BIN_EXE_worktree");

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

struct Fixture {
    root: TempDir,
    repo: PathBuf,
    home: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path().join("project ' $x `quoted`");
        let home = root.path().join("home");
        fs::create_dir(&repo).unwrap();
        fs::create_dir(&home).unwrap();
        git(&repo, &["init", "-b", "main"]);
        git(&repo, &["config", "user.name", "Test"]);
        git(&repo, &["config", "user.email", "test@example.invalid"]);
        git(&repo, &["commit", "--allow-empty", "-m", "initial"]);
        Self { root, repo, home }
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(BINARY)
            .args(args)
            .current_dir(&self.repo)
            .env("HOME", &self.home)
            .env_remove("WORKTREE_HOME")
            .env("CODEX_HOME", self.home.join(".codex"))
            .output()
            .unwrap()
    }
}

#[test]
fn commands_support_base_override_json_and_missing_worktrees() {
    let f = Fixture::new();
    let base = f.root.path().join("custom base");
    let created = f.run(&["create", "foobar", "--base", base.to_str().unwrap()]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let listing = f.run(&["list", "--json"]);
    assert!(listing.status.success());
    let trees: Vec<serde_json::Value> = serde_json::from_slice(&listing.stdout).unwrap();
    assert_eq!(trees[0]["key"], "foobar");
    assert!(
        trees[0]["path"]
            .as_str()
            .unwrap()
            .starts_with(fs::canonicalize(base).unwrap().to_str().unwrap())
    );
    assert!(f.run(&["open", "foobar"]).status.success());
    let missing = f.run(&["open", "missing"]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("does not exist"));
    assert!(f.run(&["remove", "foobar"]).status.success());
}

#[test]
fn environment_base_and_explicit_base_have_the_right_precedence() {
    let f = Fixture::new();
    let env_base = f.root.path().join("from-env");
    let flag_base = f.root.path().join("from-flag");
    let output = Command::new(BINARY)
        .args(["create", "env-branch"])
        .current_dir(&f.repo)
        .env("HOME", &f.home)
        .env("WORKTREE_HOME", &env_base)
        .output()
        .unwrap();
    assert!(output.status.success());
    let output = Command::new(BINARY)
        .args(["create", "flag-branch", "--base"])
        .arg(&flag_base)
        .current_dir(&f.repo)
        .env("HOME", &f.home)
        .env("WORKTREE_HOME", &env_base)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(env_base.join("env-branch").exists());
    assert!(flag_base.join("flag-branch").exists());
    assert!(!env_base.join("flag-branch").exists());
}

#[test]
fn picker_explains_when_it_is_run_without_a_terminal() {
    let f = Fixture::new();
    let output = f.run(&[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("needs a terminal"));
}

fn shell_test(shell: &str) {
    let f = Fixture::new();
    let init = f.run(&["init", shell]);
    assert!(init.status.success());
    let script = f.root.path().join("init.sh");
    fs::write(&script, init.stdout).unwrap();
    let script_body = r#"
set -e
. "$1"
cd "$2"
worktree create foobar
expected=$(command worktree open foobar)
[ "$PWD" = "$expected" ]
before=$PWD
if worktree open missing; then exit 10; fi
[ "$PWD" = "$before" ]
if worktree remove foobar; then exit 11; fi
cd "$2"
worktree open foobar
[ "$PWD" = "$expected" ]
cd "$2"
worktree remove foobar
[ ! -d "$expected" ]
command worktree --help >/dev/null
"#;
    let path = format!(
        "{}:{}",
        Path::new(BINARY).parent().unwrap().display(),
        std::env::var("PATH").unwrap()
    );
    let output = Command::new(shell)
        .args(["-f", "-c", script_body, "shell-test"])
        .arg(script)
        .arg(&f.repo)
        .env("HOME", &f.home)
        .env("PATH", path)
        .env_remove("WORKTREE_HOME")
        .env_remove("ZDOTDIR")
        .env("CODEX_HOME", f.home.join(".codex"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{shell}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn bash_integration_changes_parent_directory_and_preserves_errors() {
    shell_test("bash");
}

#[test]
fn zsh_integration_changes_parent_directory_and_preserves_errors() {
    if Command::new("zsh").arg("--version").output().is_ok() {
        shell_test("zsh");
    }
}
