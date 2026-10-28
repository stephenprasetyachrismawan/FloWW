//! `floww-web`: FloWW in the browser at http://127.0.0.1:7171.

use floww_engine::Engine;

const USAGE: &str = "Usage: floww-web [--port 7171] [--token <token>] [--open]

Runs FloWW's web UI on http://127.0.0.1:<port>. Open the printed link; it
contains the access token for this run. Set FLOWW_HOME to use another
config folder (default ~/.config/floww).";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "floww=info,floww_engine=info,floww_server=info".into()),
        )
        .init();

    let mut port: u16 = 7171;
    let mut token: Option<String> = std::env::var("FLOWW_TOKEN").ok();
    let mut open = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--port" | "-p" => {
                port = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| die("--port needs a number"))
            }
            "--token" => token = Some(args.next().unwrap_or_else(|| die("--token needs a value"))),
            "--open" => open = true,
            "--help" | "-h" => {
                println!("{USAGE}");
                return;
            }
            other => die(&format!("unknown option {other}\n\n{USAGE}")),
        }
    }

    let engine = match Engine::open(None).await {
        Ok(e) => e,
        Err(e) => die(&format!("could not start FloWW: {e}")),
    };
    let token = token.unwrap_or_else(floww_server::new_token);
    let (addr, handle) = match floww_server::serve(engine, port, token.clone()).await {
        Ok(v) => v,
        Err(e) => die(&format!(
            "could not listen on 127.0.0.1:{port}: {e}. Try another --port."
        )),
    };
    let url = format!("http://{addr}/?token={token}");
    println!(
        "\nFloWW is running.\n\n  Open: {url}\n\nKeep this link private: it gives full access to FloWW. Press Ctrl+C to stop.\n"
    );
    if open {
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    }
    tokio::select! {
        _ = handle => {}
        _ = tokio::signal::ctrl_c() => println!("Stopped."),
    }
}

fn die(msg: &str) -> ! {
    eprintln!("floww-web: {msg}");
    std::process::exit(2)
}
