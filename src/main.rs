mod canvas;
mod ping_receiver;

use canvas::CanvasState;

use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use axum::{
    debug_handler,
    extract::{
        ws::{Message, WebSocket},
        ConnectInfo, State, WebSocketUpgrade,
    },
    response::Response,
    routing::get,
    Router,
};

use clap::Parser;
use color_eyre::{eyre::Context, Result};
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, DefaultOnFailure, TraceLayer},
};

#[macro_use]
extern crate tracing;

fn max_canvas_fps_range(s: &str) -> std::result::Result<u16, String> {
    clap_num::number_range(s, 1, 1000)
}

#[derive(Parser)]
struct Args {
    /// Name of the interface on which to sniff on for pings
    interface: String,

    /// How often the canvas is allowed to update per second max.
    #[arg(short = 'f', long, value_parser=max_canvas_fps_range, default_value = "10")]
    max_canvas_fps: u16,

    /// Require valid imcpv6 ping checksums in oder to accept pixel updates.
    #[arg(short, long, action)]
    require_valid_checksum: bool,

    /// What address the webserver should bind to
    #[arg(short, long, default_value = "::")]
    bind: String,

    /// What port the webserver should bind to
    #[arg(short, long, default_value = "8080")]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    tracing_subscriber::fmt::init();

    let canvas_state = Arc::new(CanvasState::default());
    let canvas_state_clone = canvas_state.clone();
    let (pixel_sender, pixel_receiver) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("Pixel-Recveiver".to_owned())
        .spawn(move || {
            if let Err(err) = ping_receiver::run_pixel_receiver(
                &args.interface,
                args.require_valid_checksum,
                pixel_sender,
            ) {
                error!("Pixel-Receiver crashed: {err:#}");
                std::process::exit(1);
            }
        })?;
    std::thread::Builder::new()
        .name("Pixel-Processor".to_owned())
        .spawn(move || {
            if let Err(err) = ping_receiver::run_pixel_processor(
                pixel_receiver,
                canvas_state_clone,
                Duration::from_nanos(1_000_000_000 / args.max_canvas_fps as u64),
            ) {
                error!("Pixel-Processor crashed: {err:#}");
                std::process::exit(1);
            }
        })?;

    let app = Router::new()
        .route("/ws", get(get_ws))
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
        .serve(app.into_make_service())
        .await?;
    Ok(())
}

#[debug_handler]
async fn get_ws(
    ws: WebSocketUpgrade,
    State(canvas_state): State<Arc<CanvasState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Response {
    info!("HI");
    ws.on_failed_upgrade(|err: axum::Error| info!("Failed upgrade: {err:?}"))
        .on_upgrade(move |ws| on_websocket_upgrade(ws, canvas_state, addr))
        .map(|f| {
            info!("Resp: {:?}", f);
            f
        })
}

async fn on_websocket_upgrade(mut ws: WebSocket, canvas_state: Arc<CanvasState>, addr: SocketAddr) {
    info!("AC");
    if let Err(err) = websocket_connection(&mut ws, canvas_state, addr).await {
        warn!("Websocket: Connection to {addr} failed: {err}");
        ws.close().await.ok();
    }
}

async fn websocket_connection(
    ws: &mut WebSocket,
    canvas_state: Arc<CanvasState>,
    addr: SocketAddr,
) -> Result<()> {
    info!("Websocket: {addr} connected");
    let mut delta_canvas_receiver = canvas_state.read_encoded_delta_canvas().await.subscribe();
    ws.send(Message::Binary(
        canvas_state.read_encoded_full_canvas().await.get_encoded(),
    ))
    .await?;

    loop {
        tokio::select! {
            encoded_delta_canvas_res = delta_canvas_receiver.recv() => {
                ws.send(Message::Binary(encoded_delta_canvas_res.context("Receive encoded delta canvas")?)).await.context("Send encoded delta canvas")?;
            }
            maybe_ws_message_res = ws.recv() => {
                if maybe_ws_message_res.is_none() {
                    info!("Websocket: {addr} closed connection");
                    return Ok(())
                }
                let ws_message = maybe_ws_message_res.unwrap().context("Websocket message")?;

                match ws_message {
                    Message::Text(_text) => {
                        // TODO: Allow requesting a full canvas frame
                        //       This is to allow js to stop processing the delta canvases
                        //       if the tab is hidden and use the full canvas to get the
                        //       current state again.
                        info!("Websocket: Got a text message from {addr}")
                    }
                    _ => {}
                }
            }
        }
    }
}
