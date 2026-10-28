//! One PRD from request to done, with real git (repo, remote, worktree, brain)
//! and fake `claude` / `odoo-bin` scripts so it runs without an AI account or Odoo.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use floww_core::pipeline::Step;
use floww_engine::Engine;
use floww_engine::engine::{AddWorkspace, NewRequest, SetAi};
use floww_engine::model::{PrdType, Status, TestResult};

const FAKE_CLAUDE: &str = r#"#!/usr/bin/env python3
import json, os, subprocess, sys
args = sys.argv[1:]
prompt = args[args.index("-p") + 1]
def emit(o): print(json.dumps(o), flush=True)
emit({"type": "system", "subtype": "init", "session_id": "fake-session", "model": "fake"})
emit({"type": "assistant", "message": {"content": [{"type": "tool_use", "name": "Read", "input": {"file_path": "addons/sale_custom/models/sale.py"}}]}})
if "Find the code this request is about" in prompt:
    out = 'Today invoices count cancelled moves.\n```json\n{"summary": "Qty to invoice counts cancelled moves.", "modules": ["sale_custom"], "files": ["addons/sale_custom/models/sale.py"], "similar_prd": null}\n```'
elif "Interview the person" in prompt:
    out = '```json\n{"done": false, "question": {"topic": "Scope", "text": "Count cancelled moves?", "explain": null, "options": [{"label": "No", "why": "They were not delivered", "recommended": true}, {"label": "Yes", "why": "Simpler", "recommended": false}]}}\n```'
elif prompt.startswith("The person chose") or prompt.startswith("The person wrote"):
    out = '```json\n{"done": true}\n```'
elif "Write the PRD now" in prompt:
    out = '# Invoice dobel\n\n## Masalah\nCancelled moves are invoiced.\n\n```json\n{"criteria": [{"given": "a cancelled delivery", "when": "invoicing", "then": "cancelled qty is ignored"}], "tests": [{"kind": "Normal", "title": "Invoice once", "steps": "Deliver and invoice", "data": "SO1", "expected": "One invoice"}], "rules": [{"id": "no-double-invoice", "title": "Invoiced qty is never invoiced again", "why": "Double billing", "enforced_in": "sale.order.line._compute_qty_to_invoice"}], "modules": ["sale_custom"]}\n```'
elif "acceptance criterion" in prompt:
    path = os.path.join("addons", "sale_custom", "models", "sale.py")
    with open(path, "a") as f:
        f.write("\n# skip cancelled moves\n")
    subprocess.run(["git", "add", "-A"], check=True)
    subprocess.run(["git", "commit", "-q", "-m", "wip: criterion 1"], check=True)
    out = "Tests pass."
elif "Write the commit message" in prompt:
    out = '```json\n{"message": "[FIX] sale_custom: ignore cancelled moves\\n\\nPRD: TT-001", "pr_title": "[FIX] sale_custom: ignore cancelled moves", "pr_body": "PRD: TT-001"}\n```'
else:
    out = "OK"
emit({"type": "result", "subtype": "success", "is_error": False, "session_id": "fake-session", "result": out, "total_cost_usd": 0.01, "usage": {"input_tokens": 100, "output_tokens": 20}})
"#;

const FAKE_ODOO: &str = r#"#!/usr/bin/env python3
import sys, time
if "--stop-after-init" in sys.argv:
    print("odoo: done")
    sys.exit(0)
time.sleep(300)
"#;

fn sh(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?} failed");
}

