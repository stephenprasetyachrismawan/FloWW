//! Local HTTP API + server-sent events + the built web UI.
//!
//! Security model (the API can start AI sessions that edit code):
//! - Listens on loopback only.
//! - Rejects requests whose `Host` is not this loopback address (DNS rebinding).
//! - Every `/api` call needs the per-launch random token, sent as
//!   `Authorization: Bearer <token>` (or `?token=` for the event stream,
//!   because EventSource cannot set headers).
//! - No CORS headers are sent, so other websites cannot read responses, and
//!   the custom header forces a preflight they cannot pass.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, Query, Request, State};
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::middleware::{self, Next};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use floww_engine::Engine;
use floww_engine::engine::{AddWorkspace, NewRequest, SetAi, Settings};
use floww_engine::model::TestResult;
use futures_util::Stream;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

#[derive(Clone)]
pub struct AppState {
    pub engine: Engine,
    pub token: String,
    pub port: u16,
    /// Origins allowed to call the API cross-origin. Only the desktop app's
    /// own webview origins; never any website.
    pub cors_origins: Vec<String>,
}

/// Origins of the Tauri webview on Linux/macOS and Windows.
pub const TAURI_ORIGINS: &[&str] = &[
    "tauri://localhost",
    "http://tauri.localhost",
    "https://tauri.localhost",
];

pub fn new_token() -> String {
    use rand::Rng;
    let bytes: [u8; 24] = rand::rng().random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

struct ApiError(String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, Json(json!({ "error": self.0 }))).into_response()
    }
}

impl From<String> for ApiError {
    fn from(s: String) -> Self {
        ApiError(s)
    }
}

type Api = Result<Json<Value>, ApiError>;

fn ok<T: serde::Serialize>(v: T) -> Api {
    Ok(Json(
        serde_json::to_value(v).map_err(|e| ApiError(e.to_string()))?,
    ))
}

// ---------- guards ----------

fn host_ok(headers: &HeaderMap, port: u16) -> bool {
    let Some(host) = headers.get(header::HOST).and_then(|h| h.to_str().ok()) else {
        return false;
    };
    host == format!("127.0.0.1:{port}")
        || host == format!("localhost:{port}")
        || host == format!("[::1]:{port}")
}

fn cors_headers(res: &mut Response, origin: &str) {
    let h = res.headers_mut();
    if let Ok(v) = origin.parse() {
        h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, v);
    }
    h.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        header::HeaderValue::from_static("authorization, content-type"),
    );
    h.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        header::HeaderValue::from_static("GET, POST, PUT, DELETE"),
    );
    h.insert(header::VARY, header::HeaderValue::from_static("Origin"));
}

async fn guard(State(st): State<AppState>, req: Request, next: Next) -> Response {
    if !host_ok(req.headers(), st.port) {
        return (StatusCode::FORBIDDEN, "FloWW only answers on 127.0.0.1").into_response();
    }
    let origin = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|o| o.to_str().ok())
        .filter(|o| st.cors_origins.iter().any(|a| a == o))
        .map(str::to_string);
    if req.method() == axum::http::Method::OPTIONS {
        let mut res = StatusCode::NO_CONTENT.into_response();
        if let Some(o) = &origin {
            cors_headers(&mut res, o);
        }
        return res;
    }
    let mut res = guarded(&st, req, next).await;
    if let Some(o) = &origin {
        cors_headers(&mut res, o);
    }
    res
}

async fn guarded(st: &AppState, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    if path.starts_with("/api/") {
        let bearer = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .map(str::to_string);
        let query_token = req.uri().query().and_then(|q| {
            q.split('&')
                .find_map(|kv| kv.strip_prefix("token="))
                .map(str::to_string)
        });
        let allowed_query = path == "/api/events";
        let given = bearer.or(if allowed_query { query_token } else { None });
        if !given.map(|t| constant_eq(&t, &st.token)).unwrap_or(false) {
            return (StatusCode::UNAUTHORIZED, Json(json!({ "error": "missing or wrong token. Open the link FloWW printed when it started." }))).into_response();
        }
    }
    next.run(req).await
}

