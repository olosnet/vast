use chrono::{Duration, Utc};

use crate::{
    algos::convert::{j2000_to_jnow, jnow_to_j2000},
    base::{
        connections::Connection,
        errors::{VastError, VastErrorType, VastResult},
    },
    drivers::native::onstep::driver::OnStepClient,
    mounts::{
        VastMount, VastMountCurrStatus, VastMountPierSide, VastMountSettings, VastMountStatus,
        VastTrackingMode,
    },
    types::common::EquatorialDegrees,
};

pub struct OnStepVastMount {
    client: Option<OnStepClient>,
    current_settings: VastMountSettings,
}

impl OnStepVastMount {
    const ONSTEP_SIDEREAL_RATE_HZ: f32 = 60.0 * 1.002_737_9;

    fn client_mut(&mut self) -> VastResult<&mut OnStepClient> {
        self.client.as_mut().ok_or_else(|| {
            VastError::new(
                VastErrorType::ConnectionError,
                "OnStep mount is not connected".to_string(),
            )
        })
    }

    fn parse_sexagesimal(value: &str) -> VastResult<f64> {
        let trimmed = value.trim();
        let negative = trimmed.starts_with('-');
        let positive = trimmed.starts_with('+');
        let unsigned = if negative || positive {
            &trimmed[1..]
        } else {
            trimmed
        };

        let parts: Vec<&str> = unsigned.split(':').collect();
        if parts.len() != 3 {
            return Err(VastError::new(
                VastErrorType::InvalidInput,
                format!("Invalid sexagesimal value: {}", value),
            ));
        }

        let hours_or_deg = parts[0].parse::<f64>().map_err(|err| {
            VastError::new(
                VastErrorType::InvalidInput,
                format!("Invalid sexagesimal component '{}': {}", parts[0], err),
            )
        })?;
        let minutes = parts[1].parse::<f64>().map_err(|err| {
            VastError::new(
                VastErrorType::InvalidInput,
                format!("Invalid sexagesimal component '{}': {}", parts[1], err),
            )
        })?;
        let seconds = parts[2].parse::<f64>().map_err(|err| {
            VastError::new(
                VastErrorType::InvalidInput,
                format!("Invalid sexagesimal component '{}': {}", parts[2], err),
            )
        })?;

        let sign = if negative { -1.0 } else { 1.0 };
        Ok(sign * (hours_or_deg.abs() + minutes / 60.0 + seconds / 3600.0))
    }

    fn format_ra_hours(hours: f64) -> String {
        let wrapped = hours.rem_euclid(24.0);
        let total_seconds = (wrapped * 3600.0).round() as i64;
        let hh = (total_seconds / 3600).rem_euclid(24);
        let mm = (total_seconds % 3600) / 60;
        let ss = total_seconds % 60;
        format!("{hh:02}:{mm:02}:{ss:02}")
    }

    fn format_signed_degrees(degrees: f64) -> String {
        let sign = if degrees < 0.0 { '-' } else { '+' };
        let absolute = degrees.abs();
        let total_seconds = (absolute * 3600.0).round() as i64;
        let dd = total_seconds / 3600;
        let mm = (total_seconds % 3600) / 60;
        let ss = total_seconds % 60;
        format!("{sign}{dd:02}:{mm:02}:{ss:02}")
    }

    fn parse_ra_hours(value: &str) -> VastResult<f64> {
        Self::parse_sexagesimal(value)
    }

    fn parse_signed_degrees(value: &str) -> VastResult<f64> {
        Self::parse_sexagesimal(value)
    }

    fn parse_mount_tracking_mode(rate: f32) -> VastTrackingMode {
        const EPSILON: f32 = 0.01;
        let normalized_rate = Self::normalize_tracking_rate(rate);

        let candidates = [
            (VastTrackingMode::Sidereal, (normalized_rate - 1.0).abs()),
            (
                VastTrackingMode::Solar,
                (normalized_rate - 0.997_269_6).abs(),
            ),
            (
                VastTrackingMode::Lunar,
                (normalized_rate - 0.962_365_15).abs(),
            ),
            (VastTrackingMode::Lunar, (normalized_rate - 1.035_05).abs()),
        ];

        candidates
            .into_iter()
            .min_by(|(_, lhs), (_, rhs)| lhs.total_cmp(rhs))
            .and_then(|(mode, delta)| (delta < EPSILON).then_some(mode))
            .unwrap_or(VastTrackingMode::Custom)
    }

