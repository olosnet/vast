use crate::base::errors::VastError;
use std::{fmt::Display, sync::Arc};

/// Broad class of camera device.
pub enum CameraType {
    /// DSLR or mirrorless reflex-style camera.
    Reflex,
    /// Dedicated astronomy camera with color sensor.
    DedicatedRGB,
    /// Dedicated astronomy camera with mono sensor.
    DedicatedMono,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Sensor Bayer color filter arrangement.
pub enum CameraBayerPattern {
    /// Red/green row followed by green/blue row.
    RGGB,
    /// Blue/green row followed by green/red row.
    BGGR,
    /// Green/red row followed by blue/green row.
    GRBG,
    /// Green/blue row followed by red/green row.
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
/// Pixel format returned by a camera frame.
pub enum CameraFrameFormat {
    /// 8-bit raw mono or Bayer data.
    RAW8,
    /// 16-bit raw mono or Bayer data.
    RAW16,
    /// 14-bit raw data stored in 16-bit words.
    RAW14,
    /// 12-bit raw data stored in 16-bit words.
    RAW12,
    /// 10-bit raw data stored in 16-bit words.
    RAW10,
    /// 24-bit RGB/BGR data.
    RGB24,
    /// 32-bit RGB/BGR data with alpha or padding channel.
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
/// Camera identifier used by drivers.
pub enum VastCameraID {
    /// String camera identifier.
    StrID(String),
    /// Integer camera identifier.
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

/// Camera information available before opening a connection.
pub struct VastCameraInfo {
    /// Driver-specific camera identifier.
    pub id: VastCameraID,
    /// Human-readable camera name.
    pub name: String,
    /// Camera serial number when available.
    pub serial_number: String,
    /// Driver-specific extra information for diagnostics.
    pub raw_extra_info: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// One image frame acquired from a camera.
pub struct VastCameraFrame {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Pixel format of `data`.
    pub format: CameraFrameFormat,
    /// Raw pixel bytes in camera output order.
    pub data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// ST4 guide direction.
pub enum VastCameraGuideDirection {
    /// North guide pulse.
    North,
    /// South guide pulse.
    South,
    /// East guide pulse.
    East,
    /// West guide pulse.
    West,
}

#[derive(Clone)]
/// Gain capability range.
pub struct VastCameraCapGain {
    /// Minimum gain value.
    pub min: u32,
    /// Maximum gain value.
    pub max: u32,
    /// Suggested gain step.
    pub step: u32,
}

#[derive(Clone)]
/// ISO capability range for camera classes that expose ISO.
pub struct VastCameraCapISO {
    /// Minimum ISO value.
    pub min: u32,
    /// Maximum ISO value.
    pub max: u32,
    /// ISO multiplier between adjacent values.
    pub multiplier: u32,
}

#[derive(Clone)]
/// Offset or black-level capability range.
pub struct VastCameraCapOffset {
    /// Minimum offset value.
    pub min: u32,
    /// Maximum offset value.
    pub max: u32,
    /// Suggested offset step.
    pub step: u32,
}

#[derive(Clone)]
/// Generic integer capability range.
pub struct VastCameraCapRange {
    /// Minimum value.
    pub min: u32,
    /// Maximum value.
    pub max: u32,
    /// Suggested step.
    pub step: u32,
}

#[derive(Clone)]
/// White balance capability ranges for red, green, and blue channels.
pub struct VastCameraCapWhiteBalance {
    /// Red-channel range.
    pub red: VastCameraCapRange,
    /// Green-channel range.
    pub green: VastCameraCapRange,
    /// Blue-channel range.
    pub blue: VastCameraCapRange,
}

#[derive(Clone)]
/// Cooler target-temperature capability range.
pub struct VastCameraCapCooler {
    /// Minimum target temperature in Celsius.
    pub min: f32,
    /// Maximum target temperature in Celsius.
    pub max: f32,
    /// Suggested target-temperature step.
    pub step: f32,
}

#[derive(Clone)]
/// Exposure capability range.
pub struct VastCameraCapExposure {
    /// Minimum exposure time in microseconds.
    pub min_microseconds: u64,
    /// Maximum exposure time in microseconds.
    pub max_microseconds: u64,
    /// Suggested exposure step.
    pub step: u8,
}

impl VastCameraCapExposure {
    /// Minimum exposure time in milliseconds.
    pub fn min_milliseconds(&self) -> u64 {
        self.min_microseconds / 1_000
    }

