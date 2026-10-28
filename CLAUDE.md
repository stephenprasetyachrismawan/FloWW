# FloWW

## Commands
- Domain tests: `cargo test -p floww-core`
- Lint: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
- UI: `pnpm check` (tsc) and `pnpm build`
- App: `pnpm tauri dev`

## Conventions
- Business logic goes in `crates/floww-core` (no Tauri dependency) with unit tests. `src-tauri` commands stay thin.
- TS types in `src/lib/types.ts` mirror serde output of floww-core. Change both together.
- Phase ranges live in `crates/floww-core/src/pipeline.rs` and `src/lib/pipeline.ts`. Keep them equal.
- UI: one accent color per screen, only for the main action. Status is a dot plus text. Use `@/components/ui/select` for dropdowns, never native `<select>`.
- Errors shown to users say what is wrong and how to fix it.
- Never store secrets in config files; store keyring references (`keyring:<name>`).
