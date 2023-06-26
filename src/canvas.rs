//! Canvas State struct and update/subscribe logic as well as encoding the canvas to a PNG binary.

use std::{
    io::Cursor,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use color_eyre::{eyre::ensure, Result};
use image::{codecs::png::PngEncoder, DynamicImage, GenericImageView, ImageEncoder};
use serde::Serialize;
use tokio::sync::{
    broadcast::{Receiver, Sender},
    RwLock, RwLockReadGuard,
};

#[derive(Serialize, Clone)]
pub struct PpsInfo {
    /// Total
    pub pps: usize,
    #[cfg(feature = "per_user_pps")]
    pub per_user_pps: fxhash::FxHashMap<u64, usize>,
}

#[derive(Copy, Clone)]
pub struct NudityResult {
    pub is_nude: bool,
}

pub struct CanvasState {
    /// Base64 of png, starting with "data:image/png;base64," to denote this
    encoded_full_canvas: RwLock<EncodedCanvas>,
    encoded_delta_canvas: RwLock<EncodedCanvas>,
    pps_publisher: Sender<PpsInfo>,
    ws_connection_count: Arc<AtomicUsize>,
    ws_connection_count_publisher: Sender<usize>,
    nudity_result: RwLock<NudityResult>,
    nudity_result_publisher: Sender<NudityResult>,
}

impl CanvasState {
    pub async fn read_encoded_full_canvas(&self) -> RwLockReadGuard<EncodedCanvas> {
        self.encoded_full_canvas.read().await
    }

    pub async fn read_encoded_delta_canvas(&self) -> RwLockReadGuard<EncodedCanvas> {
        self.encoded_delta_canvas.read().await
    }

    pub fn blocking_update_full_canvas(&self, canvas: &DynamicImage) -> Result<()> {
        ensure!(
            canvas.as_rgb8().is_some(),
            "Full canvas is expected to have no alpha layer!"
        );
        self.encoded_full_canvas.blocking_write().update(canvas)
    }

    pub fn blocking_update_delta_canvas(&self, canvas: &DynamicImage) -> Result<()> {
        ensure!(
            canvas.as_rgba8().is_some(),
            "Delta canvas is expected to have an alpha layer!"
        );
        self.encoded_delta_canvas.blocking_write().update(canvas)
    }

    pub fn update_pps(&self, pps: PpsInfo) {
        self.pps_publisher.send(pps).ok();
    }

    pub fn subscribe_to_pps(&self) -> Receiver<PpsInfo> {
        self.pps_publisher.subscribe()
    }

    pub fn track_new_websocket(&self) -> WsConnectionCountTracker {
        WsConnectionCountTracker::new(
            self.ws_connection_count.clone(),
            self.ws_connection_count_publisher.clone(),
        )
    }

    pub fn subscribe_to_websocket_count(&self) -> Receiver<usize> {
        self.ws_connection_count_publisher.subscribe()
    }

    pub fn websocket_count(&self) -> usize {
        self.ws_connection_count.load(Ordering::Relaxed)
    }

    pub fn subscribe_to_nudity_results(&self) -> Receiver<NudityResult> {
        self.nudity_result_publisher.subscribe()
    }

    pub async fn nudity_result(&self) -> NudityResult {
        self.nudity_result.read().await.clone()
    }

    pub fn blocking_update_nudity_result(&self, new_nudity_result: NudityResult) {
        *self.nudity_result.blocking_write() = new_nudity_result;
        self.nudity_result_publisher.send(new_nudity_result).ok();
    }
}

pub(crate) const CANVASW: u16 = 512;
pub(crate) const CANVASH: u16 = 512;

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            encoded_full_canvas: RwLock::new(
                EncodedCanvas::new(&DynamicImage::new_rgb8(CANVASW.into(), CANVASH.into()))
                    .unwrap(),
            ),
            encoded_delta_canvas: RwLock::new(
                EncodedCanvas::new(&DynamicImage::new_rgba8(CANVASW.into(), CANVASH.into()))
                    .unwrap(),
            ),
            pps_publisher: tokio::sync::broadcast::channel(64).0,
            ws_connection_count: Arc::new(AtomicUsize::new(0)),
            ws_connection_count_publisher: tokio::sync::broadcast::channel(64).0,
            nudity_result: RwLock::new(NudityResult { is_nude: false }),
            nudity_result_publisher: tokio::sync::broadcast::channel(64).0,
        }
    }
}

/// Used to track how many websocket connections are active
pub struct WsConnectionCountTracker {
    count: Arc<AtomicUsize>,
    publisher: Sender<usize>,
}

impl WsConnectionCountTracker {
    fn new(count: Arc<AtomicUsize>, publisher: Sender<usize>) -> Self {
        let last_val = count.fetch_add(1, Ordering::Relaxed);
        publisher.send(last_val + 1).ok();
        Self { count, publisher }
    }
}

impl Drop for WsConnectionCountTracker {
    fn drop(&mut self) {
        let last_val = self.count.fetch_sub(1, Ordering::Relaxed);
        self.publisher.send(last_val - 1).ok();
    }
}

#[derive(Clone)]
pub struct EncodedCanvas {
    encoded: Vec<u8>,
    publisher: Sender<Vec<u8>>,
}

impl EncodedCanvas {
    fn encode(canvas: &DynamicImage) -> Result<Vec<u8>> {
        ensure!(
            canvas.width() == CANVASW as u32 && canvas.height() == CANVASH as u32,
            "Canvas has correct dimensions"
        );

        // Encode as png into the writer
        let mut png_writer = Cursor::new(Vec::with_capacity(1024 * 64));
        let (width, height) = canvas.dimensions();
        PngEncoder::new_with_quality(
            &mut png_writer,
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::default(),
        )
        .write_image(canvas.as_bytes(), width, height, canvas.color())?;
        Ok(png_writer.into_inner())
    }

    pub fn new(canvas: &DynamicImage) -> Result<Self> {
        Ok(Self {
            encoded: Self::encode(canvas)?,
            publisher: tokio::sync::broadcast::channel(64).0,
        })
    }

    pub fn update(&mut self, canvas: &DynamicImage) -> Result<()> {
        let encoded = Self::encode(canvas)?;
        self.encoded = encoded.clone();
        self.publisher.send(encoded).ok();
        Ok(())
    }

    pub fn subscribe(&self) -> Receiver<Vec<u8>> {
        self.publisher.subscribe()
    }

    pub fn get_encoded(&self) -> Vec<u8> {
        self.encoded.clone()
    }
}
