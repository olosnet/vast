use crate::{
    base::errors::VastError,
    types::camera::{CameraFrameFormat, VastCameraFrame},
};
use std::fmt::Display;

pub trait ImageFrameFormat: Copy + Clone + std::fmt::Debug + PartialEq + Eq {
    fn name(&self) -> &'static str;
    fn bytes_per_pixel(&self) -> usize;
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum StandardImageFrameFormat {
    RAW8,
    RAW16,
    RAW14,
    RAW12,
    RAW10,
    RGB24,
    RGB32,
}

impl ImageFrameFormat for StandardImageFrameFormat {
    fn name(&self) -> &'static str {
        match self {
            StandardImageFrameFormat::RAW8 => "RAW8",
            StandardImageFrameFormat::RAW16 => "RAW16",
            StandardImageFrameFormat::RAW14 => "RAW14",
            StandardImageFrameFormat::RAW12 => "RAW12",
            StandardImageFrameFormat::RAW10 => "RAW10",
            StandardImageFrameFormat::RGB24 => "RGB24",
            StandardImageFrameFormat::RGB32 => "RGB32",
        }
    }

    fn bytes_per_pixel(&self) -> usize {
        match self {
            StandardImageFrameFormat::RAW8 => 1,
            StandardImageFrameFormat::RAW16
            | StandardImageFrameFormat::RAW14
            | StandardImageFrameFormat::RAW12
            | StandardImageFrameFormat::RAW10 => 2,
            StandardImageFrameFormat::RGB24 => 3,
            StandardImageFrameFormat::RGB32 => 4,
        }
    }
}

impl Display for StandardImageFrameFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageFrame {
    pub width: u32,
    pub height: u32,
    pub format: StandardImageFrameFormat,
    pub data: Vec<u8>,
}

impl From<CameraFrameFormat> for StandardImageFrameFormat {
    fn from(value: CameraFrameFormat) -> Self {
        match value {
            CameraFrameFormat::RAW8 => Self::RAW8,
            CameraFrameFormat::RAW16 => Self::RAW16,
            CameraFrameFormat::RAW14 => Self::RAW14,
            CameraFrameFormat::RAW12 => Self::RAW12,
            CameraFrameFormat::RAW10 => Self::RAW10,
            CameraFrameFormat::RGB24 => Self::RGB24,
            CameraFrameFormat::RGB32 => Self::RGB32,
        }
    }
}

impl From<StandardImageFrameFormat> for CameraFrameFormat {
    fn from(value: StandardImageFrameFormat) -> Self {
        match value {
            StandardImageFrameFormat::RAW8 => Self::RAW8,
            StandardImageFrameFormat::RAW16 => Self::RAW16,
            StandardImageFrameFormat::RAW14 => Self::RAW14,
            StandardImageFrameFormat::RAW12 => Self::RAW12,
            StandardImageFrameFormat::RAW10 => Self::RAW10,
            StandardImageFrameFormat::RGB24 => Self::RGB24,
            StandardImageFrameFormat::RGB32 => Self::RGB32,
        }
    }
}

impl From<VastCameraFrame> for ImageFrame {
    fn from(value: VastCameraFrame) -> Self {
        Self {
            width: value.width,
            height: value.height,
            format: value.format.into(),
            data: value.data,
        }
    }
}