    fn normalize_tracking_rate(rate: f32) -> f32 {
        if rate.abs() > 10.0 {
            rate / Self::ONSTEP_SIDEREAL_RATE_HZ
        } else {
            rate
        }
    }

    fn tracking_rate_to_onstep_hz(rate: f32) -> f32 {
        if rate.abs() > 10.0 {
            rate
        } else {
            rate * Self::ONSTEP_SIDEREAL_RATE_HZ
        }
    }

    fn validate_tracking_settings(settings: &VastMountSettings) -> VastResult<()> {
        if settings.tracking_mode() == VastTrackingMode::Custom
            && settings.custom_tracking_value() <= 0
        {
            return Err(VastError::new(
                VastErrorType::InvalidInput,
                "Custom tracking mode requires a positive custom_tracking_value".to_string(),
            ));
        }

        Ok(())
    }

    fn local_datetime_from_utc(
        datetime_utc: chrono::DateTime<Utc>,
        utc_offset_minutes: i32,
    ) -> VastResult<chrono::DateTime<Utc>> {
        datetime_utc
            .checked_add_signed(Duration::minutes(i64::from(utc_offset_minutes)))
            .ok_or_else(|| {
                VastError::new(
                    VastErrorType::InvalidInput,
                    format!(
                        "UTC offset {} minutes overflows local mount time conversion",
                        utc_offset_minutes
                    ),
                )
            })
    }

    fn apply_tracking_mode(
        client: &mut OnStepClient,
        settings: &VastMountSettings,
    ) -> VastResult<()> {
        match settings.tracking_mode() {
            VastTrackingMode::Off => {
                client.tracking_off()?;
            }
            VastTrackingMode::Sidereal => {
                client.tracking_sidereal()?;
                client.tracking_on()?;
            }
            VastTrackingMode::Solar => {
                client.tracking_solar()?;
                client.tracking_on()?;
            }
            VastTrackingMode::Lunar => {
                client.tracking_lunar()?;
                client.tracking_on()?;
            }
            VastTrackingMode::Custom => {
                let normalized_rate = settings.custom_tracking_value() as f32 / 1000.0;
                let tracking_rate_hz = Self::tracking_rate_to_onstep_hz(normalized_rate);

                if !client.set_tracking_rate(tracking_rate_hz)? {
                    return Err(VastError::new(
                        VastErrorType::InvalidInput,
                        format!(
                            "OnStep rejected custom tracking rate {:.5} Hz",
                            tracking_rate_hz
                        ),
                    ));
                }

                client.tracking_on()?;
            }
        }

        Ok(())
    }

    fn map_pier_side(client: &OnStepClient) -> Option<VastMountPierSide> {
        match client.pier_side() {
            Some("East") => Some(VastMountPierSide::East),
            Some("West") => Some(VastMountPierSide::West),
            _ => None,
        }
    }

    fn map_mount_status(client: &OnStepClient) -> VastMountStatus {
        if client.is_parked() {
            VastMountStatus::Parked
        } else if client.is_slewing() {
            VastMountStatus::Slewing
        } else if client.is_tracking() {
            VastMountStatus::Tracking
        } else {
            VastMountStatus::Stopped
        }
    }
}

impl VastMount for OnStepVastMount {
    fn new() -> Self {
        Self {
            client: None,
            current_settings: VastMountSettings::new(
                false,
                VastTrackingMode::Off,
                0,
                Utc::now(),
                0,
                0.0,
                0.0,
            ),
        }
    }

    fn connect(&mut self, connection: Box<dyn Connection>) -> VastResult<()> {
        self.client = Some(OnStepClient::new(connection, false));
        let _ = self.get_current_status()?;
        let _ = self.get_current_settings()?;
        Ok(())
    }

    fn get_name(&mut self) -> String {
        "OnStep".to_string()
    }

    fn get_version(&mut self) -> String {
        self.client_mut()
            .and_then(OnStepClient::get_version)
            .unwrap_or_default()
    }

