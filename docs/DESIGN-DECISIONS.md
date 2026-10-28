# FloWW — Design Decisions (Fase 1)

Status: **disepakati lewat grill-me**, belum ada kode. Fokus dokumen ini hanya **fase 1**.
Fitur yang ditunda ada di bagian 9.

FloWW adalah aplikasi desktop (Rust + Tauri) yang mengorkestrasi alur:
request → PRD → branch → AI coding (TDD/SDD) → run & test lokal → feedback manusia → commit/push/PR,
dengan **Obsidian sebagai brain per workspace**. Odoo jadi preset pertama; arsitektur tetap generik.

---

## 1. Ringkasan keputusan

### Platform & AI

| Topik | Keputusan |
|---|---|
| UI stack | **Tauri v2** + **React + TypeScript + shadcn/ui** |
| OS | **Linux** |
| Engine AI | **Claude Code CLI headless** untuk *semua* step (setup, chat, PRD, coding, commit msg, ringkasan brain) |
| Auth AI | Diset sekali di first run: **subscription** (`claude login`) atau **9router** (`ANTHROPIC_BASE_URL` + `ANTHROPIC_AUTH_TOKEN` di env child process) |
| Model | **Model default global** + **override per step** di settings workspace. Daftar model: auto-fetch dari 9router (kalau tersedia) / alias Claude Code, plus input bebas. Dikirim via `--model` |
| Izin AI saat develop | **Bebas di dalam worktree** (`bypassPermissions`) |
| Izin AI saat setup workspace | **Bebas penuh** (boleh buat venv, pip install, createdb, launch.json, branch preprod, install graphify) |
| Loop TDD | Maks **5 iterasi atau 30 menit** per step (bisa diatur), lalu berhenti dan eskalasi ke user |
| Biaya | Tampilkan token/biaya dari `stream-json` per step & per PRD. Mode subscription: token saja |
| Secret | **OS keyring** (crate `keyring`). Config hanya simpan referensi `keyring:<nama>` |

### Workspace, git, data

| Topik | Keputusan |
|---|---|
| Workspace | **1 workspace = 1 folder repo**. Kanban, config, DB, brain semuanya per workspace |
| Config | **Dua lapis.** `brain/_floww/team.toml` = aturan kerja tim (shared via `floww/brain`): preset, base branch, pola branch, format commit, skill per step, batas loop, template PRD, bahasa AI. `.floww/workspace.toml` = per orang (gitignored): DB lokal, port, venv, launch config, auth & model AI. Lokal boleh override team |
| Edit rules | **Semua anggota boleh ubah, dengan jejak**: tiap perubahan rule = commit sendiri di `floww/brain` (author + alasan), tampil di Team activity. Perubahan rule oleh AI selalu minta konfirmasi manusia |
| Brain | **Orphan branch `floww/brain`**, worktree di `.floww/brain/`. Semua anggota auto commit + push. Note agregat di-regenerate saat konflik; konflik di note manual → tampilkan diff, user pilih |
| Git host | **GitHub** (trait `GitHost`) |
| Base branch | Step 4 selalu baca `base_branch` (preprod) terbaru setelah `git fetch`. Kalau preprod belum ada → dibuat saat setup |
| Isolasi PRD | **Git worktree per PRD** (`.floww/worktrees/<prd-id>`), coding AI paralel |
| DB | **DB lokal yang ditentukan di workspace**. Dipakai untuk introspeksi (read-only), run, test, dan human test. SSH dihapus dari fase 1 |
| DB lock | Satu DB dipakai bersama → step yang menyentuh DB antre. **Antre + "Take over" dengan konfirmasi**. Lock lepas otomatis saat proses selesai/crash (heartbeat) |
| Run Odoo | **Native venv via `.vscode/launch.json`**. Config default per workspace, bisa di-override per PRD. FloWW menambah `-d`, `--http-port`, `-u` sendiri tanpa mengubah launch.json |
| Modul terdampak | **Auto dari diff** (folder ber-`__manifest__.py` yang disentuh, opsional + modul yang `depends`), dikonfirmasi user sebelum upgrade |
| Versi manifest | **Wajib bump kalau ada migration**. Rust validasi versi tidak turun dan folder `migrations/<v>` = versi baru. Tanpa migration: bump patch opsional |

