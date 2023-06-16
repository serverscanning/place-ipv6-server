//! Handles connected websockets and defines how it is used.

use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{
        ws::{Message, WebSocket},
        ConnectInfo, State, WebSocketUpgrade,
    },
    response::Response,
};
use color_eyre::{eyre::Context, Result};
use serde::{Deserialize, Serialize};

use crate::canvas::CanvasState;

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

pub async fn get_ws(
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