impl From<ImageFrame> for VastCameraFrame {
    fn from(value: ImageFrame) -> Self {
        Self {
            width: value.width,
            height: value.height,
            format: value.format.into(),
            data: value.data,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
/// Optional metadata used to populate FITS header keywords.
pub struct ImageHeaders {
    /// Capture software name written as `CREATOR`.
    pub software: Option<String>,
    /// Frame type written as `IMAGETYP`.
    pub image_type: Option<String>,
    /// Target object name written as `OBJECT`.
    pub object: Option<String>,
    /// Camera or instrument name written as `INSTRUME`.
    pub instrument: Option<String>,
    /// Telescope name written as `TELESCOP`.
    pub telescope: Option<String>,
    /// Observer name written as `OBSERVER`.
    pub observer: Option<String>,
    /// UTC observation start written as `DATE-OBS`.
    pub date_obs: Option<String>,
    /// Exposure duration in seconds written as `EXPTIME` and `EXPOSURE`.
    pub exposure_seconds: Option<f64>,
    /// Filter name written as `FILTER`.
    pub filter: Option<String>,
    /// Camera gain written as `GAIN`.
    pub gain: Option<u32>,
    /// Camera offset written as `OFFSET`.
    pub offset: Option<u32>,
    /// Sensor temperature in Celsius written as `CCD-TEMP`.
    pub ccd_temperature: Option<f64>,
    /// Cooler target temperature in Celsius written as `SET-TEMP`.
    pub target_temperature: Option<f64>,
    /// Horizontal binning written as `XBINNING`.
    pub bin_x: Option<u32>,
    /// Vertical binning written as `YBINNING`.
    pub bin_y: Option<u32>,
    /// Subframe X origin written as `XORGSUBF`.
    pub frame_x: Option<u32>,
    /// Subframe Y origin written as `YORGSUBF`.
    pub frame_y: Option<u32>,
    /// Subframe width in pixels.
    pub frame_width: Option<u32>,
    /// Subframe height in pixels.
    pub frame_height: Option<u32>,
    /// Bayer pattern string written as `BAYERPAT`.
    pub bayer_pattern: Option<String>,
    /// Right ascension in degrees written as `RA`.
    pub ra_degrees: Option<f64>,
    /// Declination in degrees written as `DEC`.
    pub dec_degrees: Option<f64>,
    /// Pixel scale in arcseconds per pixel written as `SCALE`.
    pub pixel_scale_arcsec: Option<f64>,
    /// Telescope focal length in millimeters written as `FOCALLEN`.
    pub focal_length_mm: Option<f64>,
    /// Telescope aperture in millimeters written as `APTDIA`.
    pub aperture_mm: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
/// Typed FITS header value.
pub enum FitsHeaderValue {
    /// String header value.
    String(String),
    /// Integer header value.
    Integer(i64),
    /// Floating-point header value.
    Float(f64),
    /// Boolean header value.
    Boolean(bool),
}

impl From<&str> for FitsHeaderValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for FitsHeaderValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<u32> for FitsHeaderValue {
    fn from(value: u32) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<i64> for FitsHeaderValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<f64> for FitsHeaderValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<bool> for FitsHeaderValue {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
/// One FITS header card before serialization to the 80-byte FITS card format.
pub struct HeaderCard {
    /// FITS keyword. Must be valid for the target writer.
    pub key: &'static str,
    /// FITS keyword value.
    pub value: FitsHeaderValue,
    /// FITS comment associated with the keyword.
    pub comment: &'static str,
}

impl ImageHeaders {
    /// Converts optional image metadata into FITS header cards.
    pub fn to_fits_headers(&self) -> Vec<HeaderCard> {
        let mut headers = Vec::new();

        headers.push(HeaderCard {
            key: "SIMPLE",
            value: true.into(),
            comment: "file conforms to FITS standard",
        });
        headers.push(HeaderCard {
            key: "EXTEND",
            value: true.into(),
            comment: "FITS dataset may contain extensions",
        });

        push_optional(
            &mut headers,
            "CREATOR",
            self.software.clone(),
            "Capture software",
        );
        push_optional(
            &mut headers,
            "IMAGETYP",
            self.image_type.clone(),
            "Image type",
        );
        push_optional(&mut headers, "OBJECT", self.object.clone(), "Target name");
        push_optional(
            &mut headers,
            "INSTRUME",
            self.instrument.clone(),
            "Imaging instrument",
        );
        push_optional(
            &mut headers,
            "TELESCOP",
            self.telescope.clone(),
            "Telescope",
        );
        push_optional(&mut headers, "OBSERVER", self.observer.clone(), "Observer");
        push_optional(
            &mut headers,
            "DATE-OBS",
            self.date_obs.clone(),
            "UTC observation start",
        );
        push_optional(&mut headers, "FILTER", self.filter.clone(), "Filter name");
        push_optional(
            &mut headers,
            "EXPTIME",
            self.exposure_seconds,
            "Exposure time (s)",
        );
        push_optional(
            &mut headers,
            "EXPOSURE",
            self.exposure_seconds,
            "Exposure time (s)",
        );
        push_optional(&mut headers, "GAIN", self.gain, "Camera gain");
        push_optional(&mut headers, "OFFSET", self.offset, "Camera offset");
        push_optional(
            &mut headers,
            "CCD-TEMP",
            self.ccd_temperature,
            "CCD temperature (C)",
        );
        push_optional(
            &mut headers,
            "SET-TEMP",
            self.target_temperature,
            "Target temperature (C)",
        );
        push_optional(&mut headers, "XBINNING", self.bin_x, "X binning");
        push_optional(&mut headers, "YBINNING", self.bin_y, "Y binning");
        push_optional(&mut headers, "XORGSUBF", self.frame_x, "Subframe X origin");
        push_optional(&mut headers, "YORGSUBF", self.frame_y, "Subframe Y origin");
        push_optional(
            &mut headers,
            "XPIXSZ",
            self.frame_width,
            "Subframe width (px)",
        );
        push_optional(
            &mut headers,
            "YPIXSZ",
            self.frame_height,
            "Subframe height (px)",
        );
        push_optional(
            &mut headers,
            "BAYERPAT",
            self.bayer_pattern.clone(),
            "Bayer pattern",
        );
        push_optional(
            &mut headers,
            "RA",
            self.ra_degrees,
            "Right ascension (degrees)",
        );
        push_optional(
            &mut headers,
            "DEC",
            self.dec_degrees,
            "Declination (degrees)",
        );
        push_optional(
            &mut headers,
            "SCALE",
            self.pixel_scale_arcsec,
            "Image scale (arcsec/pixel)",
        );
        push_optional(
            &mut headers,
            "FOCALLEN",
            self.focal_length_mm,
            "Focal length (mm)",
        );
        push_optional(
            &mut headers,
            "APTDIA",
            self.aperture_mm,
            "Aperture diameter (mm)",
        );

        headers
    }
}

fn push_optional<T: Into<FitsHeaderValue>>(
    headers: &mut Vec<HeaderCard>,
    key: &'static str,
    value: Option<T>,
    comment: &'static str,
) {
    if let Some(value) = value {
        headers.push(HeaderCard {
            key,
            value: value.into(),
            comment,
        });
    }
}

pub trait ImageSaver {
    fn supported_formats(&self) -> &'static [StandardImageFrameFormat];

    /// Saves image bytes with optional header cards to `path`.
    fn save(
        &self,
        data: Vec<u8>,
        image_headers: Option<Vec<HeaderCard>>,
        path: String,
    ) -> Result<(), VastError>;
}

pub trait ImageReader {
    fn supported_formats(&self) -> &'static [StandardImageFrameFormat];

    /// Reads image file from `path` and returns decoded frame bytes.
    fn read(&self, path: String) -> Result<ImageFrame, VastError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_and_image_formats_round_trip() {
        for format in [
            CameraFrameFormat::RAW8,
            CameraFrameFormat::RAW16,
            CameraFrameFormat::RAW14,
            CameraFrameFormat::RAW12,
            CameraFrameFormat::RAW10,
            CameraFrameFormat::RGB24,
            CameraFrameFormat::RGB32,
        ] {
            let image_format: StandardImageFrameFormat = format.into();
            let camera_format: CameraFrameFormat = image_format.into();
            assert_eq!(camera_format, format);
        }
    }

    #[test]
    fn camera_and_image_frames_round_trip() {
        let camera_frame = VastCameraFrame {
            width: 2,
            height: 1,
            format: CameraFrameFormat::RGB24,
            data: vec![1, 2, 3, 4, 5, 6],
        };

        let image_frame: ImageFrame = camera_frame.clone().into();
        let converted_camera_frame: VastCameraFrame = image_frame.into();

        assert_eq!(converted_camera_frame, camera_frame);
    }
}
