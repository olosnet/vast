use crate::{
    base::errors::{VastError, VastErrorType},
    imageformats::types::{
        FitsHeaderValue, HeaderCard, ImageFrame, ImageFrameFormat, ImageReader, ImageSaver,
        StandardImageFrameFormat,
    },
};
use std::collections::HashMap;

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
    pub format: StandardImageFrameFormat,
}

impl FitsImageSaver {
    /// Creates a FITS saver for frames with the given dimensions and format.
    pub fn new(width: u32, height: u32, format: StandardImageFrameFormat) -> Self {
        Self {
            width,
            height,
            format,
        }
    }
}

impl ImageSaver for FitsImageSaver {
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

impl ImageReader for FitsImageSaver {
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

    fn read(&self, path: String) -> Result<ImageFrame, VastError> {
        let bytes = std::fs::read(&path)
            .map_err(|err| file_error(format!("failed to read FITS file {path}: {err}")))?;

        let (headers, data_offset) = parse_header_cards(&bytes)?;
        let naxis = header_u32(&headers, "NAXIS")?;
        let naxis1 = header_u32(&headers, "NAXIS1")?;
        let naxis2 = header_u32(&headers, "NAXIS2")?;
        let naxis3 = header_optional_u32(&headers, "NAXIS3")?;
        let bitpix = header_i64(&headers, "BITPIX")?;
        let bzero = header_f64(&headers, "BZERO").unwrap_or(0.0);
        let format = frame_format_from_headers(&headers, naxis, naxis1, naxis3, bitpix, bzero)?;
        let (width, height) = image_dimensions_from_axes(naxis, naxis1, naxis2, naxis3, format)?;
        let data_len = expected_data_len(width, height, format)?;
        let data_end = data_offset
            .checked_add(data_len)
            .ok_or_else(|| file_error("FITS data length overflow".to_string()))?;

        if bytes.len() < data_end {
            return Err(file_error(format!(
                "invalid FITS payload length: got {}, expected at least {}",
                bytes.len(),
                data_end
            )));
        }

        Ok(ImageFrame {
            width,
            height,
            format,
            data: read_fits_data(&bytes[data_offset..data_end], format),
        })
    }
}

fn write_required_headers(
    bytes: &mut Vec<u8>,
    width: u32,
    height: u32,
    format: StandardImageFrameFormat,
) -> Result<(), VastError> {
    let (naxis, naxis1, naxis2, naxis3) = image_axes(width, height, format);

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
            value: naxis.into(),
            comment: "number of data axes",
        },
        HeaderCard {
            key: "NAXIS1",
            value: naxis1.into(),
            comment: "axis 1 length",
        },
        HeaderCard {
            key: "NAXIS2",
            value: naxis2.into(),
            comment: "axis 2 length",
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
            key: "LVFMT",
            value: format.name().into(),
            comment: "original lvast frame format",
        },
        HeaderCard {
            key: "EXTEND",
            value: true.into(),
            comment: "FITS dataset may contain extensions",
        },
    ] {
        write_header_card(bytes, &header)?;
    }

    if let Some(naxis3) = naxis3 {
        write_header_card(
            bytes,
            &HeaderCard {
                key: "NAXIS3",
                value: naxis3.into(),
                comment: "axis 3 length",
            },
        )?;
    }

    Ok(())
}

fn image_axes(
    width: u32,
    height: u32,
    format: StandardImageFrameFormat,
) -> (u32, u32, u32, Option<u32>) {
    match format {
        StandardImageFrameFormat::RAW8
        | StandardImageFrameFormat::RAW10
        | StandardImageFrameFormat::RAW12
        | StandardImageFrameFormat::RAW14
        | StandardImageFrameFormat::RAW16 => (2, width, height, None),
        StandardImageFrameFormat::RGB24 => (3, 3, width, Some(height)),
        StandardImageFrameFormat::RGB32 => (3, 4, width, Some(height)),
    }
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

fn parse_header_cards(bytes: &[u8]) -> Result<(HashMap<String, String>, usize), VastError> {
    let mut headers = HashMap::new();
    let mut offset = 0;

    while offset + FITS_CARD_SIZE <= bytes.len() {
        let card = std::str::from_utf8(&bytes[offset..offset + FITS_CARD_SIZE])
            .map_err(|err| file_error(format!("invalid FITS header encoding: {err}")))?;
        let key = card[..8].trim();

        offset += FITS_CARD_SIZE;

        if key == "END" {
            let header_end = offset.div_ceil(FITS_BLOCK_SIZE) * FITS_BLOCK_SIZE;
            return Ok((headers, header_end));
        }

        if let Some(value) = parse_card_value(card) {
            headers.insert(key.to_string(), value);
        }
    }

    Err(file_error("missing FITS END header".to_string()))
}

fn parse_card_value(card: &str) -> Option<String> {
    let (_, rest) = card.split_once('=')?;
    let value = rest.split_once('/').map(|(value, _)| value).unwrap_or(rest);
    Some(value.trim().trim_matches(' ').trim_matches('"').trim_matches('\'').to_string())
}

fn format_header_value(value: &FitsHeaderValue) -> String {
    match value {
        FitsHeaderValue::String(value) => format!("'{:<8}'", escape_header_string(value)),
        FitsHeaderValue::Integer(value) => format!("{value:>20}"),
        FitsHeaderValue::Float(value) => format!("{value:>20.10}"),
        FitsHeaderValue::Boolean(value) => format!("{:>20}", if *value { "T" } else { "F" }),
    }
}

fn header_u32(headers: &HashMap<String, String>, key: &str) -> Result<u32, VastError> {
    headers
        .get(key)
        .ok_or_else(|| file_error(format!("missing FITS header {key}")))?
        .parse::<u32>()
        .map_err(|err| file_error(format!("invalid FITS header {key}: {err}")))
}

fn header_optional_u32(headers: &HashMap<String, String>, key: &str) -> Result<Option<u32>, VastError> {
    headers
        .get(key)
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|err| file_error(format!("invalid FITS header {key}: {err}")))
        })
        .transpose()
}

