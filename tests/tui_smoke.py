"""Exercise real terminal input using only Python's standard library (macOS/Linux)."""
import errno
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import shlex
import shutil
import struct
import subprocess
import sys
import tempfile
import termios
import time


BINARY = str(Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/worktree").resolve())


def run(args, cwd, env):
    return subprocess.run(args, cwd=cwd, env=env, check=True, capture_output=True).stdout


def read_until(fd, needle, timeout=10):
    output = bytearray()
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        readable, _, _ = select.select([fd], [], [], 0.1)
        if readable:
            try:
                chunk = os.read(fd, 65536)
            except OSError as error:
                if error.errno == errno.EIO:
                    break
                raise
            if not chunk:
                break
            output.extend(chunk)
            if needle in output:
                return bytes(output)
    raise AssertionError(f"Terminal did not show {needle!r}: {bytes(output)!r}")


def wait_terminal(process, master):
    output = bytearray()
    deadline = time.monotonic() + 10
    while process.poll() is None and time.monotonic() < deadline:
        readable, _, _ = select.select([master], [], [], 0.1)
        if readable:
            try:
                output.extend(os.read(master, 65536))
            except OSError as error:
                if error.errno != errno.EIO:
                    raise
                break
    assert process.poll() is not None, f"Terminal command did not exit: {bytes(output)!r}"
    assert process.wait() == 0, f"Terminal command failed: {bytes(output)!r}"


def choose(repo, env, keys, expected_key):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 100, 0, 0))
    original = termios.tcgetattr(slave)
    destination = repo.parent / "destination"
    destination.write_bytes(b"")
    process = subprocess.Popen(
        [BINARY, "--shell-file", str(destination)], cwd=repo, env=env,
        stdin=slave, stdout=slave, stderr=slave,
    )
    try:
        read_until(master, b"Search")
        os.write(master, keys)
        wait_terminal(process, master)
        listing = json.loads(run([BINARY, "list", "--json"], repo, env))
        if expected_key is not None:
            expected = next(tree["path"] for tree in listing if tree["key"] == expected_key)
            assert destination.read_bytes() == os.fsencode(expected) + b"\0"
        else:
            assert destination.read_bytes() == b""
        assert termios.tcgetattr(slave) == original, "Raw terminal mode was not restored"
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
        os.close(slave)


def completion(repo, env):
    if not shutil.which("zsh"):
        return
    init = repo.parent / "init.zsh"
    init.write_bytes(run([BINARY, "init", "zsh"], repo, env))
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 100, 0, 0))
    shell_env = dict(env, PATH=str(Path(BINARY).parent) + ":" + env["PATH"], PS1="CHECK> ")
    process = subprocess.Popen(["zsh", "-f"], cwd=repo, env=shell_env,
                               stdin=slave, stdout=slave, stderr=slave)
    try:
        os.write(master, ("source " + shlex.quote(str(init)) + "; PS1='CHECK> '\n").encode())
        read_until(master, b"CHECK> ")
        os.write(master, b"worktree op\talpha\n")
        read_until(master, b"CHECK> ")
        os.write(master, b"pwd\n")
        expected = next(tree["path"] for tree in json.loads(run([BINARY, "list", "--json"], repo, env)) if tree["key"] == "alpha")
        read_until(master, os.fsencode(expected))
        os.write(master, ("cd " + shlex.quote(str(repo)) + "\n").encode())
        read_until(master, b"CHECK> ")
        os.write(master, b"worktree open al\t\n")
        read_until(master, b"CHECK> ")
        os.write(master, b"pwd\n")
        read_until(master, os.fsencode(expected))
        os.write(master, b"exit\n")
        wait_terminal(process, master)
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
        os.close(slave)


with tempfile.TemporaryDirectory(prefix="worktree-tui-") as temporary:
    root = Path(temporary).resolve()
    repo = root / "project"
    repo.mkdir()
    home = root / "home"
    home.mkdir()
    env = dict(os.environ, HOME=str(home), CODEX_HOME=str(home / ".codex"), TERM="xterm-256color",
               GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1")
    env.pop("WORKTREE_HOME", None)
    run(["git", "init", "-b", "main"], repo, env)
    run(["git", "config", "user.name", "Test"], repo, env)
    run(["git", "config", "user.email", "test@example.invalid"], repo, env)
    run(["git", "commit", "--allow-empty", "-m", "initial"], repo, env)
    run([BINARY, "create", "alpha"], repo, env)
    run([BINARY, "create", "beta"], repo, env)
    choose(repo, env, b"bt\r", "beta")
    choose(repo, env, b"\x1b[B\r", "beta")
    choose(repo, env, b"\x1b[<0;5;4M", "alpha")
    choose(repo, env, b"\x0enew-tree\r", "new-tree")
    choose(repo, env, b"alpha\x06forked\r", "forked")
    completion(repo, env)
    choose(repo, env, b"beta\x04yes\r\x03", None)
    assert all(tree["key"] != "beta" for tree in json.loads(run([BINARY, "list", "--json"], repo, env)))
    print("TUI: fuzzy search, arrows, mouse, create, fork, remove, zsh completion and terminal restoration passed")