### Konten

| Topik | Keputusan |
|---|---|
| Grill-me | Claude keluarkan JSON `{question, options, recommendation, why}` per turn → kartu di UI → jawaban via `claude -p --resume <session_id>` |
| Format PRD | 1) Latar & problem 2) Goal / non-goal 3) User story 4) Acceptance criteria (Given/When/Then) 5) Desain teknis (model/field/view/security) 6) Migration & data 7) Modul terdampak 8) Risiko & edge case 9) Test plan 10) Q&A grill log |
| PRD HTML | Render dari markdown, bisa **Copy HTML / Copy Markdown / Open in Obsidian / Edit**. **Approve PRD wajib** sebelum Building |
| Commit message | **Format Odoo**: `[FIX] sale_custom: prevent double invoice` (tag FIX/IMP/ADD/REF/MIG/REM), body = why + PRD id. Draft AI, bisa diedit |
| Skills | Pakai **skill Claude Code** yang ada. Menu **Install skill**: AI install + scan **skillspector**. High-risk diblokir kecuali override manual; medium/low = warning + konfirmasi. User pilih skill per step |
| Graph | **Graphify CLI** (venv dikelola FloWW) + **layer domain Odoo** dibuat Rust (manifest, model, field, `_inherit`, XML view). Full build saat workspace dibuat, incremental tiap commit/merge. Claude hanya untuk ringkasan fitur & business flow |
| Human test | **Checklist dari acceptance criteria** (normal + edge): langkah, data, expected, Pass/Fail, catatan, screenshot |

---

## 2. Arsitektur

```
┌──────────────────────── Tauri UI (React + shadcn/ui) ─────────────────────┐
│ Workspace switcher · Kanban · PRD detail · Grill cards · PRD viewer       │
│ AI timeline · Test checklist · Ship page · Brain & Graph · Skills · Runs  │
└───────────────▲───────────────────────────────────────┬───────────────────┘
                │ events (tauri emit)                    │ commands (invoke)
┌───────────────┴──────────────────── Rust core ─────────▼──────────────────┐
│ WorkspaceManager  workspace.toml, switcher, first-run AI config           │
│ SetupAgent        Claude inspeksi repo + device, isi/siapkan config       │
│ PipelineEngine    state machine per PRD, persisted (.floww/runs/*.json)   │
│ ClaudeRunner      spawn `claude -p`, parse stream-json, resume, usage     │
│ AuthProfile       subscription | 9router (env, health check, model list)  │
│ BrainWriter       job queue + worker → .floww/brain (git sync)            │
│ GraphBuilder      graphify CLI + Odoo domain layer → brain notes          │
│ FrameworkAdapter  preset odoo (perintah run/test/upgrade, deteksi modul)  │
│ Runner            launch.json → child process, port, log, upgrade         │
│ DbInspector       postgres lokal, read-only                               │
│ GitOps            worktree, branch, commit, push                          │
│ GitHost           trait → GitHub (octocrab)                               │
│ SkillManager      list/install skill + skillspector gate                  │
│ Locks             DB run lock (queue, take over, heartbeat)               │
│ Notifier          notifikasi desktop                                      │
│ Secrets           keyring                                                 │
└───────────────────────────────────────────────────────────────────────────┘
```

Prinsip: **deterministik untuk fakta, LLM untuk penilaian.** Modul terdampak, versi manifest,
migration, diff, dan hasil test dihitung Rust. Claude merangkum, memutuskan desain, dan menulis kode.

### Pemanggilan Claude Code

```
claude -p "<prompt step>" \
  --model <model step> \
  --output-format stream-json --verbose \
  --resume <session_id> \
  --append-system-prompt "<konteks step + skill terpilih>" \
  --permission-mode bypassPermissions      # step develop & setup
```

