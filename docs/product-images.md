# Product image provenance

Captured 2026-10-10 from the real `worktree` binary, rebuilt with `cargo build --locked`
from source commit `ffe08b57f87c5a849f5a75592e3d455794fcbad9`. The CLI and picker
source files were unchanged during capture.

- `product-picker.png`: 1596 × 596; 110-column, 18-row picker, after one down-arrow.

These images render the actual ANSI output captured from a pseudo-terminal, including
its cell positions, styles, selection, borders and keyboard hints. Rendering uses a dark
terminal palette and Menlo. No interface elements or result rows were invented or edited.

All repository contents, identities and branches are synthetic demonstration fixtures.
Only the repository names borrow from well-known open-source projects: React, ripgrep
and Rust. The fixtures are empty local Git repositories with a placeholder README; they
are not clones or representations of upstream branches. All displayed paths are under
`/private/tmp/worktree-demo/`. Capture uses isolated home, worktree and Codex directories,
with global/system Git configuration disabled. No personal repository, path or file data
was loaded into the picker.

The real `worktree create` command made eight fixture worktrees. Capture assertions
checked their names and paths, the 8/8 initial count, the 1/8 filtered count, and absence
of personal home paths. The PNG was visually inspected. The existing
`python3 tests/tui_smoke.py target/debug/worktree` passed, covering fuzzy search,
arrows, mouse, create, fork, remove, zsh completion and terminal restoration.

The temporary capture script and raw ANSI recordings are local generation intermediates,
not website dependencies. The website needs only the PNG assets.
