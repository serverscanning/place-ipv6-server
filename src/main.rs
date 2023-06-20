//! Main method (obviously), most of webserver routes and kicking off other threads.

mod canvas;
mod canvas_processor;
mod cli_args;
mod ping_listener;
mod websocket_handler;

use crate::canvas::CANVASH;
use crate::canvas::CANVASW;
use canvas::CanvasState;
use cli_args::CliArgs;
use serde::{Deserialize, Serialize};

use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    extract::{Query, State},
    http::header,
    response::{AppendHeaders, IntoResponse},
    routing::get,
    Json, Router,
};
use clap::Parser;
use color_eyre::{eyre::Context, Result};
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, DefaultOnFailure, TraceLayer},
};

#[macro_use]
extern crate tracing;

#[derive(Serialize, Clone)]
struct ServerConfig {
    public_prefix: Option<String>,
    width: u16,
    height: u16,
}

static SERVER_CONFIG: Mutex<ServerConfig> = Mutex::new(ServerConfig {
    public_prefix: None,
    height: CANVASH,
    width: CANVASW,
});

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    if std::env::var("RUST_LOG").is_err() {
        // Set default logging level if none specified using the environment variable "RUST_LOG"
        std::env::set_var("RUST_LOG", "debug,hyper=info")
    }
    tracing_subscriber::fmt::init();

    let canvas_state = Arc::new(CanvasState::default());
    let canvas_state_clone = canvas_state.clone();
    let (pixel_sender, pixel_receiver) = crossbeam_channel::unbounded();
    std::thread::Builder::new()
        .name("Ping-Listener".to_owned())
        .spawn(move || {
            if let Err(err) = ping_listener::run_ping_listener(
                &args.interface,
                args.require_valid_checksum,
                pixel_sender,
            ) {
                error!("Ping-Listener crashed: {err:#}\nIf this error is permission related either run this program as root/admin or, on linux, give it the capability CAP_NET_RAW (e.g. \"sudo setcap CAP_NET_RAW+ep ./path/to/binary\").");
                std::process::exit(1);
            }
        })?;
    std::thread::Builder::new()
        .name("Canvas-Processor".to_owned())
        .spawn(move || {
            if let Err(err) = canvas_processor::run_canvas_processor(
                pixel_receiver,
                canvas_state_clone,
                Duration::from_nanos(1_000_000_000 / args.max_canvas_fps as u64),
            ) {
                error!("Canvas-Processor crashed: {err:#}");
                std::process::exit(1);
            }
        })?;

    SERVER_CONFIG.lock().unwrap().public_prefix = args.public_prefix.clone();

    let app = Router::new()
        .route("/ws", get(websocket_handler::get_ws))
        .route("/canvas.png", get(get_canvas))
        .route("/serverconfig.json", get(get_server_config))
        .fallback_service(ServeDir::new("./static"))
        .with_state(canvas_state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new())
                .on_failure(DefaultOnFailure::new()),
        );
    let webserver_addr = SocketAddr::new(
        IpAddr::from_str(&args.bind).context("Parsing ip to bind webserver to")?,
        args.port,
    );

    info!(
        "Starting webserver on {} port {} for {}x{} canvas...",
        webserver_addr.ip(),
        webserver_addr.port(),
        CANVASW,
        CANVASH
    );

    axum::Server::bind(&webserver_addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;
    Ok(())
}

#[derive(Deserialize)]
struct CanvasQueryParams {
    #[serde(default)]
    allow_cache: bool,
}

async fn get_canvas(
    State(canvas_state): State<Arc<CanvasState>>,
    Query(params): Query<CanvasQueryParams>,
) -> impl IntoResponse {
    let mut headers = vec![(header::CONTENT_TYPE, "image/png")];
    if !params.allow_cache {
        headers.push((header::CACHE_CONTROL, "no-store"));
    }
    (
        AppendHeaders(headers),
        canvas_state.read_encoded_full_canvas().await.get_encoded(),
    )
}

async fn get_server_config() -> Json<ServerConfig> {
    Json(SERVER_CONFIG.lock().unwrap().clone())
}