fn script(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

async fn wait_for(
    engine: &Engine,
    ws: &str,
    id: &str,
    what: &str,
    f: impl Fn(&floww_engine::model::Prd) -> bool,
) -> floww_engine::model::Prd {
    for _ in 0..300 {
        let p = engine.get_prd(ws, id).await.unwrap();
        if f(&p) {
            return p;
        }
        if p.status == Status::Failed {
            panic!(
                "PRD failed while waiting for {what}: {} | {:#?}",
                p.sub, p.timeline
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let p = engine.get_prd(ws, id).await.unwrap();
    panic!(
        "timed out waiting for {what}; step {:?} status {:?} sub {}",
        p.step, p.status, p.sub
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn prd_from_request_to_done() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let repo = tmp.path().join("acme");
    let remote = tmp.path().join("remote.git");
    let bin = tmp.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    script(&bin.join("claude"), FAKE_CLAUDE);
    script(&bin.join("odoo-bin"), FAKE_ODOO);
    // SAFETY: set before the engine starts any task; this test owns the process env.
    unsafe {
        std::env::set_var("FLOWW_CLAUDE_BIN", bin.join("claude"));
        std::env::set_var("GIT_AUTHOR_NAME", "Test User");
        std::env::set_var("GIT_AUTHOR_EMAIL", "t@example.com");
        std::env::set_var("GIT_COMMITTER_NAME", "Test User");
        std::env::set_var("GIT_COMMITTER_EMAIL", "t@example.com");
    }

    // A small Odoo repo with a remote and a preprod branch.
    sh(
        tmp.path(),
        &[
            "init",
            "-q",
            "--bare",
            "-b",
            "main",
            remote.to_str().unwrap(),
        ],
    );
    std::fs::create_dir_all(repo.join("addons/sale_custom/models")).unwrap();
    std::fs::create_dir_all(repo.join(".vscode")).unwrap();
    std::fs::write(
        repo.join("addons/sale_custom/__manifest__.py"),
        "{'name': 'Sale custom', 'version': '17.0.1.0.0', 'depends': ['sale']}",
    )
    .unwrap();
    std::fs::write(repo.join("addons/sale_custom/models/sale.py"), "from odoo import fields, models\n\nclass SaleOrderLine(models.Model):\n    _inherit = 'sale.order.line'\n\n    def _compute_qty_to_invoice(self):\n        pass\n").unwrap();
    std::fs::write(
        repo.join(".vscode/launch.json"),
        format!(r#"{{ "configurations": [ {{ "name": "Odoo test", "program": "{}", "args": ["-c", "odoo.conf"] }} ] }}"#, bin.join("odoo-bin").display()),
    )
    .unwrap();
    sh(&repo, &["init", "-q", "-b", "main"]);
    sh(&repo, &["add", "-A"]);
    sh(&repo, &["commit", "-q", "-m", "init"]);
    sh(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    sh(&repo, &["push", "-q", "-u", "origin", "main"]);

    let engine = Engine::open(Some(home.clone())).await.unwrap();
    engine
        .set_ai(SetAi {
            mode: "subscription".into(),
            url: None,
            token: None,
            model: Some("fake".into()),
        })
        .await
        .unwrap();

    // Setup detects the project, then the workspace is created (preprod made from main).
    let det = engine.detect(repo.to_str().unwrap()).await.unwrap();
    assert_eq!(det.preset, "odoo");
    assert_eq!(det.launches.len(), 1);
    assert!(!det.has_preprod);
    let ws = engine
        .add_workspace(AddWorkspace {
            path: repo.to_string_lossy().into(),
            name: "acme".into(),
            initials: "TT".into(),
            db: String::new(),
            base_branch: "preprod".into(),
            launch_config: Some("Odoo test".into()),
            http_port: 8199,
            language: Some("en".into()),
        })
        .await
        .unwrap();
    assert!(repo.join(".floww/brain/_floww/team.toml").exists());
    assert!(repo.join(".floww/brain/modules/sale_custom.md").exists());

    // Request → context → first question.
    let id = engine
        .create_request(
            &ws,
            NewRequest {
                title: "Invoice dobel saat pengiriman parsial".into(),
                description: "Two invoices for one delivery".into(),
                kind: PrdType::Bug,
                reporter: "Dewi".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(id, "TT-001");
    let p = wait_for(&engine, &ws, &id, "first question", |p| {
        p.status == Status::Needs && p.step == Step::GrillPrd
    })
    .await;
    assert_eq!(p.modules, vec!["sale_custom"]);
    assert!(p.grill.as_ref().unwrap().current.is_some());
    assert!(p.worktree.as_ref().unwrap().join(".git").exists());

    engine.answer(&ws, &id, "No".into(), false).await.unwrap();
    wait_for(&engine, &ws, &id, "questions done", |p| {
        p.status == Status::Needs && p.action.as_deref() == Some("Write the PRD")
    })
    .await;
    engine.finish_grill(&ws, &id).await.unwrap();
    let p = wait_for(&engine, &ws, &id, "PRD ready", |p| {
        p.step == Step::ApprovePrd && p.status == Status::Needs
    })
    .await;
    assert_eq!(p.criteria.len(), 1);
    assert!(
        repo.join(".floww/brain/rules/no-double-invoice.md")
            .exists()
    );
    assert!(repo.join(".floww/brain/decisions/TT-001.md").exists());

    // Approve → branch → develop (fake AI edits + commits) → tests (fake odoo) → ready to test.
    engine.approve(&ws, &id).await.unwrap();
    let p = wait_for(&engine, &ws, &id, "ready to test", |p| {
        p.step == Step::StartOdoo && p.status == Status::Needs
    })
    .await;
    assert_eq!(
        p.branch.as_deref(),
        Some("feature/TT-001-invoice-dobel-saat")
    );
    assert!(p.criteria.iter().all(|c| c.done));
    let wt = p.worktree.clone().unwrap();
    assert!(wt.join("CLAUDE.local.md").exists());
    assert!(repo.join(".floww/brain/changes/TT-001.md").exists());

    // Human test in Odoo, with the shared database lock.
    engine.start_odoo(&ws, &id).await.unwrap();
    wait_for(&engine, &ws, &id, "odoo running", |p| {
        p.step == Step::HumanTest && p.status == Status::Needs
    })
    .await;
    assert_eq!(
        engine.lock_state(&ws).await.unwrap().holder.as_deref(),
        Some("TT-001")
    );
    engine
        .mark_test(&ws, &id, 0, Some(TestResult::Pass), None)
        .await
        .unwrap();
    engine.all_good(&ws, &id).await.unwrap();
    let p = wait_for(&engine, &ws, &id, "commit message", |p| {
        p.step == Step::CommitMessage && p.status == Status::Needs
    })
    .await;
    assert!(engine.lock_state(&ws).await.unwrap().holder.is_none());
    assert!(
        p.ship
            .as_ref()
            .unwrap()
            .message
            .starts_with("[FIX] sale_custom")
    );

    // Ship: one squashed commit, push, no PR.
    engine
        .commit(
            &ws,
            &id,
            "[FIX] sale_custom: ignore cancelled moves\n\nPRD: TT-001".into(),
        )
        .await
        .unwrap();
    let log = Command::new("git")
        .args([
            "log",
            "--format=%s",
            "preprod..feature/TT-001-invoice-dobel-saat",
        ])
        .current_dir(&repo)
        .output()
        .unwrap();
    let subjects = String::from_utf8_lossy(&log.stdout);
    assert_eq!(
        subjects.lines().collect::<Vec<_>>(),
        vec!["[FIX] sale_custom: ignore cancelled moves"]
    );
    engine.push(&ws, &id).await.unwrap();
    let remote_branches = Command::new("git")
        .args(["branch", "--list", "feature/*"])
        .current_dir(&remote)
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&remote_branches.stdout).contains("feature/TT-001"));
    engine.skip_pr(&ws, &id).await.unwrap();
    let p = engine.get_prd(&ws, &id).await.unwrap();
    assert_eq!(p.status, Status::Done);
    assert!(!wt.exists(), "worktree should be removed");
    assert!(p.cost_usd > 0.0);

    // The brain branch was pushed to the remote for teammates.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let heads = Command::new("git")
        .args(["branch", "--list", "floww/brain"])
        .current_dir(&remote)
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&heads.stdout).contains("floww/brain"));

    // Reopening the engine reads the same state from disk.
    drop(engine);
    let again = Engine::open(Some(home)).await.unwrap();
    let p = again.get_prd(&ws, &id).await.unwrap();
    assert_eq!(p.status, Status::Done);
}
