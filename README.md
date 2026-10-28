# FloWW

Desktop app (Rust + Tauri) that takes a user requirement through PRD, branch, AI coding with TDD,
local Odoo testing, human feedback, and commit/push/PR, with an Obsidian vault as the team's shared brain.

Design: [`docs/DESIGN-DECISIONS.md`](docs/DESIGN-DECISIONS.md) ·
Clickable mockup: [`docs/prototype/floww-prototype.html`](docs/prototype/floww-prototype.html) ·
Research notes: [`docs/research/context-engineering.md`](docs/research/context-engineering.md)

## Status

Scaffold. The app opens, shows the board with example data, and reads real workspace config and
`.vscode/launch.json` from a repo folder (Settings). The pipeline engine, Claude Code runner,
brain and graph come next.

## Layout

```
crates/floww-core/   Domain logic, no Tauri: config, pipeline, PRD ids, Claude Code runner, Odoo preset
src-tauri/           Desktop shell: thin Tauri commands over floww-core
src/                 React + TypeScript UI (Tailwind v4, shadcn/ui)
docs/                Design, prototype, research
```

## Requirements (Linux)

- Rust (stable), Node 22+, pnpm 10+
- Tauri system libraries:

  ```sh
  sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev \
    libayatana-appindicator3-dev libssl-dev libsecret-1-dev patchelf
  ```

- [Claude Code](https://code.claude.com) CLI, signed in (or a 9router instance), for AI steps

## Commands

```sh
pnpm install
pnpm tauri dev                 # run the desktop app with hot reload
pnpm dev                       # UI only in a browser, with example data
cargo test -p floww-core       # domain tests
pnpm check                     # TypeScript check
pnpm tauri build               # .deb and AppImage
```

## Fonts

The UI prefers Anthropic Sans/Serif/Mono when they are installed on your machine (loaded with
`local()`); they are not bundled. Open-licensed fallbacks (Plus Jakarta Sans, Source Serif 4,
JetBrains Mono) ship with the app.
