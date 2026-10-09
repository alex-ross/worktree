"""Check installer paths, shell activation and repeat installation in a temporary home."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


project = Path(__file__).resolve().parent.parent
actual_home = Path.home()
with tempfile.TemporaryDirectory(prefix="worktree-install-") as temporary:
    home = Path(temporary).resolve()
    prefix = home / "installed ' $x `bin`"
    shell = "zsh" if shutil.which("zsh") else "bash"
    env = dict(os.environ, HOME=str(home), SHELL=f"/bin/{shell}",
               CARGO_HOME=os.environ.get("CARGO_HOME", str(actual_home / ".cargo")),
               RUSTUP_HOME=os.environ.get("RUSTUP_HOME", str(actual_home / ".rustup")))
    env.pop("ZDOTDIR", None)
    rc = home / (".zshrc" if shell == "zsh" else ".bash_profile" if os.uname().sysname == "Darwin" else ".bashrc")
    rc.write_text("# existing settings\n")
    command = ["sh", str(project / "install.sh"), "--prefix", str(prefix)]
    subprocess.run(command, cwd=project, env=env, check=True, capture_output=True)
    first = rc.read_bytes()
    subprocess.run(command, cwd=project, env=env, check=True, capture_output=True)
    assert rc.read_bytes() == first, "Installer duplicated its startup entry"
    assert first.startswith(b"# existing settings\n")
    result = subprocess.run([shell, "-f", "-c", 'source "$1"; command worktree --version; worktree --help >/dev/null', "check", str(rc)],
                            env=env, check=True, capture_output=True)
    assert b"worktree 0.1.0" in result.stdout
    assert (prefix / "bin/worktree").is_file()
    print("Installer: build, quoted paths, PATH, shell integration and idempotence passed")