- cwd = worktree PRD (develop) atau root repo (setup).
- Mode 9router: env `ANTHROPIC_BASE_URL=http://localhost:<port>` (tanpa `/v1`), `ANTHROPIC_AUTH_TOKEN` dari keyring.
- Step baca saja (brain, DB, codebase, PRD) tetap dijalankan tanpa tool tulis.
- Tanpa deny-list. Commit & push di flow tetap lewat tombol manusia.

---

## 3. Pipeline (main flow)

| # | Step | Siapa | Kolom kanban |
|---|---|---|---|
| 1 | New request (form) | Manusia | Request |
| 2 | Cek brain (mirip/duplikat?) | Claude | Request |
| 3 | Introspeksi DB lokal workspace | Rust + Claude | Request |
| 4 | Baca codebase di base branch | Claude | Request |
| 5 | Update brain | BrainWriter (async) | Request |
| 6 | PRD dengan grill-me | Claude + manusia | Planning |
| 7 | PRD HTML + **Approve** | Manusia | Planning |
| 8 | Buat branch + worktree dari preprod | GitOps | Building |
| 9 | Develop TDD + SDD (skill pilihan) | Claude | Building |
| 10 | Full suite test (opsional) | Runner | Building |
| 11 | Log perubahan kode + migration ke brain | BrainWriter (async) | Building |
| 12 | Jalankan Odoo lokal (lock DB) | Runner | Testing |
| 13 | Upgrade / install modul (konfirmasi daftar) | Runner | Testing |
| 14 | Human test checklist | Manusia | Testing |
| 15 | Feedback → **loop ke step 9** sampai puas | Manusia | Testing |
| 16 | Commit message (draft AI, editable) | Claude → manusia | Ship |
| 17 | Commit (tombol) | Manusia | Ship |
| 18 | Push (tombol) | Manusia | Ship |
| 19 | PR ke preprod (opsional) | GitHost | Ship |
| — | PR merged → log brain, hapus worktree (konfirmasi) | Otomatis | Done |

Step 5, 11, dan update graph berjalan **paralel di background**.

---

## 4. Struktur file

```
<repo>/
├── .floww/                   # seluruhnya gitignored kecuali brain (branch sendiri)
│   ├── workspace.toml        # config per orang
│   ├── brain/                # worktree orphan branch floww/brain (vault Obsidian)
│   ├── worktrees/<prd-id>/
│   └── runs/<prd-id>.json    # state pipeline
└── .vscode/launch.json

~/.config/floww/app.toml      # first run: auth AI, model default, daftar workspace
```

Contoh `brain/_floww/team.toml` (shared):

```toml
[workspace]
preset = "odoo"
language = "id"

[git]
base_branch = "preprod"
feature_branch = "feature/{prd_id}-{slug}"
commit_format = "odoo"

[steps.prd]
skills = ["grill-me"]
template = "_floww/templates/prd.md"
[steps.develop]
skills = ["tdd", "sdd"]
max_iterations = 5
max_minutes = 30
[steps.test]
full_suite = false
```

Contoh `.floww/workspace.toml` (per orang; key yang sama dengan team.toml berarti override):

```toml
[workspace]
name = "acme-odoo"
initials = "SP"                # prefix ID PRD

[git]
github_token = "keyring:floww-github"

[run]
launch_config = "Odoo 17"
db = "acme_dev"
http_port = 8069

[models]                       # override per step; kosong = default global
develop = "opus"
brain = "haiku"
commit = "haiku"

[budget]
per_prd_usd = 0                # 0 = tanpa batas
```

### Brain (vault Obsidian)

Brain terdiri dari beberapa **sub-brain**. Prinsipnya: semua isinya harus bisa dibaca dan dipahami manusia, dan AI juga mencatat **apa yang sudah dan belum dipahami manusia** supaya AI tahu celahnya.

