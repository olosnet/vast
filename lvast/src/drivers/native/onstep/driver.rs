//! Native OnStep mount client.
//!
//! This module adapts an existing OnStep client implementation to `lvast` connection and error
//! types. It keeps a synchronous command/response model over a boxed [`Connection`].
//!
//! Test target:
//!
//! - verify exact command strings sent to OnStep
//! - verify parsing of `:GU#` mount status response
//! - verify command sequencing for multi-step queries
//! - verify local input validation before transport I/O
//!
//! Test strategy:
//!
//! - unit tests use a mocked [`Connection`]
//! - each test registers queued responses before executing client code
//! - each test asserts sent command list and returned/parsing behavior
//! - no real serial or TCP device is required

use crate::base::connections::Connection;
use crate::base::errors::{VastError, VastErrorType, VastResult};
use crate::base::workers::{ConnectionWorker, ReceiveOptions};
use chrono::{DateTime, Utc};
use std::time::Duration;

/// Synchronous OnStep command client backed by a boxed [`Connection`].
///
/// The client stores a cached subset of parsed mount state extracted from `:GU#` responses and
/// provides convenience helpers for common OnStep commands.
///
/// Threading model:
///
/// - the client is blocking and request/response oriented
/// - one dedicated worker thread owns the underlying connection
/// - transport I/O is serialized through a request queue
/// - cached state in [`OnStepClient`] is still mutated on the caller thread, so shared access
///   should use `Arc<Mutex<OnStepClient>>`
pub struct OnStepClient {
    worker: ConnectionWorker,
    show_commands: bool,
    is_slewing: bool,
    is_tracking: bool,
    is_parked: bool,
    parking_status: String,
    mount_type: String,
    home_wait: bool,
    is_home: bool,
    pier_side: String,
    pec_recorded: bool,
    pec: String,
    pps: bool,
    guide_status: String,
    pulse_guide_rate: String,
    guide_rate: String,
    general_error: i32,
    last_update: DateTime<Utc>,
}

impl OnStepClient {
    /// Creates a new OnStep client over an existing connection.
    ///
    /// If `show_commands` is `true`, outgoing OnStep commands are logged with `log::info!`.
    pub fn new(connection: Box<dyn Connection>, show_commands: bool) -> Self {
        OnStepClient {
            worker: ConnectionWorker::new("OnStep", connection),
            show_commands,
            is_slewing: false,
            is_tracking: false,
            is_parked: false,
            parking_status: "".to_string(),
            mount_type: "".to_string(),
            home_wait: false,
            is_home: false,
            pier_side: "".to_string(),
            pec_recorded: false,
            pec: "".to_string(),
            pps: false,
            guide_status: "".to_string(),
            pulse_guide_rate: "".to_string(),
            guide_rate: "".to_string(),
            general_error: 0,
            last_update: Utc::now(),
        }
    }

    fn invalid_input(message: impl Into<String>) -> VastError {
        VastError::new(VastErrorType::InvalidInput, message.into())
    }

    fn parse_onstep_utc_offset_minutes(value: &str) -> VastResult<i32> {
        let trimmed = value.trim();
        let (sign, unsigned) = if let Some(rest) = trimmed.strip_prefix('-') {
            (-1, rest)
        } else if let Some(rest) = trimmed.strip_prefix('+') {
            (1, rest)
        } else {
            (1, trimmed)
        };

        let (hours_str, minutes) = if let Some((hours, minutes_str)) = unsigned.split_once(':') {
            let minutes = match minutes_str {
                "00" => 0,
                "30" => 30,
                "45" => 45,
                _ => {
                    return Err(Self::invalid_input(format!(
                        "Invalid UTC offset minutes '{}': {}",
                        value, minutes_str
                    )));
                }
            };
            (hours, minutes)
        } else {
            (unsigned, 0)
        };

        let hours = hours_str.parse::<i32>().map_err(|err| {
            Self::invalid_input(format!("Invalid UTC offset '{}': {}", value, err))
        })?;

        Ok(sign * (hours.abs() * 60 + minutes))
    }