    /// Maximum exposure time in milliseconds.
    pub fn max_milliseconds(&self) -> u64 {
        self.max_microseconds / 1_000
    }

    /// Minimum exposure time in seconds.
    pub fn min_seconds(&self) -> f64 {
        self.min_microseconds as f64 / 1_000_000.0
    }

    /// Maximum exposure time in seconds.
    pub fn max_seconds(&self) -> f64 {
        self.max_microseconds as f64 / 1_000_000.0
    }
}

#[derive(Clone)]
/// One supported ROI geometry for a specific binning mode.
pub struct VastCameraCapRoiCombination {
    /// Binning factor for this ROI combination.
    pub bin: u32,
    /// Maximum ROI width after binning.
    pub max_width: u32,
    /// Maximum ROI height after binning.
    pub max_height: u32,
    /// Required width alignment step.
    pub width_step: u32,
    /// Required height alignment step.
    pub height_step: u32,
}

#[derive(Clone)]
/// Region-of-interest capability.
pub struct VastCameraCapRoi {
    /// Supported ROI combinations by binning mode.
    pub combinations: Vec<VastCameraCapRoiCombination>,
}

#[derive(Clone)]
/// Supported square binning modes.
pub struct VastCameraCapBinning {
    /// Binning factors, for example `1`, `2`, or `4`.
    pub modes: Vec<u32>,
}

#[derive(Clone)]
/// Guide-port capability.
pub struct VastCameraCapGuiding {
    /// Whether pulse guiding is supported.
    pub pulse_guide: bool,
}

#[derive(Clone)]
/// Camera hardware and driver capabilities.
pub struct VastCameraCapabilities {
    /// Gain capability when supported.
    pub gain: Option<VastCameraCapGain>,
    /// ISO capability when supported.
    pub iso: Option<VastCameraCapISO>,
    /// Offset capability when supported.
    pub offset: Option<VastCameraCapOffset>,
    /// Cooler capability when supported.
    pub cooler: Option<VastCameraCapCooler>,
    /// White balance capability when supported.
    pub white_balance: Option<VastCameraCapWhiteBalance>,
    /// Contrast capability when supported.
    pub contrast: Option<VastCameraCapRange>,
    /// Sharpness capability when supported.
    pub sharpness: Option<VastCameraCapRange>,
    /// Saturation capability when supported.
    pub saturation: Option<VastCameraCapRange>,
    /// USB speed capability when supported.
    pub usb_speed: Option<VastCameraCapRange>,
    /// ROI capability when supported.
    pub roi: Option<VastCameraCapRoi>,
    /// Binning capability when supported.
    pub binning: Option<VastCameraCapBinning>,
    /// Guiding capability when supported.
    pub guiding: Option<VastCameraCapGuiding>,
    /// Exposure capability.
    pub exposure: VastCameraCapExposure,
    /// Supported camera frame formats.
    pub frame_formats: Vec<CameraFrameFormat>,
    /// Sensor Bayer pattern for color cameras.
    pub bayer_pattern: Option<CameraBayerPattern>,
    /// Maximum sensor height in pixels.
    pub max_height: u32,
    /// Maximum sensor width in pixels.
    pub max_width: u32,
    /// ADC bit depth reported by the camera.
    pub adc_bits: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
/// Desired or current camera settings.
pub struct VastCameraSettings {
    /// Exposure time in microseconds.
    pub exposure_microseconds: Option<u64>,
    /// Camera gain.
    pub gain: Option<u32>,
    /// Camera ISO.
    pub iso: Option<u32>,
    /// Camera offset or black level.
    pub offset: Option<u32>,
    /// Cooler state and target temperature in Celsius.
    pub cooler: Option<(bool, u32)>,
    /// White balance values for red, green, and blue channels.
    pub white_balance: Option<(u32, u32, u32)>,
    /// Contrast setting.
    pub contrast: Option<u32>,
    /// Sharpness setting.
    pub sharpness: Option<u32>,
    /// Saturation setting.
    pub saturation: Option<u32>,
    /// USB speed setting.
    pub usb_speed: Option<u32>,
    /// Region of interest as `(x, y, width, height)`.
    pub roi: Option<(u32, u32, u32, u32)>,
    /// Binning as `(horizontal, vertical)`.
    pub binning: Option<(u32, u32)>,
}

impl VastCameraSettings {
    /// Formats settings as a multi-line human-readable summary.
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

/// Common interface for camera driver discovery and metadata.
pub trait VastCameraDriver: Send + Sync {
    /// Creates a new driver instance.
    fn new() -> Self;
    /// Initializes the driver and lists connected cameras.
    fn init(&mut self) -> Result<Vec<VastCameraInfo>, VastError>;
    /// Stable driver identifier.
    fn id(&self) -> &str;
    /// Camera manufacturer name.
    fn get_manufacturer(&self) -> &str;
    /// Native SDK or driver version string.
    fn get_version(&self) -> &str;
}

/// Common interface for connected camera control.
pub trait VastCamera<IDT, T: VastCameraDriver>: Send + Sync {
    /// Creates a camera object backed by `driver`.
    fn new(driver: Arc<T>) -> Self;

