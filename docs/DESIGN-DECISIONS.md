# FloWW — Design Decisions (hasil grill-me #1)

Status: **disepakati**, belum ada kode. Dokumen ini jadi acuan awal implementasi.

FloWW adalah aplikasi desktop (Rust + Tauri) yang mengorkestrasi alur:
requirement → PRD → branch → AI coding (TDD/SDD) → run & test lokal → feedback manusia → commit/push/PR,
dengan **Obsidian sebagai brain per project**. Odoo jadi preset pertama, arsitektur generik untuk framework lain.

---

## 1. Ringkasan keputusan

| # | Topik | Keputusan |
|---|---|---|
| 1 | UI stack | **Tauri v2** (backend Rust, frontend web: React/Svelte) |
| 2 | Engine AI | **Claude Code CLI headless** untuk *semua* step (coding + chat). FloWW spawn `claude -p --output-format stream-json` |
| 3 | Auth AI | Dua mode: **(a) subscription** (`claude login` yang sudah ada), **(b) 9router** (`ANTHROPIC_BASE_URL` + `ANTHROPIC_AUTH_TOKEN` di env child process) |
| 4 | Brain (Obsidian) | **Orphan branch `floww/brain`**, di-checkout sebagai worktree `.floww/brain/`. Auto commit + `pull --rebase` + push. Dipakai tim |
| 5 | Git host | **GitHub** dulu, di balik trait `GitHost` |
| 6 | Akses DB (local/SSH) | **Introspeksi read-only** (schema + metadata Odoo). Tidak pernah menulis ke remote |
| 7 | Preprod | Kalau belum ada: buat **branch + konvensi** + **provision environment di server tujuan** (docker compose via SSH). CI GitHub di-skip |
| 8 | Run Odoo lokal | **Native venv via `.vscode/launch.json`** (parse args `odoo-bin`) |
| 9 | Grill-me di UI | Claude keluarkan **JSON terstruktur** `{question, options, recommendation}` per turn → kartu pilihan di UI → jawaban dikirim via `claude -p --resume <session_id>` |
| 10 | Isolasi PRD | **Git worktree per PRD** (`.floww/worktrees/<prd-id>`), coding AI bisa paralel |
| 11 | DB lokal | **Satu DB dipakai bersama + run lock**: step yang menyentuh DB (test, run, upgrade, human test) antre satu PRD pada satu waktu. Opsi DB per PRD menyusul |
| 12 | Skills | Pakai **skill Claude Code** yang ada. Menu **Install Skill**: AI install + **inspeksi dengan skillspector**. User pilih skill per tahap |
| 13 | Skill gate | Temuan **high-risk diblokir** kecuali override manual; medium/low = warning + konfirmasi. Hasil scan dicatat di brain |
| 14 | Graph | **Graphify + layer domain Odoo** (model, field, `_inherit` chain, view, fitur→subfitur, business flow). Incremental, background worker |
| 15 | Human test | **Checklist dari PRD + rekam browser (Playwright)** → dikonversi jadi **test native framework** (Odoo: `HttpCase` + tour) dan di-commit di branch PRD |
| 16 | Release flow | **Deploy preprod saja** (tombol manual + backup DB + upgrade modul + health check). Ke prod **berhenti di cherry-pick** preprod → release branch memakai pola skill `release-to-<target>` + `<framework>-cherry-pick-guard` |
| 17 | Guard belum ada | **Generate dari blueprint** (jawab A–G → approval user → buat guard + release skill → tes → skillspector → install). Release terkunci sampai guard lolos |
| 18 | Secret | **OS keyring** (crate `keyring`, Linux Secret Service). Config hanya simpan referensi (`keyring:<nama>`). SSH via ssh-agent/key file |
| 19 | OS | **Linux** dulu |
| 20 | Pengguna | **Tim dari awal** |
| 21 | Scope MVP | **Main flow Odoo end-to-end + graphify** |

---

## 2. Arsitektur