    fn format_onstep_utc_offset_minutes(utc_offset_minutes: i32) -> VastResult<String> {
        let onstep_offset_minutes = -utc_offset_minutes;
        let sign = if onstep_offset_minutes < 0 { '-' } else { '+' };
        let absolute_minutes = onstep_offset_minutes.abs();
        let hours = absolute_minutes / 60;
        let minutes = absolute_minutes % 60;

        if !matches!(minutes, 0 | 30 | 45) {
            return Err(Self::invalid_input(format!(
                "OnStep supports only whole-hour, half-hour, or 45-minute UTC offsets, got {} minutes",
                utc_offset_minutes
            )));
        }

        if minutes == 0 {
            Ok(format!("{sign}{hours}"))
        } else {
            Ok(format!("{sign}{hours:02}:{minutes:02}"))
        }
    }

    fn send_request(&self, command: &str) -> VastResult<()> {
        self.worker.send(command)
    }

    fn send_receive_request(&self, command: &str, delay: Duration) -> VastResult<String> {
        self.worker.send_receive_with_options(
            command,
            ReceiveOptions {
                delay,
                trim_suffix: Some('#'),
            },
        )
    }

    fn send_command_result_with_delay(
        &mut self,
        command: &str,
        delay: Duration,
    ) -> VastResult<String> {
        if self.show_commands {
            log::info!("Sending OnStep command: {}", command);
        }

        self.send_receive_request(command, delay)
    }

    /// Sends one command and returns the response payload without the trailing `#` terminator.
    pub fn send_command_result(&mut self, command: &str) -> VastResult<String> {
        self.send_command_result_with_delay(command, Duration::from_millis(0))
    }

    /// Sends one command and returns the response payload without the trailing `#` terminator.
    ///
    /// This is a compatibility wrapper around [`Self::send_command_result`] that logs failures
    /// and returns an empty string on error.
    pub fn send_command(&mut self, command: &str) -> String {
        match self.send_command_result(command) {
            Ok(response) => response,
            Err(err) => {
                log::error!("No response for command {}: {}", command, err);
                String::new()
            }
        }
    }

    /// Refreshes cached mount state by parsing the `:GU#` status response.
    pub fn update_status(&mut self) -> VastResult<()> {
        self.last_update = Utc::now();

        let s = self.send_command_result(":GU#")?;

        self.is_tracking = !s.contains('n');
        self.is_slewing = !s.contains('N');

        if s.contains('P') {
            self.is_parked = true;
            self.parking_status = "Parked".to_string();
        }

        if s.contains('p') {
            self.is_parked = false;
            self.parking_status = "Unparked".to_string();
        }

        if s.contains('I') {
            self.is_parked = false;
            self.parking_status = "Parking in progress".to_string();
        }

        if s.contains('F') {
            self.is_parked = false;
            self.parking_status = "Parking failed".to_string();
        }

        if s.contains('H') {
            self.is_home = true;
        } else {
            self.is_home = false;
        }

        if s.contains('w') {
            self.home_wait = true;
        } else {
            self.home_wait = false;
        }

        if s.contains('G') {
            self.guide_status = "Guide pulse active".to_string();
        } else {
            self.guide_status = "".to_string();
        }

        if s.contains('S') {
            self.pps = true;
        } else {
            self.pps = false;
        }

        if s.contains('R') {
            self.pec_recorded = true;
        } else {
            self.pec_recorded = false;
        }

        if s.contains('/') {
            self.pec = "Ignore".to_string();
        }
        if s.contains(',') {
            self.pec = "Ready to Play".to_string();
        }
        if s.contains('~') {
            self.pec = "Playing".to_string();
        }
        if s.contains(';') {
            self.pec = "Ready to Record".to_string();
        }
        if s.contains('^') {
            self.pec = "Recording".to_string();
        }

        if s.contains('E') {
            self.mount_type = "Equatorial".to_string();
        }
        if s.contains('K') {
            self.mount_type = "Fork".to_string();
        }
        if s.contains('k') {
            self.mount_type = "Fork Alternate".to_string();
        }
        if s.contains('A') {
            self.mount_type = "AltAz".to_string();
        }

        if s.contains('o') {
            self.pier_side = "None".to_string();
        }
        if s.contains('T') {
            self.pier_side = "East".to_string();
        }
        if s.contains('W') {
            self.pier_side = "West".to_string();
        }

        let chars: Vec<char> = s.chars().collect();
        if chars.len() > 3 {
            self.pulse_guide_rate = chars[chars.len() - 3].to_string();
            self.guide_rate = chars[chars.len() - 2].to_string();
            self.general_error = chars[chars.len() - 1].to_digit(10).unwrap_or(0) as i32;
        }

        Ok(())
    }

