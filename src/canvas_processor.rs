//! Maintains the canvas state and receives Pixel Updates
//! (information extracted from valid Ping Dest IPv6 Addresses)
//! which update this state.
//! Als sends updates in specified interval to all subscribers.

use color_eyre::Result;
use image::{DynamicImage, Rgb, Rgba};
use std::{
    net::Ipv6Addr,
    sync::{mpsc::Receiver, Arc},
    time::{Duration, Instant},
};

use crate::canvas::CanvasState;
use crate::canvas::CANVASH;
use crate::canvas::CANVASW;

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

#[derive(Debug)]
pub struct PixelInfo {
    pub pos: Pos,
    pub color: Rgb<u8>,
    pub size: Size,
}

impl PixelInfo {
    pub fn from_addr(ip_addr: Ipv6Addr) -> Option<PixelInfo> {
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
}

pub fn run_canvas_processor(
    pixel_receiver: Receiver<PixelInfo>,
    canvas_state: Arc<CanvasState>,
    min_update_interval: Duration,
) -> Result<()> {
    let mut canvas = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(CANVASW.into(), CANVASH.into(), Rgb([0xFF; 3])));
    canvas_state.blocking_update_full_canvas(&canvas)?;
    let mut delta_canvas = DynamicImage::new_rgba8(CANVASW.into(), CANVASH.into());

    info!("Started. Listening for Pixel updates to update and encode canvas...");

    let mut pending_update = false;
    let mut last_updated_at = Instant::now();

    let mut pps_counter_reset_at = Instant::now();
    let mut pps_counter: usize = 0;

    let recv_timeout = min_update_interval / 2;
    loop {
        let recv_result = pixel_receiver.recv_timeout(recv_timeout);
        let now = Instant::now();

        if now - pps_counter_reset_at >= Duration::from_secs(1) {
            // Could be better, but good enough for now
            pps_counter_reset_at = Instant::now();
            canvas_state.update_pps(pps_counter);
            pps_counter = 0;
        }

        match recv_result {
            Ok(pixel_info) => {
                pps_counter += 1;

                for x_offset in 0..(pixel_info.size as u16) {
                    let x = pixel_info.pos.x + x_offset;
                    if x >= CANVASW {
                        break;
                    }
                    for y_offset in 0..(pixel_info.size as u16) {
                        let y = pixel_info.pos.y + y_offset;
                        if y >= CANVASH {
                            break;
                        }

                        canvas.as_mut_rgb8().unwrap().put_pixel(
                            x as u32,
                            y as u32,
                            pixel_info.color,
                        );
                        delta_canvas.as_mut_rgba8().unwrap().put_pixel(
                            x as u32,
                            y as u32,
                            Rgba([
                                pixel_info.color.0[0],
                                pixel_info.color.0[1],
                                pixel_info.color.0[2],
                                0xFF,
                            ]),
                        );
                    }
                }
                pending_update = true;
            }
            Err(_err) => {} // Timeout hit
        }

        if pending_update && now - last_updated_at >= min_update_interval {
            //let start = Instant::now();
            canvas_state.blocking_update_full_canvas(&canvas)?;
            canvas_state.blocking_update_delta_canvas(&delta_canvas)?;
            delta_canvas = DynamicImage::new_rgba8(CANVASW.into(), CANVASH.into());
            //debug!("Encoded and updated canvas in {:?}.", start.elapsed());
            last_updated_at = now;
            pending_update = false;
        }
    }
}
