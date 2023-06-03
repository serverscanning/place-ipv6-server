mod canvas;
mod ping_receiver;

use canvas::CanvasState;

use std::{convert::Infallible, sync::Arc};

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

use color_eyre::Result;
use futures_util::Stream;
use tower_http::{services::ServeDir, trace::TraceLayer};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let canvas_state = Arc::new(CanvasState::default());
    let canvas_state_clone = canvas_state.clone();
    std::thread::Builder::new()
        .name("Ping-Receiver".to_owned())
        .spawn(|| {
            if let Err(err) = ping_receiver::start_listener(canvas_state_clone) {
                tracing::error!("Ping-Receiver crashed: {err:#}");
            }
        })?;

    let app = Router::new()
        .route("/events", get(events))
        .fallback_service(ServeDir::new("./static"))
        .with_state(canvas_state)
        .layer(TraceLayer::new_for_http());
    tracing::debug!("Created app. Starting server on port 8080...");

    tokio::task::spawn(async move {});

    axum::Server::bind(&"[::]:8080".parse()?)
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
