# worktree

**Byt uppgift på sekunder. Behåll ditt flow.**

Skapa, hitta och öppna Git-worktrees utan katalogletande. Skriv `worktree` för
en terminalväljare med fuzzy-sökning, piltangenter och musklick. Byggt i Rust
för macOS och Linux, med stöd för Codex-worktrees.

## Installera

Med Homebrew:

```sh
brew tap alex-ross/worktree https://github.com/alex-ross/worktree
brew trust --formula alex-ross/worktree/worktree
brew install --formula --HEAD alex-ross/worktree/worktree
```

Lägg till i `~/.zshrc` för katalogbyte och autocomplete:

```sh
source "$(brew --prefix worktree)/share/worktree/init.zsh"
```

För bash: använd `init.bash` i `~/.bashrc` eller `~/.bash_profile`.
Öppna en ny terminal. Git behövs vid körning; Rust behövs bara vid bygge.

Bygg själv: installera [Rust](https://rustup.rs), klona projektet och kör `sh install.sh`.

## Använd

Kör i ett Git-projekt:

```sh
worktree create foobar          # skapa branch + worktree och gå dit
worktree open foobar            # gå till en befintlig worktree
worktree                       # sök, välj och öppna
worktree fork foobar experiment # kopiera till en ny branch
worktree remove experiment      # ta bort worktree, behåll branch
worktree list                  # lista worktrees; --json för JSON
```

Standardplats: `~/.worktree/<branch>/<projekt>`. Ändra med `--base /sökväg` eller
`WORKTREE_HOME`. En befintlig branch används; annars skapas den från HEAD.

I väljaren: **Ctrl-N** skapar, **Ctrl-F** forkar och **Ctrl-D** tar bort.
Fork behåller staged/unstaged ändringar och untracked filer. Borttagning skyddar
lokala filer; `--force` kastar dem. Lämna worktreen innan du tar bort den.

Lägg relativa filer eller globmönster i `.worktreeinclude` för att kopiera även
ignorerade filer vid create/fork, exempelvis `.env` och `secrets/`. Symboliska
länkar och sökvägar utanför projektet avvisas.

`worktree register /sökväg/till/projekt` gör projektets worktrees synliga överallt.
Codex-worktrees upptäcks under `${CODEX_HOME:-~/.codex}/worktrees`.

## Utveckla

Backend: [`worktree-core`](crates/worktree-core), separat från CLI och terminalväljaren.
Varje PR testas på macOS/Linux, Intel/ARM, inklusive shell, terminal och installation.
Homebrew-installationen testas på macOS och Linux.

```sh
cargo test --workspace --locked
docker build -f Dockerfile.test -t worktree-test-linux .
docker run --rm worktree-test-linux
```
