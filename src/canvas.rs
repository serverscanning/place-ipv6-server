use color_eyre::{eyre::ensure, Result};
use image::{codecs::png::PngEncoder, DynamicImage, GenericImageView, ImageEncoder};
use tokio::sync::{
    broadcast::{Receiver, Sender},
    RwLock, RwLockReadGuard,
};

#[derive(Default)]
pub struct CanvasState {
    /// Base64 of png, starting with "data:image/png;base64," to denote this
    encoded_canvas: RwLock<EncodedCanvas>,
}

impl CanvasState {
    pub async fn read_encoded_canvas(&self) -> RwLockReadGuard<EncodedCanvas> {
        self.encoded_canvas.read().await
    }

    pub fn blocking_update_canvas(&self, canvas: &DynamicImage) -> Result<()> {
        self.encoded_canvas.blocking_write().update(canvas)
    }
}

#[derive(Clone)]
pub struct EncodedCanvas {
    encoded_data: String,
    publisher: Sender<String>,
}

impl EncodedCanvas {
    fn to_encoded_data(canvas: &DynamicImage) -> Result<String> {
        ensure!(
            canvas.width() == 512 && canvas.height() == 512,
            "Canvas has correct dimensions of 512x512"
        );

        // Encode
        let mut png_base64_writer =
            base64::write::EncoderStringWriter::new(&base64::engine::general_purpose::STANDARD);
        let (width, height) = canvas.dimensions();
        // Encode as png into the writer, which also encodes into base64 on-the-fly
        PngEncoder::new_with_quality(
            &mut png_base64_writer,
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::default(),
        )
        .write_image(canvas.as_bytes(), width, height, canvas.color())?;

        let mut encoded_data = String::from("data:image/png;base64,");
        encoded_data.push_str(&png_base64_writer.into_inner());
        Ok(encoded_data)
    }

    pub fn new(canvas: &DynamicImage) -> Result<Self> {
        Ok(Self {
            encoded_data: Self::to_encoded_data(canvas)?,
            publisher: tokio::sync::broadcast::channel(64).0,
        })
    }

    pub fn update(&mut self, canvas: &DynamicImage) -> Result<()> {
        let encoded_data = Self::to_encoded_data(canvas)?;
        self.encoded_data = encoded_data.clone();
        self.publisher.send(encoded_data).ok();
        Ok(())
    }

    pub fn subscribe(&self) -> Receiver<String> {
        self.publisher.subscribe()
    }

    pub fn get_encoded_data(&self) -> String {
        self.encoded_data.clone()
    }
}

impl Default for EncodedCanvas {
    fn default() -> Self {
        Self::new(&DynamicImage::new_rgb8(512, 512)).unwrap()
    }
}
