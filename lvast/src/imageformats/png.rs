use crate::{
    base::errors::{VastError, VastErrorType},
    imageformats::types::{
        HeaderCard, ImageFrame, ImageFrameFormat, ImageReader, ImageSaver,
        StandardImageFrameFormat,
    },
};
use image::codecs::png::PngEncoder;
use image::{ColorType, ExtendedColorType, ImageEncoder};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PngImageSaver {
    pub width: u32,
    pub height: u32,
    pub format: StandardImageFrameFormat,
}

impl PngImageSaver {
    pub fn new(width: u32, height: u32, format: StandardImageFrameFormat) -> Self {
        Self {
            width,
            height,
            format,
        }
    }
}

impl ImageSaver for PngImageSaver {
    fn supported_formats(&self) -> &'static [StandardImageFrameFormat] {
        &[
            StandardImageFrameFormat::RAW8,
            StandardImageFrameFormat::RAW10,
            StandardImageFrameFormat::RAW12,
            StandardImageFrameFormat::RAW14,
            StandardImageFrameFormat::RAW16,
            StandardImageFrameFormat::RGB24,
            StandardImageFrameFormat::RGB32,
        ]
    }

    fn save(
        &self,
        data: Vec<u8>,
        _image_headers: Option<Vec<HeaderCard>>,
        path: String,
    ) -> Result<(), VastError> {
        let expected_len = expected_data_len(self.width, self.height, self.format)?;
        if data.len() != expected_len {
            return Err(file_error(format!(
                "invalid PNG image data length: got {}, expected {}",
                data.len(),
                expected_len
            )));
        }

        let (encoded_data, color_type) = encode_data(data, self.format);
        let mut bytes = Vec::new();
        PngEncoder::new(&mut bytes)
            .write_image(&encoded_data, self.width, self.height, color_type)
            .map_err(|err| file_error(format!("failed to encode PNG image: {err}")))?;

        std::fs::write(&path, bytes)
            .map_err(|err| file_error(format!("failed to write PNG file {path}: {err}")))
    }
}

impl ImageReader for PngImageSaver {
    fn supported_formats(&self) -> &'static [StandardImageFrameFormat] {
        &[
            StandardImageFrameFormat::RAW8,
            StandardImageFrameFormat::RAW16,
            StandardImageFrameFormat::RGB24,
            StandardImageFrameFormat::RGB32,
        ]
    }

    fn read(&self, path: String) -> Result<ImageFrame, VastError> {
        let image = image::open(&path)
            .map_err(|err| file_error(format!("failed to read PNG file {path}: {err}")))?;
        let width = image.width();
        let height = image.height();
        let color = image.color();

        let (format, data) = match color {
            ColorType::L8 => (StandardImageFrameFormat::RAW8, image.into_luma8().into_raw()),
            ColorType::L16 => {
                let raw = image.into_luma16().into_raw();
                let data = raw
                    .into_iter()
                    .flat_map(|value| value.to_be_bytes())
                    .collect();
                (StandardImageFrameFormat::RAW16, data)
            }
            ColorType::Rgb8 => (StandardImageFrameFormat::RGB24, image.into_rgb8().into_raw()),
            ColorType::Rgba8 => (StandardImageFrameFormat::RGB32, image.into_rgba8().into_raw()),
            _ => {
                let rgba = image.into_rgba8();
                (StandardImageFrameFormat::RGB32, rgba.into_raw())
            }
        };

        Ok(ImageFrame {
            width,
            height,
            format,
            data,
        })
    }
}

fn encode_data(data: Vec<u8>, format: StandardImageFrameFormat) -> (Vec<u8>, ExtendedColorType) {
    match format {
        StandardImageFrameFormat::RAW8 => (data, ColorType::L8.into()),
        StandardImageFrameFormat::RAW10
        | StandardImageFrameFormat::RAW12
        | StandardImageFrameFormat::RAW14
        | StandardImageFrameFormat::RAW16 => (native_u16_to_big_endian(data), ColorType::L16.into()),
        StandardImageFrameFormat::RGB24 => (data, ColorType::Rgb8.into()),
        StandardImageFrameFormat::RGB32 => (data, ColorType::Rgba8.into()),
    }
}

fn expected_data_len(
    width: u32,
    height: u32,
    format: StandardImageFrameFormat,
) -> Result<usize, VastError> {
    let bytes_per_pixel = format.bytes_per_pixel();

    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(bytes_per_pixel))
        .ok_or_else(|| file_error("PNG image dimensions overflow".to_string()))
}

fn native_u16_to_big_endian(data: Vec<u8>) -> Vec<u8> {
    data.chunks_exact(2)
        .flat_map(|chunk| u16::from_ne_bytes([chunk[0], chunk[1]]).to_be_bytes())
        .collect()
}

fn file_error(message: String) -> VastError {
    VastError {
        error_type: VastErrorType::FileError,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file_path(extension: &str) -> String {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("/tmp/opencode/lvast-png-test-{unique}.{extension}")
    }

    #[test]
    fn saves_raw16_png() {
        let path = temp_file_path("png");
        let saver = PngImageSaver::new(2, 1, StandardImageFrameFormat::RAW16);

        saver
            .save(vec![0x34, 0x12, 0x78, 0x56], None, path.clone())
            .unwrap();

        let image = image::open(&path).unwrap();
        assert_eq!(image.width(), 2);
        assert_eq!(image.height(), 1);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn saves_rgb32_png() {
        let path = temp_file_path("png");
        let saver = PngImageSaver::new(1, 1, StandardImageFrameFormat::RGB32);

        saver
            .save(vec![255, 0, 0, 128], None, path.clone())
            .unwrap();

        let image = image::open(&path).unwrap();
        assert_eq!(image.color(), ColorType::Rgba8);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn reads_raw16_png() {
        let path = temp_file_path("png");
        let saver = PngImageSaver::new(2, 1, StandardImageFrameFormat::RAW16);

        saver
            .save(vec![0x34, 0x12, 0x78, 0x56], None, path.clone())
            .unwrap();

        let frame = saver.read(path.clone()).unwrap();

        assert_eq!(frame.width, 2);
        assert_eq!(frame.height, 1);
        assert_eq!(frame.format, StandardImageFrameFormat::RAW16);
        assert_eq!(frame.data, vec![0x34, 0x12, 0x78, 0x56]);
        std::fs::remove_file(path).unwrap();
    }
}
