//! Handles connected websockets and defines how it is used.

use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{
        ws::{Message, WebSocket},
        ConnectInfo, State, WebSocketUpgrade,
    },
    http::HeaderMap,
    response::Response,
};
use color_eyre::{eyre::Context, Result};
use serde::{Deserialize, Serialize};

use crate::canvas::{CanvasState, PpsInfo};

/// Client -> Server
#[derive(Deserialize)]
#[serde(tag = "request", rename_all = "snake_case")]
enum WsRequest {
    GetFullCanvasOnce,
    DeltaCanvasStream { enabled: bool },
    PpsUpdates { enabled: bool },
    WsCountUpdates { enabled: bool },
    GetWsCountUpdateOnce,
    NudityUpdates { enabled: bool },
    GetNudityUpdateOnce,
}

/// Server -> Client
#[derive(Serialize)]
#[serde(tag = "message", rename_all = "snake_case")]
enum WsMessage {
    PpsUpdate {
        #[serde(flatten)]
        pps_info: PpsInfo,
    },
    WsCountUpdate {
        ws_connections: usize,
    },
    NudityUpdate {
        is_nude: bool,
    },
}

pub async fn get_ws(
    ws: WebSocketUpgrade,
    State(canvas_state): State<Arc<CanvasState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    // This IP:Port combination will not be bogus (port may not be from real ip),
    // but should serve as a somewhat decent identifier for connections.
    let addr = SocketAddr::new(crate::get_real_ip(addr.ip(), &headers), addr.port());
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
    let _ws_tracker = canvas_state.track_new_websocket();

    let mut delta_canvas_receiver = canvas_state.read_encoded_delta_canvas().await.subscribe();
    let mut pps_receiver = canvas_state.subscribe_to_pps();
    let mut ws_count_receiver = canvas_state.subscribe_to_websocket_count();
    let mut nudity_results_receiver = canvas_state.subscribe_to_nudity_results();

    let mut delta_canvas_stream_enabled = false;
    let mut pps_updates_enabled = false;
    let mut ws_count_updates_enabled = false;
    let mut nudity_updates_enabled = false;

    loop {
        tokio::select! {
            encoded_delta_canvas_res = delta_canvas_receiver.recv() => {
                if delta_canvas_stream_enabled {
                    ws.send(Message::Binary(encoded_delta_canvas_res.context("Receive encoded delta canvas")?)).await.context("Send encoded delta canvas")?;
                }
            }
            pps_info_res = pps_receiver.recv() => {
                if pps_updates_enabled {
                    let message = WsMessage::PpsUpdate { pps_info: pps_info_res.context("Receive pps update")? };
                    ws.send(Message::Text(serde_json::to_string(&message).context("Encode pps update")?)).await.context("Send pps update")?;
                }
            }
            ws_count_res = ws_count_receiver.recv() => {
                if ws_count_updates_enabled {
                    let message = WsMessage::WsCountUpdate { ws_connections: ws_count_res.context("Receive ws count update")? };
                    ws.send(Message::Text(serde_json::to_string(&message).context("Encode ws count update")?)).await.context("Send ws count update")?;
                }
            }
            nudity_result_res = nudity_results_receiver.recv() => {
                if nudity_updates_enabled {
                    let message = WsMessage::NudityUpdate { is_nude: nudity_result_res.context("Receive nudity update")?.is_nude };
                    ws.send(Message::Text(serde_json::to_string(&message).context("Encode nudity update")?)).await.context("Send nudity update")?;
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
                            WsRequest::WsCountUpdates { enabled } => {
                                ws_count_updates_enabled = enabled;
                                debug!("Websocket: {addr} {} ws count updates", if enabled { "enabled" } else { "disabled" })
                            },
                            WsRequest::GetWsCountUpdateOnce => {
                                debug!("Websocket: {addr} requested ws count once");
                                let message = WsMessage::WsCountUpdate { ws_connections: canvas_state.websocket_count() };
                                ws.send(Message::Text(serde_json::to_string(&message).context("Encode ws count update")?)).await.context("Send ws count update")?;
                            },
                            WsRequest::NudityUpdates { enabled } => {
                                nudity_updates_enabled = enabled;
                                debug!("Websocket: {addr} {} nudity result updates", if enabled { "enabled" } else { "disabled" })
                            },
                            WsRequest::GetNudityUpdateOnce => {
                                debug!("Websocket: {addr} requested nudity result once");
                                let message = WsMessage::NudityUpdate { is_nude: canvas_state.nudity_result().await.is_nude };
                                ws.send(Message::Text(serde_json::to_string(&message).context("Encode nudity update")?)).await.context("Send nudity update")?;
                            },
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