    /// Logs the cached mount status after forcing a fresh `:GU#` update.
    pub fn dump_status(&mut self) -> VastResult<()> {
        self.update_status()?;
        log::info!("OnStep mount type: {}", self.mount_type);
        log::info!("OnStep is slewing: {}", self.is_slewing);
        log::info!("OnStep is tracking: {}", self.is_tracking);
        log::info!("OnStep is parked: {}", self.is_parked);
        log::info!("OnStep parking status: {}", self.parking_status);
        log::info!("OnStep home wait: {}", self.home_wait);
        log::info!("OnStep is home: {}", self.is_home);
        log::info!("OnStep pier side: {}", self.pier_side);
        log::info!("OnStep PEC recorded: {}", self.pec_recorded);
        log::info!("OnStep PEC: {}", self.pec);
        log::info!("OnStep PPS: {}", self.pps);
        log::info!("OnStep guide status: {}", self.guide_status);
        log::info!("OnStep pulse guide rate: {}", self.pulse_guide_rate);
        log::info!("OnStep guide rate: {}", self.guide_rate);
        log::info!("OnStep general error: {}", self.general_error);
        log::info!("OnStep last update: {}", self.last_update);

        Ok(())
    }

    /// Returns cached tracking state from last `:GU#` refresh.
    pub fn is_tracking(&self) -> bool {
        self.is_tracking
    }

    /// Returns cached slewing state from last `:GU#` refresh.
    pub fn is_slewing(&self) -> bool {
        self.is_slewing
    }

    /// Returns cached parked state from last `:GU#` refresh.
    pub fn is_parked(&self) -> bool {
        self.is_parked
    }

    /// Returns cached pier side from last `:GU#` refresh.
    pub fn pier_side(&self) -> Option<&str> {
        match self.pier_side.as_str() {
            "East" => Some("East"),
            "West" => Some("West"),
            _ => None,
        }
    }

    /// Reads current tracking rate.
    ///
    /// Returns an error if the response cannot be parsed.
    pub fn get_tracking_rate(&mut self) -> VastResult<f32> {
        let s = self.send_command_result(":GT#")?;
        s.parse::<f32>()
            .map_err(|err| Self::invalid_input(format!("Invalid tracking rate '{}': {}", s, err)))
    }

    /// Starts alignment with the requested number of stars.
    pub fn align(&mut self, num_stars: u8) -> VastResult<()> {
        let command = format!(":A{}#", num_stars);
        self.send_request(&command)
    }

    /// Returns current alignment status.
    pub fn get_align_status(&mut self) -> VastResult<String> {
        self.send_command_result(":A?#")
    }

    /// Enables tracking.
    pub fn tracking_on(&mut self) -> VastResult<String> {
        self.send_command_result(":Te#")
    }

    /// Disables tracking.
    pub fn tracking_off(&mut self) -> VastResult<String> {
        self.send_command_result(":Td#")
    }

    /// Selects sidereal tracking rate.
    pub fn tracking_sidereal(&mut self) -> VastResult<()> {
        self.send_request(":TQ#")
    }

    /// Selects solar tracking rate.
    pub fn tracking_solar(&mut self) -> VastResult<()> {
        self.send_request(":TS#")
    }