    fn get_current_settings(&mut self) -> VastResult<VastMountSettings> {
        let now = Utc::now();
        let client = self.client_mut()?;
        client.update_status()?;

        let tracking_rate = client.get_tracking_rate()?;
        let tracking_mode = if client.is_tracking() {
            Self::parse_mount_tracking_mode(tracking_rate)
        } else {
            VastTrackingMode::Off
        };
        let custom_tracking_value = if tracking_mode == VastTrackingMode::Custom {
            (Self::normalize_tracking_rate(tracking_rate) * 1000.0).round() as i32
        } else {
            0
        };

        let settings = VastMountSettings::new(
            client.is_parked(),
            tracking_mode,
            custom_tracking_value,
            now,
            client.get_utc_offset_minutes()?,
            f64::from(client.get_longitude()?),
            f64::from(client.get_latitude()?),
        );

        self.current_settings = settings.clone();
        Ok(settings)
    }

    fn get_current_status(&mut self) -> VastResult<VastMountCurrStatus> {
        let settings = self.get_current_settings()?;
        let client = self.client_mut()?;

        let ra_jnow = Self::parse_ra_hours(&client.get_ra(None)?)?;
        let dec_jnow = Self::parse_signed_degrees(&client.get_dec()?)?;
        let alt = Self::parse_signed_degrees(&client.get_alt()?)?;
        let azm = Self::parse_signed_degrees(&client.get_azm()?)?;
        let (ra_j2000, dec_j2000) = jnow_to_j2000(
            ra_jnow,
            dec_jnow,
            crate::algos::convert::datetime_to_julian_day(settings.datetime()),
        );
        let status = Self::map_mount_status(client);
        let park_mode = client.is_parked();
        let pier_side = Self::map_pier_side(client);

        Ok(VastMountCurrStatus::new(
            status,
            park_mode,
            EquatorialDegrees::from_ra_hours_dec_degrees(ra_j2000, dec_j2000),
            alt,
            azm,
            pier_side,
        ))
    }

    fn goto(&mut self, coords_j2000: EquatorialDegrees) -> VastResult<()> {
        let jd = crate::algos::convert::datetime_to_julian_day(Utc::now());
        let (ra_j2000, dec_j2000) = coords_j2000.to_ra_hours_dec_degrees();
        let (ra_jnow, dec_jnow) = j2000_to_jnow(ra_j2000, dec_j2000, jd);
        let client = self.client_mut()?;

        client.set_target_ra(&Self::format_ra_hours(ra_jnow))?;
        client.set_target_dec(&Self::format_signed_degrees(dec_jnow))?;
        let (code, message) = client.slew_equ()?;

        if code == "0" || code == "5" {
            Ok(())
        } else {
            Err(VastError::new(
                VastErrorType::ConnectionError,
                format!("OnStep goto failed ({}): {}", code, message),
            ))
        }
    }

    fn goto_home(&mut self) -> VastResult<()> {
        self.client_mut()?.return_home().map(|_| ())
    }

    fn set_settings(&mut self, settings: VastMountSettings) -> VastResult<()> {
        Self::validate_tracking_settings(&settings)?;
        let client = self.client_mut()?;
        let local_datetime =
            Self::local_datetime_from_utc(settings.datetime(), settings.utc_offset_minutes())?;

        client.set_utc_offset_minutes(settings.utc_offset_minutes())?;
        client.set_local_date(local_datetime)?;
        client.set_local_time(local_datetime)?;
        client.set_longitude(settings.longitude() as f32)?;
        client.set_latitude(settings.latitude() as f32)?;

        Self::apply_tracking_mode(client, &settings)?;

        self.current_settings = settings;
        Ok(())
    }

    fn stop(&mut self) -> VastResult<()> {
        self.client_mut()?.stop()
    }

    fn move_east(&mut self) -> VastResult<()> {
        self.client_mut()?.move_direction('e')
    }

    fn move_west(&mut self) -> VastResult<()> {
        self.client_mut()?.move_direction('w')
    }

    fn move_north(&mut self) -> VastResult<()> {
        self.client_mut()?.move_direction('n')
    }

    fn move_south(&mut self) -> VastResult<()> {
        self.client_mut()?.move_direction('s')
    }

    fn disconnect(&mut self) -> VastResult<()> {
        self.client = None;
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
