use crate::{Error, Result};
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};

fn safe(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.components().any(|part| match part {
            Component::Normal(name) => name == ".git",
            _ => true,
        })
    {
        return Err(Error(format!(
            "Unsafe .worktreeinclude path: {}",
            path.display()
        )));
    }
    Ok(())
}

pub(crate) fn no_symlinks(root: &Path, relative: &Path) -> Result<()> {
    safe(relative)?;
    let mut path = root.to_path_buf();
    for part in relative.components() {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error(format!(
                    ".worktreeinclude cannot copy through symlinks: {}",
                    path.display()
                )));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

pub(crate) fn collect(root: &Path, relative: &Path, files: &mut BTreeSet<PathBuf>) -> Result<()> {
    no_symlinks(root, relative)?;
    let path = root.join(relative);
    let metadata = fs::metadata(&path)?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            collect(root, &relative.join(entry?.file_name()), files)?;
        }
    } else if metadata.is_file() {
        files.insert(relative.to_path_buf());
    } else {
        return Err(Error(format!(
            "Unsupported .worktreeinclude file: {}",
            path.display()
        )));
    }
    Ok(())
}

pub(crate) fn plan(source: &Path) -> Result<Vec<PathBuf>> {
    let manifest = source.join(".worktreeinclude");
    no_symlinks(source, Path::new(".worktreeinclude"))?;
    let contents = match fs::read_to_string(&manifest) {
        Ok(contents) => contents,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut files = BTreeSet::new();
    for line in contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let pattern = line.trim_end_matches('/');
        safe(Path::new(pattern))?;
        no_symlinks(source, Path::new(pattern))?;
        // Match relative paths so special characters in the project path stay literal.
        let options = glob::MatchOptions {
            require_literal_separator: true,
            require_literal_leading_dot: false,
            case_sensitive: true,
        };
        let escaped = glob::Pattern::escape(&source.to_string_lossy());
        let matches = glob::glob_with(&format!("{escaped}/{pattern}"), options)
            .map_err(|e| Error(e.to_string()))?;
        for entry in matches {
            let entry = entry.map_err(|e| Error(e.to_string()))?;
            let relative = entry
                .strip_prefix(source)
                .map_err(|e| Error(e.to_string()))?;
            collect(source, relative, &mut files)?;
        }
    }
    if !files.is_empty() {
        files.insert(PathBuf::from(".worktreeinclude"));
    }
    Ok(files.into_iter().collect())
}

pub(crate) fn copy(source: &Path, target: &Path, files: &[PathBuf]) -> Result<()> {
    for relative in files {
        no_symlinks(source, relative)?;
        no_symlinks(target, relative)?;
        let destination = target.join(relative);
        fs::create_dir_all(destination.parent().unwrap())?;
        fs::copy(source.join(relative), destination)?;
    }
    Ok(())
}