```
brain/
├── index.md                       # di-generate ulang
├── _floww/                        # aturan kerja tim (shared)
│   ├── team.toml
│   ├── templates/prd.md
│   └── .counters/<user>.json      # counter ID PRD per user
├── requirements/<id>.md           # request mentah dari user field
├── prd/<id>.md
├── decisions/<prd-id>.md          # keputusan grill-me: pertanyaan, opsi, rekomendasi AI, pilihan manusia, alasan
├── opinions/<topic>.md            # pendapat / preferensi manusia (mis. "lebih suka constraint di model, bukan di view")
├── rules/<rule>.md                # business rules
├── flows/<business-flow>.md       # flow normal (order-to-cash, procure-to-pay, ...)
├── understanding/
│   ├── people/<user>.md           # yang dikuasai, yang masih ragu, yang belum tahu
│   └── team.md                    # peta gabungan + area yang tidak ada orang paham
├── features/<feature>.md          # fitur → subfitur
├── modules/<module>.md            # model, field, _inherit, view
├── changes/<prd-id>.md            # perubahan kode + migration
├── skills/scans/<skill>.md        # hasil skillspector
└── graph/                         # output graphify
```

| Sub-brain | Isi | Sumber | Dipakai untuk |
|---|---|---|---|
| `decisions/` | Tiap pertanyaan grill-me: opsi, rekomendasi AI + alasan, pilihan manusia, alasan manusia (kalau ada), tanggal, siapa | Otomatis dari grill-me | Konteks PRD berikutnya, mencegah pertanyaan yang sama diulang |
| `opinions/` | Preferensi & pendapat manusia yang bukan aturan bisnis | Jawaban "Other", hint, catatan feedback, edit PRD manual | Rekomendasi grill & gaya kode menyesuaikan tim |
| `rules/` | Pernyataan aturan, alasan bisnis, sumber (PRD id), tempat ditegakkan (model/method/constraint), test yang menjaga | AI ekstrak dari PRD saat approve + dari kode saat graph build | Grill & develop wajib membaca rules terkait; peringatan kalau perubahan melanggar rule |
| `flows/` | Langkah flow normal + rules dan modul yang terlibat | AI ringkas dari kode, PRD, dan jawaban grill | Konteks request baru, penjelasan ke manusia |
| `understanding/` | Per orang: paham / ragu / belum tahu, per topik; tim: siapa paham apa, area tanpa pemilik | Disimpulkan AI dari sinyal, **selalu dikonfirmasi** manusia | Lihat di bawah |

**Mencatat pemahaman manusia**
- Sinyal: jawaban "Other" atau "tidak tahu" di grill, pertanyaan balik, catatan feedback human test, hint ke AI, PRD yang diedit manual.
- AI tidak mencatat diam-diam. Tiap kesimpulan tampil sebagai kartu kecil: *"Saya catat: kamu belum yakin soal rantai backorder. Benar?"* → **Ya / Tidak / Ubah**.

**Memakai celah pemahaman**
1. **Grill-me** menyertakan pertanyaan untuk menutup celah saat PRD menyentuh topik itu, didahului penjelasan singkat.
2. **PRD & human test** diberi kotak *"Konteks: kenapa begini"* untuk topik yang ditandai belum dipahami pembaca.
3. **Peringatan saat AI ragu**: kalau AI menyentuh area yang tidak dipahami siapa pun di tim (`understanding/team.md`), AI meminta konfirmasi lebih ketat sebelum lanjut.

Contoh `rules/no-double-invoice.md`:

```markdown
---
type: rule
status: active
source: [[prd/SP-012]]
enforced_in: sale_custom / sale.order.line._compute_qty_to_invoice
tests: test_cancel_then_redeliver_invoices_once, test_backorder_twice
flows: [[flows/order-to-cash]]
---
# Qty yang sudah ditagih tidak boleh ditagih lagi

**Kenapa:** 7 kasus invoice dobel di Oktober (Rp 41,8 juta).
**Berlaku:** semua SO dengan pengiriman parsial, batal, atau backorder.
```

---

## 5. Onboarding (AI-first)

**First run (sekali per aplikasi)**
1. Pilih auth: subscription (cek `claude` CLI + login) atau 9router (URL + token).
2. Pilih model default dari daftar (auto-fetch / alias + input bebas) → **Test AI**.

