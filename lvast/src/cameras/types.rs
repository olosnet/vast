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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VastCameraFrame {
    pub width: u32,
    pub height: u32,
    pub format: CameraFrameFormat,
    pub data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VastCameraGuideDirection {
    North,
    South,
    East,
    West,
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
pub struct VastCameraCapRange {
    pub min: u32,
    pub max: u32,
    pub step: u32,
}

#[derive(Clone)]
pub struct VastCameraCapWhiteBalance {
    pub red: VastCameraCapRange,
    pub green: VastCameraCapRange,
    pub blue: VastCameraCapRange,
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
pub struct VastCameraCapGuiding {
    pub pulse_guide: bool,
}

#[derive(Clone)]
pub struct VastCameraCapabilities {
    pub gain: Option<VastCameraCapGain>,
    pub iso: Option<VastCameraCapISO>,
    pub offset: Option<VastCameraCapOffset>,
    pub cooler: Option<VastCameraCapCooler>,
    pub white_balance: Option<VastCameraCapWhiteBalance>,
    pub contrast: Option<VastCameraCapRange>,
    pub sharpness: Option<VastCameraCapRange>,
    pub saturation: Option<VastCameraCapRange>,
    pub usb_speed: Option<VastCameraCapRange>,
    pub roi: Option<VastCameraCapRoi>,
    pub binning: Option<VastCameraCapBinning>,
    pub guiding: Option<VastCameraCapGuiding>,
    pub exposure: VastCameraCapExposure,
    pub frame_formats: Vec<CameraFrameFormat>,
    pub bayer_pattern: Option<CameraBayerPattern>,
    pub max_height: u32,
    pub max_width: u32,
    pub adc_bits: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VastCameraSettings {
    pub exposure_microseconds: Option<u64>,
    pub gain: Option<u32>,
    pub iso: Option<u32>,
    pub offset: Option<u32>,
    pub cooler: Option<(bool, u32)>,
    pub white_balance: Option<(u32, u32, u32)>,
    pub contrast: Option<u32>,
    pub sharpness: Option<u32>,
    pub saturation: Option<u32>,
    pub usb_speed: Option<u32>,
    pub roi: Option<(u32, u32, u32, u32)>,
    pub binning: Option<(u32, u32)>,
}

impl VastCameraSettings {
    pub fn fancy_info_str(&self) -> String {
        let mut lines = vec!["Camera settings".to_string()];

        if let Some(exposure) = self.exposure_microseconds {
            lines.push(format!("- Exposure: {exposure} us"));
        }

        if let Some(gain) = self.gain {
            lines.push(format!("- Gain: {gain}"));
        }

        if let Some(iso) = self.iso {
            lines.push(format!("- ISO: {iso}"));
        }

        if let Some(offset) = self.offset {
            lines.push(format!("- Offset: {offset}"));
        }

        if let Some((enabled, temperature)) = self.cooler {
            lines.push(format!(
                "- Cooler: {}, target {} C",
                if enabled { "on" } else { "off" },
                temperature
            ));
        }

        if let Some((red, green, blue)) = self.white_balance {
            lines.push(format!("- White balance: R={red}, G={green}, B={blue}"));
        }

        if let Some(contrast) = self.contrast {
            lines.push(format!("- Contrast: {contrast}"));
        }

        if let Some(sharpness) = self.sharpness {
            lines.push(format!("- Sharpness: {sharpness}"));
        }

        if let Some(saturation) = self.saturation {
            lines.push(format!("- Saturation: {saturation}"));
        }

        if let Some(usb_speed) = self.usb_speed {
            lines.push(format!("- USB speed: {usb_speed}"));
        }

        if let Some((x, y, width, height)) = self.roi {
            lines.push(format!("- ROI: x={x}, y={y}, {}x{} px", width, height));
        }

        if let Some((horizontal, vertical)) = self.binning {
            lines.push(format!("- Binning: {}x{}", horizontal, vertical));
        }

        lines.join("\n")
    }
}

impl Default for VastCameraCapabilities {
    fn default() -> Self {
        Self {
            gain: None,
            iso: None,
            offset: None,
            cooler: None,
            white_balance: None,
            contrast: None,
            sharpness: None,
            saturation: None,
            usb_speed: None,
            roi: None,
            binning: None,
            guiding: None,
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

pub trait VastCameraDriver: Send + Sync {
    fn new() -> Self;
    fn init(&mut self) -> Result<Vec<VastCameraInfo>, VastError>;
    fn id(&self) -> &str;
    fn get_manufacturer(&self) -> &str;
    fn get_version(&self) -> &str;
}

pub trait VastCamera<IDT, T: VastCameraDriver>: Send + Sync {
    fn new(driver: Arc<T>) -> Self;

    fn connect(&mut self, camera_id: IDT) -> Result<(), VastError>;

    fn get_name(&self) -> &str;
    fn get_capabilities(&self) -> VastCameraCapabilities;

    fn get_current_offset(&self) -> u32;
    fn get_current_cooler(&self) -> (bool, u32);
    fn get_current_temperature(&self) -> f32;

    fn set_camera_settings(&mut self, settings: VastCameraSettings) -> Result<(), VastError>;
    fn get_camera_settings(&mut self) -> Result<VastCameraSettings, VastError>;
    fn get_settings(&self) -> VastCameraSettings;

    fn disconnect(&mut self) -> Result<(), VastError>;
}

pub trait VastCameraAcquireImage: Send + Sync {
    fn start_image_acquisition(&mut self) -> Result<(), VastError>;
    fn abort_image_acquisition(&mut self) -> Result<(), VastError>;
    fn get_acquired_image(&mut self, timeout_millis: u32) -> Result<VastCameraFrame, VastError>;
}

pub trait VastCameraGuide: Send + Sync {
    fn pulse_guide(
        &mut self,
        direction: VastCameraGuideDirection,
        duration_millis: u32,
    ) -> Result<(), VastError>;
}

pub trait VastCameraStreamingPreview: Send + Sync {
    fn start_streaming_preview(&mut self) -> Result<(), VastError>;
    fn get_streaming_preview_frame(
        &mut self,
        timeout_millis: u32,
    ) -> Result<VastCameraFrame, VastError>;
    fn stop_streaming_preview(&mut self) -> Result<(), VastError>;
}

impl VastCameraCapabilities {
    pub fn fancy_info_str(&self) -> String {
        let mut lines = vec![
            "Camera capabilities".to_string(),
            format!("- Sensor: {}x{} px", self.max_width, self.max_height),
        ];

        if let Some(pattern) = &self.bayer_pattern {
            lines.push("- Type: Color".to_string());
            lines.push(format!("- Bayer pattern: {pattern}"));
        } else {
            lines.push("- Type: Mono".to_string());
            lines.push("- Bayer pattern: none".to_string());
        }

        if self.adc_bits > 0 {
            lines.push(format!("- ADC: {} bit", self.adc_bits));
        }

        if self.frame_formats.is_empty() {
            lines.push("- Formats: none reported".to_string());
        } else {
            let formats = self
                .frame_formats
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("- Formats: {formats}"));
        }

        if let Some(gain) = &self.gain {
            lines.push(format!(
                "- Gain: {}..{} step {}",
                gain.min, gain.max, gain.step
            ));
        }

        if self.exposure.max_microseconds > 0 {
            lines.push(format!(
                "- Exposure: {}..{} us ({}..{} ms, {:.3}..{:.3} s) step {}",
                self.exposure.min_microseconds,
                self.exposure.max_microseconds,
                self.exposure.min_milliseconds(),
                self.exposure.max_milliseconds(),
                self.exposure.min_seconds(),
                self.exposure.max_seconds(),
                self.exposure.step
            ));
        }

        if let Some(roi) = &self.roi {
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

        if let Some(binning) = &self.binning {
            let modes = binning
                .modes
                .iter()
                .map(|mode| format!("{}x{}", mode, mode))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("- Binning: {modes}"));
        }

        if let Some(white_balance) = &self.white_balance {
            lines.push(format!(
                "- White balance: R {}..{}, G {}..{}, B {}..{}",
                white_balance.red.min,
                white_balance.red.max,
                white_balance.green.min,
                white_balance.green.max,
                white_balance.blue.min,
                white_balance.blue.max
            ));
        }

        if let Some(contrast) = &self.contrast {
            lines.push(format!("- Contrast: {}..{}", contrast.min, contrast.max));
        }

        if let Some(sharpness) = &self.sharpness {
            lines.push(format!("- Sharpness: {}..{}", sharpness.min, sharpness.max));
        }

        if let Some(saturation) = &self.saturation {
            lines.push(format!(
                "- Saturation: {}..{}",
                saturation.min, saturation.max
            ));
        }

        if let Some(usb_speed) = &self.usb_speed {
            lines.push(format!("- USB speed: {}..{}", usb_speed.min, usb_speed.max));
        }

        if let Some(guiding) = &self.guiding {
            lines.push(format!(
                "- Guide: pulse guide {}",
                if guiding.pulse_guide { "yes" } else { "no" }
            ));
        }

        if let Some(iso) = &self.iso {
            lines.push(format!(
                "- ISO: {}..{} x{}",
                iso.min, iso.max, iso.multiplier
            ));
        }

        if let Some(offset) = &self.offset {
            lines.push(format!(
                "- Offset: {}..{} step {}",
                offset.min, offset.max, offset.step
            ));
        }

        if let Some(cooler) = &self.cooler {
            lines.push(format!(
                "- Cooler: {:.1}..{:.1} C step {:.1}",
                cooler.min, cooler.max, cooler.step
            ));
        }

        lines.join("\n")
    }
}