    /// Selects lunar tracking rate.
    pub fn tracking_lunar(&mut self) -> VastResult<()> {
        self.send_request(":TL#")
    }

    /// Sets custom tracking rate in OnStep `:ST...#` units.
    pub fn set_tracking_rate(&mut self, rate_hz: f32) -> VastResult<bool> {
        if !rate_hz.is_finite() {
            return Err(Self::invalid_input(format!(
                "Invalid OnStep tracking rate: {}",
                rate_hz
            )));
        }

        let command = format!(":ST{rate_hz:.5}#");
        let s = self.send_command_result(&command)?;
        Ok(s == "1")
    }

    /// Sets target azimuth.
    pub fn set_target_azm(&mut self, azm: &str) -> VastResult<String> {
        let command = format!(":Sz{}#", azm);
        self.send_command_result(&command)
    }

    /// Sets target altitude.
    pub fn set_target_alt(&mut self, alt: &str) -> VastResult<String> {
        let command = format!(":Sa{}#", alt);
        self.send_command_result(&command)
    }

    /// Sets target right ascension.
    pub fn set_target_ra(&mut self, ra: &str) -> VastResult<String> {
        let command = format!(":Sr{}#", ra);
        self.send_command_result(&command)
    }

    /// Sets target declination.
    pub fn set_target_dec(&mut self, dec: &str) -> VastResult<String> {
        let command = format!(":Sd{}#", dec);
        self.send_command_result(&command)
    }

    /// Starts an equatorial slew and maps known result codes to a human-readable message.
    pub fn slew_equ(&mut self) -> VastResult<(String, String)> {
        let r = self.send_command_result_with_delay(":MS#", Duration::from_secs(3))?;

        Ok(match r.as_str() {
            "0" => ("0".to_string(), "Goto is possible".to_string()),
            "1" => ("1".to_string(), "below the horizon limit".to_string()),
            "2" => ("2".to_string(), "above overhead limit".to_string()),
            "3" => ("3".to_string(), "controller in standby".to_string()),
            "4" => ("4".to_string(), "mount is parked".to_string()),
            "5" => ("5".to_string(), "Goto in progress".to_string()),
            "6" => (
                "6".to_string(),
                "outside limits (MaxDec, MinDec, UnderPoleLimit, MeridianLimit)".to_string(),
            ),
            "7" => ("7".to_string(), "hardware fault".to_string()),
            "8" => ("8".to_string(), "already in motion".to_string()),
            _ => (r, "unspecified error".to_string()),
        })
    }

    /// Starts a horizontal slew and maps known result codes to a human-readable message.
    pub fn slew_hor(&mut self) -> VastResult<(String, String)> {
        let r = self.send_command_result_with_delay(":MA#", Duration::from_secs(3))?;

        Ok(match r.as_str() {
            "0" => ("0".to_string(), "Goto is possible".to_string()),
            "1" => ("1".to_string(), "below the horizon limit".to_string()),
            "2" => ("2".to_string(), "above overhead limit".to_string()),
            "3" => ("3".to_string(), "controller in standby".to_string()),
            "4" => ("4".to_string(), "mount is parked".to_string()),
            "5" => ("5".to_string(), "Goto in progress".to_string()),
            "6" => (
                "6".to_string(),
                "outside limits (MaxDec, MinDec, UnderPoleLimit, MeridianLimit)".to_string(),
            ),
            "7" => ("7".to_string(), "hardware fault".to_string()),
            "8" => ("8".to_string(), "already in motion".to_string()),
            _ => (r, "unspecified error".to_string()),
        })
    }