**Add workspace**
1. Pilih folder repo.
2. **Setup agent** (model default) memeriksa repo + device: preset, launch.json, odoo.conf, venv/python, versi Odoo, postgres & DB, branch preprod, graphify.
   - Kalau sudah ada → diisi otomatis.
   - Kalau belum ada → AI menyiapkannya langsung (bebas penuh), tampil di timeline.
   - Kalau ambigu (mis. 3 odoo.conf) → **kartu grill** dengan rekomendasi + alasan.
3. Layar lanjutan (git, run, models, skills per step, brain) tampil **dengan default yang dipilihkan AI** → user konfirmasi/ubah.
4. Ringkasan config → **Looks good** → simpan `workspace.toml`, buat/clone `floww/brain`, build graph di background.

---

## 6. Desain UI

### Gaya
- **Font: Anthropic Sans / Serif / Mono** dipanggil lewat `local()` dari font yang terpasang di mesin user, dengan fallback font terbuka. File font **tidak** dibundel atau di-commit, karena repo sumbernya tidak punya lisensi dan hak fontnya milik Anthropic.
- **Ramah non-teknis**: lapang, kartu besar, bahasa sederhana.
- **Light default** + dark mode. Aksen **indigo**.
- Status selalu warna + ikon + teks: hijau = selesai, amber = needs you, biru = AI working, merah = gagal.
- Label UI bahasa **Inggris**, struktur i18n siap untuk Bahasa Indonesia.

### Navigasi

```
┌──────────────┬──────────────────────────────────────────────────────┐
│ acme-odoo ▾  │  (konten halaman)                                    │
│──────────────│                                                      │
│ ▣ Board      │                                                      │
│ ◈ Brain      │                                                      │
│ ✦ Skills     │                                                      │
│ ▶ Runs       │                                                      │
│ ⚙ Settings   │                                                      │
└──────────────┴──────────────────────────────────────────────────────┘
```

- **Workspace switcher** kiri atas: nama, path, jumlah "needs you". Item "Add workspace".
- **Board**: kanban.
- **Brain & Graph**: tab per sub-brain (Decisions, Rules, Flows, Opinions, Understanding, Graph) + "Open vault in Obsidian". Kartu konfirmasi pemahaman muncul di sini dan di PRD terkait.
- **Skills**: skill terinstall, install baru + hasil skillspector, mapping skill per step.
- **Runs & DB lock**: proses Odoo, port, pemegang lock + antrean, log server.
- **Settings**: form untuk `workspace.toml` (git, run, model per step, batas loop/biaya).

### Board (kanban)

```
[ + New request ]                         [ Needs me ◻ ]   🔍
┌ Request ──┬ Planning ─┬ Building ─┬ Testing ──┬ Ship ─────┬ Done ───┐
│ ┌───────┐ │ ┌───────┐ │ ┌───────┐ │           │           │ ┌─────┐ │
│ │SP-013│ │ │SP-011│ │ │SP-012│ │           │           │ │SP010│ │
│ └───────┘ │ └───────┘ │ └───────┘ │           │           │ └─────┘ │
└───────────┴───────────┴───────────┴───────────┴───────────┴─────────┘
```

- 6 kolom = fase otomatis. **Tidak bisa drag**; aksi lewat tombol di kartu.
- Filter **Needs me**, pencarian.
- Kartu:

```
┌────────────────────────────┐
│ SP-012  Fix double invoice │
│ 🐛 bug · sale_custom        │
│ Making tests pass ▪▪▪▫▫     │
│ ⟳ AI working        $0.84  │
└────────────────────────────┘
```
  Kartu needs-you: border amber + label aksi ("Answer 3 questions", "Approve PRD", "Start testing", "Review commit"). Hover: ringkasan PRD 2 kalimat. DB antre: 🔒 "Waiting for DB".

### New request (form)
Judul · deskripsi (markdown) · tipe (bug / improvement / feature) · prioritas · pelapor · lampiran screenshot.
Setelah submit, step 2–5 jalan otomatis. Kalau mirip PRD lama → banner "Similar to SP-007?".

### PRD detail (fullscreen)