    /// Opens a connection to the camera identified by `camera_id`.
    fn connect(&mut self, camera_id: IDT) -> Result<(), VastError>;

    /// Human-readable connected camera name.
    fn get_name(&self) -> &str;
    /// Hardware and driver capabilities of the connected camera.
    fn get_capabilities(&self) -> VastCameraCapabilities;

    /// Reads current offset value.
    fn get_current_offset(&self) -> u32;
    /// Reads cooler enabled state and target temperature.
    fn get_current_cooler(&self) -> (bool, u32);
    /// Reads current sensor temperature in Celsius.
    fn get_current_temperature(&self) -> f32;

    /// Applies camera settings supported by the implementation.
    fn set_camera_settings(&mut self, settings: VastCameraSettings) -> Result<(), VastError>;
    /// Reads camera settings from hardware and updates cached settings.
    fn get_camera_settings(&mut self) -> Result<VastCameraSettings, VastError>;
    /// Returns last cached camera settings.
    fn get_settings(&self) -> VastCameraSettings;

    /// Closes the camera connection.
    fn disconnect(&mut self) -> Result<(), VastError>;
}

/// Interface for still-image acquisition.
pub trait VastCameraAcquireImage: Send + Sync {
    /// Starts a still-image acquisition using current camera settings.
    fn start_image_acquisition(&mut self) -> Result<(), VastError>;
    /// Aborts an active still-image acquisition.
    fn abort_image_acquisition(&mut self) -> Result<(), VastError>;
    /// Waits for and returns the acquired image frame.
    fn get_acquired_image(&mut self, timeout_millis: u32) -> Result<VastCameraFrame, VastError>;
}

/// Interface for ST4 pulse guiding.
pub trait VastCameraGuide: Send + Sync {
    /// Sends a guide pulse in `direction` for `duration_millis`.
    fn pulse_guide(
        &mut self,
        direction: VastCameraGuideDirection,
        duration_millis: u32,
    ) -> Result<(), VastError>;
}

/// Interface for video or live-view streaming.
pub trait VastCameraStreamingPreview: Send + Sync {
    /// Starts streaming preview capture.
    fn start_streaming_preview(&mut self) -> Result<(), VastError>;
    /// Reads one streaming preview frame.
    fn get_streaming_preview_frame(
        &mut self,
        timeout_millis: u32,
    ) -> Result<VastCameraFrame, VastError>;
    /// Stops streaming preview capture.
    fn stop_streaming_preview(&mut self) -> Result<(), VastError>;
}

impl VastCameraCapabilities {
    /// Formats capabilities as a multi-line human-readable summary.
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
