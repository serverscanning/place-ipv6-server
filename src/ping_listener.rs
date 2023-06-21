//! Sniffs on the network and parsing ICMPv6 ping packets to pass along to canvas_processor.rs

use color_eyre::Result;
use crossbeam_channel::Sender;
use std::{
    io::{Cursor, Read},
    net::Ipv6Addr,
};

use crate::canvas_processor::PixelInfo;

/// Source and destination IP of a ping
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

/// Create a pseudo IPv6 Header.
/// It is used to calculate the checksum of an ICMPv6 packet
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

/// Calculate the checksum of an ICMPv6 packet
/// See: https://datatracker.ietf.org/doc/html/rfc1071
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

/// Analysze a packet, check if it is a valid IPv6 Ping and extract some information from it
/// Returns Ok(None) if packet is not a valid IPv6 ping packet.
#[inline]
pub fn check_for_icmpv6_ping(
    data: &[u8],
    is_ethernet: bool,
    require_valid_icmpv6_checksum: bool,
) -> Result<Option<IpInfo>> {
    //debug!("PACKET: {:x?}", data);
    let mut reader = Cursor::new(data);

    // Ethernet header
    if is_ethernet {
        let mut mac_buf = [0u8; 6];
        reader.read_exact(&mut mac_buf)?; // Dest Mac Addr
        reader.read_exact(&mut mac_buf)?; // Src Mac Addr

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
    }

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
    if (icmp_packet[0] != 0x80 && icmp_packet[0] != 0x81) || icmp_packet[1] != 0x00 {
        // not ping request or reply or not Code (0x00)!
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

    Ok(Some(ip_info))
}

/// Listen for icmpv6 packets on a given interface and pass on valid pings
/// as PixelInfo to pixel_sender.
/// Requires admin/root or the capability CAP_NET_RAW (linux)
pub fn run_ping_listener(
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
        "Started. Listening for IPv6 pings on {iface_name} using {}...",
        lib.version().to_string().trim()
    );

    iface.loop_infinite_dyn(&|packet| {
        let res = check_for_icmpv6_ping(&packet, is_ethernet, require_valid_icmpv6_checksum);
        match res {
            Ok(Some(ip_info)) => {
                //info!("Got ping from {} to {}", ip_info.src_ip, ip_info.dest_ip);
                let pixel_info: Option<PixelInfo> = PixelInfo::from_ip_info(ip_info);
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