```
← Board   SP-012 Fix double invoice                    ● Needs you
①Request ✓ ─ ②Planning ✓ ─ ③Building ● ─ ④Testing ─ ⑤Ship
[ Now ] [ PRD ] [ Context ] [ Activity ] [ Changes ] [ Brain ] [ Cost ]
```

- Stepper 5 fase (sub-step bisa di-expand).
- **Now**: konten step aktif (lihat di bawah).
- **PRD**: HTML rendered + Copy HTML / Copy Markdown / Open in Obsidian / Edit / **Approve PRD**.
- **Context**: hasil step 2–4 (note brain terkait, schema DB relevan, file kode relevan).
- **Activity**: timeline AI.
- **Changes**: diff viewer + daftar modul/migration.
- **Brain**: note PRD ini di vault.
- **Cost**: token/biaya per step.

### Grill (step 6)

```
Q4 · Data model                                   ●●●◉○○
┌──────────────────────────────────────────────────────┐
│ Block re-invoice on SO, or on invoice?               │
│ ★ On sale.order (recommended)                        │
│   why: single source of truth                        │
│ ○ On account.move                                    │
│ ○ Other: ____________                                │
└──────────────────────────────────────────────────────┘
[ ← Back ]                              [ Enough, write PRD ]
```
Kiri: ringkasan jawaban sebelumnya (bisa diubah).

### AI activity (step 9, dst.)
Timeline kalimat sederhana dengan ikon ("Reading sale.py", "Wrote 2 tests", "Tests passing 12/12").
Toggle **Show technical details** → raw tool call, output terminal, diff.
Tombol **Stop** · **Give a hint**. Lewat batas loop → panel eskalasi: test gagal + ringkasan percobaan → Continue / Give hint / Edit PRD.

### Human test (step 14–15)

```
┌ Test cases (4/6) ─────────────┐   [ Open Odoo ↗ ]
│ ✓ Normal: invoice once         │   localhost:8069 · db acme_dev
│ ✗ Edge: partial delivery       │
│   note: still creates 2 inv    │
│ ○ Edge: cancel then reinvoice  │
└────────────────────────────────┘
[ Send feedback to AI ]                    [ All good → Ship ]
```
Tiap case: langkah, data, expected, Pass/Fail, catatan, screenshot. Odoo dibuka di browser default.

### Ship (step 16–19), satu halaman
1. Ringkasan perubahan (file, modul, migration, test) + diff.
2. Editor commit message (draft AI, format Odoo).
3. Tombol berurutan: **Commit → Push → Create PR to preprod** (judul/body PR draft AI, editable, opsional).

### Notifikasi desktop
Needs you · AI gagal / batas loop · PR merged · DB lock bebas.

---

## 7. Kriteria "fase 1 selesai"

- Satu PRD Odoo bisa dibawa dari request sampai PR tanpa keluar dari FloWW.
- Feedback loop (step 15 → 9) berjalan berulang sampai user puas.
- Dua PRD bisa di-develop paralel; step DB antre dengan benar.
- Brain ter-update (requirement, PRD, changes, graph) dan tersinkron ke `floww/brain` untuk anggota lain.
- Setup workspace baru di mesin bersih bisa selesai lewat setup agent.
- Mode subscription dan 9router sama-sama jalan.

---

## 8. Keputusan tambahan

1. **Tanpa guard** di mode bypass. AI bebas penuh di worktree.
2. **Skill** `grill-me`, `tdd`, `sdd` memakai skill yang sudah ada di Claude Code (`~/.claude/skills` / project). Tidak dibundel.
3. **ID PRD** = inisial user + nomor urut per user, mis. `SP-012`. Counter disimpan di `brain/.counters/<user>.json`, jadi antar anggota tidak pernah bentrok. Inisial diambil dari `git config user.name`, bisa diubah di settings.

## 9. Ditunda (bukan fase 1)

- Introspeksi DB remote via SSH
- Rekam browser → test native (`HttpCase` + tour)
- Deploy preprod, provisioning server (docker compose via SSH)
- Flow release: cherry-pick preprod → release branch + guard generator dari blueprint
- Preset framework lain, DB per PRD, GitLab/Gitea, macOS/Windows
