# FloWW

Desktop and web app (Rust + Tauri) that takes a user requirement through PRD, branch, AI coding
with TDD, local Odoo testing, human feedback, and commit/push/PR, with an Obsidian vault as the
team's shared brain.

Design: [`docs/DESIGN-DECISIONS.md`](docs/DESIGN-DECISIONS.md) ·
Clickable mockup: [`docs/prototype/floww-prototype.html`](docs/prototype/floww-prototype.html) ·
Research notes: [`docs/research/context-engineering.md`](docs/research/context-engineering.md)

## Install

Download from [Releases](https://github.com/stephenprasetyachrismawan/FloWW/releases/latest)
(Linux x86_64). Pick one.

**Desktop app, .deb (Ubuntu/Debian)**

```sh
curl -LO https://github.com/stephenprasetyachrismawan/FloWW/releases/download/v0.1.5/FloWW_0.1.5_amd64.deb
sudo apt install ./FloWW_0.1.5_amd64.deb
floww                               # or open "FloWW" from the app menu
```

**Desktop app, AppImage (any distro)**

```sh
curl -LO https://github.com/stephenprasetyachrismawan/FloWW/releases/download/v0.1.5/FloWW_0.1.5_amd64.AppImage
chmod +x FloWW_0.1.5_amd64.AppImage
./FloWW_0.1.5_amd64.AppImage        # needs libfuse2 (libfuse2t64 on Ubuntu 24.04)
```

**Web UI in your browser (floww-web)**

```sh
curl -LO https://github.com/stephenprasetyachrismawan/FloWW/releases/download/v0.1.5/floww-web-v0.1.5-linux-x86_64.tar.gz
tar xzf floww-web-v0.1.5-linux-x86_64.tar.gz
install -Dm755 floww-web-v0.1.5-linux-x86_64/floww-web ~/.local/bin/floww-web
floww-web --open                    # default port 7171
```

`floww-web` prints a link like `http://127.0.0.1:7171/?token=…` and `--open` opens it. The token
is new on every start and gives full access, so keep the link private. Other options:
`--port 8080`, `--token <value>` for a fixed link, `FLOWW_HOME=/path` for another config folder
(default `~/.config/floww`). It listens on 127.0.0.1 only; from another machine use
`ssh -L 7171:127.0.0.1:7171 your-server`.

**What FloWW needs on the machine**

- [Claude Code](https://code.claude.com) CLI, signed in (`npm install -g @anthropic-ai/claude-code`,
  then `claude login`), or a 9router address and key
- git and Python 3
- Optional: `psql` to read the local Odoo database (read-only), `secret-tool` (libsecret-tools) to
  keep tokens in the keyring (or set `FLOWW_SECRET_<NAME>`), `gh` CLI or a GitHub token for PRs

**Updating:** press **Check for updates** at the bottom of the sidebar. If a newer release exists,
FloWW shows what changes and which old installer files it will delete, then:

- desktop app: it closes, a small window shows the download and install, and the new version
  opens (the .deb asks for your password; the AppImage replaces itself);
- floww-web: it downloads the new version, restarts on the same port and link, and the page
  reloads.

Every download is checked against the release's `SHA256SUMS.txt` before anything is replaced.
Your settings (`~/.config/floww`), workspaces and brain are never touched. Old FloWW release files
(`FloWW_*.deb`, `FloWW_*.AppImage`, `floww-web-v*` tarballs and folders) older than the new version
are removed from `~/Downloads`, `~/Unduhan` and the folder FloWW runs from. If nothing newer
exists, it says you have the latest version.

**First run:** choose your Claude login or 9router, pick a model and send a test message, then
**Add workspace** and press **Choose folder…** to pick your repo. FloWW finds the Odoo command in `.vscode/launch.json`, the
base branch, and the local database.

## Run from source

### Web UI on localhost

```sh
pnpm install && pnpm build          # build the UI once
cargo run --release -p floww-server --bin floww-web -- --port 7171
```

### Desktop app

```sh
pnpm tauri dev                      # development, hot reload
pnpm tauri build                    # .deb and AppImage in target/release/bundle
```

The desktop app runs the same local server on a random port; nothing else changes.

### Build requirements (Linux)

- Rust (stable), Node 22+, pnpm 10+, plus the runtime tools above
- Desktop build only:

  ```sh
  sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev \
    libayatana-appindicator3-dev libssl-dev libsecret-1-dev patchelf
  ```

### Releasing

Bump the version in `src-tauri/tauri.conf.json`, add `docs/release-notes/vX.Y.Z.md`, and merge to
`main`. The Release workflow sees a version with no tag yet, builds the .deb, AppImage and
floww-web tarball, creates tag `vX.Y.Z` and publishes the release. Pushing the tag yourself works
too.

## How a request flows

1. **New request** → FloWW checks the brain, reads the local database (read-only) and the code on
   the base branch in a separate worktree.
2. **Grill-me** → one question at a time with a suggested answer; answers go to the brain.
3. **PRD** → written by the AI, shown as HTML (copy HTML/Markdown), approved by you.
4. **Building** → a branch from the development branch, named by your template (default
   `dev/{ddmmyyyy}_{slug}`, e.g. `dev/28102026_invoice-dobel-saat`; both set in Settings), then one
   Claude Code session builds the whole PRD: the PRD is the spec, tests come first, and it runs the
   module tests itself. If tests still fail it asks you; your hint continues the same session.
5. **Testing** → FloWW upgrades the modules and starts Odoo (one request at a time on the shared
   database); you mark each case Works/Broken; feedback goes back to step 4.
6. **Ship** → AI-drafted commit message you can edit, one squashed commit, push, optional PR.

The brain lives on the orphan branch `floww/brain` (worktree `.floww/brain`), synced for the team.

## Layout

```
crates/floww-core/     Domain types: config, pipeline steps, PRD ids, Claude Code args, Odoo launch.json
crates/floww-engine/   Runs the work: PRD store, Claude runner, git/worktrees, DB lock, Odoo, brain, skills
crates/floww-server/   Local HTTP API + live events + embedded UI; binary `floww-web`
src-tauri/             Desktop shell that starts floww-server internally
src/                   React + TypeScript UI (Tailwind v4, shadcn/ui)
e2e/                   Browser test of the whole flow
docs/                  Design, prototype, research
```

## Checks

```sh
cargo test -p floww-core -p floww-engine -p floww-server   # includes an end-to-end PRD run
cargo clippy --workspace --all-targets -- -D warnings
pnpm check && pnpm build
cargo build -p floww-server && node e2e/ui-flow.mjs           # browser test (needs Playwright)
```

The end-to-end tests use a real git repo and remote with fake `claude` and `odoo-bin` scripts, so
they run without an AI account or Odoo.

## Fonts

The UI prefers Anthropic Sans/Serif/Mono when they are installed on your machine (loaded with
`local()`); they are not bundled. Open-licensed fallbacks (Plus Jakarta Sans, Source Serif 4,
JetBrains Mono) ship with the app.
