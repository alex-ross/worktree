mod git;
mod include;

use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Error(pub String);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self(value.to_string())
    }
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worktree {
    pub path: PathBuf,
    pub repository: PathBuf,
    pub project: String,
    pub key: String,
    pub branch: Option<String>,
    pub main: bool,
    pub locked: bool,
}

/// Backend configuration is explicit so any UI can reuse it without shell state.
pub struct Manager {
    pub base: PathBuf,
    pub home: PathBuf,
    pub codex_home: PathBuf,
}

impl Manager {
    pub fn new(home: PathBuf, base: Option<PathBuf>, codex_home: Option<PathBuf>) -> Result<Self> {
        let base = base.unwrap_or_else(|| home.join(".worktree"));
        let base = if base.is_absolute() {
            base
        } else {
            std::env::current_dir()?.join(base)
        };
        let codex_home = codex_home.unwrap_or_else(|| home.join(".codex"));
        Ok(Self {
            base,
            home,
            codex_home,
        })
    }

    pub fn repository(&self, cwd: &Path) -> Result<PathBuf> {
        let bytes = git::run(
            cwd,
            ["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?;
        let common = fs::canonicalize(git::path(bytes.strip_suffix(b"\n").unwrap_or(&bytes)))?;
        if common.file_name() != Some(OsStr::new(".git")) {
            return Err(Error(
                "Bare repositories and separate Git directories are not supported".into(),
            ));
        }
        Ok(common.parent().unwrap().to_path_buf())
    }

    pub fn list_repository(&self, cwd: &Path) -> Result<Vec<Worktree>> {
        let repository = self.repository(cwd)?;
        let output = git::run(cwd, ["worktree", "list", "--porcelain", "-z"])?;
        let project = repository
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let mut trees = Vec::new();
        let mut current: Option<Worktree> = None;
        for field in output.split(|b| *b == 0) {
            if field.is_empty() {
                if let Some(tree) = current.take() {
                    trees.push(tree);
                }
            } else if let Some(value) = field.strip_prefix(b"worktree ") {
                let path = git::path(value);
                current = Some(Worktree {
                    main: path == repository,
                    key: path
                        .parent()
                        .and_then(Path::file_name)
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    path,
                    repository: repository.clone(),
                    project: project.clone(),
                    branch: None,
                    locked: false,
                });
            } else if let Some(tree) = current.as_mut() {
                if let Some(branch) = field.strip_prefix(b"branch refs/heads/") {
                    let branch = String::from_utf8_lossy(branch).into_owned();
                    tree.key = branch.clone();
                    tree.branch = Some(branch);
                } else if field == b"locked" || field.starts_with(b"locked ") {
                    tree.locked = true;
                }
            }
        }
        Ok(trees)
    }

    fn registry(&self) -> PathBuf {
        self.home.join(".worktree/repositories.json")
    }

    pub fn register(&self, cwd: &Path) -> Result<()> {
        let repository = self.repository(cwd)?;
        let registry = self.registry();
        fs::create_dir_all(registry.parent().unwrap())?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(registry.with_extension("lock"))?;
        lock.lock()?;
        let contents = match fs::read_to_string(&registry) {
            Ok(contents) => contents,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.into()),
        };
        let mut repositories: BTreeSet<PathBuf> = if contents.is_empty() {
            BTreeSet::new()
        } else {
            serde_json::from_str(&contents)
                .map_err(|e| Error(format!("Invalid project registry: {e}")))?
        };
        if repositories.insert(repository) {
            let bytes = serde_json::to_vec(&repositories).map_err(|e| Error(e.to_string()))?;
            let temporary = registry.with_extension("tmp");
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(temporary, registry)?;
        }
        Ok(())
    }

    /// Discover known projects, the current repository, and managed/Codex worktrees.
    pub fn discover(&self, cwd: &Path) -> Result<Vec<Worktree>> {
        let mut repositories = BTreeSet::new();
        match fs::File::open(self.registry()) {
            Ok(mut file) => {
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)?;
                if !bytes.is_empty() {
                    repositories = serde_json::from_slice(&bytes)
                        .map_err(|e| Error(format!("Invalid project registry: {e}")))?;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        if let Ok(repo) = self.repository(cwd) {
            repositories.insert(repo);
        }
        self.scan(&self.base, 3, &mut repositories)?;
        self.scan(&self.codex_home.join("worktrees"), 3, &mut repositories)?;
        let mut trees = Vec::new();
        for repo in repositories {
            if repo.join(".git").exists() {
                trees.extend(self.list_repository(&repo)?);
            }
        }
        trees.retain(|tree| !tree.main && tree.path.is_dir());
        trees.sort_by(|a, b| (&a.project, &a.key, &a.path).cmp(&(&b.project, &b.key, &b.path)));
        trees.dedup_by(|a, b| a.path == b.path);
        Ok(trees)
    }

    fn scan(&self, root: &Path, depth: usize, repositories: &mut BTreeSet<PathBuf>) -> Result<()> {
        if root.join(".git").exists() {
            if let Ok(repo) = self.repository(root) {
                repositories.insert(repo);
            }
            return Ok(());
        }
        if depth == 0 {
            return Ok(());
        }
        let entries = match fs::read_dir(root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                self.scan(&entry.path(), depth - 1, repositories)?;
            }
        }
        Ok(())
    }

    pub fn resolve(&self, cwd: &Path, key: &str) -> Result<Worktree> {
        let trees = self.discover(cwd)?;
        let candidates: Vec<_> = trees
            .into_iter()
            .filter(|tree| tree.key == key || tree.path == Path::new(key))
            .collect();
        if let Ok(repo) = self.repository(cwd) {
            let local: Vec<_> = candidates
                .iter()
                .filter(|tree| tree.repository == repo)
                .collect();
            match local.as_slice() {
                [tree] => return Ok((*tree).clone()),
                [] => {}
                _ => {
                    return Err(Error(format!(
                        "Worktree '{key}' is ambiguous in this project; use its full path"
                    )));
                }
            }
        }
        match candidates.as_slice() {
            [tree] => Ok(tree.clone()),
            [] => Err(Error(format!("Worktree '{key}' does not exist"))),
            _ => Err(Error(format!(
                "Worktree '{key}' exists in multiple projects; run inside the project or use its full path"
            ))),
        }
    }

    pub fn create(&self, cwd: &Path, key: &str) -> Result<Worktree> {
        self.add(cwd, key, false)
    }

    /// Fork HEAD, staged/unstaged changes, untracked files and .worktreeinclude files.
    pub fn fork(&self, source: &Path, key: &str) -> Result<Worktree> {
        self.add(source, key, true)
    }

    fn add(&self, source: &Path, key: &str, fork: bool) -> Result<Worktree> {
        let repository = self.repository(source)?;
        git::run(source, ["check-ref-format", "--branch", key])?;
        git::run(source, ["check-ref-format", &format!("refs/heads/{key}")])?;
        if key.starts_with('-')
            || Path::new(key)
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err(Error(
                "Worktree keys must be safe, relative branch names".into(),
            ));
        }
        let source_bytes = git::run(source, ["rev-parse", "--show-toplevel"])?;
        let source = git::path(source_bytes.strip_suffix(b"\n").unwrap_or(&source_bytes));
        let mut files: BTreeSet<_> = include::plan(&source)?.into_iter().collect();
        let (staged, unstaged) = if fork {
            // shortcut: submodule working files are not copied; extend fork when that is needed.
            for file in git::run(
                &source,
                ["ls-files", "--others", "--exclude-standard", "-z"],
            )?
            .split(|b| *b == 0)
            .filter(|file| !file.is_empty())
            {
                include::collect(&source, &git::path(file), &mut files)?;
            }
            (
                git::run(
                    &source,
                    [
                        "diff",
                        "--cached",
                        "--binary",
                        "--no-ext-diff",
                        "--no-textconv",
                        "--no-color",
                        "--src-prefix=a/",
                        "--dst-prefix=b/",
                        "HEAD",
                    ],
                )?,
                git::run(
                    &source,
                    [
                        "diff",
                        "--binary",
                        "--no-ext-diff",
                        "--no-textconv",
                        "--no-color",
                        "--src-prefix=a/",
                        "--dst-prefix=b/",
                    ],
                )?,
            )
        } else {
            (Vec::new(), Vec::new())
        };
        fs::create_dir_all(&self.base)?;
        let base = fs::canonicalize(&self.base)?;
        include::no_symlinks(&base, Path::new(key))?;
        let target = base.join(key).join(repository.file_name().unwrap());
        if fs::symlink_metadata(&target).is_ok() {
            return Err(Error(format!(
                "Destination already exists: {}",
                target.display()
            )));
        }
        let exists = git::command(
            &source,
            [
                "show-ref",
                "--verify",
                "--quiet",
                &format!("refs/heads/{key}"),
            ],
        )?;
        if !exists.status.success() && exists.status.code() != Some(1) {
            return Err(Error("Could not check branch existence".into()));
        }
        if fork && exists.status.success() {
            return Err(Error(format!(
                "Branch '{key}' already exists; choose a new name for the fork"
            )));
        }
        fs::create_dir_all(target.parent().unwrap())?;
        let mut args = vec![OsStr::new("worktree"), OsStr::new("add")];
        if !exists.status.success() {
            args.extend([OsStr::new("-b"), OsStr::new(key)]);
        }
        args.extend([OsStr::new("--"), target.as_os_str()]);
        args.push(if exists.status.success() {
            OsStr::new(key)
        } else {
            OsStr::new("HEAD")
        });
        git::run(&source, args)?;
        let copied = (|| {
            // Included files may rely on private parent directories in the source.
            fs::set_permissions(&target, fs::Permissions::from_mode(0o700))?;
            git::apply(&target, &staged, true)?;
            git::apply(&target, &unstaged, false)?;
            include::copy(&source, &target, &files.into_iter().collect::<Vec<_>>())
        })();
        if let Err(error) = copied {
            // Keep partial copies for inspection instead of risking deletion of user data.
            return Err(Error(format!(
                "Worktree created at {}, but copying .worktreeinclude failed: {error}",
                target.display()
            )));
        }
        self.register(&source).map_err(|e| {
            Error(format!(
                "Worktree created at {}, but registering the project failed: {e}",
                target.display()
            ))
        })?;
        self.list_repository(&source)?
            .into_iter()
            .find(|tree| tree.path == target)
            .ok_or_else(|| Error("Created worktree was not returned by Git".into()))
    }

    pub fn remove(&self, tree: &Worktree, force: bool) -> Result<()> {
        let actual = self
            .list_repository(&tree.repository)?
            .into_iter()
            .find(|item| item.path == tree.path)
            .ok_or_else(|| Error("Worktree is no longer registered with Git".into()))?;
        if actual.main {
            return Err(Error("The main working directory cannot be removed".into()));
        }
        if actual.locked {
            return Err(Error(
                "Worktree is locked; unlock it with Git before removing it".into(),
            ));
        }
        let cwd = fs::canonicalize(std::env::current_dir()?)?;
        if cwd.starts_with(fs::canonicalize(&actual.path)?) {
            return Err(Error("Leave this worktree before removing it".into()));
        }
        if !force {
            let dirty = git::run(
                &actual.path,
                [
                    "status",
                    "--porcelain",
                    "--untracked-files=all",
                    "--ignored",
                ],
            )?;
            if !dirty.is_empty() {
                return Err(Error("Worktree contains changed, untracked or ignored files; use --force to discard them".into()));
            }
        }
        let mut args = vec![OsStr::new("worktree"), OsStr::new("remove")];
        if force {
            args.push(OsStr::new("--force"));
        }
        args.extend([OsStr::new("--"), actual.path.as_os_str()]);
        git::run(&actual.repository, args)?;
        Ok(())
    }
}
