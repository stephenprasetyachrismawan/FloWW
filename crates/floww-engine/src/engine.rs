//! The engine: workspaces, PRD runs, and the driver that moves each PRD
//! through the pipeline. AI and app steps run on their own; the driver stops
//! at every step that needs a person and waits for an action.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use floww_core::claude::{Event as AiEvent, Request};
use floww_core::config::{
    self, AiAuth, AppConfig, EffectiveConfig, ModelStep, TeamConfig, WorkspaceConfig,
    WorkspaceEntry,
};
use floww_core::odoo::{self, Mode, OdooLaunch};
use floww_core::pipeline::Step;
use floww_core::prd_id;
use serde::{Deserialize, Serialize};
use tokio::process::Child;
use tokio::sync::{Mutex, RwLock, broadcast, mpsc, watch};

use crate::events::{Event, NotifyKind};
use crate::lock::{DbLock, Guard, LockState};
use crate::model::*;
use crate::paths::{self, WorkspacePaths};
use crate::runner::{self, AiRun};
use crate::{
    brain, db, git, github, jsonx, odoo_facts, proc, prompts, secrets, setup, skills, store,
    worktree_files,
};

type R<T> = Result<T, String>;

pub struct Workspace {
    pub id: String,
    pub paths: WorkspacePaths,
    cfg: RwLock<EffectiveConfig>,
    prds: Mutex<BTreeMap<String, Prd>>,
    pub lock: DbLock,
    cancels: Mutex<HashMap<String, watch::Sender<bool>>>,
    running: Mutex<HashSet<String>>,
    odoo: Mutex<HashMap<String, (Child, Guard)>>,
    brain_mu: Mutex<()>,
}

struct Inner {
    home: PathBuf,
    app: RwLock<AppConfig>,
    workspaces: RwLock<BTreeMap<String, Arc<Workspace>>>,
    events: broadcast::Sender<Event>,
}

#[derive(Clone)]
pub struct Engine(Arc<Inner>);

// ---------- views returned to the UI ----------