    /// Starts a polar slew and maps known result codes to a human-readable message.
    pub fn slew_polar(&mut self) -> VastResult<(String, String)> {
        let r = self.send_command_result_with_delay(":MP#", Duration::from_secs(3))?;

        Ok(match r.as_str() {
            "0" => ("0".to_string(), "Goto is possible".to_string()),
            "1" => ("1".to_string(), "below the horizon limit".to_string()),
            "2" => ("2".to_string(), "above overhead limit".to_string()),
            "3" => ("3".to_string(), "controller in standby".to_string()),
            "4" => ("4".to_string(), "mount is parked".to_string()),
            "5" => ("5".to_string(), "Goto in progress".to_string()),
            "6" => (
                "6".to_string(),
                "outside limits (MaxDec, MinDec, UnderPoleLimit, MeridianLimit)".to_string(),
            ),
            "7" => ("7".to_string(), "hardware fault".to_string()),
            "8" => ("8".to_string(), "already in motion".to_string()),
            _ => (r, "unspecified error".to_string()),
        })
    }

    /// Sends a sync command if the mount is currently tracking.
    pub fn sync(&mut self) -> VastResult<()> {
        self.update_status()?;

        // Sync only if the scope is tracking
        if self.is_tracking {
            self.send_request(":CM#")?;
        }

        Ok(())
    }

    /// Sets backlash value for one axis.
    ///
    /// `axis = 1` targets RA, `axis = 2` targets DEC. Invalid values are rejected locally.
    pub fn set_backlash(&mut self, axis: Option<u8>, value: Option<u8>) -> VastResult<String> {
        let axis = axis.unwrap_or(1);
        let value = value.unwrap_or(0);
        let ax = if axis == 1 {
            "R"
        } else if axis == 2 {
            "D"
        } else {
            return Err(Self::invalid_input(format!(
                "Invalid axis for OnStep backlash command: {}",
                axis
            )));
        };

        let command = format!(":$B{}{value}#", ax);
        self.send_command_result(&command)
    }

    /// Reads backlash value for one axis.
    pub fn get_backlash(&mut self, axis: Option<u8>) -> VastResult<String> {
        let axis = axis.unwrap_or(1);
        let ax = match axis {
            1 => "R",
            2 => "D",
            _ => {
                return Err(Self::invalid_input(format!(
                    "Invalid axis for OnStep backlash query: {}",
                    axis
                )));
            }
        };

        let command = format!(":%B{}#", ax);
        self.send_command_result(&command)
    }

    /// Returns an OnStep equatorial debug string.
    pub fn get_debug_equ(&mut self) -> VastResult<String> {
        self.send_command_result(":GXFE#")
    }

    /// Returns motor position for one axis.
    pub fn get_ax_motor_pos(&mut self, axis: Option<u8>) -> VastResult<String> {
        let axis = axis.unwrap_or(1);
        let ax = match axis {
            1 => "8",
            2 => "9",
            _ => {
                return Err(Self::invalid_input(format!(
                    "Invalid axis for OnStep motor position query: {}",
                    axis
                )));
            }
        };

        let command = format!(":GXF{}#", ax);
        self.send_command_result(&command)
    }

    /// Returns axis speed diagnostic for one axis.
    pub fn get_spd(&mut self, axis: Option<u8>) -> VastResult<String> {
        let axis = axis.unwrap_or(1);
        let ax = match axis {
            1 => "4",
            2 => "5",
            _ => {
                return Err(Self::invalid_input(format!(
                    "Invalid axis for OnStep speed query: {}",
                    axis
                )));
            }
        };

        let command = format!(":GXE{}#", ax);
        self.send_command_result(&command)
    }

    /// Returns correction/diagnostic value from `:GX04#`.
    pub fn get_cor_do(&mut self) -> VastResult<String> {
        self.send_command_result(":GX04#")
    }

    /// Sets signed local civil-time offset from UTC in minutes.
    ///
    /// OnStep expects opposite sign: number of hours to add to local time to obtain UTC.
    pub fn set_utc_offset_minutes(&mut self, utc_offset_minutes: i32) -> VastResult<bool> {
        let offset = Self::format_onstep_utc_offset_minutes(utc_offset_minutes)?;
        let command = format!(":SG{offset}#");
        let s = self.send_command_result_with_delay(&command, Duration::from_secs(1))?;
        Ok(s == "1")
    }

