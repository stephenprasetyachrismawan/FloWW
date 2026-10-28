# FloWW

## Commands
- Tests: `cargo test -p floww-core -p floww-engine -p floww-server`
- Lint: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
- UI: `pnpm check` (tsc) and `pnpm build`
- Web: `cargo run -p floww-server --bin floww-web` · Desktop: `pnpm tauri dev`
- Browser test: `pnpm build && cargo build -p floww-server && node e2e/ui-flow.mjs`

## Conventions
- Pure types/logic in `crates/floww-core`; anything that runs processes in `crates/floww-engine`; HTTP only in `crates/floww-server`. `src-tauri` just starts the server.
- The UI talks to the server over HTTP + SSE (`src/lib/api.ts`), in both desktop and web.
- TS types in `src/lib/types.ts` mirror serde output of the Rust structs. Change both together.
- Step → phase mapping lives in `crates/floww-core/src/pipeline.rs` and `src/lib/pipeline.ts`. Keep them equal.
- The server binds 127.0.0.1 only, checks Host, and requires the per-run token on `/api`. Do not weaken this.
- UI: one accent color per screen, only for the main action. Status is a dot plus text. Use `@/components/ui/select` for dropdowns, never native `<select>`.
- Errors shown to users say what is wrong and how to fix it.
- Never store secrets in config files; store keyring references (`keyring:<name>`).
