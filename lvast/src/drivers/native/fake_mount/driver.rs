use chrono::Utc;

use crate::{
    base::{
        connections::Connection,
        errors::{VastError, VastErrorType, VastResult},
    },
    types::{
        common::EquatorialDegrees,
        mount::{VastMountCurrStatus, VastMountPierSide, VastMountSettings, VastMountStatus, VastMountType, VastTrackingMode},
    },
};

const DEFAULT_NAME: &str = "Fake Mount";
const DEFAULT_VERSION: &str = "0.1";
const DEFAULT_ALTITUDE_DEG: f64 = 45.0;
const DEFAULT_AZIMUTH_DEG: f64 = 180.0;

fn connection_error(message: impl Into<String>) -> VastError {
    VastError::new(VastErrorType::ConnectionError, message.into())
}

fn invalid_input(message: impl Into<String>) -> VastError {
    VastError::new(VastErrorType::InvalidInput, message.into())
}

fn normalize_ra_deg(ra: f64) -> f64 {
    ra.rem_euclid(360.0)
}

pub struct FakeMount {
    connection: Option<Box<dyn Connection>>,
    mount_type: VastMountType,
    name: String,
    version: String,
    settings: VastMountSettings,
    coords_j2000: EquatorialDegrees,
    altitude: f64,
    azimuth: f64,
    pier_side: Option<VastMountPierSide>,
    status: VastMountStatus,
}

impl FakeMount {
    pub fn new() -> Self {
        Self {
            connection: None,
            mount_type: VastMountType::Eq,
            name: DEFAULT_NAME.to_string(),
            version: DEFAULT_VERSION.to_string(),
            settings: VastMountSettings::new(
                false,
                VastTrackingMode::Sidereal,
                0,
                Utc::now(),
                0,
                0.0,
                0.0,
            ),
            coords_j2000: EquatorialDegrees { ra: 0.0, dec: 0.0 },
            altitude: DEFAULT_ALTITUDE_DEG,
            azimuth: DEFAULT_AZIMUTH_DEG,
            pier_side: Some(VastMountPierSide::East),
            status: VastMountStatus::Tracking,
        }
    }

    fn ensure_connected(&mut self) -> VastResult<()> {
        let connected = self
            .connection
            .as_mut()
            .map(|connection| connection.is_connected())
            .unwrap_or(false);
        if connected {
            Ok(())
        } else {
            Err(connection_error("Fake mount is not connected"))
        }
    }

    pub fn mount_type(&self) -> VastMountType {
        self.mount_type
    }

    pub fn set_mount_type(&mut self, mount_type: VastMountType) {
        self.mount_type = mount_type;
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    pub fn set_version(&mut self, version: impl Into<String>) {
        self.version = version.into();
    }

    fn update_tracking_status(&mut self) {
        self.status = if self.settings.park_mode() {
            VastMountStatus::Parked
        } else if self.settings.tracking_mode() == VastTrackingMode::Off {
            VastMountStatus::Stopped
        } else {
            VastMountStatus::Tracking
        };
    }

    fn set_direction_motion(&mut self, altitude: f64, azimuth: f64) {
        self.altitude = altitude;
        self.azimuth = azimuth;
        self.status = VastMountStatus::Slewing;
    }
}

impl Default for FakeMount {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeMount {
    pub(crate) fn connect_inner(&mut self, connection: Box<dyn Connection>) -> VastResult<()> {
        self.connection = Some(connection);
        self.settings = VastMountSettings::new(
            false,
            VastTrackingMode::Sidereal,
            0,
            Utc::now(),
            0,
            0.0,
            0.0,
        );
        self.coords_j2000 = EquatorialDegrees { ra: 0.0, dec: 0.0 };
        self.altitude = DEFAULT_ALTITUDE_DEG;
        self.azimuth = DEFAULT_AZIMUTH_DEG;
        self.pier_side = Some(VastMountPierSide::East);
        self.status = VastMountStatus::Tracking;
        Ok(())
    }

