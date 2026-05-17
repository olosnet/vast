use crate::base::errors::VastError;
use std::{fmt::Display, sync::Arc};

pub enum CameraType {
    Reflex,
    DedicatedRGB,
    DedicatedMono,
}

#[derive(Clone)]
pub enum CameraBayerPattern {
    RGGB,
    BGGR,
    GRBG,
    GBRG,
}

impl Display for CameraBayerPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CameraBayerPattern::RGGB => write!(f, "RGGB"),
            CameraBayerPattern::BGGR => write!(f, "BGGR"),
            CameraBayerPattern::GRBG => write!(f, "GRBG"),
            CameraBayerPattern::GBRG => write!(f, "GBRG"),
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum CameraFrameFormat {
    RAW8,
    RAW16,
    RAW14,
    RAW12,
    RAW10,
    RGB24,
    RGB32,
}

impl Display for CameraFrameFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CameraFrameFormat::RAW8 => write!(f, "RAW8"),
            CameraFrameFormat::RAW16 => write!(f, "RAW16"),
            CameraFrameFormat::RAW14 => write!(f, "RAW14"),
            CameraFrameFormat::RAW12 => write!(f, "RAW12"),
            CameraFrameFormat::RAW10 => write!(f, "RAW10"),
            CameraFrameFormat::RGB24 => write!(f, "RGB24"),
            CameraFrameFormat::RGB32 => write!(f, "RGB32"),
        }
    }
}

pub enum CameraCapabilities {
    Color,
    Mono,
    Gain,
    ISO,
    Offset,
    Cooler,
    Roi,
    Binning,
    PulseGuide,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VastCameraID {
    StrID(String),
    IntID(i32),
}

impl Display for VastCameraID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VastCameraID::StrID(s) => write!(f, "{}", s),
            VastCameraID::IntID(i) => write!(f, "{}", i),
        }
    }
}

impl From<VastCameraID> for i32 {
    fn from(value: VastCameraID) -> Self {
        match value {
            VastCameraID::StrID(_) => 0,
            VastCameraID::IntID(i) => i,
        }
    }
}

impl From<VastCameraID> for String {
    fn from(value: VastCameraID) -> Self {
        match value {
            VastCameraID::StrID(s) => s,
            VastCameraID::IntID(i) => i.to_string(),
        }
    }
}

pub struct VastCameraInfo {
    pub id: VastCameraID,
    pub name: String,
    pub serial_number: String,
    pub raw_extra_info: String,
}

#[derive(Clone)]
pub struct VastCameraGain {
    pub min: u32,
    pub max: u32,
    pub step: u32,
}

#[derive(Clone)]
pub struct VastCameraISO {
    pub min: u32,
    pub max: u32,
    pub multiplier: u32,
}

#[derive(Clone)]
pub struct VastCameraOffset {
    pub min: u32,
    pub max: u32,
    pub step: u32,
}

#[derive(Clone)]
pub struct VastCameraCooler {
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

#[derive(Clone)]
pub struct VastCameraCapabilities {
    pub gain: Option<VastCameraGain>,
    pub iso: Option<VastCameraISO>,
    pub offset: Option<VastCameraOffset>,
    pub cooler: Option<VastCameraCooler>,
    pub frame_formats: Vec<CameraFrameFormat>,
    pub bayer_pattern: Option<CameraBayerPattern>,
    pub max_height: u32,
    pub max_width: u32,
}

impl Default for VastCameraCapabilities {
    fn default() -> Self {
        Self {
            gain: None,
            iso: None,
            offset: None,
            cooler: None,
            frame_formats: Vec::new(),
            bayer_pattern: None,
            max_height: 0,
            max_width: 0,
        }
    }
}

pub trait VastCameraDriver {
    fn new() -> Self;
    fn init(&mut self) -> Result<Vec<VastCameraInfo>, VastError>;
    fn id(&self) -> &str;
    fn get_manufacturer(&self) -> &str;
    fn get_version(&self) -> &str;
}

pub trait VastCamera<IDT, T: VastCameraDriver> {
    fn new(driver: Arc<T>) -> Self;

    fn connect(&mut self, camera_id: IDT) -> Result<(), VastError>;

    fn camera_info_str(&self) -> String;

    fn get_name(&self) -> &str;
    fn get_capabilities(&self) -> VastCameraCapabilities;

    fn get_bayer_pattern(&self) -> &Option<CameraBayerPattern>;
    fn get_max_height(&self) -> u32;
    fn get_max_width(&self) -> u32;

    fn get_current_binning(&self) -> Result<(u32, u32), VastError>;
    fn get_current_roi(&self) -> Result<(u32, u32, u32, u32), VastError>;
    fn get_current_gain(&self) -> u32;
    fn get_current_iso(&self) -> u32;
    fn get_current_offset(&self) -> u32;
    fn get_current_cooler(&self) -> (bool, u32);

    fn set_gain(&mut self, gain: u32);
    fn set_iso(&mut self, iso: u32);
    fn set_offset(&mut self, offset: u32);
    fn set_cooler(&mut self, on: bool, temperature: u32);
    fn set_roi(&mut self, x: u32, y: u32, width: u32, height: u32);
    fn set_binning(&mut self, h: u32, v: u32);

    fn disconnect(&mut self) -> Result<(), VastError>;
}

pub fn fancy_info_str(capabilities: &VastCameraCapabilities) -> String {
    let mut lines = vec![
        "Camera capabilities".to_string(),
        format!(
            "- Sensor: {}x{} px",
            capabilities.max_width, capabilities.max_height
        ),
    ];

    if let Some(pattern) = &capabilities.bayer_pattern {
        lines.push(format!("- Color: yes ({pattern} Bayer)"));
    } else {
        lines.push("- Color: mono".to_string());
    }

    if capabilities.frame_formats.is_empty() {
        lines.push("- Formats: none reported".to_string());
    } else {
        let formats = capabilities
            .frame_formats
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("- Formats: {formats}"));
    }

    if let Some(gain) = &capabilities.gain {
        lines.push(format!(
            "- Gain: {}..{} step {}",
            gain.min, gain.max, gain.step
        ));
    }

    if let Some(iso) = &capabilities.iso {
        lines.push(format!(
            "- ISO: {}..{} x{}",
            iso.min, iso.max, iso.multiplier
        ));
    }

    if let Some(offset) = &capabilities.offset {
        lines.push(format!(
            "- Offset: {}..{} step {}",
            offset.min, offset.max, offset.step
        ));
    }

    if let Some(cooler) = &capabilities.cooler {
        lines.push(format!(
            "- Cooler: {:.1}..{:.1} C step {:.1}",
            cooler.min, cooler.max, cooler.step
        ));
    }

    lines.join("\n")
}