```
┌──────────────────────────── Tauri UI (web) ────────────────────────────┐
│ Project picker · Pipeline board · Grill cards · PRD HTML viewer        │
│ Log stream · Human test checklist · Commit editor · Skill manager      │
└───────────────▲───────────────────────────────────────┬────────────────┘
                │ events (tauri emit)                    │ commands
┌───────────────┴───────────────── Rust core ────────────▼────────────────┐
│ PipelineEngine   state machine per PRD, persisted (.floww/runs/*.json)   │
│ ClaudeRunner     spawn `claude -p`, parse stream-json, resume session    │
│ AuthProfile      subscription | 9router (env injection, health check)    │
│ BrainWriter      job queue + worker → .floww/brain (git sync)            │
│ GraphBuilder     graphify + Odoo domain layer → brain notes              │
│ FrameworkAdapter preset (odoo, …) + override dari project.toml           │
│ Runner           launch.json → child process, port, log, upgrade module  │
│ DbInspector      postgres local / SSH tunnel, read-only                  │
│ GitOps           worktree, branch, commit, push (git2 / git CLI)         │
│ GitHost          trait → GitHub (octocrab)                               │
│ SkillManager     list/install skill + skillspector scan + gate           │
│ Locks            DB run lock, brain write lock                           │
│ Secrets          keyring                                                 │
└──────────────────────────────────────────────────────────────────────────┘
```

Prinsip: **deterministik untuk fakta, LLM untuk penilaian** (diambil dari blueprint cherry-pick).
Daftar modul yang berubah, versi manifest, migration, dan diff dihitung Rust/script; Claude merangkum dan menilai.

### Pemanggilan Claude Code

```
claude -p "<prompt step>" \
  --output-format stream-json --verbose \
  --resume <session_id>            # untuk lanjut grill / feedback loop
  --append-system-prompt "<konteks step + skill terpilih>" \
  --allowedTools "<per step>"      # mis. step PRD: read-only; step develop: Edit/Bash
```

- cwd = worktree PRD.
- Env: mode 9router → `ANTHROPIC_BASE_URL=http://localhost:<port>` (tanpa `/v1`), `ANTHROPIC_AUTH_TOKEN=<dari keyring>`. Mode subscription → env dibiarkan.
- Skill per step: FloWW memerintahkan Claude memakai skill yang dipilih (nama skill di prompt/system prompt).

---

## 3. Pipeline (main flow)

| # | Step | Siapa | Output |
|---|---|---|---|
| 1 | User requirement logging | Manusia | `brain/requirements/<id>.md` |
| 2 | Lihat yang sudah ada di brain | Claude (read) | Ringkasan terkait + link ke note |
| 3 | Lihat database (local / SSH, read-only) | Rust + Claude | Snapshot schema/modul relevan |
| 4 | Lihat codebase (pilih branch) | Claude (read) | Konteks kode |
| 5 | Update brain | BrainWriter (async) | Note fitur/modul diperbarui |
| 6 | Buat PRD dengan grill-me | Claude + manusia | `brain/prd/<id>.md` + Q&A log |
| 7 | PRD → HTML developer-readable, bisa di-copy | Rust (template) | `prd/<id>.html` |
| 8 | Buat branch dari preprod (worktree) | GitOps | `feature/<id>-<slug>` |
| 9 | Develop dengan TDD + SDD (skill pilihan user) | Claude | Commit kerja lokal (belum push) |
| 10 | Full suite test (opsional) | Runner | Laporan test |
| 11 | Log ke brain: perubahan kode + migration | BrainWriter (async) | `brain/changes/<id>.md` |
| 12 | Jalankan local dev | Runner (lock DB) | Odoo hidup di port X |
| 13 | Upgrade / install modul | Runner (lock DB) | `-u <mods> --stop-after-init` |
| 14 | Human test (normal + edge) + rekam browser | Manusia | Checklist pass/fail + rekaman |
| 15 | Human feedback | Manusia | Ada feedback/fail → **loop ke step 9** |
| 16 | Commit message (editable) | Claude → manusia | Pesan final |
| 17 | Human klik commit | Manusia | Commit |
| 18 | Push branch | GitOps | — |
| 19 | PR ke preprod (opsional) | GitHost | PR. Saat merged → log ke brain |

Step 5, 11, dan update graph berjalan **paralel di background**, tidak menahan pipeline.

### Flow release (tambahan)

