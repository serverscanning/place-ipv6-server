mod canvas;
mod ping_receiver;

use canvas::CanvasState;

use std::{
    convert::Infallible,
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use async_fn_stream::fn_stream;
use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive},
        Sse,
    },
    routing::get,
    Router,
};

use clap::Parser;
use color_eyre::{eyre::Context, Result};
use futures_util::Stream;
use tower_http::{services::ServeDir, trace::TraceLayer};

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
        .route("/events", get(events))
        .fallback_service(ServeDir::new("./static"))
        .with_state(canvas_state)
        .layer(TraceLayer::new_for_http());
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

async fn events(
    State(canvas_state): State<Arc<CanvasState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = fn_stream(|emitter| async move {
        /*let mut i = 0;
        loop {
            i += 1;
            tokio::time::sleep(Duration::from_millis(1000)).await;
            emitter
                .emit(Ok(Event::default().event("i").data(i.to_string())))
                .await;
            if i > 10 {
                break;
            }
        }*/
        let encoded_canvas = canvas_state.read_encoded_canvas().await;
        emitter
            .emit(Ok(Event::default()
                .event("canvas_image")
                .data(encoded_canvas.get_encoded_data())))
            .await;
        let mut event_receiver = encoded_canvas.subscribe();
        drop(encoded_canvas);

        while let Ok(encoded_data) = event_receiver.recv().await {
            emitter
                .emit(Ok(Event::default()
                    .event("canvas_image")
                    .data(encoded_data)))
                .await;
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
