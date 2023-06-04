use color_eyre::Result;
use image::{DynamicImage, Rgb};
use mac_address::MacAddress;
use std::{
    io::{Cursor, Read},
    net::Ipv6Addr,
    sync::{
        mpsc::{Receiver, Sender},
        Arc,
    },
    time::{Duration, Instant},
};

use crate::canvas::CanvasState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Size {
    SinglePixel = 1,
    Area2x2 = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    pub x: u16,
    pub y: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpInfo {
    pub src_ip: Ipv6Addr,
    pub dest_ip: Ipv6Addr,
}

impl IpInfo {
    pub fn new(src_ip: Ipv6Addr, dest_ip: Ipv6Addr) -> Self {
        Self { src_ip, dest_ip }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EthernetInfo {
    pub src_mac: MacAddress,
    pub dest_mac: MacAddress,
}

impl EthernetInfo {
    pub fn new(src_mac: MacAddress, dest_mac: MacAddress) -> Self {
        Self { src_mac, dest_mac }
    }
}

#[derive(Debug)]
pub struct PixelInfo {
    pos: Pos,
    color: Rgb<u8>,
    size: Size,
}

pub fn from_addr(ip_addr: Ipv6Addr) -> Option<PixelInfo> {
    /*Ipv6Addr::new(
        0x2602,
        0xfa9b,
        0x202,
        pos.x | ((size as u16) << 12),
        pos.y,
        color.red as u16,
        color.green as u16,
        color.blue as u16,
    )*/

    let segments = ip_addr.segments();
    let size = (segments[4] & 0xf000) >> 12;
    let x = segments[4] & 0x0fff;
    let y = segments[5] & 0x0fff;
    let red = (segments[6] & 0x00ff) as u8;
    let green = ((segments[7] & 0xff00) >> 8) as u8;
    let blue = (segments[7] & 0x00ff) as u8;

    let size = match size {
        1 => Size::SinglePixel,
        2 => Size::Area2x2,
        _ => return None,
    };
    if x >= 512 || y >= 512 {
        return None;
    }
    Some(PixelInfo {
        pos: Pos { x, y },
        color: Rgb([red, green, blue]),
        size,
    })
}

// https://datatracker.ietf.org/doc/html/rfc1071
pub fn icmpv6_checksum(src_ip: Ipv6Addr, dest_ip: Ipv6Addr, icmpv6_packet: &[u8]) -> u16 {
    let mut data = make_ipv6_pseudo_header(src_ip, dest_ip, icmpv6_packet.len() as u16);
    icmpv6_packet.iter().for_each(|byte| data.push(*byte));

    let mut total: u32 = 0;
    let mut i = 0;
    let mut words = (data.len() + 1) / 2;

    // Iterate over 16-bit words
    loop {
        if words <= 0 {
            break;
        }
        words -= 1;

        let val = ((if i + 1 < data.len() {
            data[i + 1] as u32
        } else {
            0x00
        }) << 8)
            | (data[i] as u32);
        total += val;
        i += 2;
    }

    while (total & 0xffff0000) > 0 {
        total = (total >> 16) + (total & 0xffff);
    }

    return !(total as u16);
}

pub fn make_ipv6_pseudo_header(
    src_ip: Ipv6Addr,
    dest_ip: Ipv6Addr,
    icmp_packet_len: u16,
) -> Vec<u8> {
    let mut data = Vec::new();
    src_ip.octets().into_iter().for_each(|byte| data.push(byte)); // Source Address
    dest_ip
        .octets()
        .into_iter()
        .for_each(|byte| data.push(byte)); // Destination Address

    data.push((icmp_packet_len >> 8) as u8);
    data.push((icmp_packet_len & 0xFF) as u8);

    data.push(0x00);
    data.push(0x00);
    data.push(0x00);
    data.push(0x3a); // Next header: ICMPv6 (58)
    data
}

pub fn check_for_icmpv6_ping(
    data: &[u8],
    is_ethernet: bool,
    require_valid_icmpv6_checksum: bool,
) -> Result<Option<(Option<EthernetInfo>, IpInfo)>> {
    //debug!("PACKET: {:x?}", data);
    let mut reader = Cursor::new(data);

    // Ethernet header
    let ethernet_info = if is_ethernet {
        let mut mac_buf = [0u8; 6];
        reader.read_exact(&mut mac_buf)?;
        let dest_mac = MacAddress::new(mac_buf);
        reader.read_exact(&mut mac_buf)?;
        let src_mac = MacAddress::new(mac_buf);

        let mut next_header = [0u8; 2];
        reader.read_exact(&mut next_header)?;
        if next_header[0] != 0x86 || next_header[1] != 0xdd {
            // Next header is not an IPv6 packet!
            /*debug!(
                "Fault: Ethernet: Not an IPv6 packet (got: {:02x}, {:02x}, expected: 0x86, 0xdd)",
                next_header[0],
                next_header[1]
            );*/
            return Ok(None);
        }

        Some(EthernetInfo::new(src_mac, dest_mac))
    } else {
        None
    };

    // IPv6 Header
    let mut ip_header = [0u8; 8 + 16 + 16];
    reader.read_exact(&mut ip_header[..1])?;
    if ip_header[0] != 0x60 {
        // This is most likely an IPv4 packet, not IPv6!
        /*debug!(
            "Fault: IP: Not an IPv6 packet (got: {:02x}, expected: 0x60)",
            ip_header[0]
        );*/
        return Ok(None);
    }
    reader.read_exact(&mut ip_header[1..])?;

    // ip_header[1..3] are something with traffic classes
    if ip_header[6] != 0x3a {
        // "Next header" is not indicating an ICMPv6 packet. We don't care about Non-ICMP packets!
        //debug!("Fault: Next header is not ICMPv6");
        return Ok(None);
    }
    // ip_header[7] is the hop limit

    let payload_length: u16 = ((ip_header[4] as u16) << 8) as u16 | ((ip_header[5] & 0xFF) as u16);

    let src_ip = Ipv6Addr::new(
        (ip_header[8 + 0] as u16) << 8 | ip_header[8 + 1] as u16,
        (ip_header[8 + 2] as u16) << 8 | ip_header[8 + 3] as u16,
        (ip_header[8 + 4] as u16) << 8 | ip_header[8 + 5] as u16,
        (ip_header[8 + 6] as u16) << 8 | ip_header[8 + 7] as u16,
        (ip_header[8 + 8] as u16) << 8 | ip_header[8 + 9] as u16,
        (ip_header[8 + 10] as u16) << 8 | ip_header[8 + 11] as u16,
        (ip_header[8 + 12] as u16) << 8 | ip_header[8 + 13] as u16,
        (ip_header[8 + 14] as u16) << 8 | ip_header[8 + 15] as u16,
    );
    let dest_ip = Ipv6Addr::new(
        (ip_header[24 + 0] as u16) << 8 | ip_header[24 + 1] as u16,
        (ip_header[24 + 2] as u16) << 8 | ip_header[24 + 3] as u16,
        (ip_header[24 + 4] as u16) << 8 | ip_header[24 + 5] as u16,
        (ip_header[24 + 6] as u16) << 8 | ip_header[24 + 7] as u16,
        (ip_header[24 + 8] as u16) << 8 | ip_header[24 + 9] as u16,
        (ip_header[24 + 10] as u16) << 8 | ip_header[24 + 11] as u16,
        (ip_header[24 + 12] as u16) << 8 | ip_header[24 + 13] as u16,
        (ip_header[24 + 14] as u16) << 8 | ip_header[24 + 15] as u16,
    );
    let ip_info: IpInfo = IpInfo::new(src_ip, dest_ip);

    if payload_length < 8 {
        // The ICMPv6 Packet is smaller than the smallest ping possible!
        //debug!("Fault: ICMPv6 Header too small");
        return Ok(None);
    }

    let mut icmp_packet = vec![0u8; payload_length as usize];
    reader.read_exact(&mut icmp_packet)?;
    if icmp_packet[0] != 0x80 || icmp_packet[1] != 0x00 {
        // Wrong type (0x80) or Code (0x00)!
        return Ok(None);
    }

    let icmp_checksum: u16 = ((icmp_packet[2] & 0xFF) as u16) | ((icmp_packet[3] as u16) << 8);
    //let icmp_identifier: u16 = ((icmp_packet[4] as u16) << 8) | ((icmp_packet[5] & 0xFF) as u16);
    //let icmp_sequence: u16 = ((icmp_packet[6] as u16) << 8) | ((icmp_packet[7] & 0xFF) as u16);

    if require_valid_icmpv6_checksum {
        // Zero out checksum for checksum calc
        icmp_packet[2] = 0x00;
        icmp_packet[3] = 0x00;
        let expected_icmp_checksum = icmpv6_checksum(src_ip, dest_ip, &icmp_packet);
        if expected_icmp_checksum != icmp_checksum {
            // Wrong checksum!
            /*debug!(
                "Fault: Wrong checksum (expected: {:04x}, got: {:04x})",
                expected_icmp_checksum,
                icmp_checksum
            );*/
            return Ok(None);
        }
    }

    Ok(Some((ethernet_info, ip_info)))
}

pub fn run_pixel_receiver(
    iface_name: &str,
    require_valid_icmpv6_checksum: bool,
    pixel_sender: Sender<PixelInfo>,
) -> Result<()> {
    let lib = rawsock::open_best_library()?;
    let mut iface = lib.open_interface(iface_name)?;
    iface.set_filter("icmp6")?;
    let is_ethernet = match iface.data_link() {
        rawsock::DataLink::Ethernet => true,
        _ => false,
    };

    info!(
        "Ping-Receiver started. Listening for icmpv6 packets on {iface_name} using {}...",
        lib.version().to_string().trim()
    );

    iface.loop_infinite_dyn(&|packet| {
        let res = check_for_icmpv6_ping(&packet, is_ethernet, require_valid_icmpv6_checksum);
        match res {
            Ok(Some((_ethernet_info, ip_info))) => {
                //info!("Got ping from {} to {}", ip_info.src_ip, ip_info.dest_ip);
                let pixel_info: Option<PixelInfo> = from_addr(ip_info.dest_ip);
                if let Some(pixel_info) = pixel_info {
                    pixel_sender.send(pixel_info).ok();
                }
            }
            _ => {}
        }
    })?;
    Err(color_eyre::eyre::eyre!(
        "Infinite loop ended unexpectedly (something must have went wrong)"
    ))
}

pub fn run_pixel_processor(
    pixel_receiver: Receiver<PixelInfo>,
    canvas_state: Arc<CanvasState>,
    min_update_interval: Duration,
) -> Result<()> {
    let mut canvas = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(512, 512, Rgb([0xFF; 3])));
    canvas_state.blocking_update_canvas(&canvas)?;

    info!("Ping-Processor started. Listening for Pixel updates to update and encode canvas...");

    let mut pending_update = false;
    let mut last_updated_at = Instant::now();

    let recv_timeout = min_update_interval / 2;
    loop {
        match pixel_receiver.recv_timeout(recv_timeout) {
            Ok(pixel_info) => {
                for x_offset in 0..(pixel_info.size as u16) {
                    let x = pixel_info.pos.x + x_offset;
                    if x >= 512 {
                        break;
                    }
                    for y_offset in 0..(pixel_info.size as u16) {
                        let y = pixel_info.pos.y + y_offset;
                        if y >= 512 {
                            break;
                        }

                        canvas.as_mut_rgb8().unwrap().put_pixel(
                            x as u32,
                            y as u32,
                            pixel_info.color,
                        );
                    }
                }
                pending_update = true;
            }
            Err(_err) => {} // Timeout hit
        }

        if pending_update && last_updated_at.elapsed() >= min_update_interval {
            //let start = Instant::now();
            canvas_state.blocking_update_canvas(&canvas)?;
            //debug!("Encoded and updated canvas in {:?}.", start.elapsed());
            last_updated_at = Instant::now();
            pending_update = false;
        }
    }
}
