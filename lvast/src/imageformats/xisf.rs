//! Minimal XISF image reader/writer.
//!
//! Limitations:
//! - Implements only monolithic XISF files with one image and one attached uncompressed data block.
//! - Reader expects files close to `lvast` output and does not implement general XISF parsing.
//! - Compression, distributed storage, multiple images, signatures, and rich metadata are not supported.

use crate::{
    base::errors::{VastError, VastErrorType},
    imageformats::types::{
        HeaderCard, ImageFrame, ImageFrameFormat, ImageReader, ImageSaver,
        StandardImageFrameFormat,
    },
};

const XISF_SIGNATURE: &[u8; 8] = b"XISF0100";
const XISF_RESERVED_BYTES: [u8; 4] = [0, 0, 0, 0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XisfImageSaver {
    pub width: u32,
    pub height: u32,
    pub format: StandardImageFrameFormat,
}

impl XisfImageSaver {
    pub fn new(width: u32, height: u32, format: StandardImageFrameFormat) -> Self {
        Self {
            width,
            height,
            format,
        }
    }
}

impl ImageSaver for XisfImageSaver {
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
                "invalid XISF image data length: got {}, expected {}",
                data.len(),
                expected_len
            )));
        }

        let data_block = encode_data_block(data, self.format);
        let image_geometry = image_geometry(self.width, self.height, self.format);
        let sample_format = sample_format(self.format);
        let color_space = color_space(self.format);

        let mut header = String::new();
        let provisional_location = format!("attachment:{:020}:{:020}", 0, data_block.len());
        header.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
        header.push_str(
            "<xisf version=\"1.0\" xmlns=\"http://www.pixinsight.com/xisf\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:schemaLocation=\"http://www.pixinsight.com/xisf http://pixinsight.com/xisf/xisf-1.0.xsd\">",
        );
        header.push_str("<Metadata>");
        header.push_str(
            "<Property id=\"XISF:CreationTime\" type=\"String\" value=\"1970-01-01T00:00:00Z\"/>",
        );
        header.push_str(
            "<Property id=\"XISF:CreatorApplication\" type=\"String\" value=\"lvast\"/>",
        );
        header.push_str("</Metadata>");
        header.push_str(&format!(
            "<Image geometry=\"{image_geometry}\" sampleFormat=\"{sample_format}\"{} location=\"{provisional_location}\">",
            color_space
                .map(|space| format!(" colorSpace=\"{space}\""))
                .unwrap_or_default()
        ));
        header.push_str(&format!(
            "<Property id=\"LVFMT\" type=\"String\" value=\"{}\"/>",
            self.format.name()
        ));
        header.push_str("</Image></xisf>");

        let initial_header_len = header.len();
        let data_offset = 16 + initial_header_len;
        let final_location = format!(
            "attachment:{:020}:{:020}",
            data_offset,
            data_block.len()
        );
        let final_header = header.replacen(&provisional_location, &final_location, 1);
        if final_header.len() != initial_header_len {
            return Err(file_error(
                "XISF header size changed while fixing attachment location".to_string(),
            ));
        }

        let header_len = u32::try_from(final_header.len())
            .map_err(|_| file_error("XISF header too large".to_string()))?;

        let mut bytes = Vec::with_capacity(16 + final_header.len() + data_block.len());
        bytes.extend_from_slice(XISF_SIGNATURE);
        bytes.extend_from_slice(&header_len.to_le_bytes());
        bytes.extend_from_slice(&XISF_RESERVED_BYTES);
        bytes.extend_from_slice(final_header.as_bytes());
        bytes.extend_from_slice(&data_block);

        std::fs::write(&path, bytes)
            .map_err(|err| file_error(format!("failed to write XISF file {path}: {err}")))
    }
}

impl ImageReader for XisfImageSaver {
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
            .map_err(|err| file_error(format!("failed to read XISF file {path}: {err}")))?;
        if bytes.len() < 16 || &bytes[..8] != XISF_SIGNATURE {
            return Err(file_error("invalid XISF file signature".to_string()));
        }

        let header_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        let header_start: usize = 16;
        let header_end = header_start
            .checked_add(header_len)
            .ok_or_else(|| file_error("XISF header length overflow".to_string()))?;
        if bytes.len() < header_end {
            return Err(file_error("truncated XISF header".to_string()));
        }

