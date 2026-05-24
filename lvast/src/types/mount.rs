use std::fmt::Display;

use chrono::{DateTime, Utc};

use crate::{
    base::{connections::Connection, errors::VastResult},
    types::common::EquatorialDegrees,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VastMountType {
    Eq,
    AltAZ,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VastTrackingMode {
    Off,
    Sidereal,
    Solar,
    Lunar,
    Custom,
}

#[derive(Clone, Debug)]
pub struct VastMountSettings {
    park_mode: bool,
    tracking_mode: VastTrackingMode,
    custom_tracking_value: i32,
    datetime: DateTime<Utc>,
    timezone_offset: u8,
    longitude: f64,
    latitude: f64,
}

impl VastMountSettings {
    pub fn new(
        park_mode: bool,
        tracking_mode: VastTrackingMode,
        custom_tracking_value: i32,
        datetime: DateTime<Utc>,
        timezone_offset: u8,
        longitude: f64,
        latitude: f64,
    ) -> Self {
        Self {
            park_mode,
            tracking_mode,
            custom_tracking_value,
            datetime,
            timezone_offset,
            longitude,
            latitude,
        }
    }

    pub fn park_mode(&self) -> bool {
        self.park_mode
    }

    pub fn tracking_mode(&self) -> VastTrackingMode {
        self.tracking_mode
    }

    pub fn custom_tracking_value(&self) -> i32 {
        self.custom_tracking_value
    }

    pub fn datetime(&self) -> DateTime<Utc> {
        self.datetime
    }

    pub fn timezone_offset(&self) -> u8 {
        self.timezone_offset
    }

    pub fn longitude(&self) -> f64 {
        self.longitude
    }

    pub fn latitude(&self) -> f64 {
        self.latitude
    }
}

#[derive(Clone, Debug)]
pub struct VastMountCurrStatus {
    is_tracking: bool,
    park_mode: bool,
    coords_j2000: EquatorialDegrees,
    altitude: f64,
    azimuth: f64,
}

impl VastMountCurrStatus {
    pub fn new(
        is_tracking: bool,
        park_mode: bool,
        coords_j2000: EquatorialDegrees,
        altitude: f64,
        azimuth: f64,
    ) -> Self {
        Self {
            is_tracking,
            park_mode,
            coords_j2000,
            altitude,
            azimuth,
        }
    }

    pub fn is_tracking(&self) -> bool {
        self.is_tracking
    }

    pub fn park_mode(&self) -> bool {
        self.park_mode
    }

    pub fn coords_j2000(&self) -> EquatorialDegrees {
        self.coords_j2000
    }

    pub fn altitude(&self) -> f64 {
        self.altitude
    }

    pub fn azimuth(&self) -> f64 {
        self.azimuth
    }
}

impl Display for VastTrackingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VastTrackingMode::Off => write!(f, "off"),
            VastTrackingMode::Sidereal => write!(f, "sidereal"),
            VastTrackingMode::Solar => write!(f, "solar"),
            VastTrackingMode::Lunar => write!(f, "lunar"),
            VastTrackingMode::Custom => write!(f, "custom"),
        }
    }
}

pub trait VastMount {
    fn new() -> Self;

    fn connect(&mut self, connection: Box<dyn Connection>) -> VastResult<()>;

    fn get_name(&mut self) -> String;
    fn get_version(&mut self) -> String;
    fn get_current_settings(&mut self) -> VastResult<VastMountSettings>;
    fn get_current_status(&mut self) -> VastResult<VastMountCurrStatus>;

    fn goto(&mut self, coords_j2000: EquatorialDegrees) -> VastResult<()>;
    fn goto_home(&mut self) -> VastResult<()>;
    fn set_settings(&mut self, settings: VastMountSettings) -> VastResult<()>;

    fn stop(&mut self) -> VastResult<()>;
    fn move_east(&mut self) -> VastResult<()>;
    fn move_west(&mut self) -> VastResult<()>;
    fn move_north(&mut self) -> VastResult<()>;
    fn move_south(&mut self) -> VastResult<()>;

    fn disconnect(&mut self) -> VastResult<()>;
}
