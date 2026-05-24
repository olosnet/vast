use chrono::Utc;

use crate::{
    algos::convert::{j2000_to_jnow, jnow_to_j2000},
    base::{
        connections::Connection,
        errors::{VastError, VastErrorType, VastResult},
    },
    drivers::native::onstep::driver::OnStepClient,
    mounts::{VastMount, VastMountCurrStatus, VastMountSettings, VastTrackingMode},
    types::common::EquatorialDegrees,
};

pub struct OnStepVastMount {
    client: Option<OnStepClient>,
    current_settings: VastMountSettings,
}

impl OnStepVastMount {
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

        let candidates = [
            (VastTrackingMode::Sidereal, (rate - 1.0).abs()),
            (VastTrackingMode::Solar, (rate - 0.997_269_6).abs()),
            (VastTrackingMode::Lunar, (rate - 1.035_05).abs()),
        ];

        candidates
            .into_iter()
            .min_by(|(_, lhs), (_, rhs)| lhs.total_cmp(rhs))
            .and_then(|(mode, delta)| (delta < EPSILON).then_some(mode))
            .unwrap_or(VastTrackingMode::Custom)
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

        let tracking_mode = Self::parse_mount_tracking_mode(client.get_tracking_rate()?);
        let custom_tracking_value = if tracking_mode == VastTrackingMode::Custom {
            (client.get_tracking_rate()? * 1000.0).round() as i32
        } else {
            0
        };

        let settings = VastMountSettings::new(
            false,
            tracking_mode,
            custom_tracking_value,
            now,
            client.get_utc_offset()?.unsigned_abs() as u8,
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

        Ok(VastMountCurrStatus::new(
            settings.tracking_mode() != VastTrackingMode::Off,
            settings.park_mode(),
            EquatorialDegrees::from_ra_hours_dec_degrees(ra_j2000, dec_j2000),
            alt,
            azm,
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
        let client = self.client_mut()?;

        client.set_utc_offset(i16::from(settings.timezone_offset()))?;
        client.set_date(settings.datetime())?;
        client.set_time(settings.datetime())?;
        client.set_longitude(settings.longitude() as f32)?;
        client.set_latitude(settings.latitude() as f32)?;

        match settings.tracking_mode() {
            VastTrackingMode::Off => {
                client.tracking_off()?;
            }
            VastTrackingMode::Sidereal => {
                client.tracking_on()?;
            }
            VastTrackingMode::Solar | VastTrackingMode::Lunar | VastTrackingMode::Custom => {
                return Err(VastError::new(
                    VastErrorType::InvalidInput,
                    format!(
                        "OnStepVastMount does not yet support setting tracking mode '{}' through VastMountSettings",
                        settings.tracking_mode()
                    ),
                ));
            }
        }

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
