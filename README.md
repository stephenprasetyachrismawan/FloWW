# FloWW

Desktop and web app (Rust + Tauri) that takes a user requirement through PRD, branch, AI coding
with TDD, local Odoo testing, human feedback, and commit/push/PR, with an Obsidian vault as the
team's shared brain.

Design: [`docs/DESIGN-DECISIONS.md`](docs/DESIGN-DECISIONS.md) ·
Clickable mockup: [`docs/prototype/floww-prototype.html`](docs/prototype/floww-prototype.html) ·
Research notes: [`docs/research/context-engineering.md`](docs/research/context-engineering.md)

## Run it

### Web UI on localhost

```sh
pnpm install && pnpm build          # build the UI once
cargo run --release -p floww-server --bin floww-web -- --port 7171
```

Open the link it prints (`http://127.0.0.1:7171/?token=…`). The token is new on every start and
gives full access, so keep the link private. `--open` opens your browser, `--token` sets a fixed
token, `FLOWW_HOME` picks another config folder (default `~/.config/floww`).

### Desktop app

```sh
pnpm tauri dev                      # development, hot reload
pnpm tauri build                    # .deb and AppImage in target/release/bundle
```

The desktop app runs the same local server on a random port; nothing else changes.

### Requirements (Linux)

- Rust (stable), Node 22+, pnpm 10+, git, Python 3
- [Claude Code](https://code.claude.com) CLI, signed in (`claude login`) or a 9router instance
- `psql` for reading the local Odoo database (optional, read-only)
- `secret-tool` (libsecret-tools) to keep tokens in the keyring; or set `FLOWW_SECRET_<NAME>`
- `gh` CLI or a GitHub token for pull requests
- Desktop build only:

  ```sh
  sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev \
    libayatana-appindicator3-dev libssl-dev libsecret-1-dev patchelf
  ```

## How a request flows

1. **New request** → FloWW checks the brain, reads the local database (read-only) and the code on
   the base branch in a separate worktree.
2. **Grill-me** → one question at a time with a suggested answer; answers go to the brain.
3. **PRD** → written by the AI, shown as HTML (copy HTML/Markdown), approved by you.
4. **Building** → branch `feature/<id>-<slug>`, one fresh Claude Code session per acceptance
   criterion (test first), module tests after each, stop and ask after the loop limit.
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
