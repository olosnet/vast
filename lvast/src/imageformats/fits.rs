use crate::{
    base::errors::{VastError, VastErrorType},
    imageformats::types::{FitsHeaderValue, HeaderCard, ImageSaver},
    types::camera::CameraFrameFormat,
};

const FITS_BLOCK_SIZE: usize = 2880;
const FITS_CARD_SIZE: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// FITS saver configured for one image geometry and camera frame format.
pub struct FitsImageSaver {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Raw camera frame format used to encode FITS pixel data.
    pub format: CameraFrameFormat,
}

impl FitsImageSaver {
    /// Creates a FITS saver for frames with the given dimensions and format.
    pub fn new(width: u32, height: u32, format: CameraFrameFormat) -> Self {
        Self {
            width,
            height,
            format,
        }
    }
}

impl ImageSaver for FitsImageSaver {
    fn save(
        &self,
        data: Vec<u8>,
        image_headers: Option<Vec<HeaderCard>>,
        path: String,
    ) -> Result<(), VastError> {
        let expected_len = expected_data_len(self.width, self.height, self.format)?;
        if data.len() != expected_len {
            return Err(file_error(format!(
                "invalid FITS image data length: got {}, expected {}",
                data.len(),
                expected_len
            )));
        }

        let mut bytes = Vec::new();
        write_required_headers(&mut bytes, self.width, self.height, self.format)?;
        if let Some(headers) = image_headers {
            for header in headers {
                if is_managed_header(header.key) {
                    continue;
                }
                write_header_card(&mut bytes, &header)?;
            }
        }
        write_end_card(&mut bytes);
        pad_to_fits_block(&mut bytes);

        bytes.extend(fits_data_bytes(data, self.format));
        pad_to_fits_block(&mut bytes);

        std::fs::write(&path, bytes)
            .map_err(|err| file_error(format!("failed to write FITS file {path}: {err}")))
    }
}

fn write_required_headers(
    bytes: &mut Vec<u8>,
    width: u32,
    height: u32,
    format: CameraFrameFormat,
) -> Result<(), VastError> {
    for header in [
        HeaderCard {
            key: "SIMPLE",
            value: true.into(),
            comment: "file conforms to FITS standard",
        },
        HeaderCard {
            key: "BITPIX",
            value: bitpix(format).into(),
            comment: "bits per data value",
        },
        HeaderCard {
            key: "NAXIS",
            value: 2_u32.into(),
            comment: "number of data axes",
        },
        HeaderCard {
            key: "NAXIS1",
            value: width.into(),
            comment: "image width",
        },
        HeaderCard {
            key: "NAXIS2",
            value: height.into(),
            comment: "image height",
        },
        HeaderCard {
            key: "BZERO",
            value: bzero(format).into(),
            comment: "data zero offset",
        },
        HeaderCard {
            key: "BSCALE",
            value: 1.0.into(),
            comment: "data scale factor",
        },
        HeaderCard {
            key: "EXTEND",
            value: true.into(),
            comment: "FITS dataset may contain extensions",
        },
    ] {
        write_header_card(bytes, &header)?;
    }
    Ok(())
}

fn write_header_card(bytes: &mut Vec<u8>, header: &HeaderCard) -> Result<(), VastError> {
    validate_header_key(header.key)?;

    let value = format_header_value(&header.value);
    let mut card = if header.comment.is_empty() {
        format!("{:<8}= {}", header.key, value)
    } else {
        format!("{:<8}= {} / {}", header.key, value, header.comment)
    };
    card.truncate(FITS_CARD_SIZE);
    bytes.extend(format!("{card:<FITS_CARD_SIZE$}").as_bytes());
    Ok(())
}

fn write_end_card(bytes: &mut Vec<u8>) {
    bytes.extend(format!("{:<FITS_CARD_SIZE$}", "END").as_bytes());
}

fn format_header_value(value: &FitsHeaderValue) -> String {
    match value {
        FitsHeaderValue::String(value) => format!("'{:<8}'", escape_header_string(value)),
        FitsHeaderValue::Integer(value) => format!("{value:>20}"),
        FitsHeaderValue::Float(value) => format!("{value:>20.10}"),
        FitsHeaderValue::Boolean(value) => format!("{:>20}", if *value { "T" } else { "F" }),
    }
}

fn escape_header_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn validate_header_key(key: &str) -> Result<(), VastError> {
    if key.is_empty()
        || key.len() > 8
        || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(file_error(format!("invalid FITS header key: {key}")));
    }
    Ok(())
}

fn is_managed_header(key: &str) -> bool {
    matches!(
        key,
        "SIMPLE" | "BITPIX" | "NAXIS" | "NAXIS1" | "NAXIS2" | "BZERO" | "BSCALE" | "EXTEND"
    )
}

fn expected_data_len(
    width: u32,
    height: u32,
    format: CameraFrameFormat,
) -> Result<usize, VastError> {
    let bytes_per_pixel = match format {
        CameraFrameFormat::RAW8 => 1,
        CameraFrameFormat::RAW10
        | CameraFrameFormat::RAW12
        | CameraFrameFormat::RAW14
        | CameraFrameFormat::RAW16 => 2,
        CameraFrameFormat::RGB24 => 3,
        CameraFrameFormat::RGB32 => 4,
    };

    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(bytes_per_pixel))
        .ok_or_else(|| file_error("FITS image dimensions overflow".to_string()))
}

fn bitpix(format: CameraFrameFormat) -> i64 {
    match format {
        CameraFrameFormat::RAW8 => 8,
        CameraFrameFormat::RAW10
        | CameraFrameFormat::RAW12
        | CameraFrameFormat::RAW14
        | CameraFrameFormat::RAW16 => 16,
        CameraFrameFormat::RGB24 | CameraFrameFormat::RGB32 => 8,
    }
}

fn bzero(format: CameraFrameFormat) -> f64 {
    match format {
        CameraFrameFormat::RAW10
        | CameraFrameFormat::RAW12
        | CameraFrameFormat::RAW14
        | CameraFrameFormat::RAW16 => 32768.0,
        CameraFrameFormat::RAW8 | CameraFrameFormat::RGB24 | CameraFrameFormat::RGB32 => 0.0,
    }
}

fn fits_data_bytes(data: Vec<u8>, format: CameraFrameFormat) -> Vec<u8> {
    match format {
        CameraFrameFormat::RAW10
        | CameraFrameFormat::RAW12
        | CameraFrameFormat::RAW14
        | CameraFrameFormat::RAW16 => data
            .chunks_exact(2)
            .flat_map(|chunk| {
                let value = u16::from_ne_bytes([chunk[0], chunk[1]]) as i32;
                let signed_value = (value - 32768) as i16;
                signed_value.to_be_bytes()
            })
            .collect(),
        CameraFrameFormat::RAW8 | CameraFrameFormat::RGB24 | CameraFrameFormat::RGB32 => data,
    }
}

fn pad_to_fits_block(bytes: &mut Vec<u8>) {
    let padding = (FITS_BLOCK_SIZE - (bytes.len() % FITS_BLOCK_SIZE)) % FITS_BLOCK_SIZE;
    bytes.resize(bytes.len() + padding, b' ');
}

fn file_error(message: String) -> VastError {
    VastError {
        error_type: VastErrorType::FileError,
        message,
    }
}
