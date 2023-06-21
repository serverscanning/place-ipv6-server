//! Maintains the canvas state and receives Pixel Updates
//! (information extracted from valid Ping Dest IPv6 Addresses)
//! which update this state.
//! Als sends updates in specified interval to all subscribers.

use color_eyre::Result;
use crossbeam_channel::Receiver;
use image::{DynamicImage, Rgb, Rgba};
use std::{
    net::Ipv6Addr,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::canvas::CANVASW;
use crate::canvas::{PpsInfo, CANVASH};
use crate::{canvas::CanvasState, ping_listener::IpInfo};

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
    pub source: Ipv6Addr,
    pub pos: Pos,
    pub color: Rgb<u8>,
    pub size: Size,
}

impl PixelInfo {
    pub fn from_ip_info(ip_info: IpInfo) -> Option<PixelInfo> {
        let segments = ip_info.dest_ip.segments();
        let size = (segments[4] & 0xf000) >> 12;
        let x = segments[4] & 0x0fff;
        let y = segments[5] & 0xffff;
        let red = (segments[6] & 0x00ff) as u8;
        let green = ((segments[7] & 0xff00) >> 8) as u8;
        let blue = (segments[7] & 0x00ff) as u8;

        let size = match size {
            1 => Size::SinglePixel,
            2 => Size::Area2x2,
            _ => return None,
        };
        if x >= CANVASW || y >= CANVASH {
            return None;
        }
        Some(PixelInfo {
            source: ip_info.src_ip,
            pos: Pos { x, y },
            color: Rgb([red, green, blue]),
            size,
        })
    }
}

/// Get adjusted PPS value which takes lag and other irregularities into account
fn adjust_pps(elapsed_since_pps_counter_reset: Duration, pps_counter: usize) -> usize {
    ((pps_counter as u64 * 1_000_000) / elapsed_since_pps_counter_reset.as_micros() as u64) as usize
}

pub fn run_canvas_processor(
    pixel_receiver: Receiver<PixelInfo>,
    canvas_state: Arc<CanvasState>,
    update_interval: Duration,
) -> Result<()> {
    let mut canvas = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        CANVASW.into(),
        CANVASH.into(),
        Rgb([0xFF; 3]),
    ));
    canvas_state.blocking_update_full_canvas(&canvas)?;
    let mut delta_canvas = DynamicImage::new_rgba8(CANVASW.into(), CANVASH.into());

    info!("Started. Listening for Pixel updates to update and encode canvas...");

    let mut pending_update = false;

    let mut pps_counter_reset_at = Instant::now();
    let mut pps_counter: usize = 0;
    #[cfg(feature = "per_user_pps")]
    let mut per_user_pps_last_cleaned = Instant::now();

    for tick in crossbeam_channel::tick(update_interval) {
        let now = tick;

        #[cfg(feature = "per_user_pps")]
        let (mut pps_users, mut pps_next_user_id, pps_per_user_is_disabled) = {
            let mut pps_users = crate::per_user_pps::PPS_USERS.lock().unwrap();
            if now - per_user_pps_last_cleaned > Duration::from_secs(60) {
                crate::per_user_pps::cleanup(&mut pps_users, now);
                per_user_pps_last_cleaned = now;
            }
            let pps_next_user_id = crate::per_user_pps::PPS_NEXT_USER_ID.lock().unwrap();
            let mut pps_disabled_until = crate::per_user_pps::PPS_USERS_DISABLED_UNTIIL
                .lock()
                .unwrap();
            let pps_per_user_is_disabled =
                crate::per_user_pps::is_disabled(&mut pps_users, &mut pps_disabled_until, now);
            (pps_users, pps_next_user_id, pps_per_user_is_disabled)
        };

        let elapsed_since_pps_counter_reset = now - pps_counter_reset_at;
        if elapsed_since_pps_counter_reset >= Duration::from_secs(1) {
            // Should be accurate but counting total packets with it won't be possible anymore accurately
            let pps_adjusted = adjust_pps(elapsed_since_pps_counter_reset, pps_counter);
            pps_counter_reset_at = now;
            #[cfg(feature = "per_user_pps")]
            let per_user_pps = {
                let map = crate::per_user_pps::get_all_pps_counters_and_reset(&mut pps_users);
                let mut per_user_pps = fxhash::FxHashMap::with_capacity_and_hasher(
                    map.len(),
                    fxhash::FxBuildHasher::default(),
                );
                for (user, pps) in map {
                    per_user_pps.insert(user.id, adjust_pps(elapsed_since_pps_counter_reset, pps));
                }
                per_user_pps
            };
            let pps_info = PpsInfo {
                pps: pps_adjusted,
                #[cfg(feature = "per_user_pps")]
                per_user_pps,
            };
            canvas_state.update_pps(pps_info);
            pps_counter = 0;
        }

        for pixel_info in pixel_receiver.try_iter() {
            pps_counter += 1;
            #[cfg(feature = "per_user_pps")]
            {
                if !pps_per_user_is_disabled {
                    crate::per_user_pps::ensure_existing_activity_updated_and_migrated(
                        &mut pps_users,
                        &mut pps_next_user_id,
                        now,
                        pixel_info.source,
                    );
                    if let Some(user_info) = crate::per_user_pps::find_user_info_data_mut(
                        &mut pps_users,
                        pixel_info.source,
                    ) {
                        user_info.pps_counter += 1;
                    }
                }
            }

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

                    canvas
                        .as_mut_rgb8()
                        .unwrap()
                        .put_pixel(x as u32, y as u32, pixel_info.color);
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

        if pending_update {
            //let start = Instant::now();
            canvas_state.blocking_update_full_canvas(&canvas)?;
            canvas_state.blocking_update_delta_canvas(&delta_canvas)?;
            delta_canvas = DynamicImage::new_rgba8(CANVASW.into(), CANVASH.into());
            //debug!("Encoded and updated canvas in {:?}.", start.elapsed());
            pending_update = false;
        }
    }
    Ok(())
}