        let header = std::str::from_utf8(&bytes[header_start..header_end])
            .map_err(|err| file_error(format!("invalid XISF header encoding: {err}")))?;
        let image_tag = extract_element(header, "Image")?;
        let geometry = parse_attr(&image_tag, "geometry")?;
        let sample_format_value = parse_attr(&image_tag, "sampleFormat")?;
        let location = parse_attr(&image_tag, "location")?;
        let lvfmt = extract_property_value(&image_tag, "LVFMT").ok();

        let (width, height, channels) = parse_geometry(&geometry)?;
        let format = if let Some(lvfmt) = lvfmt {
            parse_format_name(&lvfmt)?
        } else {
            infer_format_from_attrs(&sample_format_value, channels)?
        };

        let (data_offset, data_len) = parse_attachment_location(&location)?;
        let data_end = data_offset
            .checked_add(data_len)
            .ok_or_else(|| file_error("XISF data block length overflow".to_string()))?;
        if bytes.len() < data_end {
            return Err(file_error("truncated XISF data block".to_string()));
        }

        Ok(ImageFrame {
            width,
            height,
            format,
            data: decode_data_block(&bytes[data_offset..data_end], format),
        })
    }
}

fn encode_data_block(data: Vec<u8>, format: StandardImageFrameFormat) -> Vec<u8> {
    match format {
        StandardImageFrameFormat::RAW8
        | StandardImageFrameFormat::RGB24
        | StandardImageFrameFormat::RGB32 => data,
        StandardImageFrameFormat::RAW10
        | StandardImageFrameFormat::RAW12
        | StandardImageFrameFormat::RAW14
        | StandardImageFrameFormat::RAW16 => data
            .chunks_exact(2)
            .flat_map(|chunk| u16::from_ne_bytes([chunk[0], chunk[1]]).to_le_bytes())
            .collect(),
    }
}

fn decode_data_block(data: &[u8], format: StandardImageFrameFormat) -> Vec<u8> {
    match format {
        StandardImageFrameFormat::RAW8
        | StandardImageFrameFormat::RGB24
        | StandardImageFrameFormat::RGB32 => data.to_vec(),
        StandardImageFrameFormat::RAW10
        | StandardImageFrameFormat::RAW12
        | StandardImageFrameFormat::RAW14
        | StandardImageFrameFormat::RAW16 => data
            .chunks_exact(2)
            .flat_map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]).to_ne_bytes())
            .collect(),
    }
}

fn image_geometry(width: u32, height: u32, format: StandardImageFrameFormat) -> String {
    format!("{width}:{height}:{}", channel_count(format))
}

fn channel_count(format: StandardImageFrameFormat) -> u32 {
    match format {
        StandardImageFrameFormat::RAW8
        | StandardImageFrameFormat::RAW10
        | StandardImageFrameFormat::RAW12
        | StandardImageFrameFormat::RAW14
        | StandardImageFrameFormat::RAW16 => 1,
        StandardImageFrameFormat::RGB24 => 3,
        StandardImageFrameFormat::RGB32 => 4,
    }
}

fn sample_format(format: StandardImageFrameFormat) -> &'static str {
    match format {
        StandardImageFrameFormat::RAW8
        | StandardImageFrameFormat::RGB24
        | StandardImageFrameFormat::RGB32 => "UInt8",
        StandardImageFrameFormat::RAW10
        | StandardImageFrameFormat::RAW12
        | StandardImageFrameFormat::RAW14
        | StandardImageFrameFormat::RAW16 => "UInt16",
    }
}

fn color_space(format: StandardImageFrameFormat) -> Option<&'static str> {
    match format {
        StandardImageFrameFormat::RAW8
        | StandardImageFrameFormat::RAW10
        | StandardImageFrameFormat::RAW12
        | StandardImageFrameFormat::RAW14
        | StandardImageFrameFormat::RAW16 => None,
        StandardImageFrameFormat::RGB24 | StandardImageFrameFormat::RGB32 => Some("RGB"),
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
        .ok_or_else(|| file_error("XISF image dimensions overflow".to_string()))
}

fn extract_element(header: &str, name: &str) -> Result<String, VastError> {
    let start = header
        .find(&format!("<{name} "))
        .ok_or_else(|| file_error(format!("missing XISF element {name}")))?;
    let end = header[start..]
        .find(&format!("</{name}>"))
        .map(|index| start + index + name.len() + 3)
        .ok_or_else(|| file_error(format!("unterminated XISF element {name}")))?;
    Ok(header[start..end].to_string())
}

