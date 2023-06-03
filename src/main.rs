use std::{convert::Infallible, time::Duration};

use async_fn_stream::fn_stream;
use axum::{
    response::{
        sse::{Event, KeepAlive},
        Sse,
    },
    routing::get,
    Router,
};

use color_eyre::Result;
use futures_util::Stream;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/events", get(events))
        .route("/", get(index));
    tracing::debug!("Created app. Starting server on port 8080...");

    axum::Server::bind(&"[::]:8080".parse()?)
        .serve(app.into_make_service())
        .await?;
    Ok(())
}

async fn index() -> String {
    String::from("Hello!")
}

async fn events() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = fn_stream(|emitter| async move {
        let mut i = 0;
        loop {
            i += 1;
            tokio::time::sleep(Duration::from_millis(1000)).await;
            emitter
                .emit(Ok(Event::default().event("i").data(i.to_string())))
                .await;
            if i > 10 {
                break;
            }
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