    /// Reads signed local civil-time offset from UTC in minutes.
    ///
    /// Returns an error if the response cannot be parsed.
    pub fn get_utc_offset_minutes(&mut self) -> VastResult<i32> {
        let rx = self.send_command_result(":GG#")?;
        Self::parse_onstep_utc_offset_minutes(&rx).map(|minutes| -minutes)
    }

    /// Sets mount local date and returns `true` on OnStep success response.
    pub fn set_local_date(&mut self, date: DateTime<Utc>) -> VastResult<bool> {
        let command = format!(":SC{}#", date.format("%m/%d/%y"));
        let s = self.send_command_result_with_delay(&command, Duration::from_secs(4))?;
        Ok(s == "1")
    }

    /// Reads current mount date.
    pub fn get_date(&mut self) -> VastResult<String> {
        self.send_command_result(":GC#")
    }

    /// Sets mount local time and returns `true` on OnStep success response.
    pub fn set_local_time(&mut self, time: DateTime<Utc>) -> VastResult<bool> {
        let command = format!(":SL{}#", time.format("%H:%M:%S"));
        let s = self.send_command_result_with_delay(&command, Duration::from_secs(3))?;
        Ok(s == "1")
    }

    /// Reads current mount time.
    pub fn get_time(&mut self, high_precision: Option<bool>) -> VastResult<String> {
        let command = if high_precision.unwrap_or(false) {
            ":GLa#"
        } else {
            ":GL#"
        };
        self.send_command_result(&command)
    }

    /// Reads current sidereal time.
    pub fn get_sidereal_time(&mut self, high_precision: Option<bool>) -> VastResult<String> {
        let command = if high_precision.unwrap_or(false) {
            ":GSa#"
        } else {
            ":GS#"
        };
        self.send_command_result(&command)
    }

    /// Sets horizon limit.
    ///
    /// Valid range is `-30.0..=30.0`. Invalid values are rejected before I/O.
    pub fn set_horizon_limit(&mut self, limit: f32) -> VastResult<bool> {
        if limit < -30.0 || limit > 30.0 {
            return Err(Self::invalid_input(format!(
                "Invalid OnStep horizon limit: {}",
                limit
            )));
        }

        let command = format!(":Sh{limit}#");
        let s = self.send_command_result_with_delay(&command, Duration::from_secs(1))?;
        Ok(s == "1")
    }

    /// Reads horizon limit.
    pub fn get_horizon_limit(&mut self) -> VastResult<String> {
        self.send_command_result(":GhsDD#")
    }

    /// Sets overhead limit.
    ///
    /// Valid range is `60.0..=90.0`. Invalid values are rejected before I/O.
    pub fn set_overhead_limit(&mut self, limit: f32) -> VastResult<bool> {
        if limit < 60.0 || limit > 90.0 {
            return Err(Self::invalid_input(format!(
                "Invalid OnStep overhead limit: {}",
                limit
            )));
        }

        let command = format!(":So{limit}#");
        let s = self.send_command_result_with_delay(&command, Duration::from_secs(1))?;
        Ok(s == "1")
    }

    /// Reads overhead limit.
    pub fn get_overhead_limit(&mut self) -> VastResult<String> {
        self.send_command_result(":SoDD#")
    }

    /// Sets mount longitude.
    ///
    /// Valid range is `-180.0..=180.0`. Invalid values are rejected before I/O.
    pub fn set_longitude(&mut self, longitude: f32) -> VastResult<bool> {
        if longitude < -180.0 || longitude > 180.0 {
            return Err(Self::invalid_input(format!(
                "Invalid OnStep longitude: {}",
                longitude
            )));
        }

        let command = format!(":Sg{longitude}#");
        let s = self.send_command_result_with_delay(&command, Duration::from_secs(2))?;
        Ok(s == "1")
    }

    /// Reads longitude.
    ///
    /// Returns an error if the response cannot be parsed.
    pub fn get_longitude(&mut self) -> VastResult<f32> {
        let rx = self.send_command_result(":Gg#")?;
        rx.parse::<f32>()
            .map_err(|err| Self::invalid_input(format!("Invalid longitude '{}': {}", rx, err)))
    }