fn parse_attr(tag: &str, name: &str) -> Result<String, VastError> {
    let needle = format!("{name}=\"");
    let start = tag
        .find(&needle)
        .map(|index| index + needle.len())
        .ok_or_else(|| file_error(format!("missing XISF attribute {name}")))?;
    let end = tag[start..]
        .find('"')
        .map(|index| start + index)
        .ok_or_else(|| file_error(format!("unterminated XISF attribute {name}")))?;
    Ok(tag[start..end].to_string())
}

fn extract_property_value(tag: &str, id: &str) -> Result<String, VastError> {
    let needle = format!("<Property id=\"{id}\"");
    let start = tag
        .find(&needle)
        .ok_or_else(|| file_error(format!("missing XISF property {id}")))?;
    let property_end = tag[start..]
        .find("/>")
        .map(|index| start + index + 2)
        .ok_or_else(|| file_error(format!("unterminated XISF property {id}")))?;
    parse_attr(&tag[start..property_end], "value")
}

fn parse_geometry(value: &str) -> Result<(u32, u32, u32), VastError> {
    let mut parts = value.split(':');
    let width = parts
        .next()
        .ok_or_else(|| file_error("invalid XISF geometry".to_string()))?
        .parse::<u32>()
        .map_err(|err| file_error(format!("invalid XISF geometry width: {err}")))?;
    let height = parts
        .next()
        .ok_or_else(|| file_error("invalid XISF geometry".to_string()))?
        .parse::<u32>()
        .map_err(|err| file_error(format!("invalid XISF geometry height: {err}")))?;
    let channels = parts
        .next()
        .ok_or_else(|| file_error("invalid XISF geometry".to_string()))?
        .parse::<u32>()
        .map_err(|err| file_error(format!("invalid XISF geometry channels: {err}")))?;
    Ok((width, height, channels))
}

fn parse_attachment_location(value: &str) -> Result<(usize, usize), VastError> {
    let mut parts = value.split(':');
    let kind = parts
        .next()
        .ok_or_else(|| file_error("invalid XISF attachment location".to_string()))?;
    if kind != "attachment" {
        return Err(file_error(format!(
            "unsupported XISF location kind: {kind}"
        )));
    }

    let offset = parts
        .next()
        .ok_or_else(|| file_error("invalid XISF attachment location".to_string()))?
        .parse::<usize>()
        .map_err(|err| file_error(format!("invalid XISF attachment offset: {err}")))?;
    let len = parts
        .next()
        .ok_or_else(|| file_error("invalid XISF attachment location".to_string()))?
        .parse::<usize>()
        .map_err(|err| file_error(format!("invalid XISF attachment size: {err}")))?;
    Ok((offset, len))
}

fn infer_format_from_attrs(
    sample_format: &str,
    channels: u32,
) -> Result<StandardImageFrameFormat, VastError> {
    match (sample_format, channels) {
        ("UInt8", 1) => Ok(StandardImageFrameFormat::RAW8),
        ("UInt8", 3) => Ok(StandardImageFrameFormat::RGB24),
        ("UInt8", 4) => Ok(StandardImageFrameFormat::RGB32),
        ("UInt16", 1) => Ok(StandardImageFrameFormat::RAW16),
        _ => Err(file_error(format!(
            "unsupported XISF sampleFormat/channels combination: {sample_format}/{channels}"
        ))),
    }
}

fn parse_format_name(value: &str) -> Result<StandardImageFrameFormat, VastError> {
    match value.trim() {
        "RAW8" => Ok(StandardImageFrameFormat::RAW8),
        "RAW10" => Ok(StandardImageFrameFormat::RAW10),
        "RAW12" => Ok(StandardImageFrameFormat::RAW12),
        "RAW14" => Ok(StandardImageFrameFormat::RAW14),
        "RAW16" => Ok(StandardImageFrameFormat::RAW16),
        "RGB24" => Ok(StandardImageFrameFormat::RGB24),
        "RGB32" => Ok(StandardImageFrameFormat::RGB32),
        _ => Err(file_error(format!("unsupported XISF LVFMT value: {value}"))),
    }
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
        format!("/tmp/opencode/lvast-xisf-test-{unique}.{extension}")
    }

    #[test]
    fn saves_and_reads_raw16_xisf() {
        let path = temp_file_path("xisf");
        let saver = XisfImageSaver::new(2, 1, StandardImageFrameFormat::RAW16);

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
    fn saves_and_reads_rgb32_xisf() {
        let path = temp_file_path("xisf");
        let saver = XisfImageSaver::new(1, 1, StandardImageFrameFormat::RGB32);

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