#[derive(Debug, Clone, Serialize)]
pub struct AppView {
    pub ai_ready: bool,
    pub ai: Option<AiAuth>,
    pub default_model: Option<String>,
    pub workspaces: Vec<WorkspaceSummary>,
    pub version: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceSummary {
    pub id: String,
    pub name: String,
    pub path: String,
    pub needs_you: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceView {
    pub id: String,
    pub name: String,
    pub path: String,
    pub config: EffectiveConfig,
    pub lock: LockState,
    pub launches: Vec<OdooLaunch>,
    pub brain_ready: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub kind: PrdType,
    #[serde(default)]
    pub reporter: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AddWorkspace {
    pub path: String,
    pub name: String,
    pub initials: String,
    pub db: String,
    pub base_branch: String,
    pub launch_config: Option<String>,
    pub http_port: u16,
    #[serde(default)]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SetAi {
    pub mode: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub token: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Settings {
    // Team (shared through the brain)
    pub base_branch: Option<String>,
    pub language: Option<String>,
    pub max_iterations: Option<u32>,
    pub full_suite: Option<bool>,
    // This computer
    pub db: Option<String>,
    pub launch_config: Option<String>,
    pub http_port: Option<u16>,
    pub model: Option<String>,
    pub develop_model: Option<String>,
    pub github_token: Option<String>,
}

fn new_id() -> String {
    use rand::Rng;
    let n: u32 = rand::rng().random();
    format!("w{n:08x}")
}

fn shell_join(program: &str, args: &[String]) -> String {
    let q = |s: &str| {
        if s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=,:@${}".contains(c))
        {
            s.to_string()
        } else {
            format!("'{}'", s.replace('\'', "'\\''"))
        }
    };
    std::iter::once(q(program))
        .chain(args.iter().map(|a| q(a)))
        .collect::<Vec<_>>()
        .join(" ")
}

impl Engine {
    /// Open the engine, load registered workspaces and their PRDs.
    pub async fn open(home: Option<PathBuf>) -> R<Self> {
        let home = home.unwrap_or_else(paths::app_home);
        std::fs::create_dir_all(&home)
            .map_err(|e| format!("could not create {}: {e}", home.display()))?;
        let app: AppConfig =
            config::read_toml_or_default(&paths::app_config(&home)).map_err(|e| e.to_string())?;
        let (events, _) = broadcast::channel(512);
        let engine = Engine(Arc::new(Inner {
            home,
            app: RwLock::new(app.clone()),
            workspaces: RwLock::new(BTreeMap::new()),
            events,
        }));
        for entry in &app.workspaces {
            if let Err(e) = engine.load_workspace(entry).await {
                tracing::warn!("could not open workspace {}: {e}", entry.path.display());
            }
        }
        Ok(engine)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.0.events.subscribe()
    }

    fn emit(&self, e: Event) {
        let _ = self.0.events.send(e);
    }

    fn notify(&self, title: impl Into<String>, body: impl Into<String>, kind: NotifyKind) {
        self.emit(Event::Notify {
            title: title.into(),
            body: body.into(),
            kind,
        });
    }

    async fn save_app(&self) -> R<()> {
        let app = self.0.app.read().await.clone();
        config::write_toml(&paths::app_config(&self.0.home), &app).map_err(|e| e.to_string())
    }

    async fn load_workspace(&self, entry: &WorkspaceEntry) -> R<Arc<Workspace>> {
        let paths = WorkspacePaths::new(&entry.path);
        let cfg = config::load(&paths.repo).map_err(|e| e.to_string())?;
        let mut prds = BTreeMap::new();
        for mut p in store::load_all(&paths.runs()) {
            // Work that was running when the app closed cannot continue by itself.
            if p.status == Status::Ai || p.status == Status::Wait {
                p.status = Status::Failed;
                p.action = Some("Continue".into());
                p.sub = "Stopped when FloWW closed".into();
            }
            if p.odoo != OdooState::Off {
                p.odoo = OdooState::Off;
            }
            prds.insert(p.id.clone(), p);
        }
        let events = self.0.events.clone();
        let wid = entry.id.clone();
        let ws = Arc::new(Workspace {
            id: entry.id.clone(),
            paths,
            cfg: RwLock::new(cfg),
            prds: Mutex::new(prds),
            lock: DbLock::new(move || {
                let _ = events.send(Event::LockChanged {
                    workspace: wid.clone(),
                });
            }),
            cancels: Mutex::new(HashMap::new()),
            running: Mutex::new(HashSet::new()),
            odoo: Mutex::new(HashMap::new()),
            brain_mu: Mutex::new(()),
        });
        self.0
            .workspaces
            .write()
            .await
            .insert(entry.id.clone(), ws.clone());
        Ok(ws)
    }

    pub async fn ws(&self, id: &str) -> R<Arc<Workspace>> {
        self.0
            .workspaces
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| format!("no workspace with id {id}"))
    }

    // ---------- app & AI ----------

    pub async fn app_view(&self) -> AppView {
        let app = self.0.app.read().await.clone();
        let mut workspaces = Vec::new();
        for e in &app.workspaces {
            let needs_you = match self.ws(&e.id).await {
                Ok(ws) => ws
                    .prds
                    .lock()
                    .await
                    .values()
                    .filter(|p| matches!(p.status, Status::Needs | Status::Failed))
                    .count(),
                Err(_) => 0,
            };
            workspaces.push(WorkspaceSummary {
                id: e.id.clone(),
                name: e.name.clone(),
                path: e.path.to_string_lossy().into_owned(),
                needs_you,
            });
        }
        AppView {
            ai_ready: app.ai.is_some(),
            ai: app.ai.clone(),
            default_model: app.default_model.clone(),
            workspaces,
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    pub async fn set_ai(&self, req: SetAi) -> R<()> {
        let auth = match req.mode.as_str() {
            "subscription" => AiAuth::Subscription,
            "router" => {
                let url = req
                    .url
                    .clone()
                    .filter(|u| !u.trim().is_empty())
                    .ok_or("enter the 9router address")?;
                let token_ref = match req.token.as_deref().filter(|t| !t.is_empty()) {
                    Some(t) => secrets::store("9router", t).await?,
                    None => "keyring:9router".into(),
                };
                AiAuth::Router { url, token_ref }
            }
            other => return Err(format!("unknown AI mode \"{other}\"")),
        };
        {
            let mut app = self.0.app.write().await;
            app.ai = Some(auth);
            if req.model.is_some() {
                app.default_model = req.model.clone();
            }
        }
        self.save_app().await
    }

    /// Send a tiny prompt to check that Claude Code answers with the chosen auth.
    pub async fn test_ai(&self, model: Option<String>) -> R<String> {
        let app = self.0.app.read().await.clone();
        let auth = app.ai.clone().unwrap_or(AiAuth::Subscription);
        let token = self.router_token(&auth).await?;
        let req = Request {
            prompt: "Reply with exactly: OK".into(),
            cwd: std::env::temp_dir(),
            model: model.or(app.default_model),
            ..Default::default()
        };
        let (_tx, rx) = watch::channel(false);
        let started = std::time::Instant::now();
        let run = runner::run(
            &req,
            &auth,
            token.as_deref(),
            Duration::from_secs(120),
            rx,
            |_| {},
        )
        .await?;
        Ok(format!(
            "Answered in {:.1} s: {}",
            started.elapsed().as_secs_f32(),
            run.text.trim()
        ))
    }

    pub async fn models(&self) -> Vec<String> {
        let app = self.0.app.read().await.clone();
        let mut list = vec![
            "claude-opus-5-5".to_string(),
            "claude-sonnet-5-5".into(),
            "claude-haiku-5-5".into(),
            "claude-fable-5-1".into(),
        ];
        if let Some(AiAuth::Router { url, token_ref }) = &app.ai
            && let Ok(token) = secrets::resolve(token_ref).await
        {
            let base = floww_core::claude::normalize_router_url(url);
            if let Ok(res) = reqwest::Client::new()
                .get(format!("{base}/v1/models"))
                .bearer_auth(token)
                .timeout(Duration::from_secs(5))
                .send()
                .await
                && let Ok(v) = res.json::<serde_json::Value>().await
                && let Some(arr) = v["data"].as_array()
            {
                let ids: Vec<String> = arr
                    .iter()
                    .filter_map(|m| m["id"].as_str().map(str::to_string))
                    .collect();
                if !ids.is_empty() {
                    list = ids;
                }
            }
        }
        list
    }

    async fn router_token(&self, auth: &AiAuth) -> R<Option<String>> {
        match auth {
            AiAuth::Router { token_ref, .. } => Ok(Some(secrets::resolve(token_ref).await?)),
            AiAuth::Subscription => Ok(None),
        }
    }

    async fn auth_for(&self, ws: &Workspace) -> AiAuth {
        let cfg = ws.cfg.read().await;
        if let Some(a) = &cfg.workspace.ai {
            return a.clone();
        }
        self.0
            .app
            .read()
            .await
            .ai
            .clone()
            .unwrap_or(AiAuth::Subscription)
    }

    async fn model_for(&self, ws: &Workspace, step: ModelStep) -> Option<String> {
        let cfg = ws.cfg.read().await;
        cfg.model_for(step)
            .map(str::to_string)
            .or(self.0.app.read().await.default_model.clone())
    }

    // ---------- workspaces ----------

    pub async fn detect(&self, path: &str) -> R<setup::Detection> {
        let p = PathBuf::from(path.trim());
        if !p.is_dir() {
            return Err(format!("{} is not a folder", p.display()));
        }
        let mut used = Vec::new();
        for ws in self.0.workspaces.read().await.values() {
            used.push(ws.cfg.read().await.workspace.http_port);
        }
        Ok(setup::detect(&p, &used).await)
    }

    pub async fn add_workspace(&self, req: AddWorkspace) -> R<String> {
        let repo = PathBuf::from(req.path.trim());
        if !git::is_repo(&repo).await {
            return Err(format!("{} is not a git repository", repo.display()));
        }
        if self
            .0
            .app
            .read()
            .await
            .workspaces
            .iter()
            .any(|w| w.path == repo)
        {
            return Err("this folder is already a workspace".into());
        }
        let paths = WorkspacePaths::new(&repo);
        std::fs::create_dir_all(paths.floww()).map_err(|e| e.to_string())?;
        git::exclude(&repo, &[".floww/"]).await?;

        // Base branch: create it from the default branch when missing.
        let base = req.base_branch.trim().to_string();
        if !git::local_branch_exists(&repo, &base).await
            && !git::remote_branch_exists(&repo, &base).await
        {
            let from = git::default_branch(&repo)
                .await
                .ok_or("could not find the default branch to create the base branch from")?;
            git::git(&repo, &["branch", &base, &from]).await?;
            if git::has_remote(&repo).await {
                let _ = git::git(&repo, &["push", "-u", "origin", &base]).await;
            }
        }

        brain::init(&repo, &paths.brain()).await?;
        let team_path = repo.join(config::TEAM_CONFIG_PATH);
        if !team_path.exists() {
            let team = TeamConfig {
                base_branch: base.clone(),
                language: req.language.clone().unwrap_or_else(|| "id".into()),
                ..TeamConfig::default()
            };
            config::write_toml(&team_path, &team).map_err(|e| e.to_string())?;
        }
        let local = WorkspaceConfig {
            name: req.name.clone(),
            initials: req.initials.trim().to_uppercase(),
            db: req.db.clone(),
            http_port: req.http_port,
            launch_config: req.launch_config.clone(),
            ..WorkspaceConfig::default()
        };
        config::write_toml(&repo.join(config::WORKSPACE_CONFIG_PATH), &local)
            .map_err(|e| e.to_string())?;

        // Code facts into the brain.
        for m in odoo_facts::read_all(&repo) {
            brain::write(
                &paths.brain(),
                &format!("modules/{}.md", m.name),
                &brain::module_note(&m),
            )?;
        }
        brain::commit(&paths.brain(), "brain: workspace setup").await?;

        let entry = WorkspaceEntry {
            id: new_id(),
            name: req.name,
            path: repo,
        };
        self.0.app.write().await.workspaces.push(entry.clone());
        self.save_app().await?;
        self.load_workspace(&entry).await?;
        let ws = self.ws(&entry.id).await?;
        self.spawn_brain_sync(ws);
        self.emit(Event::WorkspacesChanged);
        Ok(entry.id)
    }

    pub async fn remove_workspace(&self, id: &str) -> R<()> {
        self.0.app.write().await.workspaces.retain(|w| w.id != id);
        self.0.workspaces.write().await.remove(id);
        self.save_app().await?;
        self.emit(Event::WorkspacesChanged);
        Ok(())
    }

    pub async fn workspace_view(&self, id: &str) -> R<WorkspaceView> {
        let ws = self.ws(id).await?;
        let config = ws.cfg.read().await.clone();
        let launch_path = ws.paths.repo.join(".vscode/launch.json");
        let launches = if launch_path.exists() {
            odoo::read_launch_configs(&launch_path).unwrap_or_default()
        } else {
            Vec::new()
        };
        Ok(WorkspaceView {
            id: ws.id.clone(),
            name: config.workspace.name.clone(),
            path: ws.paths.repo.to_string_lossy().into_owned(),
            lock: ws.lock.state().await,
            brain_ready: brain::is_ready(&ws.paths.brain()).await,
            config,
            launches,
        })
    }

    async fn reload_config(&self, ws: &Workspace) -> R<()> {
        let cfg = config::load(&ws.paths.repo).map_err(|e| e.to_string())?;
        *ws.cfg.write().await = cfg;
        Ok(())
    }

    pub async fn save_settings(&self, id: &str, s: Settings) -> R<()> {
        let ws = self.ws(id).await?;
        let repo = ws.paths.repo.clone();
        let team_path = repo.join(config::TEAM_CONFIG_PATH);
        let local_path = repo.join(config::WORKSPACE_CONFIG_PATH);
        let mut team: TeamConfig =
            config::read_toml_or_default(&team_path).map_err(|e| e.to_string())?;
        let mut local: WorkspaceConfig =
            config::read_toml_or_default(&local_path).map_err(|e| e.to_string())?;
        let mut team_changed = Vec::new();
        if let Some(v) = s.base_branch.filter(|v| *v != team.base_branch) {
            team.base_branch = v;
            team_changed.push("base branch");
        }
        if let Some(v) = s.language.filter(|v| *v != team.language) {
            team.language = v;
            team_changed.push("AI language");
        }
        if let Some(v) = s
            .max_iterations
            .filter(|v| *v != team.steps.develop.max_iterations)
        {
            team.steps.develop.max_iterations = v;
            team_changed.push("AI tries");
        }
        if let Some(v) = s.full_suite.filter(|v| *v != team.steps.full_suite) {
            team.steps.full_suite = v;
            team_changed.push("full test suite");
        }
        if let Some(v) = s.db {
            local.db = v;
        }
        if let Some(v) = s.launch_config {
            local.launch_config = Some(v);
        }
        if let Some(v) = s.http_port {
            local.http_port = v;
        }
        if let Some(v) = s.model {
            local.models.default = Some(v).filter(|m| !m.is_empty());
        }
        if let Some(v) = s.develop_model {
            local.models.develop = Some(v).filter(|m| !m.is_empty() && m != "default");
        }
        if let Some(t) = s.github_token.filter(|t| !t.is_empty()) {
            local.github_token_ref = Some(secrets::store("github", &t).await?);
        }
        config::write_toml(&local_path, &local).map_err(|e| e.to_string())?;
        if !team_changed.is_empty() {
            config::write_toml(&team_path, &team).map_err(|e| e.to_string())?;
            let _g = ws.brain_mu.lock().await;
            brain::commit(
                &ws.paths.brain(),
                &format!("team: change {}", team_changed.join(", ")),
            )
            .await?;
            drop(_g);
            self.spawn_brain_sync(ws.clone());
        }
        self.reload_config(&ws).await
    }

    pub async fn set_step_skills(&self, id: &str, step: &str, list: Vec<String>) -> R<()> {
        let ws = self.ws(id).await?;
        let team_path = ws.paths.repo.join(config::TEAM_CONFIG_PATH);
        let mut team: TeamConfig =
            config::read_toml_or_default(&team_path).map_err(|e| e.to_string())?;
        let slot = match step {
            "prd" => &mut team.steps.prd,
            "develop" => &mut team.steps.develop,
            "test" => &mut team.steps.test,
            "commit" => &mut team.steps.commit,
            "brain" => &mut team.steps.brain,
            other => return Err(format!("unknown step \"{other}\"")),
        };
        slot.skills = list;
        config::write_toml(&team_path, &team).map_err(|e| e.to_string())?;
        {
            let _g = ws.brain_mu.lock().await;
            brain::commit(&ws.paths.brain(), &format!("team: skills for {step}")).await?;
        }
        self.spawn_brain_sync(ws.clone());
        self.reload_config(&ws).await
    }

    // ---------- PRDs ----------

    pub async fn list_prds(&self, id: &str) -> R<Vec<Prd>> {
        let ws = self.ws(id).await?;
        let mut v: Vec<Prd> = ws.prds.lock().await.values().cloned().collect();
        v.sort_by_key(|p| std::cmp::Reverse(p.created_at));
        Ok(v)
    }

    pub async fn get_prd(&self, id: &str, prd: &str) -> R<Prd> {
        let ws = self.ws(id).await?;
        ws.prds
            .lock()
            .await
            .get(prd)
            .cloned()
            .ok_or_else(|| format!("no PRD {prd}"))
    }

    /// Change a PRD, save it, and tell the UI.
    async fn update<T>(&self, ws: &Workspace, id: &str, f: impl FnOnce(&mut Prd) -> T) -> R<T> {
        let mut prds = ws.prds.lock().await;
        let p = prds.get_mut(id).ok_or_else(|| format!("no PRD {id}"))?;
        let before = (p.status, p.action.clone());
        let out = f(p);
        store::save(&ws.paths.runs(), p)?;
        let became_needs =
            p.status == Status::Needs && (before.0 != Status::Needs || before.1 != p.action);
        let snapshot = p.clone();
        drop(prds);
        if became_needs {
            self.notify(
                format!(
                    "{}: {}",
                    snapshot.id,
                    snapshot.action.clone().unwrap_or_default()
                ),
                snapshot.title.clone(),
                NotifyKind::You,
            );
        }
        if snapshot.status == Status::Failed && before.0 != Status::Failed {
            self.notify(
                format!("{} stopped", snapshot.id),
                snapshot.sub.clone(),
                NotifyKind::Bad,
            );
        }
        self.emit(Event::PrdChanged {
            workspace: ws.id.clone(),
            prd: Box::new(snapshot),
        });
        Ok(out)
    }

    pub async fn create_request(&self, id: &str, req: NewRequest) -> R<String> {
        let ws = self.ws(id).await?;
        if req.title.trim().is_empty() {
            return Err("give the request a title".into());
        }
        let initials = ws.cfg.read().await.workspace.initials.clone();
        let initials = if initials.is_empty() {
            "ME".to_string()
        } else {
            initials
        };
        let brain_dir = ws.paths.brain();
        let prd_id = if brain::is_ready(&brain_dir).await {
            prd_id::next_id(&brain_dir, &initials).map_err(|e| e.to_string())?
        } else {
            prd_id::next_id(&ws.paths.floww(), &initials).map_err(|e| e.to_string())?
        };
        let author = initials.clone();
        let prd = Prd::new(
            prd_id.clone(),
            req.title.trim().to_string(),
            req.description.trim().to_string(),
            req.kind,
            req.reporter.trim().to_string(),
            author,
        );
        store::save(&ws.paths.runs(), &prd)?;
        ws.prds.lock().await.insert(prd_id.clone(), prd.clone());
        self.emit(Event::PrdChanged {
            workspace: ws.id.clone(),
            prd: Box::new(prd),
        });
        self.spawn_advance(ws, prd_id.clone());
        Ok(prd_id)
    }

    // ---------- driver ----------

    fn spawn_advance(&self, ws: Arc<Workspace>, id: String) {
        let engine = self.clone();
        tokio::spawn(async move {
            {
                let mut running = ws.running.lock().await;
                if !running.insert(id.clone()) {
                    return;
                }
            }
            let result = engine.advance(&ws, &id).await;
            ws.running.lock().await.remove(&id);
            if let Err(e) = result {
                if e == "stopped" {
                    let _ = engine
                        .update(&ws, &id, |p| {
                            p.status = Status::Paused;
                            p.action = Some("Resume".into());
                            p.sub = "Paused".into();
                            p.log(EntryState::Info, "Paused", None);
                        })
                        .await;
                } else {
                    let _ = engine
                        .update(&ws, &id, |p| p.fail("Try again", e.clone()))
                        .await;
                }
            }
        });
    }

    async fn cancel_rx(&self, ws: &Workspace, id: &str) -> watch::Receiver<bool> {
        let mut map = ws.cancels.lock().await;
        let tx = map
            .entry(id.to_string())
            .or_insert_with(|| watch::channel(false).0);
        if *tx.borrow() {
            *tx = watch::channel(false).0;
        }
        tx.subscribe()
    }

    /// Run AI/app steps until the PRD needs a person.
    async fn advance(&self, ws: &Arc<Workspace>, id: &str) -> R<()> {
        loop {
            let prd = self.get_prd(&ws.id, id).await?;
            if matches!(
                prd.status,
                Status::Needs | Status::Paused | Status::Review | Status::Done
            ) {
                return Ok(());
            }
            match prd.step {
                Step::NewRequest | Step::CheckBrain => self.step_check_brain(ws, &prd).await?,
                Step::ReadDatabase => self.step_read_db(ws, &prd).await?,
                Step::ReadCodebase => self.step_read_code(ws, &prd).await?,
                Step::UpdateBrain => self.step_update_brain(ws, &prd).await?,
                Step::GrillPrd => self.step_grill(ws, &prd).await?,
                Step::ApprovePrd => return Ok(()),
                Step::CreateBranch => self.step_create_branch(ws, &prd).await?,
                Step::Develop | Step::Feedback => self.step_develop(ws, &prd).await?,
                Step::FullSuite => self.step_full_suite(ws, &prd).await?,
                Step::LogChanges => self.step_log_changes(ws, &prd).await?,
                Step::StartOdoo | Step::UpgradeModules | Step::HumanTest => return Ok(()),
                Step::CommitMessage => self.step_commit_message(ws, &prd).await?,
                Step::Commit | Step::Push | Step::PullRequest | Step::Done => return Ok(()),
            }
        }
    }

    /// Run Claude Code for a PRD and stream tool use into its timeline.
    async fn ai(&self, ws: &Arc<Workspace>, id: &str, req: Request, minutes: u32) -> R<AiRun> {
        let auth = self.auth_for(ws).await;
        let token = self.router_token(&auth).await?;
        let cancel = self.cancel_rx(ws, id).await;
        let (tx, mut rx) = mpsc::unbounded_channel::<AiEvent>();
        let engine = self.clone();
        let ws2 = ws.clone();
        let id2 = id.to_string();
        let consumer = tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                if let AiEvent::ToolUse { name, summary } = ev {
                    let text = match name.as_str() {
                        "Read" => format!("Reading {summary}"),
                        "Edit" | "Write" | "MultiEdit" => format!("Changing {summary}"),
                        "Bash" => {
                            format!("Running {}", summary.chars().take(80).collect::<String>())
                        }
                        "Grep" | "Glob" => format!("Searching for {summary}"),
                        other => other.to_string(),
                    };
                    let _ = engine
                        .update(&ws2, &id2, |p| p.log(EntryState::Run, text, None))
                        .await;
                }
            }
        });
        let timeout = Duration::from_secs(u64::from(minutes.max(1)) * 60);
        let result = runner::run(&req, &auth, token.as_deref(), timeout, cancel, move |ev| {
            let _ = tx.send(ev.clone());
        })
        .await;
        let _ = consumer.await;
        let run = result?;
        self.update(ws, id, |p| {
            p.cost_usd += run.cost_usd;
            p.tokens_in += run.tokens_in;
            p.tokens_out += run.tokens_out;
        })
        .await?;
        Ok(run)
    }

    async fn pack_lists(
        &self,
        ws: &Workspace,
        prd: &Prd,
        with_api_facts: bool,
    ) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
        let brain_dir = ws.paths.brain();
        let query = format!(
            "{} {} {}",
            prd.title,
            prd.description,
            prd.modules.join(" ")
        );
        let mut rules = Vec::new();
        for n in brain::list(&brain_dir, "rules") {
            if n.trust == "confirmed" {
                rules.push(format!("{} [{}]", n.title, n.path));
            }
        }
        let mut decisions: Vec<String> = Vec::new();
        if let Some(g) = &prd.grill {
            for a in &g.answers {
                decisions.push(format!("{} → {}", a.question.text, a.answer));
            }
        }
        for n in brain::search(&brain_dir, &query, 6) {
            match n.kind.as_str() {
                // Other PRDs' decisions: only the answered rows, never the table header.
                "decisions" if n.meta.get("run").map(|r| r != &prd.id).unwrap_or(true) => {
                    if let Some((_, text)) = brain::read_note(&brain_dir, &n.path) {
                        for row in text.lines().filter(|l| l.starts_with("| D-")) {
                            let cells: Vec<&str> = row.split('|').map(str::trim).collect();
                            if cells.len() >= 5 {
                                decisions.push(format!("{} → {} [{}]", cells[2], cells[4], n.path));
                            }
                        }
                    }
                }
                "opinion" | "flow" => {
                    decisions.push(format!("{}: {} [{}]", n.title, n.excerpt, n.path))
                }
                _ => {}
            }
        }
        let mut facts = Vec::new();
        if let Some(ctx) = &prd.context {
            if !ctx.summary.is_empty() {
                facts.push(format!("How it works today: {}", ctx.summary));
            }
            facts.extend(ctx.db_facts.iter().cloned());
        }
        let modules: Vec<String> = if prd.modules.is_empty() {
            prd.context
                .as_ref()
                .map(|c| c.modules.clone())
                .unwrap_or_default()
        } else {
            prd.modules.clone()
        };
        let all = odoo_facts::read_all(&ws.paths.repo);
        let db_name = ws.cfg.read().await.workspace.db.clone();
        for m in all.iter().filter(|m| modules.contains(&m.name)) {
            for model in &m.models {
                facts.push(format!(
                    "{} in {} ({}): methods {}",
                    model.name,
                    m.name,
                    model.file,
                    model.methods.join(", ")
                ));
                if with_api_facts
                    && !db_name.is_empty()
                    && let Ok(fields) = db::model_fields(&db_name, &model.name).await
                    && !fields.is_empty()
                {
                    facts.push(format!(
                        "Exact fields of {}",
                        db::format_fields(&model.name, &fields).trim_end()
                    ));
                }
            }
        }
        let mut gaps = Vec::new();
        let who = ws.cfg.read().await.workspace.initials.clone();
        if let Some((_, text)) =
            brain::read_note(&brain_dir, &format!("understanding/people/{who}.md"))
        {
            let mut in_unsure = false;
            for line in text.lines() {
                if line.starts_with("## ") {
                    in_unsure = line.to_lowercase().contains("not sure");
                } else if in_unsure && let Some(item) = line.strip_prefix("- ") {
                    gaps.push(item.to_string());
                }
            }
        }
        (rules, decisions, facts, gaps)
    }

    async fn step_check_brain(&self, ws: &Arc<Workspace>, prd: &Prd) -> R<()> {
        self.update(ws, &prd.id, |p| {
            p.working(Step::CheckBrain, "Checking the brain");
            p.log(
                EntryState::Run,
                "Looking for related notes and similar requests",
                None,
            );
        })
        .await?;
        let query = format!("{} {}", prd.title, prd.description);
        let notes: Vec<String> = brain::search(&ws.paths.brain(), &query, 8)
            .into_iter()
            .map(|n| n.path)
            .collect();
        let words: HashSet<String> = prd
            .title
            .to_lowercase()
            .split_whitespace()
            .filter(|w| w.len() >= 5)
            .map(str::to_string)
            .collect();
        let similar = ws
            .prds
            .lock()
            .await
            .values()
            .filter(|o| o.id != prd.id)
            .find(|o| {
                o.title
                    .to_lowercase()
                    .split_whitespace()
                    .filter(|w| words.contains(*w))
                    .count()
                    >= 2
            })
            .map(|o| o.id.clone());
        let n = notes.len();
        self.update(ws, &prd.id, |p| {
            let ctx = p.context.get_or_insert_with(Context::default);
            ctx.brain_notes = notes;
            ctx.similar = similar.clone();
            let msg = match &similar {
                Some(s) => format!("Found {n} related notes; looks similar to {s}"),
                None => format!("Found {n} related notes in the brain"),
            };
            p.log(EntryState::Done, msg, None);
            p.step = Step::ReadDatabase;
        })
        .await
    }

    async fn step_read_db(&self, ws: &Arc<Workspace>, prd: &Prd) -> R<()> {
        self.update(ws, &prd.id, |p| {
            p.working(Step::ReadDatabase, "Reading the local database")
        })
        .await?;
        let db_name = ws.cfg.read().await.workspace.db.clone();
        let mut facts = Vec::new();
        let text = if db_name.is_empty() {
            "No database set for this workspace; skipped".to_string()
        } else {
            match db::installed_modules(&db_name).await {
                Ok(installed) => {
                    let local: Vec<String> = odoo_facts::module_dirs(&ws.paths.repo)
                        .iter()
                        .filter_map(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
                        .collect();
                    let ours: Vec<&String> =
                        installed.iter().filter(|m| local.contains(m)).collect();
                    facts.push(format!(
                        "Installed project modules in {db_name}: {}",
                        ours.iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                    format!(
                        "Read {db_name}: {} modules installed, {} from this project",
                        installed.len(),
                        ours.len()
                    )
                }
                Err(e) => {
                    format!("Could not read {db_name} ({e}); continuing without database facts")
                }
            }
        };
        self.update(ws, &prd.id, |p| {
            p.context.get_or_insert_with(Context::default).db_facts = facts;
            p.log(
                EntryState::Done,
                text,
                Some(format!("psql -d {db_name} (read-only)")),
            );
            p.step = Step::ReadCodebase;
        })
        .await
    }

    async fn ensure_worktree(&self, ws: &Arc<Workspace>, prd: &Prd) -> R<PathBuf> {
        let wt = ws.paths.worktree(&prd.id);
        if wt.join(".git").exists() {
            return Ok(wt);
        }
        let base = ws.cfg.read().await.team.base_branch.clone();
        git::fetch(&ws.paths.repo).await?;
        let base_ref = git::base_ref(&ws.paths.repo, &base).await?;
        git::worktree_add_detached(&ws.paths.repo, &wt, &base_ref).await?;
        let commit = git::rev_parse(&wt, "HEAD").await?;
        let wt2 = wt.clone();
        self.update(ws, &prd.id, |p| {
            p.worktree = Some(wt2);
            p.base_commit = Some(commit);
            p.log(
                EntryState::Done,
                format!("Checked out {base_ref} in a separate worktree"),
                Some(format!(
                    "git worktree add --detach .floww/worktrees/{} {base_ref}",
                    p.id
                )),
            );
        })
        .await?;
        Ok(wt)
    }

    async fn step_read_code(&self, ws: &Arc<Workspace>, prd: &Prd) -> R<()> {
        self.update(ws, &prd.id, |p| {
            p.working(Step::ReadCodebase, "Reading the code")
        })
        .await?;
        let wt = self.ensure_worktree(ws, prd).await?;
        let prd = self.get_prd(&ws.id, &prd.id).await?;
        let cfg = ws.cfg.read().await.clone();
        let (rules, decisions, facts, gaps) = self.pack_lists(ws, &prd, false).await;
        let pack = prompts::Pack {
            language: &cfg.team.language,
            skills: &[],
            rules: &rules,
            decisions: &decisions,
            facts: &facts,
            gaps: &gaps,
            budget_tokens: cfg.team.steps.prd.context_tokens as usize,
        };
        let base_ref = cfg.team.base_branch.clone();
        let req = Request {
            prompt: prompts::context(&prd, &pack, &base_ref),
            cwd: wt,
            model: self.model_for(ws, ModelStep::Prd).await,
            ..Default::default()
        };
        let run = self
            .ai(ws, &prd.id, req, cfg.team.steps.prd.max_minutes)
            .await?;
        #[derive(Deserialize, Default)]
        struct Out {
            #[serde(default)]
            summary: String,
            #[serde(default)]
            modules: Vec<String>,
            #[serde(default)]
            files: Vec<String>,
            #[serde(default)]
            similar_prd: Option<String>,
        }
        let out: Out = jsonx::extract(&run.text).unwrap_or(Out {
            summary: run.text.chars().take(600).collect(),
            ..Default::default()
        });
        self.update(ws, &prd.id, |p| {
            let ctx = p.context.get_or_insert_with(Context::default);
            ctx.summary = out.summary;
            ctx.modules = out.modules.clone();
            ctx.files = out.files;
            if ctx.similar.is_none() {
                ctx.similar = out.similar_prd.filter(|s| !s.is_empty() && s != "null");
            }
            p.module = out.modules.first().cloned();
            p.modules = out.modules;
            p.log(
                EntryState::Done,
                format!("Read the code: {}", p.modules.join(", ")),
                None,
            );
            p.step = Step::UpdateBrain;
        })
        .await
    }

    async fn step_update_brain(&self, ws: &Arc<Workspace>, prd: &Prd) -> R<()> {
        let brain_dir = ws.paths.brain();
        if brain::is_ready(&brain_dir).await {
            let _g = ws.brain_mu.lock().await;
            brain::write(
                &brain_dir,
                &format!("requirements/{}.md", prd.id),
                &brain::requirement_note(prd),
            )?;
            brain::commit(&brain_dir, &format!("brain: request {}", prd.id)).await?;
            drop(_g);
            self.spawn_brain_sync(ws.clone());
            self.emit(Event::BrainChanged {
                workspace: ws.id.clone(),
            });
        }
        self.update(ws, &prd.id, |p| {
            p.log(EntryState::Done, "Saved the request to the brain", None);
            p.step = Step::GrillPrd;
            p.grill = Some(Grill::default());
        })
        .await
    }

    async fn apply_grill_reply(&self, ws: &Arc<Workspace>, id: &str, run: &AiRun) -> R<()> {
        #[derive(Deserialize)]
        struct Out {
            done: bool,
            #[serde(default)]
            question: Option<GrillQuestion>,
        }
        let out: Out = jsonx::extract(&run.text)
            .map_err(|e| format!("the AI's question could not be read ({e}). Try again."))?;
        let session = run.session_id.clone();
        self.update(ws, id, |p| {
            let g = p.grill.get_or_insert_with(Grill::default);
            if session.is_some() {
                g.session_id = session;
            }
            match (out.done, out.question) {
                (false, Some(q)) => {
                    g.current = Some(q);
                    g.state = GrillState::Asking;
                    let n = g.answers.len() + 1;
                    p.needs(
                        Step::GrillPrd,
                        "Answer the AI's questions",
                        format!("Question {n} ready"),
                    );
                }
                _ => {
                    g.current = None;
                    g.state = GrillState::Finished;
                    p.needs(Step::GrillPrd, "Write the PRD", "Questions done");
                }
            }
        })
        .await
    }

    async fn step_grill(&self, ws: &Arc<Workspace>, prd: &Prd) -> R<()> {
        let g = prd.grill.clone().unwrap_or_default();
        if g.state == GrillState::Writing {
            return self.write_prd(ws, prd).await;
        }
        if g.current.is_some() || g.state == GrillState::Finished {
            return self
                .update(ws, &prd.id, |p| {
                    let action = if p
                        .grill
                        .as_ref()
                        .map(|g| g.state == GrillState::Finished)
                        .unwrap_or(false)
                    {
                        "Write the PRD"
                    } else {
                        "Answer the AI's questions"
                    };
                    p.needs(Step::GrillPrd, action, "Waiting for you");
                })
                .await;
        }
        self.update(ws, &prd.id, |p| {
            p.working(Step::GrillPrd, "Preparing questions");
            p.log(EntryState::Run, "Working out what to ask", None);
        })
        .await?;
        let cfg = ws.cfg.read().await.clone();
        let (rules, decisions, facts, gaps) = self.pack_lists(ws, prd, false).await;
        let pack = prompts::Pack {
            language: &cfg.team.language,
            skills: &cfg.team.steps.prd.skills,
            rules: &rules,
            decisions: &decisions,
            facts: &facts,
            gaps: &gaps,
            budget_tokens: cfg.team.steps.prd.context_tokens as usize,
        };
        let summary = prd
            .context
            .as_ref()
            .map(|c| c.summary.clone())
            .unwrap_or_default();
        let req = Request {
            prompt: prompts::grill_start(prd, &pack, &summary),
            cwd: prd
                .worktree
                .clone()
                .unwrap_or_else(|| ws.paths.repo.clone()),
            model: self.model_for(ws, ModelStep::Prd).await,
            ..Default::default()
        };
        let run = self
            .ai(ws, &prd.id, req, cfg.team.steps.prd.max_minutes)
            .await?;
        self.apply_grill_reply(ws, &prd.id, &run).await
    }

    pub async fn answer(&self, wid: &str, id: &str, answer: String, own: bool) -> R<()> {
        let ws = self.ws(wid).await?;
        let answer = answer.trim().to_string();
        if answer.is_empty() {
            return Err("type an answer first".into());
        }
        let prd = self.get_prd(wid, id).await?;
        let g = prd.grill.clone().ok_or("this PRD has no open questions")?;
        let q = g.current.clone().ok_or("there is no open question")?;
        self.update(&ws, id, |p| {
            let g = p.grill.as_mut().expect("checked");
            g.answers.push(GrillAnswer {
                question: q.clone(),
                answer: answer.clone(),
                own,
            });
            g.current = None;
            p.log(EntryState::Info, format!("You answered: {answer}"), None);
            p.working(Step::GrillPrd, "Preparing the next question");
        })
        .await?;
        if own {
            self.add_confirmation(
                &ws,
                &prd.author,
                &format!("You prefer \"{answer}\" for {}", q.topic.to_lowercase()),
                id,
                "opinion",
            )
            .await;
        }
        let cfg = ws.cfg.read().await.clone();
        let engine = self.clone();
        let wid = wid.to_string();
        let id = id.to_string();
        tokio::spawn(async move {
            let req = Request {
                prompt: prompts::grill_answer(&answer, own),
                cwd: prd
                    .worktree
                    .clone()
                    .unwrap_or_else(|| ws.paths.repo.clone()),
                model: engine.model_for(&ws, ModelStep::Prd).await,
                resume_session: g.session_id.clone(),
                ..Default::default()
            };
            let res = match engine
                .ai(&ws, &id, req, cfg.team.steps.prd.max_minutes)
                .await
            {
                Ok(run) => engine.apply_grill_reply(&ws, &id, &run).await,
                Err(e) => Err(e),
            };
            if let Err(e) = res {
                let _ = engine.update(&ws, &id, |p| p.fail("Try again", e)).await;
            }
            let _ = wid;
        });
        Ok(())
    }

    pub async fn finish_grill(&self, wid: &str, id: &str) -> R<()> {
        let ws = self.ws(wid).await?;
        self.update(&ws, id, |p| {
            let g = p.grill.get_or_insert_with(Grill::default);
            g.state = GrillState::Writing;
            g.current = None;
            p.working(Step::GrillPrd, "Writing the PRD");
        })
        .await?;
        self.spawn_advance(ws, id.to_string());
        Ok(())
    }

    async fn write_prd(&self, ws: &Arc<Workspace>, prd: &Prd) -> R<()> {
        self.update(ws, &prd.id, |p| {
            p.working(Step::GrillPrd, "Writing the PRD");
            p.log(EntryState::Run, "Writing the PRD from your answers", None);
        })
        .await?;
        let g = prd.grill.clone().unwrap_or_default();
        let cfg = ws.cfg.read().await.clone();
        let req = Request {
            prompt: prompts::prd_write(&g.answers),
            cwd: prd
                .worktree
                .clone()
                .unwrap_or_else(|| ws.paths.repo.clone()),
            model: self.model_for(ws, ModelStep::Prd).await,
            resume_session: g.session_id.clone(),
            ..Default::default()
        };
        let run = self
            .ai(ws, &prd.id, req, cfg.team.steps.prd.max_minutes)
            .await?;
        #[derive(Deserialize, Default)]
        struct Out {
            #[serde(default)]
            criteria: Vec<Criterion>,
            #[serde(default)]
            tests: Vec<TestCase>,
            #[serde(default)]
            rules: Vec<Rule>,
            #[serde(default)]
            modules: Vec<String>,
        }
        let out: Out = jsonx::extract(&run.text).map_err(|e| {
            format!("the PRD's acceptance criteria could not be read ({e}). Try again.")
        })?;
        if out.criteria.is_empty() {
            return Err(
                "the PRD has no acceptance criteria. Try again, or ask more questions.".into(),
            );
        }
        let markdown = match run.text.rfind("```json") {
            Some(i) => run.text[..i].trim().to_string(),
            None => run.text.trim().to_string(),
        };
        self.update(ws, &prd.id, |p| {
            p.prd_markdown = Some(markdown);
            p.criteria = out.criteria;
            p.tests = out.tests;
            p.rules = out.rules;
            if !out.modules.is_empty() {
                p.modules = out.modules;
                p.module = p.modules.first().cloned();
            }
            if let Some(g) = p.grill.as_mut() {
                g.state = GrillState::Ready;
            }
            p.log(
                EntryState::Done,
                format!("PRD written with {} acceptance criteria", p.criteria.len()),
                None,
            );
            p.needs(Step::ApprovePrd, "Review the PRD", "PRD ready");
        })
        .await?;
        let prd = self.get_prd(&ws.id, &prd.id).await?;
        let model = self.model_for(ws, ModelStep::Prd).await.unwrap_or_default();
        let brain_dir = ws.paths.brain();
        if brain::is_ready(&brain_dir).await {
            let _g = ws.brain_mu.lock().await;
            brain::write(
                &brain_dir,
                &format!("prd/{}.md", prd.id),
                &brain::prd_note(&prd, &model),
            )?;
            brain::write(
                &brain_dir,
                &format!("decisions/{}.md", prd.id),
                &brain::decisions_note(&prd, &g.answers, &prd.author),
            )?;
            for r in &prd.rules {
                let rel = format!("rules/{}.md", r.id.replace(['/', '.'], "-"));
                if !brain_dir.join(&rel).exists() {
                    brain::write(&brain_dir, &rel, &brain::rule_note(r, &prd, &model))?;
                }
            }
            brain::commit(&brain_dir, &format!("brain: PRD {}", prd.id)).await?;
            drop(_g);
            self.spawn_brain_sync(ws.clone());
            self.emit(Event::BrainChanged {
                workspace: ws.id.clone(),
            });
        }
        Ok(())
    }

    pub async fn reopen_grill(&self, wid: &str, id: &str) -> R<()> {
        let ws = self.ws(wid).await?;
        self.update(&ws, id, |p| {
            if let Some(g) = p.grill.as_mut() {
                g.state = GrillState::Asking;
                g.current = None;
            }
            p.working(Step::GrillPrd, "Preparing more questions");
        })
        .await?;
        let prd = self.get_prd(wid, id).await?;
        let g = prd.grill.clone().unwrap_or_default();
        let cfg = ws.cfg.read().await.clone();
        let engine = self.clone();
        let id = id.to_string();
        tokio::spawn(async move {
            let req = Request {
                prompt: "The person wants to clarify more before approving. Ask the next most useful question.".to_string() + "\n" + &prompts::grill_answer("(no answer yet)", false),
                cwd: prd.worktree.clone().unwrap_or_else(|| ws.paths.repo.clone()),
                model: engine.model_for(&ws, ModelStep::Prd).await,
                resume_session: g.session_id.clone(),
                ..Default::default()
            };
            let res = match engine
                .ai(&ws, &id, req, cfg.team.steps.prd.max_minutes)
                .await
            {
                Ok(run) => engine.apply_grill_reply(&ws, &id, &run).await,
                Err(e) => Err(e),
            };
            if let Err(e) = res {
                let _ = engine.update(&ws, &id, |p| p.fail("Try again", e)).await;
            }
        });
        Ok(())
    }

    pub async fn edit_prd(&self, wid: &str, id: &str, markdown: String) -> R<()> {
        let ws = self.ws(wid).await?;
        self.update(&ws, id, |p| {
            p.prd_markdown = Some(markdown);
            p.log(EntryState::Info, "You edited the PRD", None);
        })
        .await?;
        let prd = self.get_prd(wid, id).await?;
        let brain_dir = ws.paths.brain();
        if brain::is_ready(&brain_dir).await {
            let _g = ws.brain_mu.lock().await;
            brain::write(
                &brain_dir,
                &format!("prd/{}.md", prd.id),
                &brain::prd_note(&prd, ""),
            )?;
            brain::commit(&brain_dir, &format!("brain: edit PRD {}", prd.id)).await?;
        }
        Ok(())
    }

    pub async fn approve(&self, wid: &str, id: &str) -> R<()> {
        let ws = self.ws(wid).await?;
        let prd = self.get_prd(wid, id).await?;
        if prd.step != Step::ApprovePrd {
            return Err("this PRD is not waiting for approval".into());
        }
        self.update(&ws, id, |p| {
            p.log(EntryState::Done, "You approved the PRD", None);
            p.working(Step::CreateBranch, "Creating the branch");
        })
        .await?;
        self.spawn_advance(ws, id.to_string());
        Ok(())
    }

    async fn test_command(
        &self,
        ws: &Workspace,
        prd: &Prd,
        modules: &[String],
    ) -> Option<(String, Vec<String>, String)> {
        let cfg = ws.cfg.read().await.clone();
        let name = cfg.workspace.launch_config.clone()?;
        let launches =
            odoo::read_launch_configs(&ws.paths.repo.join(".vscode/launch.json")).ok()?;
        let launch = odoo::find_launch(&launches, &name).ok()?;
        let wt = prd
            .worktree
            .clone()
            .unwrap_or_else(|| ws.paths.repo.clone());
        let mods = if modules.is_empty() {
            vec!["base".to_string()]
        } else {
            modules.to_vec()
        };
        let cmd = odoo::command(
            &launch,
            &wt,
            &cfg.workspace.db,
            cfg.workspace.http_port,
            Mode::Test(&mods),
        );
        let shown = shell_join(&cmd.program, &cmd.args);
        Some((cmd.program, cmd.args, shown))
    }

    async fn upgrade_command(
        &self,
        ws: &Workspace,
        prd: &Prd,
    ) -> Option<(String, Vec<String>, String)> {
        let cfg = ws.cfg.read().await.clone();
        let name = cfg.workspace.launch_config.clone()?;
        let launches =
            odoo::read_launch_configs(&ws.paths.repo.join(".vscode/launch.json")).ok()?;
        let launch = odoo::find_launch(&launches, &name).ok()?;
        let wt = prd
            .worktree
            .clone()
            .unwrap_or_else(|| ws.paths.repo.clone());
        let cmd = odoo::command(
            &launch,
            &wt,
            &cfg.workspace.db,
            cfg.workspace.http_port,
            Mode::Upgrade(&prd.modules),
        );
        let shown = shell_join(&cmd.program, &cmd.args);
        Some((cmd.program, cmd.args, shown))
    }

    async fn step_create_branch(&self, ws: &Arc<Workspace>, prd: &Prd) -> R<()> {
        let wt = self.ensure_worktree(ws, prd).await?;
        let cfg = ws.cfg.read().await.clone();
        let branch = cfg
            .team
            .feature_branch
            .replace("{prd_id}", &prd.id)
            .replace("{slug}", &prd.slug());
        let current = git::git_quick(&wt, &["branch", "--show-current"])
            .await
            .unwrap_or_default();
        if current.trim() != branch {
            if git::local_branch_exists(&ws.paths.repo, &branch).await {
                git::git(&wt, &["switch", &branch]).await?;
            } else {
                git::switch_new_branch(&wt, &branch).await?;
            }
        }
        let rules: Vec<String> = brain::list(&ws.paths.brain(), "rules")
            .into_iter()
            .filter(|n| n.trust == "confirmed")
            .map(|n| n.title)
            .collect();
        let prd_now = self.get_prd(&ws.id, &prd.id).await?;
        let test_cmd = self
            .test_command(ws, &prd_now, &prd_now.modules)
            .await
            .map(|t| t.2)
            .unwrap_or_else(|| {
                "(no Odoo launch config set; run the module tests your usual way)".into()
            });
        let up_cmd = self
            .upgrade_command(ws, &prd_now)
            .await
            .map(|t| t.2)
            .unwrap_or_else(|| "(no Odoo launch config set)".into());
        let fmt = match cfg.team.commit_format {
            floww_core::config::CommitFormat::Odoo => "Odoo style, e.g. [FIX] module: summary",
            floww_core::config::CommitFormat::Conventional => {
                "Conventional Commits, e.g. fix(module): summary"
            }
        };
        worktree_files::install(
            &ws.paths.repo,
            &wt,
            &worktree_files::Instructions {
                prd_id: &prd.id,
                test_command: &test_cmd,
                upgrade_command: &up_cmd,
                commit_format: fmt,
                rules: &rules,
                language: prompts::language_name(&cfg.team.language),
            },
        )
        .await?;
        let b = branch.clone();
        self.update(ws, &prd.id, |p| {
            p.branch = Some(b.clone());
            p.log(
                EntryState::Done,
                format!("Created branch {b}"),
                Some(format!("git switch -c {b}")),
            );
            p.working(Step::Develop, "Starting to build");
            p.iteration = 0;
        })
        .await
    }

    async fn run_tests(
        &self,
        ws: &Arc<Workspace>,
        prd: &Prd,
        modules: &[String],
    ) -> R<Option<(bool, String)>> {
        let Some((program, args, shown)) = self.test_command(ws, prd, modules).await else {
            return Ok(None);
        };
        let id = prd.id.clone();
        let engine = self.clone();
        let ws2 = ws.clone();
        let guard = ws
            .lock
            .acquire(&prd.id, "running tests", move |holder| {
                let holder = holder.to_string();
                tokio::spawn(async move {
                    let _ = engine
                        .update(&ws2, &id, |p| {
                            p.status = Status::Wait;
                            p.sub = format!("Waiting for the database ({holder} is using it)");
                        })
                        .await;
                });
            })
            .await;
        self.update(ws, &prd.id, |p| {
            p.status = Status::Ai;
            p.sub = "Running the tests".into();
            p.log(EntryState::Run, "Running module tests", Some(shown.clone()));
        })
        .await?;
        let wt = prd
            .worktree
            .clone()
            .unwrap_or_else(|| ws.paths.repo.clone());
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = proc::run(&program, &arg_refs, &wt, proc::LONG).await;
        drop(guard);
        let out = out?;
        let combined = format!("{}\n{}", out.stdout, out.stderr);
        let failed = !out.ok
            || combined.contains("FAIL:")
            || combined.contains("ERROR: ")
            || combined
                .lines()
                .any(|l| l.contains(" ERROR ") && l.contains("odoo"));
        let tail: Vec<&str> = combined
            .lines()
            .filter(|l| l.contains("FAIL") || l.contains("ERROR") || l.contains("Error"))
            .take(15)
            .collect();
        Ok(Some((
            !failed,
            if tail.is_empty() {
                out.tail(15)
            } else {
                tail.join("\n")
            },
        )))
    }

    async fn develop_once(
        &self,
        ws: &Arc<Workspace>,
        prd: &Prd,
        row: usize,
        criterion: &Criterion,
        extra: &str,
    ) -> R<()> {
        let cfg = ws.cfg.read().await.clone();
        let (rules, decisions, facts, gaps) = self.pack_lists(ws, prd, true).await;
        let pack = prompts::Pack {
            language: &cfg.team.language,
            skills: &cfg.team.steps.develop.skills,
            rules: &rules,
            decisions: &decisions,
            facts: &facts,
            gaps: &gaps,
            budget_tokens: cfg.team.steps.develop.context_tokens as usize,
        };
        let test_cmd = self
            .test_command(ws, prd, &prd.modules)
            .await
            .map(|t| t.2)
            .unwrap_or_else(|| "the module tests".into());
        let req = Request {
            prompt: prompts::develop(prd, &pack, row, criterion, &test_cmd, extra),
            cwd: prd.worktree.clone().ok_or("the PRD has no worktree")?,
            model: self.model_for(ws, ModelStep::Develop).await,
            bypass_permissions: true,
            ..Default::default()
        };
        self.ai(ws, &prd.id, req, cfg.team.steps.develop.max_minutes)
            .await?;
        Ok(())
    }

    async fn step_develop(&self, ws: &Arc<Workspace>, prd: &Prd) -> R<()> {
        let max = ws.cfg.read().await.team.steps.develop.max_iterations.max(1);
        // Feedback from human testing comes first, as its own short session.
        if !prd.feedback.is_empty() {
            let fb = prd.feedback.join("\n- ");
            let c = Criterion {
                given: "A person tested this branch in Odoo".into(),
                when: "they checked the test cases".into(),
                then: format!("fix what they reported:\n- {fb}"),
                done: false,
            };
            self.update(ws, &prd.id, |p| {
                p.working(
                    Step::Develop,
                    format!("Fixing feedback round {}", p.feedback_rounds),
                );
                p.log(EntryState::Run, "Writing a test for what you found", None);
            })
            .await?;
            let now = self.get_prd(&ws.id, &prd.id).await?;
            self.develop_once(
                ws,
                &now,
                now.criteria.len(),
                &c,
                "\nThis is feedback from human testing. Add a test that reproduces it.\n",
            )
            .await?;
            self.update(ws, &prd.id, |p| {
                p.feedback.clear();
                p.log(EntryState::Done, "Worked on your feedback", None);
            })
            .await?;
        }
        loop {
            let now = self.get_prd(&ws.id, &prd.id).await?;
            let Some(row) = now.criteria.iter().position(|c| !c.done) else {
                break;
            };
            let c = now.criteria[row].clone();
            let total = now.criteria.len();
            let attempt = now.iteration + 1;
            if attempt > max {
                return self
                    .update(ws, &prd.id, |p| {
                        p.needs(Step::Develop, "Help the AI", format!("Criterion {} still fails after {max} tries", row + 1));
                        p.log(EntryState::Fail, format!("Stopped after {max} tries on criterion {}. Give a hint or edit the PRD, then continue.", row + 1), None);
                    })
                    .await;
            }
            self.update(ws, &prd.id, |p| {
                p.working(
                    Step::Develop,
                    format!("Criterion {} of {total}, try {attempt} of {max}", row + 1),
                );
                p.log(
                    EntryState::Run,
                    format!("New session for criterion {}: {}", row + 1, c.then),
                    None,
                );
            })
            .await?;
            let extra = if now.iteration > 0 {
                now.timeline
                    .iter()
                    .rev()
                    .find(|e| e.state == EntryState::Fail)
                    .and_then(|e| e.cmd.clone())
                    .map(|t| format!("\nThe last try failed with:\n```\n{t}\n```\n"))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            self.develop_once(ws, &now, row, &c, &extra).await?;
            // Modules may change while building.
            let wt = now.worktree.clone().ok_or("the PRD has no worktree")?;
            let base = now.base_commit.clone().unwrap_or_else(|| "HEAD".into());
            let files: Vec<PathBuf> = git::changed_files(&wt, &base)
                .await
                .into_iter()
                .map(PathBuf::from)
                .collect();
            let touched = odoo::modules_touched(&wt, &files);
            if !touched.is_empty() {
                self.update(ws, &prd.id, |p| {
                    for m in &touched {
                        if !p.modules.contains(m) {
                            p.modules.push(m.clone());
                        }
                    }
                    if p.module.is_none() {
                        p.module = p.modules.first().cloned();
                    }
                })
                .await?;
            }
            let now = self.get_prd(&ws.id, &prd.id).await?;
            match self.run_tests(ws, &now, &now.modules).await? {
                Some((true, _)) | None => {
                    self.update(ws, &prd.id, |p| {
                        p.criteria[row].done = true;
                        p.iteration = 0;
                        p.log(
                            EntryState::Done,
                            format!("Criterion {} passes", row + 1),
                            None,
                        );
                    })
                    .await?;
                }
                Some((false, tail)) => {
                    self.update(ws, &prd.id, |p| {
                        p.iteration += 1;
                        p.log(
                            EntryState::Fail,
                            format!("Tests still fail for criterion {}", row + 1),
                            Some(tail),
                        );
                    })
                    .await?;
                }
            }
        }
        let full = ws.cfg.read().await.team.steps.full_suite;
        self.update(ws, &prd.id, |p| {
            p.step = if full {
                Step::FullSuite
            } else {
                Step::LogChanges
            };
            p.sub = "Built".into();
        })
        .await
    }

    async fn step_full_suite(&self, ws: &Arc<Workspace>, prd: &Prd) -> R<()> {
        self.update(ws, &prd.id, |p| {
            p.working(Step::FullSuite, "Running the full test suite")
        })
        .await?;
        let all: Vec<String> = odoo_facts::module_dirs(&ws.paths.repo)
            .iter()
            .filter_map(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        match self.run_tests(ws, prd, &all).await? {
            Some((false, tail)) => {
                self.update(ws, &prd.id, |p| {
                    p.log(
                        EntryState::Fail,
                        "The full test suite has failures",
                        Some(tail),
                    );
                    p.needs(
                        Step::FullSuite,
                        "Look at the failing tests",
                        "Full suite failed",
                    );
                })
                .await
            }
            _ => {
                self.update(ws, &prd.id, |p| {
                    p.log(EntryState::Done, "Full test suite passed", None);
                    p.step = Step::LogChanges;
                })
                .await
            }
        }
    }

    async fn step_log_changes(&self, ws: &Arc<Workspace>, prd: &Prd) -> R<()> {
        let wt = prd.worktree.clone().ok_or("the PRD has no worktree")?;
        let base = prd.base_commit.clone().unwrap_or_else(|| "HEAD".into());
        let files = git::changed_files(&wt, &base).await;
        let stat = git::diff_stat(&wt, &base).await;
        let brain_dir = ws.paths.brain();
        if brain::is_ready(&brain_dir).await {
            let _g = ws.brain_mu.lock().await;
            brain::write(
                &brain_dir,
                &format!("changes/{}.md", prd.id),
                &brain::changes_note(prd, &files, &stat),
            )?;
            brain::commit(&brain_dir, &format!("brain: changes {}", prd.id)).await?;
            drop(_g);
            self.spawn_brain_sync(ws.clone());
            self.emit(Event::BrainChanged {
                workspace: ws.id.clone(),
            });
        }
        let has_odoo = self.upgrade_command(ws, prd).await.is_some();
        self.update(ws, &prd.id, |p| {
            p.log(
                EntryState::Done,
                format!("Logged {} changed files to the brain", files.len()),
                None,
            );
            if p.tests.is_empty() {
                p.tests.push(TestCase {
                    kind: "Normal".into(),
                    title: "Main flow works".into(),
                    steps: "Follow the acceptance criteria".into(),
                    data: String::new(),
                    expected: "Matches the PRD".into(),
                    result: None,
                    note: String::new(),
                });
            }
            for t in &mut p.tests {
                if t.result == Some(TestResult::Fail) {
                    t.result = None;
                }
            }
            if has_odoo {
                p.needs(Step::StartOdoo, "Test it in Odoo", "Ready to test");
            } else {
                p.needs(
                    Step::HumanTest,
                    "Test it",
                    "Ready to test (no Odoo launch config set)",
                );
            }
        })
        .await
    }

    // ---------- human testing ----------

    pub async fn start_odoo(&self, wid: &str, id: &str) -> R<()> {
        let ws = self.ws(wid).await?;
        let prd = self.get_prd(wid, id).await?;
        let (program, args, shown) = self
            .upgrade_command(&ws, &prd)
            .await
            .ok_or("set an Odoo launch config in Settings first")?;
        self.update(&ws, id, |p| {
            p.odoo = OdooState::Starting;
            p.working(Step::StartOdoo, "Starting Odoo");
        })
        .await?;
        let engine = self.clone();
        let id = id.to_string();
        tokio::spawn(async move {
            let res = engine
                .start_odoo_inner(&ws, &id, program, args, shown)
                .await;
            if let Err(e) = res {
                let _ = engine
                    .update(&ws, &id, |p| {
                        p.odoo = OdooState::Off;
                        p.log(EntryState::Fail, e.clone(), None);
                        p.needs(Step::StartOdoo, "Start Odoo again", "Odoo did not start");
                    })
                    .await;
            }
        });
        Ok(())
    }

    async fn start_odoo_inner(
        &self,
        ws: &Arc<Workspace>,
        id: &str,
        program: String,
        args: Vec<String>,
        shown: String,
    ) -> R<()> {
        let id_s = id.to_string();
        let engine = self.clone();
        let ws2 = ws.clone();
        let guard = ws
            .lock
            .acquire(id, "testing in Odoo", move |holder| {
                let holder = holder.to_string();
                tokio::spawn(async move {
                    let _ = engine
                        .update(&ws2, &id_s, |p| {
                            p.status = Status::Wait;
                            p.sub = format!("Waiting for the database ({holder} is using it)");
                        })
                        .await;
                });
            })
            .await;
        let prd = self.get_prd(&ws.id, id).await?;
        let wt = prd.worktree.clone().ok_or("the PRD has no worktree")?;
        self.update(ws, id, |p| {
            p.working(
                Step::UpgradeModules,
                format!("Upgrading {}", p.modules.join(", ")),
            );
            p.log(EntryState::Run, "Upgrading modules", Some(shown));
        })
        .await?;
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = proc::run(&program, &arg_refs, &wt, proc::LONG).await?;
        if !out.ok {
            return Err(format!("Module upgrade failed:\n{}", out.tail(12)));
        }
        // Start the server for the person to test.
        let cfg = ws.cfg.read().await.clone();
        let launches = odoo::read_launch_configs(&ws.paths.repo.join(".vscode/launch.json"))
            .map_err(|e| e.to_string())?;
        let launch = odoo::find_launch(
            &launches,
            cfg.workspace.launch_config.as_deref().unwrap_or_default(),
        )
        .map_err(|e| e.to_string())?;
        let cmd = odoo::command(
            &launch,
            &wt,
            &cfg.workspace.db,
            cfg.workspace.http_port,
            Mode::Run,
        );
        std::fs::create_dir_all(ws.paths.logs()).map_err(|e| e.to_string())?;
        let log = std::fs::File::create(ws.paths.logs().join(format!("odoo-{id}.log")))
            .map_err(|e| e.to_string())?;
        let log2 = log.try_clone().map_err(|e| e.to_string())?;
        let child = tokio::process::Command::new(&cmd.program)
            .args(&cmd.args)
            .current_dir(&wt)
            .stdout(log)
            .stderr(log2)
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("could not start Odoo: {e}"))?;
        let mut preempted = guard.preempted.clone();
        ws.odoo.lock().await.insert(id.to_string(), (child, guard));
        let port = cfg.workspace.http_port;
        self.update(ws, id, |p| {
            p.odoo = OdooState::On;
            p.log(
                EntryState::Done,
                format!("Odoo is running on localhost:{port}"),
                Some(shell_join(&cmd.program, &cmd.args)),
            );
            p.needs(Step::HumanTest, "Test it in Odoo", "Odoo is ready");
        })
        .await?;
        // If someone takes the database over, stop this Odoo.
        let engine = self.clone();
        let ws2 = ws.clone();
        let id2 = id.to_string();
        tokio::spawn(async move {
            while preempted.changed().await.is_ok() {
                if *preempted.borrow() {
                    engine.stop_odoo_inner(&ws2, &id2).await;
                    let _ = engine
                        .update(&ws2, &id2, |p| {
                            p.log(
                                EntryState::Info,
                                "Another request took the database; Odoo stopped",
                                None,
                            );
                            p.needs(
                                Step::StartOdoo,
                                "Start Odoo again",
                                "Paused: database in use",
                            );
                        })
                        .await;
                    break;
                }
            }
        });
        Ok(())
    }

    async fn stop_odoo_inner(&self, ws: &Workspace, id: &str) {
        if let Some((mut child, guard)) = ws.odoo.lock().await.remove(id) {
            let _ = child.kill().await;
            drop(guard);
        }
        let _ = self.update(ws, id, |p| p.odoo = OdooState::Off).await;
    }

    pub async fn stop_odoo(&self, wid: &str, id: &str) -> R<()> {
        let ws = self.ws(wid).await?;
        self.stop_odoo_inner(&ws, id).await;
        self.update(&ws, id, |p| {
            p.log(EntryState::Info, "Stopped Odoo", None);
            p.needs(Step::StartOdoo, "Test it in Odoo", "Odoo stopped");
        })
        .await
    }

    pub async fn mark_test(
        &self,
        wid: &str,
        id: &str,
        index: usize,
        result: Option<TestResult>,
        note: Option<String>,
    ) -> R<()> {
        let ws = self.ws(wid).await?;
        self.update(&ws, id, |p| {
            let t = p.tests.get_mut(index).ok_or("no such test case")?;
            if let Some(n) = note {
                t.note = n;
            } else {
                t.result = result;
            }
            Ok::<_, String>(())
        })
        .await?
    }

    pub async fn send_feedback(&self, wid: &str, id: &str) -> R<()> {
        let ws = self.ws(wid).await?;
        self.stop_odoo_inner(&ws, id).await;
        self.update(&ws, id, |p| {
            let items: Vec<String> = p
                .tests
                .iter()
                .filter(|t| t.result == Some(TestResult::Fail) || !t.note.trim().is_empty())
                .map(|t| {
                    if t.note.trim().is_empty() {
                        format!("{} is broken (expected: {})", t.title, t.expected)
                    } else {
                        format!("{}: {}", t.title, t.note.trim())
                    }
                })
                .collect();
            if items.is_empty() {
                return Err("mark a case as broken or write a note first".to_string());
            }
            p.feedback_rounds += 1;
            p.log(
                EntryState::Info,
                format!("Your feedback: {}", items.join("; ")),
                None,
            );
            p.feedback = items;
            for t in &mut p.tests {
                t.note.clear();
            }
            p.working(Step::Feedback, "Fixing what you found");
            Ok(())
        })
        .await??;
        if let Some(a) = self.get_prd(wid, id).await?.feedback.first() {
            let who = self.get_prd(wid, id).await?.author;
            self.add_confirmation(
                &ws,
                &who,
                &format!("This behaviour was not clear from the PRD: {a}"),
                id,
                "understanding",
            )
            .await;
        }
        self.spawn_advance(ws, id.to_string());
        Ok(())
    }

    pub async fn all_good(&self, wid: &str, id: &str) -> R<()> {
        let ws = self.ws(wid).await?;
        let prd = self.get_prd(wid, id).await?;
        if prd.tests.iter().any(|t| t.result != Some(TestResult::Pass)) {
            return Err("mark every test case as working first".into());
        }
        self.stop_odoo_inner(&ws, id).await;
        self.update(&ws, id, |p| {
            p.log(
                EntryState::Done,
                format!("All {} test cases work", p.tests.len()),
                None,
            );
            p.working(Step::CommitMessage, "Drafting the commit message");
        })
        .await?;
        self.spawn_advance(ws, id.to_string());
        Ok(())
    }

    // ---------- ship ----------

    async fn step_commit_message(&self, ws: &Arc<Workspace>, prd: &Prd) -> R<()> {
        let wt = prd.worktree.clone().ok_or("the PRD has no worktree")?;
        let base = prd.base_commit.clone().unwrap_or_else(|| "HEAD".into());
        let stat = git::diff_stat(&wt, &base).await;
        let diff = git::diff(&wt, &base, 24_000).await;
        let cfg = ws.cfg.read().await.clone();
        let module = prd
            .modules
            .first()
            .cloned()
            .unwrap_or_else(|| "module".into());
        let hint = match cfg.team.commit_format {
            floww_core::config::CommitFormat::Odoo => {
                format!("{} {module}: <summary>", prd.kind.commit_tag())
            }
            floww_core::config::CommitFormat::Conventional => {
                format!("<type>({module}): <summary>")
            }
        };
        let req = Request {
            prompt: prompts::commit_message(prd, &stat, &diff, &hint),
            cwd: wt,
            model: self.model_for(ws, ModelStep::Commit).await,
            ..Default::default()
        };
        #[derive(Deserialize, Default)]
        struct Out {
            message: String,
            #[serde(default)]
            pr_title: String,
            #[serde(default)]
            pr_body: String,
        }
        let fallback = Out {
            message: format!(
                "{} {module}: {}\n\nPRD: {}",
                prd.kind.commit_tag(),
                prd.title.to_lowercase(),
                prd.id
            ),
            pr_title: format!(
                "{} {module}: {}",
                prd.kind.commit_tag(),
                prd.title.to_lowercase()
            ),
            pr_body: format!("PRD: {}", prd.id),
        };
        let out = match self
            .ai(ws, &prd.id, req, cfg.team.steps.commit.max_minutes)
            .await
        {
            Ok(run) => jsonx::extract::<Out>(&run.text)
                .ok()
                .filter(|o| !o.message.trim().is_empty())
                .unwrap_or(fallback),
            Err(e) if e == "stopped" => return Err(e),
            Err(_) => fallback,
        };
        self.update(ws, &prd.id, |p| {
            p.ship = Some(Ship {
                stage: 0,
                message: out.message,
                pr_title: if out.pr_title.is_empty() {
                    p.title.clone()
                } else {
                    out.pr_title
                },
                pr_body: out.pr_body,
                ..Default::default()
            });
            p.log(EntryState::Done, "Commit message drafted", None);
            p.needs(
                Step::CommitMessage,
                "Check the commit",
                "Commit message drafted",
            );
        })
        .await
    }

    pub async fn commit(&self, wid: &str, id: &str, message: String) -> R<()> {
        let ws = self.ws(wid).await?;
        let prd = self.get_prd(wid, id).await?;
        if message.trim().is_empty() {
            return Err("the commit message is empty".into());
        }
        let wt = prd.worktree.clone().ok_or("the PRD has no worktree")?;
        let base = prd
            .base_commit
            .clone()
            .ok_or("the PRD has no base commit")?;
        let sha = git::squash_commit(&wt, &base, message.trim()).await?;
        let short: String = sha.chars().take(7).collect();
        self.update(&ws, id, |p| {
            if let Some(s) = p.ship.as_mut() {
                s.message = message.trim().to_string();
                s.commit = Some(sha.clone());
                s.stage = 1;
            }
            p.log(
                EntryState::Done,
                format!("Committed {short}"),
                Some("git reset --soft <base> && git commit".into()),
            );
            p.needs(Step::Push, "Push the branch", "Committed");
        })
        .await
    }

    pub async fn push(&self, wid: &str, id: &str) -> R<()> {
        let ws = self.ws(wid).await?;
        let prd = self.get_prd(wid, id).await?;
        let wt = prd.worktree.clone().ok_or("the PRD has no worktree")?;
        let branch = prd.branch.clone().ok_or("the PRD has no branch")?;
        if !git::has_remote(&ws.paths.repo).await {
            return Err("this repo has no `origin` remote to push to".into());
        }
        git::push(&wt, &branch).await?;
        self.update(&ws, id, |p| {
            if let Some(s) = p.ship.as_mut() {
                s.stage = 2;
            }
            p.log(
                EntryState::Done,
                format!("Pushed {branch}"),
                Some(format!("git push -u origin {branch}")),
            );
            p.needs(Step::PullRequest, "Open a pull request", "Pushed");
        })
        .await
    }

    async fn github_token(&self, ws: &Workspace) -> Option<String> {
        let r = ws.cfg.read().await.workspace.github_token_ref.clone();
        match r {
            Some(r) => secrets::resolve(&r).await.ok(),
            None => std::env::var("GITHUB_TOKEN").ok(),
        }
    }

    pub async fn create_pr(&self, wid: &str, id: &str, title: String, body: String) -> R<()> {
        let ws = self.ws(wid).await?;
        let prd = self.get_prd(wid, id).await?;
        let branch = prd.branch.clone().ok_or("the PRD has no branch")?;
        let base = ws.cfg.read().await.team.base_branch.clone();
        let token = self.github_token(&ws).await;
        let pr = github::create(
            &ws.paths.repo,
            token.as_deref(),
            &branch,
            &base,
            &title,
            &body,
        )
        .await?;
        self.update(&ws, id, |p| {
            if let Some(s) = p.ship.as_mut() {
                s.stage = 3;
                s.pr_url = Some(pr.url.clone());
                s.pr_number = Some(pr.number);
                s.pr_title = title.clone();
                s.pr_body = body.clone();
            }
            p.step = Step::PullRequest;
            p.status = Status::Review;
            p.action = None;
            p.sub = format!("Pull request #{} open", pr.number);
            p.log(
                EntryState::Done,
                format!("Opened pull request #{}", pr.number),
                Some(pr.url.clone()),
            );
        })
        .await
    }

    pub async fn skip_pr(&self, wid: &str, id: &str) -> R<()> {
        let ws = self.ws(wid).await?;
        self.update(&ws, id, |p| {
            if let Some(s) = p.ship.as_mut() {
                s.stage = 3;
                s.skipped_pr = true;
            }
            p.step = Step::Done;
            p.status = Status::Done;
            p.action = None;
            p.sub = "Pushed".into();
            p.log(EntryState::Done, "Pushed without a pull request", None);
        })
        .await?;
        self.finish(&ws, id).await
    }

    pub async fn check_pr(&self, wid: &str, id: &str) -> R<String> {
        let ws = self.ws(wid).await?;
        let prd = self.get_prd(wid, id).await?;
        let number = prd
            .ship
            .as_ref()
            .and_then(|s| s.pr_number)
            .ok_or("no pull request yet")?;
        let token = self.github_token(&ws).await;
        let pr = github::status(&ws.paths.repo, token.as_deref(), number).await?;
        if pr.merged {
            self.update(&ws, id, |p| {
                p.step = Step::Done;
                p.status = Status::Done;
                p.sub = "Merged".into();
                p.log(
                    EntryState::Done,
                    format!("Pull request #{number} merged"),
                    None,
                );
            })
            .await?;
            self.notify(
                "Pull request merged",
                format!("{id} is done"),
                NotifyKind::Ok,
            );
            self.finish(&ws, id).await?;
            return Ok("merged".into());
        }
        Ok(pr.state)
    }

    async fn finish(&self, ws: &Arc<Workspace>, id: &str) -> R<()> {
        let prd = self.get_prd(&ws.id, id).await?;
        let brain_dir = ws.paths.brain();
        if brain::is_ready(&brain_dir).await {
            let _g = ws.brain_mu.lock().await;
            let rel = format!("changes/{id}.md");
            if let Some((_, text)) = brain::read_note(&brain_dir, &rel) {
                let merged = if prd.sub == "Merged" {
                    "merged"
                } else {
                    "pushed"
                };
                brain::write(&brain_dir, &rel, &brain::set_field(&text, "status", merged))?;
            }
            for r in &prd.rules {
                let rel = format!("rules/{}.md", r.id.replace(['/', '.'], "-"));
                let _ = brain::bump_usage(&brain_dir, &rel, true);
            }
            brain::commit(&brain_dir, &format!("brain: {id} done")).await?;
            drop(_g);
            self.spawn_brain_sync(ws.clone());
        }
        if let Some(wt) = prd.worktree.clone()
            && git::worktree_remove(&ws.paths.repo, &wt).await.is_ok()
        {
            let _ = self
                .update(ws, id, |p| {
                    p.log(EntryState::Done, "Cleaned up the worktree", None)
                })
                .await;
        }
        Ok(())
    }

    // ---------- control ----------

    pub async fn pause(&self, wid: &str, id: &str) -> R<()> {
        let ws = self.ws(wid).await?;
        if let Some(tx) = ws.cancels.lock().await.get(id) {
            let _ = tx.send(true);
        }
        ws.lock.leave(id).await;
        Ok(())
    }

    /// Continue after a pause, an error, or the loop limit.
    pub async fn resume(&self, wid: &str, id: &str) -> R<()> {
        let ws = self.ws(wid).await?;
        self.update(&ws, id, |p| {
            if p.step == Step::Develop {
                p.iteration = 0;
            }
            // Human steps go back to waiting; others run again.
            let human = matches!(
                p.step,
                Step::ApprovePrd
                    | Step::StartOdoo
                    | Step::HumanTest
                    | Step::Commit
                    | Step::Push
                    | Step::PullRequest
            ) || (p.step == Step::CommitMessage && p.ship.is_some())
                || (p.step == Step::GrillPrd
                    && p.grill
                        .as_ref()
                        .map(|g| g.current.is_some() || g.state == GrillState::Finished)
                        .unwrap_or(false));
            if human {
                p.status = Status::Needs;
            } else {
                p.status = Status::Ai;
                p.action = None;
                p.sub = "Continuing".into();
            }
            p.log(EntryState::Info, "Continuing", None);
        })
        .await?;
        self.spawn_advance(ws, id.to_string());
        Ok(())
    }

    pub async fn hint(&self, wid: &str, id: &str, text: String) -> R<()> {
        let ws = self.ws(wid).await?;
        let text = text.trim().to_string();
        if text.is_empty() {
            return Err("write a hint first".into());
        }
        self.update(&ws, id, |p| {
            p.log(EntryState::Info, format!("Your hint: {text}"), None);
            p.feedback.push(format!("Hint from the person: {text}"));
        })
        .await?;
        let prd = self.get_prd(wid, id).await?;
        self.add_confirmation(
            &ws,
            &prd.author,
            &format!("This needed a hint from you: {text}"),
            id,
            "understanding",
        )
        .await;
        Ok(())
    }

    pub async fn take_over(&self, wid: &str, id: &str) -> R<Option<String>> {
        let ws = self.ws(wid).await?;
        let paused = ws.lock.take_over(id).await;
        if let Some(other) = &paused
            && let Some(tx) = ws.cancels.lock().await.get(other)
        {
            let _ = tx.send(true);
        }
        Ok(paused)
    }

    pub async fn lock_state(&self, wid: &str) -> R<LockState> {
        Ok(self.ws(wid).await?.lock.state().await)
    }

    pub async fn diff(&self, wid: &str, id: &str) -> R<serde_json::Value> {
        let prd = self.get_prd(wid, id).await?;
        let Some(wt) = prd.worktree.clone().filter(|w| w.exists()) else {
            return Ok(serde_json::json!({ "files": [], "diff": "" }));
        };
        let base = prd.base_commit.clone().unwrap_or_else(|| "HEAD".into());
        Ok(serde_json::json!({
            "files": git::changed_files(&wt, &base).await,
            "stat": git::diff_stat(&wt, &base).await,
            "diff": git::diff(&wt, &base, 200_000).await,
        }))
    }

    pub async fn odoo_log(&self, wid: &str, id: &str) -> R<String> {
        let ws = self.ws(wid).await?;
        let text = std::fs::read_to_string(ws.paths.logs().join(format!("odoo-{id}.log")))
            .unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        Ok(lines[lines.len().saturating_sub(80)..].join("\n"))
    }

    // ---------- brain ----------

    fn spawn_brain_sync(&self, ws: Arc<Workspace>) {
        let engine = self.clone();
        tokio::spawn(async move {
            let _g = ws.brain_mu.lock().await;
            if let Err(e) = brain::sync(&ws.paths.repo, &ws.paths.brain()).await {
                tracing::warn!("brain sync: {e}");
                engine.notify("Brain not synced", e, NotifyKind::Bad);
            }
        });
    }

    pub async fn brain_sync(&self, wid: &str) -> R<()> {
        let ws = self.ws(wid).await?;
        let _g = ws.brain_mu.lock().await;
        brain::sync(&ws.paths.repo, &ws.paths.brain()).await?;
        drop(_g);
        self.emit(Event::BrainChanged {
            workspace: ws.id.clone(),
        });
        Ok(())
    }

    pub async fn brain_list(&self, wid: &str, folder: &str) -> R<Vec<brain::Note>> {
        if folder.contains("..") {
            return Err("invalid folder".into());
        }
        let ws = self.ws(wid).await?;
        Ok(brain::list(&ws.paths.brain(), folder))
    }

    pub async fn brain_note(&self, wid: &str, path: &str) -> R<serde_json::Value> {
        if path.contains("..") {
            return Err("invalid path".into());
        }
        let ws = self.ws(wid).await?;
        let (note, text) = brain::read_note(&ws.paths.brain(), path).ok_or("note not found")?;
        Ok(serde_json::json!({ "note": note, "text": text }))
    }

    pub async fn brain_confirm(&self, wid: &str, path: &str) -> R<()> {
        if path.contains("..") {
            return Err("invalid path".into());
        }
        let ws = self.ws(wid).await?;
        let who = ws.cfg.read().await.workspace.initials.clone();
        let _g = ws.brain_mu.lock().await;
        brain::confirm(&ws.paths.brain(), path, &who)?;
        brain::commit(&ws.paths.brain(), &format!("brain: {who} confirmed {path}")).await?;
        drop(_g);
        self.spawn_brain_sync(ws.clone());
        self.emit(Event::BrainChanged {
            workspace: ws.id.clone(),
        });
        Ok(())
    }

    /// Something the AI inferred about a person. Saved as a pending note that a
    /// person must confirm before it is used.
    async fn add_confirmation(
        &self,
        ws: &Arc<Workspace>,
        who: &str,
        text: &str,
        source: &str,
        kind: &str,
    ) {
        let brain_dir = ws.paths.brain();
        if !brain::is_ready(&brain_dir).await {
            return;
        }
        let n = chrono::Utc::now().timestamp_millis();
        let rel = format!("_pending/{kind}-{n}.md");
        let body = format!(
            "---\ntype: pending\nkind: {kind}\nwho: {who}\nsource: {source}\ntrust: draft\n---\n{text}\n"
        );
        let _g = ws.brain_mu.lock().await;
        let _ = brain::write(&brain_dir, &rel, &body);
        self.emit(Event::BrainChanged {
            workspace: ws.id.clone(),
        });
    }

    pub async fn pending(&self, wid: &str) -> R<Vec<serde_json::Value>> {
        let ws = self.ws(wid).await?;
        let brain_dir = ws.paths.brain();
        Ok(brain::list(&brain_dir, "_pending")
            .into_iter()
            .map(|n| {
                let text = brain::read_note(&brain_dir, &n.path).map(|(_, t)| t).unwrap_or_default();
                let body = text.rsplit("---\n").next().unwrap_or("").trim().to_string();
                serde_json::json!({ "path": n.path, "kind": n.meta.get("kind"), "who": n.meta.get("who"), "source": n.meta.get("source"), "text": body })
            })
            .collect())
    }

    /// Accept (optionally edited) or reject a pending note.
    pub async fn resolve_pending(
        &self,
        wid: &str,
        path: &str,
        accept: bool,
        text: Option<String>,
    ) -> R<()> {
        if !path.starts_with("_pending/") || path.contains("..") {
            return Err("invalid pending note".into());
        }
        let ws = self.ws(wid).await?;
        let brain_dir = ws.paths.brain();
        let (note, raw) = brain::read_note(&brain_dir, path).ok_or("note not found")?;
        let body =
            text.unwrap_or_else(|| raw.rsplit("---\n").next().unwrap_or("").trim().to_string());
        let who = note.meta.get("who").cloned().unwrap_or_default();
        let source = note.meta.get("source").cloned().unwrap_or_default();
        let _g = ws.brain_mu.lock().await;
        let _ = std::fs::remove_file(brain_dir.join(path));
        if accept {
            if note
                .meta
                .get("kind")
                .map(|k| k == "opinion")
                .unwrap_or(false)
            {
                let slug = slug(&body);
                brain::write(
                    &brain_dir,
                    &format!("opinions/{slug}.md"),
                    &brain::opinion_note(&who, &body, &source),
                )?;
            } else {
                let rel = format!("understanding/people/{who}.md");
                let current = std::fs::read_to_string(brain_dir.join(&rel)).unwrap_or_else(|_| format!("---\ntype: person\ntrust: confirmed\n---\n# {who}\n\n## Knows well\n\n## Not sure\n\n## New to\n"));
                let updated = match current.find("## Not sure\n") {
                    Some(i) => {
                        let at = i + "## Not sure\n".len();
                        format!("{}- {}\n{}", &current[..at], body, &current[at..])
                    }
                    None => format!("{current}\n## Not sure\n- {body}\n"),
                };
                brain::write(&brain_dir, &rel, &updated)?;
            }
        }
        brain::commit(
            &brain_dir,
            &format!(
                "brain: {} note from {source}",
                if accept { "keep" } else { "drop" }
            ),
        )
        .await?;
        drop(_g);
        self.spawn_brain_sync(ws.clone());
        self.emit(Event::BrainChanged {
            workspace: ws.id.clone(),
        });
        Ok(())
    }

    pub async fn graph(&self, wid: &str) -> R<odoo_facts::Graph> {
        let ws = self.ws(wid).await?;
        Ok(odoo_facts::graph(&odoo_facts::read_all(&ws.paths.repo)))
    }

    pub async fn refresh_facts(&self, wid: &str) -> R<usize> {
        let ws = self.ws(wid).await?;
        let mods = odoo_facts::read_all(&ws.paths.repo);
        let _g = ws.brain_mu.lock().await;
        for m in &mods {
            brain::write(
                &ws.paths.brain(),
                &format!("modules/{}.md", m.name),
                &brain::module_note(m),
            )?;
        }
        brain::commit(&ws.paths.brain(), "brain: refresh code facts").await?;
        drop(_g);
        self.emit(Event::BrainChanged {
            workspace: ws.id.clone(),
        });
        Ok(mods.len())
    }

    // ---------- skills ----------

    pub async fn skills(&self, wid: Option<&str>) -> Vec<skills::Skill> {
        let repo = match wid {
            Some(w) => self.ws(w).await.ok().map(|ws| ws.paths.repo.clone()),
            None => None,
        };
        skills::list(repo.as_deref())
    }

    pub async fn install_skill(&self, source: &str, force: bool) -> R<skills::InstallResult> {
        skills::install(source, force).await
    }

    pub fn home(&self) -> &Path {
        &self.0.home
    }
}
