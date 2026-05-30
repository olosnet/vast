//! TIFF image reader/writer.
//!
//! Limitations:
//! - Reader exposes only `RAW8`, `RAW16`, `RGB24`, and `RGB32`, matching common TIFF decode output.
//! - Saving `RAW10`, `RAW12`, and `RAW14` stores samples in 16-bit containers.

use crate::{
    base::errors::{VastError, VastErrorType},
    imageformats::types::{
        HeaderCard, ImageFrame, ImageFrameFormat, ImageReader, ImageSaver,
        StandardImageFrameFormat,
    },
};
use image::codecs::tiff::TiffEncoder;
use image::{ColorType, ExtendedColorType, ImageEncoder};
use std::io::Cursor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TiffImageSaver {
    pub width: u32,
    pub height: u32,
    pub format: StandardImageFrameFormat,
}

impl TiffImageSaver {
    pub fn new(width: u32, height: u32, format: StandardImageFrameFormat) -> Self {
        Self {
            width,
            height,
            format,
        }
    }
}

impl ImageSaver for TiffImageSaver {
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
                "invalid TIFF image data length: got {}, expected {}",
                data.len(),
                expected_len
            )));
        }

        let (encoded_data, color_type) = encode_data(data, self.format);
        let mut bytes = Cursor::new(Vec::new());
        TiffEncoder::new(&mut bytes)
            .write_image(&encoded_data, self.width, self.height, color_type)
            .map_err(|err| file_error(format!("failed to encode TIFF image: {err}")))?;

        std::fs::write(&path, bytes.into_inner())
            .map_err(|err| file_error(format!("failed to write TIFF file {path}: {err}")))
    }
}

impl ImageReader for TiffImageSaver {
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
            .map_err(|err| file_error(format!("failed to read TIFF file {path}: {err}")))?;
        let width = image.width();
        let height = image.height();
        let color = image.color();

        let (format, data) = match color {
            ColorType::L8 => (StandardImageFrameFormat::RAW8, image.into_luma8().into_raw()),
            ColorType::L16 => {
                let raw = image.into_luma16().into_raw();
                let data = raw.into_iter().flat_map(|value| value.to_ne_bytes()).collect();
                (StandardImageFrameFormat::RAW16, data)
            }
            ColorType::Rgb8 => (StandardImageFrameFormat::RGB24, image.into_rgb8().into_raw()),
            ColorType::Rgba8 => {
                (StandardImageFrameFormat::RGB32, image.into_rgba8().into_raw())
            }
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

fn encode_data(
    data: Vec<u8>,
    format: StandardImageFrameFormat,
) -> (Vec<u8>, ExtendedColorType) {
    match format {
        StandardImageFrameFormat::RAW8 => (data, ColorType::L8.into()),
        StandardImageFrameFormat::RAW10
        | StandardImageFrameFormat::RAW12
        | StandardImageFrameFormat::RAW14
        | StandardImageFrameFormat::RAW16 => (data, ColorType::L16.into()),
        StandardImageFrameFormat::RGB24 => (data, ColorType::Rgb8.into()),
        StandardImageFrameFormat::RGB32 => (data, ColorType::Rgba8.into()),
    }
}

fn expected_data_len(
    width: u32,
    height: u32,
    format: StandardImageFrameFormat,
) -> Result<usize, VastError> {
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(format.bytes_per_pixel()))
        .ok_or_else(|| file_error("TIFF image dimensions overflow".to_string()))
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
        format!("/tmp/opencode/lvast-tiff-test-{unique}.{extension}")
    }

    #[test]
    fn saves_and_reads_raw16_tiff() {
        let path = temp_file_path("tiff");
        let saver = TiffImageSaver::new(2, 1, StandardImageFrameFormat::RAW16);

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

    #[test]
    fn saves_and_reads_rgb32_tiff() {
        let path = temp_file_path("tiff");
        let saver = TiffImageSaver::new(1, 1, StandardImageFrameFormat::RGB32);

        saver
            .save(vec![255, 0, 0, 128], None, path.clone())
            .unwrap();

        let frame = saver.read(path.clone()).unwrap();

        assert_eq!(frame.width, 1);
        assert_eq!(frame.height, 1);
        assert_eq!(frame.format, StandardImageFrameFormat::RGB32);
        assert_eq!(frame.data, vec![255, 0, 0, 128]);
        std::fs::remove_file(path).unwrap();
    }
}
