class Worktree < Formula
  desc "Fast Git worktree management with a terminal picker"
  homepage "https://github.com/alex-ross/worktree"
  license "MIT"
  # shortcut: HEAD-only installation, pin a source archive when the first release is tagged.
  head "https://github.com/alex-ross/worktree.git", branch: "main"

  depends_on "rust" => :build
  uses_from_macos "git"

  def install
    system "cargo", "install", *std_cargo_args
    (pkgshare/"init.zsh").write Utils.safe_popen_read(bin/"worktree", "init", "zsh")
    (pkgshare/"init.bash").write Utils.safe_popen_read(bin/"worktree", "init", "bash")
  end

  def caveats
    <<~EOS
      Enable directory switching and autocomplete by adding this to ~/.zshrc:
        source "#{opt_pkgshare}/init.zsh"
      For bash, add this to ~/.bashrc (or ~/.bash_profile on macOS):
        source "#{opt_pkgshare}/init.bash"
    EOS
  end

  test do
    system "git", "init", "-b", "main"
    system "git", "config", "user.name", "Homebrew Test"
    system "git", "config", "user.email", "test@example.invalid"
    system "git", "-c", "core.hooksPath=/dev/null", "commit", "--allow-empty", "-m", "initial"
    ENV["WORKTREE_HOME"] = (testpath/"trees").to_s
    ENV["CODEX_HOME"] = (testpath/".codex").to_s
    created = shell_output("#{bin}/worktree create example").strip
    assert_path_exists Pathname.new(created)/".git"
    assert_equal created, shell_output("#{bin}/worktree open example").strip
    assert_match "example", shell_output("#{bin}/worktree list --json")
    assert_path_exists pkgshare/"init.zsh"
    assert_path_exists pkgshare/"init.bash"
    system bin/"worktree", "remove", "example"
    refute_path_exists Pathname.new(created)
  end
end
