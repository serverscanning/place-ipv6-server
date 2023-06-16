mod canvas;
mod canvas_processor;
mod cli_args;
mod ping_listener;

use canvas::CanvasState;
use cli_args::CliArgs;

use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use axum::{
    extract::{
        ws::{Message, WebSocket},
        ConnectInfo, State, WebSocketUpgrade,
    },
    http::header,
    response::{AppendHeaders, IntoResponse, Response},
    routing::get,
    Router,
};
use clap::Parser;
use color_eyre::{eyre::Context, Result};
use serde::{Deserialize, Serialize};
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, DefaultOnFailure, TraceLayer},
};

#[macro_use]
extern crate tracing;

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
    let (pixel_sender, pixel_receiver) = std::sync::mpsc::channel();
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

    let app = Router::new()
        .route("/ws", get(get_ws))
        .route("/canvas.png", get(get_canvas))
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
        "Starting webserver on {} port {}...",
        webserver_addr.ip(),
        webserver_addr.port()
    );

    axum::Server::bind(&webserver_addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;
    Ok(())
}

async fn get_canvas(State(canvas_state): State<Arc<CanvasState>>) -> impl IntoResponse {
    (
        AppendHeaders([(header::CONTENT_TYPE, "image/png")]),
        canvas_state.read_encoded_full_canvas().await.get_encoded(),
    )
}

async fn get_ws(
    ws: WebSocketUpgrade,
    State(canvas_state): State<Arc<CanvasState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Response {
    ws.on_upgrade(move |ws| on_websocket_upgrade(ws, canvas_state, addr))
}

async fn on_websocket_upgrade(mut ws: WebSocket, canvas_state: Arc<CanvasState>, addr: SocketAddr) {
    if let Err(err) = websocket_connection(&mut ws, canvas_state, addr).await {
        warn!("Websocket: Connection to {addr} failed: {err}");
        ws.close().await.ok();
    }
}

/// Client -> Server
#[derive(Deserialize)]
#[serde(tag = "request", rename_all = "snake_case")]
enum WsRequest {
    GetFullCanvasOnce,
    DeltaCanvasStream { enabled: bool },
    PpsUpdates { enabled: bool },
}

/// Server -> Client
#[derive(Serialize)]
#[serde(tag = "message", rename_all = "snake_case")]
enum WsMessage {
    PpsUpdate { pps: usize },
}

async fn websocket_connection(
    ws: &mut WebSocket,
    canvas_state: Arc<CanvasState>,
    addr: SocketAddr,
) -> Result<()> {
    info!("Websocket: {addr} connected");
    let mut delta_canvas_receiver = canvas_state.read_encoded_delta_canvas().await.subscribe();
    let mut pps_receiver = canvas_state.subscribe_to_pps();

    let mut delta_canvas_stream_enabled = false;
    let mut pps_updates_enabled = false;

    loop {
        tokio::select! {
            encoded_delta_canvas_res = delta_canvas_receiver.recv() => {
                if delta_canvas_stream_enabled {
                    ws.send(Message::Binary(encoded_delta_canvas_res.context("Receive encoded delta canvas")?)).await.context("Send encoded delta canvas")?;
                }
            }
            pps_res = pps_receiver.recv() => {
                if pps_updates_enabled {
                    let message = WsMessage::PpsUpdate { pps: pps_res.context("Receive pps update")? };
                    ws.send(Message::Text(serde_json::to_string(&message).context("Encode pps update")?)).await.context("Send pps update")?;
                }
            }
            maybe_ws_message_res = ws.recv() => {
                if maybe_ws_message_res.is_none() {
                    info!("Websocket: {addr} closed connection");
                    return Ok(())
                }
                let ws_message = maybe_ws_message_res.unwrap().context("Websocket message")?;

                match ws_message {
                    Message::Text(text) => {
                        let request: WsRequest = serde_json::from_str(&text).context("Parsing received text as WsRequest")?;
                        match request {
                            WsRequest::GetFullCanvasOnce => {
                                debug!("Websocket: {addr} requested a full canvas frame");
                                ws.send(Message::Binary(
                                        canvas_state.read_encoded_full_canvas().await.get_encoded(),
                                ))
                                .await?;
                            },
                            WsRequest::DeltaCanvasStream { enabled } => {
                                delta_canvas_stream_enabled = enabled;
                                debug!("Websocket: {addr} {} delta canvas frames", if enabled { "enabled" } else { "disabled" })
                            },
                            WsRequest::PpsUpdates { enabled } => {
                                pps_updates_enabled = enabled;
                                debug!("Websocket: {addr} {} pps updates", if enabled { "enabled" } else { "disabled" })
                            },
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
