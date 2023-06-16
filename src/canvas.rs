use std::io::Cursor;

use color_eyre::{eyre::ensure, Result};
use image::{codecs::png::PngEncoder, DynamicImage, GenericImageView, ImageEncoder};
use tokio::sync::{
    broadcast::{Receiver, Sender},
    RwLock, RwLockReadGuard,
};

pub struct CanvasState {
    /// Base64 of png, starting with "data:image/png;base64," to denote this
    encoded_full_canvas: RwLock<EncodedCanvas>,
    encoded_delta_canvas: RwLock<EncodedCanvas>,
    pps_publisher: Sender<usize>,
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

    pub fn update_pps(&self, pps: usize) {
        self.pps_publisher.send(pps).ok();
    }

    pub fn subscribe_to_pps(&self) -> Receiver<usize> {
        self.pps_publisher.subscribe()
    }
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            encoded_full_canvas: RwLock::new(
                EncodedCanvas::new(&DynamicImage::new_rgb8(512, 512)).unwrap(),
            ),
            encoded_delta_canvas: RwLock::new(
                EncodedCanvas::new(&DynamicImage::new_rgba8(512, 512)).unwrap(),
            ),
            pps_publisher: tokio::sync::broadcast::channel(64).0,
        }
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
            canvas.width() == 512 && canvas.height() == 512,
            "Canvas has correct dimensions of 512x512"
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