1. Deploy preprod: tombol manual → backup DB preprod → pull kode → upgrade modul yang berubah → health check → log brain. Gagal → tawarkan restore.
2. Rilis ke prod: berhenti di **cherry-pick preprod → `release/<ddmmyyyy>_<Fitur>`** memakai skill `release-to-<target>` + `<framework>-cherry-pick-guard` (2 stop point manusia, tanpa push/deploy).
3. Provision preprod (kalau belum ada): generate `docker-compose.preprod.yml` dari preset → upload + `docker compose up` via SSH.

---

## 4. Struktur per project

```
<repo>/
├── .floww/
│   ├── project.toml          # di-commit: preset, perintah, branch, skill per step
│   ├── templates/            # di-commit: template PRD HTML, commit msg (opsional)
│   ├── brain/                # worktree dari orphan branch floww/brain (vault Obsidian)
│   ├── worktrees/<prd-id>/   # gitignored
│   └── runs/<prd-id>.json    # gitignored: state pipeline
└── .vscode/launch.json
```

Contoh `project.toml`:

```toml
[project]
name = "acme-odoo"
preset = "odoo"            # odoo | django | laravel | rails | node | custom

[git]
base_branch = "preprod"
prod_branch = "master"
feature_branch = "feature/{prd_id}-{slug}"
release_branch = "release/{ddmmyyyy}_{feature}"

[ai]
auth = "subscription"      # subscription | 9router
router_url = "http://localhost:20128"
router_token = "keyring:floww-9router"

[database]
mode = "ssh"               # local | ssh
host = "db.internal"
ssh = "deploy@prod-bastion"
user = "readonly"
password = "keyring:acme-db-ro"

[run]
launch_config = "Odoo 17"  # nama config di launch.json
shared_db = true           # true = run lock

[steps.prd]
skills = ["grill-me"]
[steps.develop]
skills = ["tdd", "sdd"]
[steps.test]
full_suite = false

[preset.odoo]
upgrade_cmd = "{python} {odoo_bin} -c {conf} -d {db} -u {modules} --stop-after-init"
test_cmd    = "{python} {odoo_bin} -c {conf} -d {db} --test-enable --test-tags /{modules} --stop-after-init"
```

### Brain (vault Obsidian)

```
brain/
├── index.md                  # di-generate ulang, tidak di-merge
├── requirements/<id>.md
├── prd/<id>.md
├── features/<feature>.md     # fitur → subfitur (wikilinks)
├── modules/<module>.md       # Odoo: model, field, _inherit, view
├── flows/<business-flow>.md  # memori business flow
├── changes/<prd-id>.md       # perubahan kode + migration per PRD
├── releases/<tag>.md
├── skills/scans/<skill>.md   # hasil skillspector
└── graph/                    # output graphify
```

Aturan sinkron tim: tiap PRD/run menulis ke note miliknya sendiri; file agregat (`index.md`, graph) di-generate ulang setelah pull supaya jarang konflik.

---

## 5. Fase

- **Fase 1 (MVP):** main flow Odoo step 1–19, Claude runner + 2 mode auth, brain git sync, graphify + layer Odoo, run lock, skill manager + skillspector, keyring, Linux.
- **Fase 2:** rekam browser → test native, deploy preprod, provisioning preprod, flow release (cherry-pick + guard generator).
- **Fase 3:** preset framework lain, DB per PRD, GitLab/Gitea, macOS/Windows.

> Catatan: rekam browser dan release disetujui, tapi saya taruh di fase 2 supaya MVP bisa dipakai lebih cepat. Bisa dinaikkan ke fase 1 kalau perlu.

---

## 6. Pertanyaan terbuka (grill ronde berikutnya)

1. Template PRD: section apa saja (problem, scope, acceptance criteria, data model, migration, risiko)? Ada contoh PRD tim?
2. Konvensi commit message: Conventional Commits, format Odoo (`[FIX] module: ...`), atau lainnya?
3. `launch.json` punya banyak config: pilih per project atau per PRD?
4. Deteksi modul terdampak: dari diff (folder ber-`__manifest__.py`), atau user pilih manual?
5. Graphify: dijalankan sebagai skill Claude Code atau CLI Python langsung? Seberapa sering refresh penuh?
6. Run lock: kalau PRD lain sedang memegang lock, apakah boleh "rebut" setelah konfirmasi?
7. Aturan versi manifest Odoo: apakah AI wajib bump `version` saat ada migration?
8. Siapa boleh push ke `floww/brain`: semua anggota, atau perlu review?
