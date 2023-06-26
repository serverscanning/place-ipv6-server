//! Main method (obviously), most of webserver routes and kicking off other threads.

mod canvas;
mod canvas_processor;
mod cli_args;
#[cfg(feature = "per_user_pps")]
mod per_user_pps;
mod ping_listener;
mod websocket_handler;

use crate::canvas::CANVASH;
use crate::canvas::CANVASW;
use axum::extract::ConnectInfo;
use axum::http::HeaderMap;
use axum::{
    extract::{Query, State},
    http::header,
    response::{AppendHeaders, IntoResponse},
    routing::get,
    Json, Router,
};
use canvas::CanvasState;
use clap::Parser;
use cli_args::CliArgs;
use color_eyre::{eyre::Context, Result};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use std::net::Ipv6Addr;
use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};
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
    built_with_per_user_pps_support: bool,
    #[serde(skip)]
    trusted_proxy_ranges: Vec<IpNet>,
    #[serde(skip)]
    trusted_cloudflare_ranges: Vec<IpNet>,
}

static SERVER_CONFIG: Mutex<ServerConfig> = Mutex::new(ServerConfig {
    public_prefix: None,
    height: CANVASH,
    width: CANVASW,
    built_with_per_user_pps_support: if cfg!(feature = "per_user_pps") {
        true
    } else {
        false
    },
    trusted_proxy_ranges: vec![],
    trusted_cloudflare_ranges: vec![],
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
                args.nude_scan_interval,
            ) {
                error!("Canvas-Processor crashed: {err:#}");
                std::process::exit(1);
            }
        })?;

    SERVER_CONFIG.lock().unwrap().public_prefix = args.public_prefix.clone();
    SERVER_CONFIG.lock().unwrap().trusted_proxy_ranges = args.trusted_proxy_ranges.clone();
    // TODO: Add automated way to retreives these ranges. Otherwise this will break at some point or be come a security hole!
    SERVER_CONFIG.lock().unwrap().trusted_cloudflare_ranges = vec![
        // https://www.cloudflare.com/ips-v6
        IpNet::from_str("2400:cb00::/32").unwrap(),
        IpNet::from_str("2606:4700::/32").unwrap(),
        IpNet::from_str("2803:f800::/32").unwrap(),
        IpNet::from_str("2405:b500::/32").unwrap(),
        IpNet::from_str("2405:8100::/32").unwrap(),
        IpNet::from_str("2a06:98c0::/29").unwrap(),
        IpNet::from_str("2c0f:f248::/32").unwrap(),
        // https://www.cloudflare.com/ips-v4
        IpNet::from_str("173.245.48.0/20").unwrap(),
        IpNet::from_str("103.21.244.0/22").unwrap(),
        IpNet::from_str("103.22.200.0/22").unwrap(),
        IpNet::from_str("103.31.4.0/22").unwrap(),
        IpNet::from_str("141.101.64.0/18").unwrap(),
        IpNet::from_str("108.162.192.0/18").unwrap(),
        IpNet::from_str("190.93.240.0/20").unwrap(),
        IpNet::from_str("188.114.96.0/20").unwrap(),
        IpNet::from_str("197.234.240.0/22").unwrap(),
        IpNet::from_str("198.41.128.0/17").unwrap(),
        IpNet::from_str("162.158.0.0/15").unwrap(),
        IpNet::from_str("104.16.0.0/13").unwrap(),
        IpNet::from_str("104.24.0.0/14").unwrap(),
        IpNet::from_str("172.64.0.0/13").unwrap(),
        IpNet::from_str("131.0.72.0/22").unwrap(),
    ];

    let app = Router::new()
        .route("/ws", get(websocket_handler::get_ws))
        .route("/canvas.png", get(get_canvas))
        .route("/serverconfig.json", get(get_server_config))
        .route("/my_user_id", get(get_my_user_id))
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
        "Will trust proxies from these Ranges (in addition to CloudFlare's) to not lie about the source ip: {:?}",
        args.trusted_proxy_ranges
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

#[derive(Serialize)]
#[serde(untagged)]
enum MyUserIdResponse {
    #[cfg(feature = "per_user_pps")]
    Success {
        ip: Ipv6Addr,
        user_id: u64,
    },
    Error {
        error: String,
    },
    #[cfg_attr(not(feature = "my_feature"), allow(unused))]
    ErrorWithIp {
        ip: Ipv6Addr,
        error: String,
    },
}

#[cfg_attr(not(feature = "my_feature"), allow(unused_variables))]
async fn get_my_user_id(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Json<MyUserIdResponse> {
    // TODO: Support cloudflare headers (to not require querying the raw addr each time)
    // TODO: Return proper status code for errors. Doesn't work with conditional compilation and using (Json<...>, StatusCode) for some reason!

    #[cfg(not(feature = "per_user_pps"))]
    return Json(MyUserIdResponse::Error {
        error: String::from("This server was not built with the per_user_pps feature enabled!"),
    });
    #[cfg(feature = "per_user_pps")]
    return {
        let user_ip = match get_real_ip(addr.ip(), &headers) {
            IpAddr::V4(_) => None,
            IpAddr::V6(ipv6_addr) => Some(ipv6_addr.clone()),
        };
        if let Some(user_ip) = user_ip {
            let mut pps_users = per_user_pps::PPS_USERS.lock().unwrap();
            let maybe_user_info = per_user_pps::find_user_info_data(&mut pps_users, user_ip);
            if let Some(user_info) = maybe_user_info {
                Json(MyUserIdResponse::Success {
                    ip: user_ip,
                    user_id: user_info.get_user_id().id,
                })
            } else {
                Json(MyUserIdResponse::ErrorWithIp { ip: user_ip, error: String::from("Didn't find any user id for your ip. Either you never pinged this server or it was too long ago.") })
            }
        } else {
            Json(MyUserIdResponse::Error {
                error: String::from("You're not using an IPv6 address!"),
            })
        }
    };
}

pub fn get_real_ip(connected_from_ip_addr: IpAddr, headers: &HeaderMap) -> IpAddr {
    find_real_ip_from_headers(connected_from_ip_addr, headers).unwrap_or(connected_from_ip_addr)
}

fn find_real_ip_from_headers(
    connected_from_ip_addr: IpAddr,
    headers: &HeaderMap,
) -> Option<IpAddr> {
    let server_config = SERVER_CONFIG.lock().unwrap();
    if !server_config
        .trusted_proxy_ranges
        .iter()
        .any(|range| range.contains(&connected_from_ip_addr))
        && !server_config
            .trusted_cloudflare_ranges
            .iter()
            .any(|range| range.contains(&connected_from_ip_addr))
    {
        // We don't trust any headers this person gives us that could change the real ip
        return None;
    }
    drop(server_config);

    if let Some(x_forwarded_for) = headers
        .get("X-Forwarded-For")
        .or(headers.get("x-forwarded-for"))
    {
        let x_forwarded_for = x_forwarded_for.to_str().ok()?;
        return Some(IpAddr::from_str(x_forwarded_for.replace(" ", "").split(",").next()?).ok()?);
    }
    if let Some(x_real_ip) = headers.get("X-Real-IP").or(headers.get("x-real-ip")) {
        let x_real_ip = x_real_ip.to_str().ok()?;
        return Some(IpAddr::from_str(x_real_ip).ok()?);
    }
    todo!()
}