fn constant_eq(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

// ---------- static UI ----------

#[derive(rust_embed::Embed)]
#[folder = "../../dist"]
#[allow_missing = true]
struct Ui;

async fn static_file(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    let file = Ui::get(path).or_else(|| Ui::get("index.html"));
    match file {
        Some(f) => {
            let name = if Ui::get(path).is_some() {
                path
            } else {
                "index.html"
            };
            let mime = mime_guess::from_path(name).first_or_octet_stream();
            let cache = if name.starts_with("assets/") {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            };
            Response::builder()
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(header::CACHE_CONTROL, cache)
                .header("X-Content-Type-Options", "nosniff")
                .header("Referrer-Policy", "no-referrer")
                .header(
                    "Content-Security-Policy",
                    "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src 'self'; frame-ancestors 'none'",
                )
                .body(Body::from(f.data.into_owned()))
                .expect("valid response")
        }
        None => (
            StatusCode::NOT_FOUND,
            "The web UI is not built. Run `pnpm build` and rebuild floww-web.",
        )
            .into_response(),
    }
}

// ---------- handlers ----------

async fn app(State(st): State<AppState>) -> Api {
    ok(st.engine.app_view().await)
}

async fn set_ai(State(st): State<AppState>, Json(b): Json<SetAi>) -> Api {
    st.engine.set_ai(b).await?;
    ok(json!({}))
}

#[derive(Deserialize)]
struct TestAi {
    model: Option<String>,
}

async fn test_ai(State(st): State<AppState>, Json(b): Json<TestAi>) -> Api {
    ok(json!({ "message": st.engine.test_ai(b.model).await? }))
}

async fn models(State(st): State<AppState>) -> Api {
    ok(st.engine.models().await)
}

#[derive(Deserialize)]
struct PathBody {
    path: String,
}

async fn detect(State(st): State<AppState>, Json(b): Json<PathBody>) -> Api {
    ok(st.engine.detect(&b.path).await?)
}

async fn add_ws(State(st): State<AppState>, Json(b): Json<AddWorkspace>) -> Api {
    ok(json!({ "id": st.engine.add_workspace(b).await? }))
}

async fn get_ws(State(st): State<AppState>, Path(ws): Path<String>) -> Api {
    ok(st.engine.workspace_view(&ws).await?)
}

async fn del_ws(State(st): State<AppState>, Path(ws): Path<String>) -> Api {
    st.engine.remove_workspace(&ws).await?;
    ok(json!({}))
}

async fn settings(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    Json(b): Json<Settings>,
) -> Api {
    st.engine.save_settings(&ws, b).await?;
    ok(st.engine.workspace_view(&ws).await?)
}

#[derive(Deserialize)]
struct SkillsBody {
    skills: Vec<String>,
}

async fn step_skills(
    State(st): State<AppState>,
    Path((ws, step)): Path<(String, String)>,
    Json(b): Json<SkillsBody>,
) -> Api {
    st.engine.set_step_skills(&ws, &step, b.skills).await?;
    ok(json!({}))
}

async fn list_prds(State(st): State<AppState>, Path(ws): Path<String>) -> Api {
    ok(st.engine.list_prds(&ws).await?)
}

async fn create_prd(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    Json(b): Json<NewRequest>,
) -> Api {
    ok(json!({ "id": st.engine.create_request(&ws, b).await? }))
}

async fn get_prd(State(st): State<AppState>, Path((ws, id)): Path<(String, String)>) -> Api {
    ok(st.engine.get_prd(&ws, &id).await?)
}

async fn prd_html(State(st): State<AppState>, Path((ws, id)): Path<(String, String)>) -> Api {
    let p = st.engine.get_prd(&ws, &id).await?;
    let md = p.prd_markdown.unwrap_or_default();
    ok(json!({ "html": floww_engine::markdown_to_html(&md), "markdown": md }))
}

async fn prd_diff(State(st): State<AppState>, Path((ws, id)): Path<(String, String)>) -> Api {
    ok(st.engine.diff(&ws, &id).await?)
}

async fn prd_log(State(st): State<AppState>, Path((ws, id)): Path<(String, String)>) -> Api {
    ok(json!({ "log": st.engine.odoo_log(&ws, &id).await? }))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ActionBody {
    answer: Option<String>,
    own: bool,
    markdown: Option<String>,
    text: Option<String>,
    index: Option<usize>,
    result: Option<TestResult>,
    clear: bool,
    note: Option<String>,
    message: Option<String>,
    title: Option<String>,
    body: Option<String>,
}

async fn action(
    State(st): State<AppState>,
    Path((ws, id, action)): Path<(String, String, String)>,
    body: Option<Json<ActionBody>>,
) -> Api {
    let b = body.map(|Json(b)| b).unwrap_or_default();
    let e = &st.engine;
    let mut extra = json!({});
    match action.as_str() {
        "answer" => {
            e.answer(&ws, &id, b.answer.unwrap_or_default(), b.own)
                .await?
        }
        "finish-grill" => e.finish_grill(&ws, &id).await?,
        "reopen-grill" => e.reopen_grill(&ws, &id).await?,
        "edit-prd" => e.edit_prd(&ws, &id, b.markdown.unwrap_or_default()).await?,
        "approve" => e.approve(&ws, &id).await?,
        "pause" => e.pause(&ws, &id).await?,
        "resume" => e.resume(&ws, &id).await?,
        "hint" => e.hint(&ws, &id, b.text.unwrap_or_default()).await?,
        "start-odoo" => e.start_odoo(&ws, &id).await?,
        "stop-odoo" => e.stop_odoo(&ws, &id).await?,
        "test" => {
            let idx = b.index.ok_or_else(|| "missing test index".to_string())?;
            if b.note.is_some() {
                e.mark_test(&ws, &id, idx, None, b.note).await?
            } else {
                e.mark_test(&ws, &id, idx, if b.clear { None } else { b.result }, None)
                    .await?
            }
        }
        "feedback" => e.send_feedback(&ws, &id).await?,
        "all-good" => e.all_good(&ws, &id).await?,
        "commit" => e.commit(&ws, &id, b.message.unwrap_or_default()).await?,
        "push" => e.push(&ws, &id).await?,
        "pr" => {
            e.create_pr(
                &ws,
                &id,
                b.title.unwrap_or_default(),
                b.body.unwrap_or_default(),
            )
            .await?
        }
        "skip-pr" => e.skip_pr(&ws, &id).await?,
        "check-pr" => extra = json!({ "state": e.check_pr(&ws, &id).await? }),
        "take-over" => extra = json!({ "paused": e.take_over(&ws, &id).await? }),
        other => return Err(ApiError(format!("unknown action \"{other}\""))),
    }
    let prd = e.get_prd(&ws, &id).await?;
    let mut v = serde_json::to_value(prd).map_err(|e| ApiError(e.to_string()))?;
    if let (Some(obj), Some(x)) = (v.as_object_mut(), extra.as_object()) {
        for (k, val) in x {
            obj.insert(format!("_{k}"), val.clone());
        }
    }
    Ok(Json(v))
}

async fn lock(State(st): State<AppState>, Path(ws): Path<String>) -> Api {
    ok(st.engine.lock_state(&ws).await?)
}

#[derive(Deserialize)]
struct FolderQ {
    folder: String,
}

async fn brain_list(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    Query(q): Query<FolderQ>,
) -> Api {
    ok(st.engine.brain_list(&ws, &q.folder).await?)
}

#[derive(Deserialize)]
struct NoteQ {
    path: String,
}

async fn brain_note(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    Query(q): Query<NoteQ>,
) -> Api {
    ok(st.engine.brain_note(&ws, &q.path).await?)
}

async fn brain_confirm(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    Json(b): Json<PathBody>,
) -> Api {
    st.engine.brain_confirm(&ws, &b.path).await?;
    ok(json!({}))
}

async fn brain_sync(State(st): State<AppState>, Path(ws): Path<String>) -> Api {
    st.engine.brain_sync(&ws).await?;
    ok(json!({}))
}

async fn brain_refresh(State(st): State<AppState>, Path(ws): Path<String>) -> Api {
    ok(json!({ "modules": st.engine.refresh_facts(&ws).await? }))
}

async fn pending(State(st): State<AppState>, Path(ws): Path<String>) -> Api {
    ok(st.engine.pending(&ws).await?)
}

#[derive(Deserialize)]
struct PendingBody {
    path: String,
    accept: bool,
    text: Option<String>,
}

async fn resolve_pending(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    Json(b): Json<PendingBody>,
) -> Api {
    st.engine
        .resolve_pending(&ws, &b.path, b.accept, b.text)
        .await?;
    ok(json!({}))
}

async fn graph(State(st): State<AppState>, Path(ws): Path<String>) -> Api {
    ok(st.engine.graph(&ws).await?)
}

#[derive(Deserialize)]
struct WsQ {
    ws: Option<String>,
}

async fn skills(State(st): State<AppState>, Query(q): Query<WsQ>) -> Api {
    ok(st.engine.skills(q.ws.as_deref()).await)
}

#[derive(Deserialize)]
struct InstallBody {
    source: String,
    #[serde(default)]
    force: bool,
}

async fn install_skill(State(st): State<AppState>, Json(b): Json<InstallBody>) -> Api {
    ok(st.engine.install_skill(&b.source, b.force).await?)
}

async fn events(
    State(st): State<AppState>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let stream = BroadcastStream::new(st.engine.subscribe()).filter_map(|ev| {
        let ev = ev.ok()?;
        let data = serde_json::to_string(&ev).ok()?;
        Some(Ok(SseEvent::default().data(data)))
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

pub fn router(state: AppState) -> Router {
    let w = "/api/workspaces/{ws}";
    Router::new()
        .route("/api/app", get(app))
        .route("/api/ai", post(set_ai))
        .route("/api/ai/test", post(test_ai))
        .route("/api/models", get(models))
        .route("/api/detect", post(detect))
        .route("/api/workspaces", post(add_ws))
        .route(w, get(get_ws).delete(del_ws))
        .route(&format!("{w}/settings"), put(settings))
        .route(&format!("{w}/skills/{{step}}"), put(step_skills))
        .route(&format!("{w}/prds"), get(list_prds).post(create_prd))
        .route(&format!("{w}/prds/{{id}}"), get(get_prd))
        .route(&format!("{w}/prds/{{id}}/html"), get(prd_html))
        .route(&format!("{w}/prds/{{id}}/diff"), get(prd_diff))
        .route(&format!("{w}/prds/{{id}}/odoo-log"), get(prd_log))
        .route(&format!("{w}/prds/{{id}}/do/{{action}}"), post(action))
        .route(&format!("{w}/lock"), get(lock))
        .route(&format!("{w}/brain"), get(brain_list))
        .route(&format!("{w}/brain/note"), get(brain_note))
        .route(&format!("{w}/brain/confirm"), post(brain_confirm))
        .route(&format!("{w}/brain/sync"), post(brain_sync))
        .route(&format!("{w}/brain/refresh"), post(brain_refresh))
        .route(&format!("{w}/pending"), get(pending).post(resolve_pending))
        .route(&format!("{w}/graph"), get(graph))
        .route("/api/skills", get(skills))
        .route("/api/skills/install", post(install_skill))
        .route("/api/events", get(events))
        .fallback(static_file)
        .layer(middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state)
}

/// Bind on loopback and serve until the process ends. Port 0 picks a free port.
pub async fn serve(
    engine: Engine,
    port: u16,
    token: String,
) -> std::io::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    serve_with_origins(engine, port, token, Vec::new()).await
}

/// Same as `serve`, also allowing the given webview origins (the desktop app).
pub async fn serve_with_origins(
    engine: Engine,
    port: u16,
    token: String,
    cors_origins: Vec<String>,
) -> std::io::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    let addr = listener.local_addr()?;
    let app = router(AppState {
        engine,
        token,
        port: addr.port(),
        cors_origins,
    });
    let handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("server stopped: {e}");
        }
    });
    Ok((addr, handle))
}