    pub(crate) fn get_name_inner(&mut self) -> String {
        self.name.clone()
    }

    pub(crate) fn get_version_inner(&mut self) -> String {
        self.version.clone()
    }

    pub(crate) fn get_current_settings_inner(&mut self) -> VastResult<VastMountSettings> {
        self.ensure_connected()?;
        Ok(self.settings.clone())
    }

    pub(crate) fn get_current_status_inner(&mut self) -> VastResult<VastMountCurrStatus> {
        self.ensure_connected()?;
        Ok(VastMountCurrStatus::new(
            self.status,
            self.settings.park_mode(),
            self.coords_j2000,
            self.altitude,
            self.azimuth,
            self.pier_side,
        ))
    }

    pub(crate) fn goto_inner(&mut self, coords_j2000: EquatorialDegrees) -> VastResult<()> {
        self.ensure_connected()?;
        if !coords_j2000.ra.is_finite() || !coords_j2000.dec.is_finite() {
            return Err(invalid_input("Fake mount goto coordinates must be finite"));
        }
        if !(-90.0..=90.0).contains(&coords_j2000.dec) {
            return Err(invalid_input("Fake mount goto declination must be within -90..=90 degrees"));
        }

        self.coords_j2000 = EquatorialDegrees {
            ra: normalize_ra_deg(coords_j2000.ra),
            dec: coords_j2000.dec,
        };
        self.altitude = (90.0 - self.coords_j2000.dec.abs()).clamp(0.0, 90.0);
        self.azimuth = self.coords_j2000.ra.rem_euclid(360.0);
        self.pier_side = if self.coords_j2000.ra < 180.0 {
            Some(VastMountPierSide::East)
        } else {
            Some(VastMountPierSide::West)
        };
        self.update_tracking_status();
        Ok(())
    }

    pub(crate) fn goto_home_inner(&mut self) -> VastResult<()> {
        self.ensure_connected()?;
        self.coords_j2000 = EquatorialDegrees { ra: 0.0, dec: 90.0 };
        self.altitude = 90.0;
        self.azimuth = 0.0;
        self.pier_side = Some(VastMountPierSide::East);
        self.update_tracking_status();
        Ok(())
    }

    pub(crate) fn set_settings_inner(&mut self, settings: VastMountSettings) -> VastResult<()> {
        self.ensure_connected()?;
        self.settings = settings;
        self.update_tracking_status();
        Ok(())
    }

    pub(crate) fn stop_inner(&mut self) -> VastResult<()> {
        self.ensure_connected()?;
        self.status = VastMountStatus::Stopped;
        Ok(())
    }

    pub(crate) fn move_east_inner(&mut self) -> VastResult<()> {
        self.ensure_connected()?;
        self.set_direction_motion(self.altitude, (self.azimuth + 1.0).rem_euclid(360.0));
        Ok(())
    }

    pub(crate) fn move_west_inner(&mut self) -> VastResult<()> {
        self.ensure_connected()?;
        self.set_direction_motion(self.altitude, (self.azimuth - 1.0).rem_euclid(360.0));
        Ok(())
    }

    pub(crate) fn move_north_inner(&mut self) -> VastResult<()> {
        self.ensure_connected()?;
        self.set_direction_motion((self.altitude + 1.0).clamp(0.0, 90.0), self.azimuth);
        Ok(())
    }

    pub(crate) fn move_south_inner(&mut self) -> VastResult<()> {
        self.ensure_connected()?;
        self.set_direction_motion((self.altitude - 1.0).clamp(0.0, 90.0), self.azimuth);
        Ok(())
    }

    pub(crate) fn disconnect_inner(&mut self) -> VastResult<()> {
        if let Some(connection) = self.connection.as_mut() {
            connection.disconnect();
        }
        self.connection = None;
        self.status = VastMountStatus::Unknown;
        Ok(())
    }
}
