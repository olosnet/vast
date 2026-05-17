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
pub struct VastCameraCapGain {
    pub min: u32,
    pub max: u32,
    pub step: u32,
}

#[derive(Clone)]
pub struct VastCameraCapISO {
    pub min: u32,
    pub max: u32,
    pub multiplier: u32,
}

#[derive(Clone)]
pub struct VastCameraCapOffset {
    pub min: u32,
    pub max: u32,
    pub step: u32,
}

#[derive(Clone)]
pub struct VastCameraCapCooler {
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

#[derive(Clone)]
pub struct VastCameraCapExposure {
    pub min_microseconds: u64,
    pub max_microseconds: u64,
    pub step: u8,
}

impl VastCameraCapExposure {
    pub fn min_milliseconds(&self) -> u64 {
        self.min_microseconds / 1_000
    }

    pub fn max_milliseconds(&self) -> u64 {
        self.max_microseconds / 1_000
    }

    pub fn min_seconds(&self) -> f64 {
        self.min_microseconds as f64 / 1_000_000.0
    }

    pub fn max_seconds(&self) -> f64 {
        self.max_microseconds as f64 / 1_000_000.0
    }
}

#[derive(Clone)]
pub struct VastCameraCapRoiCombination {
    pub bin: u32,
    pub max_width: u32,
    pub max_height: u32,
    pub width_step: u32,
    pub height_step: u32,
}

#[derive(Clone)]
pub struct VastCameraCapRoi {
    pub combinations: Vec<VastCameraCapRoiCombination>,
}

#[derive(Clone)]
pub struct VastCameraCapBinning {
    pub modes: Vec<u32>,
}

#[derive(Clone)]
pub struct VastCameraCapGuide {
    pub pulse_guide: bool,
}

#[derive(Clone)]
pub struct VastCameraCapabilities {
    pub gain: Option<VastCameraCapGain>,
    pub iso: Option<VastCameraCapISO>,
    pub offset: Option<VastCameraCapOffset>,
    pub cooler: Option<VastCameraCapCooler>,
    pub roi: Option<VastCameraCapRoi>,
    pub binning: Option<VastCameraCapBinning>,
    pub guide: Option<VastCameraCapGuide>,
    pub exposure: VastCameraCapExposure,
    pub frame_formats: Vec<CameraFrameFormat>,
    pub bayer_pattern: Option<CameraBayerPattern>,
    pub max_height: u32,
    pub max_width: u32,
    pub adc_bits: u32,
}

impl Default for VastCameraCapabilities {
    fn default() -> Self {
        Self {
            gain: None,
            iso: None,
            offset: None,
            cooler: None,
            roi: None,
            binning: None,
            guide: None,
            frame_formats: Vec::new(),
            exposure: VastCameraCapExposure {
                min_microseconds: 0,
                max_microseconds: 0,
                step: 0,
            },
            bayer_pattern: None,
            max_height: 0,
            max_width: 0,
            adc_bits: 0,
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

    fn get_current_binning(&self) -> Result<(u32, u32), VastError>;
    fn get_current_roi(&self) -> Result<(u32, u32, u32, u32), VastError>;
    fn get_current_gain(&self) -> u32;
    fn get_current_iso(&self) -> u32;
    fn get_current_offset(&self) -> u32;
    fn get_current_cooler(&self) -> (bool, u32);
    fn get_current_exposure(&self) -> u64;

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
        lines.push("- Type: Color".to_string());
        lines.push(format!("- Bayer pattern: {pattern}"));
    } else {
        lines.push("- Type: Mono".to_string());
        lines.push("- Bayer pattern: none".to_string());
    }

    if capabilities.adc_bits > 0 {
        lines.push(format!("- ADC: {} bit", capabilities.adc_bits));
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

    if capabilities.exposure.max_microseconds > 0 {
        lines.push(format!(
            "- Exposure: {}..{} us ({}..{} ms, {:.3}..{:.3} s) step {}",
            capabilities.exposure.min_microseconds,
            capabilities.exposure.max_microseconds,
            capabilities.exposure.min_milliseconds(),
            capabilities.exposure.max_milliseconds(),
            capabilities.exposure.min_seconds(),
            capabilities.exposure.max_seconds(),
            capabilities.exposure.step
        ));
    }

    if let Some(roi) = &capabilities.roi {
        if roi.combinations.is_empty() {
            lines.push("- ROI: yes".to_string());
        } else {
            let combinations = roi
                .combinations
                .iter()
                .map(|combination| {
                    format!(
                        "bin {}: up to {}x{} px, step {}x{}",
                        combination.bin,
                        combination.max_width,
                        combination.max_height,
                        combination.width_step,
                        combination.height_step
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            lines.push(format!("- ROI: {combinations}"));
        }
    }

    if let Some(binning) = &capabilities.binning {
        let modes = binning
            .modes
            .iter()
            .map(|mode| format!("{}x{}", mode, mode))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("- Binning: {modes}"));
    }

    if let Some(guide) = &capabilities.guide {
        lines.push(format!(
            "- Guide: pulse guide {}",
            if guide.pulse_guide { "yes" } else { "no" }
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