fn header_i64(headers: &HashMap<String, String>, key: &str) -> Result<i64, VastError> {
    headers
        .get(key)
        .ok_or_else(|| file_error(format!("missing FITS header {key}")))?
        .parse::<i64>()
        .map_err(|err| file_error(format!("invalid FITS header {key}: {err}")))
}

fn header_f64(headers: &HashMap<String, String>, key: &str) -> Result<f64, VastError> {
    headers
        .get(key)
        .ok_or_else(|| file_error(format!("missing FITS header {key}")))?
        .parse::<f64>()
        .map_err(|err| file_error(format!("invalid FITS header {key}: {err}")))
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
        "SIMPLE"
            | "BITPIX"
            | "NAXIS"
            | "NAXIS1"
            | "NAXIS2"
            | "NAXIS3"
            | "BZERO"
            | "BSCALE"
            | "LVFMT"
            | "EXTEND"
    )
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
        .ok_or_else(|| file_error("FITS image dimensions overflow".to_string()))
}

fn frame_format_from_headers(
    headers: &HashMap<String, String>,
    naxis: u32,
    naxis1: u32,
    naxis3: Option<u32>,
    bitpix: i64,
    bzero: f64,
) -> Result<StandardImageFrameFormat, VastError> {
    if let Some(value) = headers.get("LVFMT") {
        return parse_frame_format(value);
    }

    match (naxis, naxis1, naxis3, bitpix, bzero) {
        (2, _, None, 8, _) => Ok(StandardImageFrameFormat::RAW8),
        (2, _, None, 16, 32768.0) => Ok(StandardImageFrameFormat::RAW16),
        (3, 3, Some(_), 8, _) => Ok(StandardImageFrameFormat::RGB24),
        (3, 4, Some(_), 8, _) => Ok(StandardImageFrameFormat::RGB32),
        _ => Err(file_error(format!(
            "unsupported FITS layout: NAXIS={naxis}, NAXIS1={naxis1}, NAXIS3={:?}, BITPIX={bitpix}, BZERO={bzero}",
            naxis3
        ))),
    }
}

fn parse_frame_format(value: &str) -> Result<StandardImageFrameFormat, VastError> {
    match value.trim() {
        "RAW8" => Ok(StandardImageFrameFormat::RAW8),
        "RAW10" => Ok(StandardImageFrameFormat::RAW10),
        "RAW12" => Ok(StandardImageFrameFormat::RAW12),
        "RAW14" => Ok(StandardImageFrameFormat::RAW14),
        "RAW16" => Ok(StandardImageFrameFormat::RAW16),
        "RGB24" => Ok(StandardImageFrameFormat::RGB24),
        "RGB32" => Ok(StandardImageFrameFormat::RGB32),
        _ => Err(file_error(format!("unsupported FITS LVFMT value: {value}"))),
    }
}

