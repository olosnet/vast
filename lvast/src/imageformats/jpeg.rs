//! JPEG image reader/writer.
//!
//! Limitations:
//! - JPEG is lossy, so saved pixel values are not preserved exactly on readback.
//! - Reader exposes only `RAW8` and `RGB24`, matching practical JPEG decode output.
//! - Higher bit-depth raw inputs are downscaled to 8-bit grayscale on save.

use crate::{
    base::errors::{VastError, VastErrorType},
    imageformats::types::{
        HeaderCard, ImageFrame, ImageFrameFormat, ImageReader, ImageSaver,
        StandardImageFrameFormat,
    },
};
use image::codecs::jpeg::JpegEncoder;
use image::{ColorType, ExtendedColorType, ImageEncoder};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JpegImageSaver {
    pub width: u32,
    pub height: u32,
    pub format: StandardImageFrameFormat,
    pub quality: u8,
}

impl JpegImageSaver {
    pub fn new(width: u32, height: u32, format: StandardImageFrameFormat) -> Self {
        Self {
            width,
            height,
            format,
            quality: 90,
        }
    }

    pub fn with_quality(mut self, quality: u8) -> Self {
        self.quality = quality.clamp(1, 100);
        self
    }
}

impl ImageSaver for JpegImageSaver {
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
                "invalid JPEG image data length: got {}, expected {}",
                data.len(),
                expected_len
            )));
        }

        let (encoded_data, color_type) = encode_data(data, self.format)?;
        let mut bytes = Vec::new();
        JpegEncoder::new_with_quality(&mut bytes, self.quality)
            .write_image(&encoded_data, self.width, self.height, color_type)
            .map_err(|err| file_error(format!("failed to encode JPEG image: {err}")))?;

        std::fs::write(&path, bytes)
            .map_err(|err| file_error(format!("failed to write JPEG file {path}: {err}")))
    }
}

impl ImageReader for JpegImageSaver {
    fn supported_formats(&self) -> &'static [StandardImageFrameFormat] {
        &[StandardImageFrameFormat::RAW8, StandardImageFrameFormat::RGB24]
    }

    fn read(&self, path: String) -> Result<ImageFrame, VastError> {
        let image = image::open(&path)
            .map_err(|err| file_error(format!("failed to read JPEG file {path}: {err}")))?;
        let width = image.width();
        let height = image.height();
        let color = image.color();

        let (format, data) = match color {
            ColorType::L8 => (StandardImageFrameFormat::RAW8, image.into_luma8().into_raw()),
            ColorType::Rgb8 => (StandardImageFrameFormat::RGB24, image.into_rgb8().into_raw()),
            _ => {
                let rgb = image.into_rgb8();
                (StandardImageFrameFormat::RGB24, rgb.into_raw())
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

fn encode_data(
    data: Vec<u8>,
    format: StandardImageFrameFormat,
) -> Result<(Vec<u8>, ExtendedColorType), VastError> {
    match format {
        StandardImageFrameFormat::RAW8 => Ok((data, ColorType::L8.into())),
        StandardImageFrameFormat::RGB24 => Ok((data, ColorType::Rgb8.into())),
        StandardImageFrameFormat::RGB32 => Ok((strip_alpha(data), ColorType::Rgb8.into())),
        StandardImageFrameFormat::RAW10
        | StandardImageFrameFormat::RAW12
        | StandardImageFrameFormat::RAW14
        | StandardImageFrameFormat::RAW16 => Ok((downscale_u16_to_u8(data)?, ColorType::L8.into())),
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
        .ok_or_else(|| file_error("JPEG image dimensions overflow".to_string()))
}

fn downscale_u16_to_u8(data: Vec<u8>) -> Result<Vec<u8>, VastError> {
    if data.len() % 2 != 0 {
        return Err(file_error(format!(
            "invalid 16-bit grayscale data length: {}",
            data.len()
        )));
    }

    Ok(data
        .chunks_exact(2)
        .map(|chunk| u16::from_ne_bytes([chunk[0], chunk[1]]) >> 8)
        .map(|value| value as u8)
        .collect())
}

fn strip_alpha(data: Vec<u8>) -> Vec<u8> {
    data.chunks_exact(4)
        .flat_map(|chunk| [chunk[0], chunk[1], chunk[2]])
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
        format!("/tmp/opencode/lvast-jpeg-test-{unique}.{extension}")
    }

    #[test]
    fn saves_rgb24_jpeg() {
        let path = temp_file_path("jpg");
        let saver = JpegImageSaver::new(2, 1, StandardImageFrameFormat::RGB24);

        saver
            .save(vec![255, 0, 0, 0, 255, 0], None, path.clone())
            .unwrap();

        let image = image::open(&path).unwrap();
        assert_eq!(image.width(), 2);
        assert_eq!(image.height(), 1);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn saves_raw16_jpeg_after_downscale() {
        let path = temp_file_path("jpg");
        let saver = JpegImageSaver::new(2, 1, StandardImageFrameFormat::RAW16);

        saver
            .save(vec![0x00, 0x00, 0xFF, 0xFF], None, path.clone())
            .unwrap();

        let image = image::open(&path).unwrap();
        assert_eq!(image.color(), ColorType::L8);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn reads_rgb24_jpeg() {
        let path = temp_file_path("jpg");
        let saver = JpegImageSaver::new(2, 1, StandardImageFrameFormat::RGB24);

        saver
            .save(vec![255, 0, 0, 0, 255, 0], None, path.clone())
            .unwrap();

        let frame = saver.read(path.clone()).unwrap();

        assert_eq!(frame.width, 2);
        assert_eq!(frame.height, 1);
        assert_eq!(frame.format, StandardImageFrameFormat::RGB24);
        assert_eq!(frame.data.len(), 6);
        std::fs::remove_file(path).unwrap();
    }
}
