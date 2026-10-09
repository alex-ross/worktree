"""Install the current checkout through Homebrew and run the formula's test."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import uuid


def run(args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


project = Path(__file__).resolve().parent.parent
existing = subprocess.run(["brew", "list", "--versions", "worktree"], capture_output=True)
if existing.returncode == 0 and existing.stdout.strip():
    raise SystemExit("Homebrew already has worktree installed; run this test in a clean environment")

env = dict(os.environ, HOMEBREW_NO_AUTO_UPDATE="1", HOMEBREW_NO_INSTALL_CLEANUP="1",
           HOMEBREW_NO_ANALYTICS="1", HOMEBREW_NO_ASK="1", GIT_CONFIG_GLOBAL="/dev/null",
           GIT_CONFIG_NOSYSTEM="1")
tap = "worktree-test/smoke-" + uuid.uuid4().hex[:12]
with tempfile.TemporaryDirectory(prefix="worktree-brew-") as temporary:
    env["XDG_CONFIG_HOME"] = str(Path(temporary) / "config")
    source = Path(temporary) / "source.git"
    shutil.copytree(project, source, ignore=shutil.ignore_patterns(".git", "target", "__pycache__"))
    formula = source / "Formula/worktree.rb"
    original = formula.read_text()
    upstream = 'head "https://github.com/alex-ross/worktree.git"'
    assert original.count(upstream) == 1
    formula.write_text(original.replace(upstream, f'head "{source.as_uri()}", using: :git'))
    run(["git", "init", "-b", "main", str(source)], env=env)
    run(["git", "-C", str(source), "add", "."], env=env)
    run(["git", "-C", str(source), "-c", "user.name=Homebrew Test", "-c", "user.email=test@example.invalid",
         "-c", "core.hooksPath=/dev/null", "commit", "-m", "Test current checkout"], env=env)
    tapped = False
    installed = False
    try:
        run(["brew", "tap", "--custom-remote", tap, source.as_uri()], env=env)
        tapped = True
        tap_path = Path(subprocess.check_output(["brew", "--repository", tap], env=env, text=True).strip())
        formula_path = str(tap_path / "Formula/worktree.rb")
        run(["brew", "trust", "--formula", tap + "/worktree"], env=env)
        run(["brew", "install", "--formula", "--HEAD", "--skip-link", formula_path], env=env)
        installed = True
        run(["brew", "test", "--force", "--HEAD", formula_path], env=env)
        print("Homebrew: source build, shell files, create/open/list/remove passed")
    finally:
        try:
            if installed:
                run(["brew", "uninstall", "--formula", tap + "/worktree"], env=env)
        finally:
            if tapped:
                run(["brew", "untap", tap], env=env)
