use std::fmt::Display;

use chrono::{DateTime, Utc};

use crate::{
    base::{connections::Connection, errors::VastResult},
    types::{common::EquatorialDegrees, imageformats::ImageHeaders},
};

/// Broad physical mount geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VastMountType {
    /// Equatorial mount.
    Eq,
    /// Altitude-azimuth mount.
    AltAZ,
}

/// Tracking mode abstraction shared across mount implementations.
///
/// `Custom` uses [`VastMountSettings::custom_tracking_value`] in thousandths of sidereal rate,
/// so `1000` means sidereal rate, `997` is approximately solar, and `962` is approximately
/// lunar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VastTrackingMode {
    /// Tracking disabled.
    Off,
    /// Sidereal tracking rate.
    Sidereal,
    /// Solar tracking rate.
    Solar,
    /// Lunar tracking rate.
    Lunar,
    /// Custom tracking rate using [`VastMountSettings::custom_tracking_value`].
    Custom,
}

/// Snapshot of configurable mount settings expressed in mount-independent units.
///
/// Time semantics:
///
/// - [`Self::datetime`] is current mount time expressed in UTC.
/// - [`Self::utc_offset_minutes`] is signed local civil-time offset from UTC in minutes.
/// - local civil time is therefore `UTC + utc_offset_minutes`.
///
/// Tracking semantics:
///
/// - [`Self::custom_tracking_value`] is in thousandths of sidereal rate.
/// - `1000` means sidereal rate.
/// - value is meaningful only when [`Self::tracking_mode`] is [`VastTrackingMode::Custom`].
#[derive(Clone, Debug)]
pub struct VastMountSettings {
    park_mode: bool,
    tracking_mode: VastTrackingMode,
    custom_tracking_value: i32,
    datetime: DateTime<Utc>,
    utc_offset_minutes: i32,
    longitude: f64,
    latitude: f64,
}

impl VastMountSettings {
    /// Creates a new mount settings snapshot.
    pub fn new(
        park_mode: bool,
        tracking_mode: VastTrackingMode,
        custom_tracking_value: i32,
        datetime: DateTime<Utc>,
        utc_offset_minutes: i32,
        longitude: f64,
        latitude: f64,
    ) -> Self {
        Self {
            park_mode,
            tracking_mode,
            custom_tracking_value,
            datetime,
            utc_offset_minutes,
            longitude,
            latitude,
        }
    }

    /// Returns `true` when mount is parked or park mode is enabled.
    pub fn park_mode(&self) -> bool {
        self.park_mode
    }

    /// Returns current tracking mode.
    pub fn tracking_mode(&self) -> VastTrackingMode {
        self.tracking_mode
    }

    /// Returns custom tracking rate in thousandths of sidereal rate.
    pub fn custom_tracking_value(&self) -> i32 {
        self.custom_tracking_value
    }

    /// Returns mount date/time expressed in UTC.
    pub fn datetime(&self) -> DateTime<Utc> {
        self.datetime
    }

    /// Returns signed local civil-time offset from UTC in minutes.
    pub fn utc_offset_minutes(&self) -> i32 {
        self.utc_offset_minutes
    }

    /// Returns site longitude in degrees.
    pub fn longitude(&self) -> f64 {
        self.longitude
    }

    /// Returns site latitude in degrees.
    pub fn latitude(&self) -> f64 {
        self.latitude
    }
}

/// The side of the pier the mount is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VastMountPierSide {
    /// Telescope is on east side of pier.
    East,
    /// Telescope is on west side of pier.
    West,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VastMountStatus {
    /// Mount is actively tracking.
    Tracking,
    /// Mount is parked.
    Parked,
    /// Mount is slewing.
    Slewing,
    /// Mount is stopped.
    Stopped,
    /// Mount status is unknown.
    Unknown,
}

