//! Defines CLI Arguments, help texts, etc.

use clap::Parser;

fn max_canvas_fps_range(s: &str) -> Result<u16, String> {
    clap_num::number_range(s, 1, 1000)
}

/// Listen for IPv6 pings and use them to draw on a canvas available on a webserver.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    /// Name of the interface on which to sniff on for pings
    pub interface: String,

    /// How often the canvas is allowed to update per second max.
    #[arg(short = 'f', long, value_parser=max_canvas_fps_range, default_value = "10")]
    pub max_canvas_fps: u16,

    /// Require valid imcpv6 ping checksums in oder to accept pixel updates.
    #[arg(short, long, action)]
    pub require_valid_checksum: bool,

    /// What address the webserver should bind to
    #[arg(short, long, default_value = "::")]
    pub bind: String,

    /// What port the webserver should bind to
    #[arg(short, long, default_value = "8080")]
    pub port: u16,

    /// The first 4 segements to be displayed in frontends for the user. Example: "aaaa:bbbb:cccc:dddd"
    #[arg(short = 'P', long)]
    pub public_prefix: Option<String>,
}