    /// Sets mount latitude.
    ///
    /// Valid range is `-90.0..=90.0`. Invalid values are rejected before I/O.
    pub fn set_latitude(&mut self, latitude: f32) -> VastResult<bool> {
        if latitude < -90.0 || latitude > 90.0 {
            return Err(Self::invalid_input(format!(
                "Invalid OnStep latitude: {}",
                latitude
            )));
        }

        let command = format!(":St{latitude}#");
        let s = self.send_command_result_with_delay(&command, Duration::from_secs(2))?;
        Ok(s == "1")
    }

    /// Reads latitude.
    ///
    /// Returns an error if the response cannot be parsed.
    pub fn get_latitude(&mut self) -> VastResult<f32> {
        let rx = self.send_command_result(":Gt#")?;
        rx.parse::<f32>()
            .map_err(|err| Self::invalid_input(format!("Invalid latitude '{}': {}", rx, err)))
    }

    /// Reads OnStep version string.
    pub fn get_version(&mut self) -> VastResult<String> {
        self.send_command_result(":GVN#")
    }

    /// Reads right ascension, refreshing mount status first.
    pub fn get_ra(&mut self, high_precision: Option<bool>) -> VastResult<String> {
        let command = if high_precision.unwrap_or(false) {
            ":GRa#"
        } else {
            ":GR#"
        };
        self.update_status()?;
        self.send_command_result(&command)
    }

    /// Reads declination, refreshing mount status first.
    pub fn get_dec(&mut self) -> VastResult<String> {
        self.update_status()?;
        self.send_command_result(":GD#")
    }

    /// Reads altitude, refreshing mount status first.
    pub fn get_alt(&mut self) -> VastResult<String> {
        self.update_status()?;
        self.send_command_result(":GA#")
    }

    /// Reads azimuth, refreshing mount status first.
    pub fn get_azm(&mut self) -> VastResult<String> {
        self.update_status()?;
        self.send_command_result(":GZ#")
    }

    /// Sends return-home command.
    pub fn return_home(&mut self) -> VastResult<String> {
        self.update_status()?;
        self.send_command_result(":hC#")
    }

    /// Sends reset-home command.
    pub fn reset_home(&mut self) -> VastResult<String> {
        self.update_status()?;
        self.send_command_result(":hF#")
    }

    /// Sets manual move speed preset.
    pub fn set_speed(&mut self, speed: Option<String>) -> VastResult<()> {
        let speed = speed.unwrap_or("20x".to_string());

        let s = match speed.as_str() {
            "0.25x" => "0",
            "0.5x" => "1",
            "1x" => "G",
            "2x" => "3",
            "4x" => "4",
            "8x" => "C",
            "20x" => "M",
            "48x" => "F",
            "half" => "S",
            "max" => "9",
            _ => "G",
        };

        let command = format!(":R{}#", s);
        self.send_request(&command)
    }

    /// Sends stop motion command.
    pub fn stop(&mut self) -> VastResult<()> {
        self.send_request(":Q#")
    }

    /// Starts manual motion in one cardinal direction.
    pub fn move_direction(&mut self, direction: char) -> VastResult<()> {
        if direction == 'n' || direction == 's' || direction == 'w' || direction == 'e' {
            let command = format!(":M{}#", direction);
            self.send_request(&command)
        } else {
            Err(Self::invalid_input(format!(
                "Invalid OnStep move direction: {}",
                direction
            )))
        }
    }

    /// Sets preferred pier side.
    pub fn set_preferred_pierside(&mut self, side: char) -> VastResult<()> {
        if side != 'W' && side != 'E' && side != 'B' {
            return Err(Self::invalid_input(format!(
                "Invalid OnStep preferred pier side: {}",
                side
            )));
        }

        let command = format!(":SX96,{}#", side);
        self.send_request(&command)
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
