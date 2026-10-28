# Context engineering: riset dan dampaknya ke FloWW

Ringkasan paper yang dipakai sebagai dasar desain brain dan cara FloWW memberi konteks ke Claude Code.
Angka hasil adalah klaim penulis paper masing-masing.

## Konteks itu anggaran terbatas

| Paper | Temuan | Dipakai di FloWW |
|---|---|---|
| [Context Length Alone Hurts LLM Performance Despite Perfect Retrieval](https://arxiv.org/abs/2510.05381) (2510.05381) | Akurasi turun 13,9–85% saat input memanjang walau semua bukti berhasil ditemukan. Mengutip bukti dulu sebelum menjawab membantu. | Batas token per step; langkah "kutip dulu" sebelum coding |
| [Lost in the Middle](https://arxiv.org/abs/2307.03172) (2307.03172, TACL) | Informasi di tengah konteks paling sering terlewat (kurva U). | Rules confirmed di awal paket, tugas di akhir |
| [A Survey of Context Engineering for LLMs](https://arxiv.org/abs/2507.13334) (2507.13334) | Taksonomi: retrieval, processing, management; RAG, memory, tool, multi-agent. | Bacaan dasar |

## File instruksi repo

| Paper | Temuan | Dipakai di FloWW |
|---|---|---|
| [Evaluating AGENTS.md](https://arxiv.org/abs/2602.11988) (2602.11988) | File konteks buatan LLM umumnya tidak menaikkan keberhasilan dan menambah biaya >20%. File tulisan developer lebih baik (~7%). Ringkasan repo tidak membantu. | CLAUDE.md worktree minimal (≤ ~60 baris): perintah + konvensi tidak standar + rules confirmed modul terkait |
| [On the Impact of AGENTS.md Files on the Efficiency of AI Coding Agents](https://arxiv.org/abs/2601.20404) (2601.20404) | Mengukur runtime dan token dengan/tanpa AGENTS.md. | Ukur sendiri per PRD |

## Mencari kode yang tepat

| Paper | Temuan | Dipakai di FloWW |
|---|---|---|
| [Agentless](https://arxiv.org/abs/2407.01489) (2407.01489, FSE 2025) | Pipeline sederhana cari lokasi → perbaiki → validasi menyaingi agent kompleks dengan biaya rendah. | Langkah "cari lokasi" eksplisit sebelum develop |
| [LocAgent](https://arxiv.org/abs/2503.09089) (2503.09089) | Graph heterogen (file, class, fungsi; import, panggilan, inheritance) untuk lokalisasi multi-hop. | Graph FloWW dipakai untuk lokalisasi |
| [RepoGraph](https://arxiv.org/abs/2410.14684) (2410.14684, ICLR 2025) | Graph level baris sebagai plug-in untuk agent maupun pipeline. | Inspirasi struktur graph kode |
| [HippoRAG](https://arxiv.org/abs/2405.14831) (2405.14831, NeurIPS 2024) | Personalized PageRank di knowledge graph = retrieval multi-langkah sekali jalan, jauh lebih murah dari retrieval iteratif. | Paket konteks: PPR dari konsep di request |
| [SWE-agent](https://arxiv.org/abs/2405.15793) (2405.15793) | Desain antarmuka agent (output ringkas, lint saat edit menolak edit rusak) menaikkan keberhasilan. | Hook cek syntax/XML/manifest setelah edit |
| [Retrieval-Augmented Code Generation survey](https://arxiv.org/pdf/2510.04905) (2510.04905) | Pergeseran dari embedding statis ke agentic search (grep/read dalam loop). | Kode dicari Claude Code sendiri; brain memberi peta + rules |

Catatan: belum ada paper terkontrol yang membandingkan grep vs embedding untuk coding agent. Bukti praktisi condong ke hybrid.

## Memori dan riwayat panjang

| Paper | Temuan | Dipakai di FloWW |
|---|---|---|
| [ACE: Agentic Context Engineering](https://arxiv.org/abs/2510.04618) (2510.04618, ICLR 2026) | Playbook yang tumbuh per butir (generate → reflect → curate) mengalahkan ringkasan; menulis ulang berulang menyebabkan "context collapse". | Update brain per baris + skor berguna/merugikan |
| [The Complexity Trap](https://arxiv.org/abs/2508.21433) (2508.21433, DL4Code @ NeurIPS 2025) | Menyembunyikan output tool lama sama efektifnya dengan ringkasan LLM, biaya ~setengah. | Sesi baru per kriteria + progress.md, bukan sesi panjang |
| [MemGPT](https://arxiv.org/abs/2310.08560) (2310.08560) | Konteks diperlakukan seperti memori OS: data dipindah masuk/keluar konteks. | Vault = memori luar; paket konteks = memori kerja |
| [Zep / Graphiti](https://arxiv.org/abs/2501.13956) (2501.13956) | Knowledge graph bi-temporal (kapan berlaku, kapan dicatat). | `valid_from` / `supersedes` pada rules dan keputusan |
| [A-MEM](https://arxiv.org/abs/2502.12110) (2502.12110) | Catatan gaya Zettelkasten yang otomatis ditautkan dan berevolusi. | Usulan link otomatis, dikonfirmasi manusia |

## Halusinasi dan keamanan memori

| Paper | Temuan | Dipakai di FloWW |
|---|---|---|
| [LLM Hallucinations in Practical Code Generation](https://arxiv.org/abs/2409.20550) (2409.20550, ISSTA 2025) | Di level repo, halusinasi sering berupa konflik dengan konteks project; RAG fakta project membantu. | Fakta API (field, signature method) model yang disentuh selalu ada di paket develop |
| [Survey keamanan memori jangka panjang agent](https://arxiv.org/html/2604.16548v1) (2604.16548) | Memori bersama rawan diracuni; perlu tata kelola tulis/baca/lupa. | Jejak asal per catatan, validator tulis, revert |