fn image_dimensions_from_axes(
    naxis: u32,
    naxis1: u32,
    naxis2: u32,
    naxis3: Option<u32>,
    format: StandardImageFrameFormat,
) -> Result<(u32, u32), VastError> {
    match format {
        StandardImageFrameFormat::RAW8
        | StandardImageFrameFormat::RAW10
        | StandardImageFrameFormat::RAW12
        | StandardImageFrameFormat::RAW14
        | StandardImageFrameFormat::RAW16 => {
            if naxis != 2 {
                return Err(file_error(format!(
                    "invalid FITS axis count for monochrome frame: {naxis}"
                )));
            }
            Ok((naxis1, naxis2))
        }
        StandardImageFrameFormat::RGB24 => {
            if naxis != 3 || naxis1 != 3 {
                return Err(file_error(format!(
                    "invalid FITS axis layout for RGB24 frame: NAXIS={naxis}, NAXIS1={naxis1}"
                )));
            }
            Ok((naxis2, naxis3.ok_or_else(|| file_error("missing FITS header NAXIS3".to_string()))?))
        }
        StandardImageFrameFormat::RGB32 => {
            if naxis != 3 || naxis1 != 4 {
                return Err(file_error(format!(
                    "invalid FITS axis layout for RGB32 frame: NAXIS={naxis}, NAXIS1={naxis1}"
                )));
            }
            Ok((naxis2, naxis3.ok_or_else(|| file_error("missing FITS header NAXIS3".to_string()))?))
        }
    }
}

fn bitpix(format: StandardImageFrameFormat) -> i64 {
    match format {
        StandardImageFrameFormat::RAW8 => 8,
        StandardImageFrameFormat::RAW10
        | StandardImageFrameFormat::RAW12
        | StandardImageFrameFormat::RAW14
        | StandardImageFrameFormat::RAW16 => 16,
        StandardImageFrameFormat::RGB24 | StandardImageFrameFormat::RGB32 => 8,
    }
}

fn bzero(format: StandardImageFrameFormat) -> f64 {
    match format {
        StandardImageFrameFormat::RAW10
        | StandardImageFrameFormat::RAW12
        | StandardImageFrameFormat::RAW14
        | StandardImageFrameFormat::RAW16 => 32768.0,
        StandardImageFrameFormat::RAW8
        | StandardImageFrameFormat::RGB24
        | StandardImageFrameFormat::RGB32 => 0.0,
    }
}

fn fits_data_bytes(data: Vec<u8>, format: StandardImageFrameFormat) -> Vec<u8> {
    match format {
        StandardImageFrameFormat::RAW10
        | StandardImageFrameFormat::RAW12
        | StandardImageFrameFormat::RAW14
        | StandardImageFrameFormat::RAW16 => data
            .chunks_exact(2)
            .flat_map(|chunk| {
                let value = u16::from_ne_bytes([chunk[0], chunk[1]]) as i32;
                let signed_value = (value - 32768) as i16;
                signed_value.to_be_bytes()
            })
            .collect(),
        StandardImageFrameFormat::RAW8
        | StandardImageFrameFormat::RGB24
        | StandardImageFrameFormat::RGB32 => data,
    }
}

fn read_fits_data(data: &[u8], format: StandardImageFrameFormat) -> Vec<u8> {
    match format {
        StandardImageFrameFormat::RAW10
        | StandardImageFrameFormat::RAW12
        | StandardImageFrameFormat::RAW14
        | StandardImageFrameFormat::RAW16 => data
            .chunks_exact(2)
            .flat_map(|chunk| {
                let value = i16::from_be_bytes([chunk[0], chunk[1]]) as i32;
                ((value + 32768) as u16).to_ne_bytes()
            })
            .collect(),
        StandardImageFrameFormat::RAW8
        | StandardImageFrameFormat::RGB24
        | StandardImageFrameFormat::RGB32 => {
            data.to_vec()
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file_path() -> String {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("/tmp/opencode/lvast-fits-test-{unique}.fits")
    }

    #[test]
    fn reads_back_raw16_fits() {
        let path = temp_file_path();
        let saver = FitsImageSaver::new(2, 1, StandardImageFrameFormat::RAW16);

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
    fn reads_back_raw12_fits() {
        let path = temp_file_path();
        let saver = FitsImageSaver::new(2, 1, StandardImageFrameFormat::RAW12);

        saver
            .save(vec![0x34, 0x12, 0x78, 0x06], None, path.clone())
            .unwrap();

        let frame = saver.read(path.clone()).unwrap();

        assert_eq!(frame.width, 2);
        assert_eq!(frame.height, 1);
        assert_eq!(frame.format, StandardImageFrameFormat::RAW12);
        assert_eq!(frame.data, vec![0x34, 0x12, 0x78, 0x06]);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn reads_back_rgb24_fits() {
        let path = temp_file_path();
        let saver = FitsImageSaver::new(2, 1, StandardImageFrameFormat::RGB24);

        saver
            .save(vec![255, 0, 0, 0, 255, 0], None, path.clone())
            .unwrap();

        let frame = saver.read(path.clone()).unwrap();

        assert_eq!(frame.width, 2);
        assert_eq!(frame.height, 1);
        assert_eq!(frame.format, StandardImageFrameFormat::RGB24);
        assert_eq!(frame.data, vec![255, 0, 0, 0, 255, 0]);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn reads_back_rgb32_fits() {
        let path = temp_file_path();
        let saver = FitsImageSaver::new(1, 1, StandardImageFrameFormat::RGB32);

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