/// Snapshot of live mount status.
#[derive(Clone, Debug)]
pub struct VastMountCurrStatus {
    status: VastMountStatus,
    park_mode: bool,
    coords_j2000: EquatorialDegrees,
    altitude: f64,
    azimuth: f64,
    pier_side: Option<VastMountPierSide>,
}

impl VastMountCurrStatus {
    /// Creates a new live mount status snapshot.
    pub fn new(
        status: VastMountStatus,
        park_mode: bool,
        coords_j2000: EquatorialDegrees,
        altitude: f64,
        azimuth: f64,
        pier_side: Option<VastMountPierSide>,
    ) -> Self {
        Self {
            status,
            park_mode,
            coords_j2000,
            altitude,
            azimuth,
            pier_side,
        }
    }

    /// Returns copy of status with pier-side information attached.
    pub fn with_pier_side(mut self, pier_side: Option<VastMountPierSide>) -> Self {
        self.pier_side = pier_side;
        self
    }

    /// Returns current high-level mount status.
    pub fn status(&self) -> VastMountStatus {
        self.status
    }

    /// Returns `true` when mount is actively tracking.
    pub fn is_tracking(&self) -> bool {
        self.status == VastMountStatus::Tracking
    }

    /// Returns `true` when mount is currently slewing.
    pub fn is_slewing(&self) -> bool {
        self.status == VastMountStatus::Slewing
    }

    /// Returns `true` when mount is currently parked.
    pub fn is_parked(&self) -> bool {
        self.status == VastMountStatus::Parked
    }

    /// Returns `true` when mount is parked.
    pub fn park_mode(&self) -> bool {
        self.park_mode
    }

    /// Returns current equatorial coordinates in J2000.
    pub fn coords_j2000(&self) -> EquatorialDegrees {
        self.coords_j2000
    }

    /// Returns current altitude in degrees.
    pub fn altitude(&self) -> f64 {
        self.altitude
    }

    /// Returns current azimuth in degrees.
    pub fn azimuth(&self) -> f64 {
        self.azimuth
    }

    /// Returns current pier side when backend can determine it.
    pub fn pier_side(&self) -> Option<VastMountPierSide> {
        self.pier_side
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

/// Common interface implemented by concrete mount backends.
///
/// Implementations must translate device-specific protocols into these shared units and
/// semantics so higher layers can interact with mounts consistently.
pub trait VastMount {
    /// Creates a disconnected mount instance.
    fn new() -> Self;

    /// Connects mount using provided transport.
    fn connect(&mut self, connection: Box<dyn Connection>) -> VastResult<()>;

    /// Returns human-readable mount name.
    fn get_name(&mut self) -> String;
    /// Returns human-readable firmware or driver version when available.
    fn get_version(&mut self) -> String;
    /// Returns current mount settings using mount-independent units.
    fn get_current_settings(&mut self) -> VastResult<VastMountSettings>;
    /// Returns current live mount status.
    fn get_current_status(&mut self) -> VastResult<VastMountCurrStatus>;

    /// Slews to target J2000 coordinates.
    fn goto(&mut self, coords_j2000: EquatorialDegrees) -> VastResult<()>;
    /// Moves mount to its home position.
    fn goto_home(&mut self) -> VastResult<()>;
    /// Applies mount settings.
    fn set_settings(&mut self, settings: VastMountSettings) -> VastResult<()>;

    /// Stops mount motion.
    fn stop(&mut self) -> VastResult<()>;
    /// Starts manual eastward motion.
    fn move_east(&mut self) -> VastResult<()>;
    /// Starts manual westward motion.
    fn move_west(&mut self) -> VastResult<()>;
    /// Starts manual northward motion.
    fn move_north(&mut self) -> VastResult<()>;
    /// Starts manual southward motion.
    fn move_south(&mut self) -> VastResult<()>;

    fn populate_image_headers(&mut self, headers: &mut ImageHeaders) -> VastResult<()>;

    /// Disconnects mount.
    fn disconnect(&mut self) -> VastResult<()>;
}
