use crate::base::errors::VastError;
use std::fmt::Display;

pub enum CameraType {
    Reflex,
    DedicatedRGB,
    DedicatedMono,
}

pub enum CameraCapabilities {
    Gain(u32),
    ISO(u32),
    Offset(u32),
    Cooler(bool),
    Roi(bool),
    Binning(bool),
}

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

pub struct VastCameraInfo {
    pub id: VastCameraID,
    pub name: String,
    pub serial_number: String,
    pub raw_extra_info: String,
}

pub trait VastCameraDriver {
    fn init(&mut self) -> Result<Vec<VastCameraInfo>, VastError>;
    fn id(&self) -> &str;
    fn get_manufacturer(&self) -> &str;
    fn get_version(&self) -> &str;
}

pub trait VastCamera {
    fn init(&mut self, driver: &mut dyn VastCameraDriver, camera_id: &str)
    -> Result<(), VastError>;

    fn get_type(&self) -> CameraType;
    fn get_name(&self) -> &str;
    fn get_manufacturer(&self) -> &str;
    fn get_capabilities(&self) -> Result<&[CameraCapabilities], VastError>;

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
}
