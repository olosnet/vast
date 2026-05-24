use chrono::{DateTime, Utc};

use crate::base::{connections::Connection, errors::VastResult};

pub enum VastMountType {
    Eq,
    AltAZ,
}

pub enum VastTrackingMode {
    Off,
    Sidereal,
    Solar,
    Lunar,
    Custom,
}

pub struct VastMountSettings {
    park_mode: bool,
    tracking_mode: VastTrackingMode,
    custom_tracking_value: i32,
    datetime: DateTime<Utc>,
    timezone_offset: u8,
    longitude: f64,
    latitude: f64,
}

pub struct VastMountCurrStatus {
    is_tracking: bool,
    park_mode: bool,
    ra_j2000: String,
    dec_j2000: String,
    altitude: f64,
    azimuth: f64,
}

pub trait VastMount {
    fn new() -> Self;

    fn connect(&mut self, connection: Box<dyn Connection>) -> VastResult<()>;

    fn get_name(&mut self) -> String;
    fn get_version(&mut self) -> String;
    fn get_current_settings(&mut self) -> VastResult<VastMountSettings>;
    fn get_current_status(&mut self) -> VastResult<VastMountCurrStatus>;

    fn goto(&mut self, ra_j2000: String, dec_j2000: String) -> VastResult<()>;
    fn goto_home(&mut self) -> VastResult<()>;
    fn set_settings(&mut self, settings: VastMountSettings) -> VastResult<()>;

    fn stop(&mut self) -> VastResult<()>;
    fn move_east(&mut self) -> VastResult<()>;
    fn move_west(&mut self) -> VastResult<()>;
    fn move_north(&mut self) -> VastResult<()>;
    fn move_south(&mut self) -> VastResult<()>;

    fn disconnect(&mut self) -> VastResult<()>;
}
