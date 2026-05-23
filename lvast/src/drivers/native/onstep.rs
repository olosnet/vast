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
use chrono::{DateTime, Utc};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

fn sleep(duration: Duration) {
    #[cfg(not(test))]
    std::thread::sleep(duration);

    #[cfg(test)]
    let _ = duration;
}

fn connection_worker_error(message: impl Into<String>) -> VastError {
    VastError::new(VastErrorType::ConnectionError, message.into())
}

enum WorkerRequest {
    Send {
        command: String,
        response: Sender<VastResult<()>>,
    },
    SendReceive {
        command: String,
        delay: Duration,
        response: Sender<VastResult<String>>,
    },
    Shutdown,
}

fn worker_loop(mut connection: Box<dyn Connection>, requests: Receiver<WorkerRequest>) {
    while let Ok(request) = requests.recv() {
        match request {
            WorkerRequest::Send { command, response } => {
                let _ = response.send(connection.send(&command));
            }
            WorkerRequest::SendReceive {
                command,
                delay,
                response,
            } => {
                let result = connection.send(&command).and_then(|_| {
                    sleep(delay);
                    let mut received = connection.receive()?;
                    if received.ends_with('#') {
                        received.pop();
                    }
                    Ok(received)
                });
                let _ = response.send(result);
            }
            WorkerRequest::Shutdown => {
                connection.disconnect();
                break;
            }
        }
    }
}

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
    worker_tx: Sender<WorkerRequest>,
    worker_handle: Option<JoinHandle<()>>,
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
        let (worker_tx, worker_rx) = mpsc::channel();
        let worker_handle = std::thread::spawn(move || worker_loop(connection, worker_rx));

        OnStepClient {
            worker_tx,
            worker_handle: Some(worker_handle),
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

    fn send_request(&self, command: &str) -> VastResult<()> {
        let (response_tx, response_rx) = mpsc::channel();
        self.worker_tx
            .send(WorkerRequest::Send {
                command: command.to_string(),
                response: response_tx,
            })
            .map_err(|_| connection_worker_error("OnStep worker thread is not available"))?;

        response_rx
            .recv()
            .map_err(|_| connection_worker_error("Failed to receive OnStep send result"))?
    }

    fn send_receive_request(&self, command: &str, delay: Duration) -> VastResult<String> {
        let (response_tx, response_rx) = mpsc::channel();
        self.worker_tx
            .send(WorkerRequest::SendReceive {
                command: command.to_string(),
                delay,
                response: response_tx,
            })
            .map_err(|_| connection_worker_error("OnStep worker thread is not available"))?;

        response_rx
            .recv()
            .map_err(|_| connection_worker_error("Failed to receive OnStep response"))?
    }

    fn send(&mut self, command: &str) -> bool {
        if self.show_commands {
            log::info!("Sending OnStep command: {}", command);
        }

        match self.send_request(command) {
            Ok(()) => true,
            Err(err) => {
                log::error!("Failed to send OnStep command '{}': {}", command, err);
                false
            }
        }
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

    fn send_command_with_delay(&mut self, command: &str, delay: Duration, context: &str) -> String {
        match self.send_command_result_with_delay(command, delay) {
            Ok(response) => response,
            Err(err) => {
                log::error!("{}: {}", context, err);
                String::new()
            }
        }
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
    pub fn update_status(&mut self) {
        self.last_update = Utc::now();

        let s = self.send_command(":GU#");

        if s.contains('n') && s.contains('N') {
            self.is_slewing = false;
            self.is_tracking = false;
        }

        if !s.contains('n') && !s.contains('N') {
            self.is_slewing = true;
            self.is_tracking = false;
        }

        if !s.contains('n') && s.contains('N') {
            self.is_slewing = false;
            self.is_tracking = true;
        }

        if s.contains('n') && !s.contains('N') {
            self.is_slewing = true;
            self.is_tracking = false;
        }

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
    }

    /// Logs the cached mount status after forcing a fresh `:GU#` update.
    pub fn dump_status(&mut self) {
        self.update_status();
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
    }

    /// Reads current tracking rate.
    ///
    /// Returns `0.0` if the response is missing or cannot be parsed.
    pub fn get_tracking_rate(&mut self) -> f32 {
        let s = self.send_command(":GT#");
        let rate = s.parse::<f32>().unwrap_or_else(|err| {
            log::warn!("Invalid tracking rate '{}': {}", s, err);
            0.0
        });
        rate
    }

    /// Starts alignment with the requested number of stars.
    pub fn align(&mut self, num_stars: u8) {
        let command = format!(":A{}#", num_stars);
        self.send(&command);
    }

    /// Returns current alignment status.
    pub fn get_align_status(&mut self) -> String {
        return self.send_command(":A?#");
    }

    /// Enables tracking.
    pub fn tracking_on(&mut self) -> String {
        return self.send_command(":Te#");
    }

    /// Disables tracking.
    pub fn tracking_off(&mut self) -> String {
        return self.send_command(":Td#");
    }

    /// Sets target azimuth.
    pub fn set_target_azm(&mut self, azm: &str) -> String {
        let command = format!(":Sz{}#", azm);
        return self.send_command(&command);
    }

    /// Sets target altitude.
    pub fn set_target_alt(&mut self, alt: &str) -> String {
        let command = format!(":Sa{}#", alt);
        return self.send_command(&command);
    }

    /// Sets target right ascension.
    pub fn set_target_ra(&mut self, ra: &str) -> String {
        let command = format!(":Sr{}#", ra);
        return self.send_command(&command);
    }

    /// Sets target declination.
    pub fn set_target_dec(&mut self, dec: &str) -> String {
        let command = format!(":Sd{}#", dec);
        return self.send_command(&command);
    }

    /// Starts an equatorial slew and maps known result codes to a human-readable message.
    pub fn slew_equ(&mut self) -> (String, String) {
        let r = self.send_command_with_delay(
            ":MS#",
            Duration::from_secs(3),
            "Cannot read slew_equ response",
        );

        match r.as_str() {
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
        }
    }

    /// Starts a horizontal slew and maps known result codes to a human-readable message.
    pub fn slew_hor(&mut self) -> (String, String) {
        let r = self.send_command_with_delay(
            ":MA#",
            Duration::from_secs(3),
            "Cannot read slew_hor response",
        );

        match r.as_str() {
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
        }
    }

    /// Starts a polar slew and maps known result codes to a human-readable message.
    pub fn slew_polar(&mut self) -> (String, String) {
        let r = self.send_command_with_delay(
            ":MP#",
            Duration::from_secs(3),
            "Cannot read slew_polar response",
        );

        match r.as_str() {
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
        }
    }

    /// Sends a sync command if the mount is currently tracking.
    pub fn sync(&mut self) {
        self.update_status();

        // Sync only if the scope is tracking
        if self.is_tracking {
            self.send(":CM#");
        }
    }

    /// Sets backlash value for one axis.
    ///
    /// `axis = 1` targets RA, `axis = 2` targets DEC. Invalid values are rejected locally.
    pub fn set_backlash(&mut self, axis: Option<u8>, value: Option<u8>) -> String {
        let axis = axis.unwrap_or(1);
        let value = value.unwrap_or(0);
        let ax = if axis == 1 {
            "R"
        } else if axis == 2 {
            "D"
        } else {
            log::warn!("Invalid axis for OnStep backlash command: {}", axis);
            return "0".to_string();
        };

        let command = format!(":$B{}{value}#", ax);
        return self.send_command(&command);
    }

    /// Reads backlash value for one axis.
    pub fn get_backlash(&mut self, axis: Option<u8>) -> String {
        let axis = axis.unwrap_or(1);
        let mut ax = "0";

        if axis == 1 {
            ax = "R";
        } else if axis == 2 {
            ax = "D";
        } else {
            log::warn!("Invalid axis for OnStep backlash query: {}", axis);
            return ax.to_string();
        }

        let command = format!(":%B{}#", ax);
        return self.send_command(&command);
    }

    /// Returns an OnStep equatorial debug string.
    pub fn get_debug_equ(&mut self) -> String {
        return self.send_command(":GXFE#");
    }

    /// Returns motor position for one axis.
    pub fn get_ax_motor_pos(&mut self, axis: Option<u8>) -> String {
        let axis = axis.unwrap_or(1);
        let mut ax = "0";

        if axis == 1 {
            ax = "8";
        } else if axis == 2 {
            ax = "9";
        } else {
            log::warn!("Invalid axis for OnStep motor position query: {}", axis);
            return ax.to_string();
        }

        let command = format!(":GXF{}#", ax);
        return self.send_command(&command);
    }

    /// Returns axis speed diagnostic for one axis.
    pub fn get_spd(&mut self, axis: Option<u8>) -> String {
        let axis = axis.unwrap_or(1);
        let mut ax = "0";

        if axis == 1 {
            ax = "4";
        } else if axis == 2 {
            ax = "5";
        } else {
            log::warn!("Invalid axis for OnStep speed query: {}", axis);
            return ax.to_string();
        }

        let command = format!(":GXE{}#", ax);
        return self.send_command(&command);
    }

    /// Returns correction/diagnostic value from `:GX04#`.
    pub fn get_cor_do(&mut self) -> String {
        return self.send_command(":GX04#");
    }

    /// Sets UTC offset and returns `true` on OnStep success response.
    pub fn set_utc_offset(&mut self, offset: i16) -> bool {
        let command = format!(":SG{offset}#");
        let s = self.send_command_with_delay(
            &command,
            Duration::from_secs(1),
            "Cannot read UTC offset set response",
        );
        return s == "1";
    }

    /// Reads UTC offset.
    ///
    /// Returns `0` if the response cannot be parsed.
    pub fn get_utc_offset(&mut self) -> i16 {
        let rx = self.send_command(":GG#");
        let offset = rx.parse::<i16>().unwrap_or_else(|err| {
            log::warn!("Invalid UTC offset '{}': {}", rx, err);
            0
        });
        return offset;
    }

    /// Sets mount date and returns `true` on OnStep success response.
    pub fn set_date(&mut self, date: DateTime<Utc>) -> bool {
        let command = format!(":SC{}#", date.format("%m/%d/%Y"));
        let s = self.send_command_with_delay(
            &command,
            Duration::from_secs(4),
            "Cannot read set_date response",
        );
        return s == "1";
    }

    /// Reads current mount date.
    pub fn get_date(&mut self) -> String {
        return self.send_command(":GC#");
    }

    /// Sets mount time and returns `true` on OnStep success response.
    pub fn set_time(&mut self, time: DateTime<Utc>) -> bool {
        let command = format!(":SL{}#", time.format("%H:%M:%S"));
        let s = self.send_command_with_delay(
            &command,
            Duration::from_secs(3),
            "Cannot read set_time response",
        );
        return s == "1";
    }

    /// Reads current mount time.
    pub fn get_time(&mut self, high_precision: Option<bool>) -> String {
        let command = if high_precision.unwrap_or(false) {
            ":GLa#"
        } else {
            ":GL#"
        };
        return self.send_command(&command);
    }

    /// Reads current sidereal time.
    pub fn get_sidereal_time(&mut self, high_precision: Option<bool>) -> String {
        let command = if high_precision.unwrap_or(false) {
            ":GSa#"
        } else {
            ":GS#"
        };
        return self.send_command(&command);
    }

    /// Sets horizon limit.
    ///
    /// Valid range is `-30.0..=30.0`. Invalid values are rejected before I/O.
    pub fn set_horizon_limit(&mut self, limit: f32) -> bool {
        if limit < -30.0 || limit > 30.0 {
            log::warn!("Invalid OnStep horizon limit: {}", limit);
            return false;
        }

        let command = format!(":Sh{limit}#");
        let s = self.send_command_with_delay(
            &command,
            Duration::from_secs(1),
            "Cannot read set_horizon_limit response",
        );
        return s == "1";
    }

    /// Reads horizon limit.
    pub fn get_horizon_limit(&mut self) -> String {
        return self.send_command(":GhsDD#");
    }

    /// Sets overhead limit.
    ///
    /// Valid range is `60.0..=90.0`. Invalid values are rejected before I/O.
    pub fn set_overhead_limit(&mut self, limit: f32) -> bool {
        if limit < 60.0 || limit > 90.0 {
            log::warn!("Invalid OnStep overhead limit: {}", limit);
            return false;
        }

        let command = format!(":So{limit}#");
        let s = self.send_command_with_delay(
            &command,
            Duration::from_secs(1),
            "Cannot read set_overhead_limit response",
        );
        return s == "1";
    }

    /// Reads overhead limit.
    pub fn get_overhead_limit(&mut self) -> String {
        return self.send_command(":SoDD#");
    }

    /// Sets mount longitude.
    ///
    /// Valid range is `-180.0..=180.0`. Invalid values are rejected before I/O.
    pub fn set_longitude(&mut self, longitude: f32) -> bool {
        if longitude < -180.0 || longitude > 180.0 {
            log::warn!("Invalid OnStep longitude: {}", longitude);
            return false;
        }

        let command = format!(":Sg{longitude}#");
        let s = self.send_command_with_delay(
            &command,
            Duration::from_secs(2),
            "Cannot read set_longitude response",
        );
        return s == "1";
    }

    /// Reads longitude.
    ///
    /// Returns `0.0` if the response cannot be parsed.
    pub fn get_longitude(&mut self) -> f32 {
        let rx = self.send_command(":Gg#");
        let longitude = rx.parse::<f32>().unwrap_or_else(|err| {
            log::warn!("Invalid longitude '{}': {}", rx, err);
            0.0
        });
        return longitude;
    }

    /// Sets mount latitude.
    ///
    /// Valid range is `-90.0..=90.0`. Invalid values are rejected before I/O.
    pub fn set_latitude(&mut self, latitude: f32) -> bool {
        if latitude < -90.0 || latitude > 90.0 {
            log::warn!("Invalid OnStep latitude: {}", latitude);
            return false;
        }

        let command = format!(":St{latitude}#");
        let s = self.send_command_with_delay(
            &command,
            Duration::from_secs(2),
            "Cannot read set_latitude response",
        );
        return s == "1";
    }

    /// Reads latitude.
    ///
    /// Returns `0.0` if the response cannot be parsed.
    pub fn get_latitude(&mut self) -> f32 {
        let rx = self.send_command(":Gt#");
        let latitude = rx.parse::<f32>().unwrap_or_else(|err| {
            log::warn!("Invalid latitude '{}': {}", rx, err);
            0.0
        });
        return latitude;
    }

    /// Reads OnStep version string.
    pub fn get_version(&mut self) -> String {
        return self.send_command(":GVN#");
    }

    /// Reads right ascension, refreshing mount status first.
    pub fn get_ra(&mut self, high_precision: Option<bool>) -> String {
        let command = if high_precision.unwrap_or(false) {
            ":GRa#"
        } else {
            ":GR#"
        };
        self.update_status();
        return self.send_command(&command);
    }

    /// Reads declination, refreshing mount status first.
    pub fn get_dec(&mut self) -> String {
        self.update_status();
        return self.send_command(":GD#");
    }

    /// Reads altitude, refreshing mount status first.
    pub fn get_alt(&mut self) -> String {
        self.update_status();
        return self.send_command(":GA#");
    }

    /// Reads azimuth, refreshing mount status first.
    pub fn get_azm(&mut self) -> String {
        self.update_status();
        return self.send_command(":GZ#");
    }

    /// Sends return-home command.
    pub fn return_home(&mut self) -> String {
        self.update_status();
        return self.send_command(":hC#");
    }

    /// Sends reset-home command.
    pub fn reset_home(&mut self) -> String {
        self.update_status();
        return self.send_command(":hF#");
    }

    /// Sets manual move speed preset.
    pub fn set_speed(&mut self, speed: Option<String>) {
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
        self.send(&command);
    }

    /// Sends stop motion command.
    pub fn stop(&mut self) {
        self.send(":Q#");
    }

    /// Starts manual motion in one cardinal direction.
    pub fn move_direction(&mut self, direction: char) {
        if direction == 'n' || direction == 's' || direction == 'w' || direction == 'e' {
            let command = format!(":M{}#", direction);
            self.send(&command);
        } else {
            log::warn!("Invalid OnStep move direction: {}", direction);
        }
    }

    /// Sets preferred pier side.
    pub fn set_preferred_pierside(&mut self, side: char) {
        if side != 'W' && side != 'E' && side != 'B' {
            log::warn!("Invalid OnStep preferred pier side: {}", side);
            return;
        }

        let command = format!(":SX96,{}#", side);
        self.send(&command);
    }
}

impl Drop for OnStepClient {
    fn drop(&mut self) {
        let _ = self.worker_tx.send(WorkerRequest::Shutdown);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::connections::{Connection, ConnectionParams};
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    fn assert_send<T: Send>() {}

    #[derive(Default)]
    struct MockState {
        connected: bool,
        sent_commands: Vec<String>,
        responses: VecDeque<VastResult<String>>,
    }

    struct MockConnection {
        state: Arc<Mutex<MockState>>,
    }

    impl MockConnection {
        fn new_with_state(state: Arc<Mutex<MockState>>) -> Self {
            Self { state }
        }
    }

    impl Connection for MockConnection {
        fn new(_params: &ConnectionParams) -> VastResult<Self>
        where
            Self: Sized,
        {
            Ok(Self {
                state: Arc::new(Mutex::new(MockState {
                    connected: true,
                    ..Default::default()
                })),
            })
        }

        fn send(&mut self, command: &str) -> VastResult<()> {
            let mut state = self.state.lock().unwrap();
            if !state.connected {
                return Err(VastError::new(
                    VastErrorType::TcpGenericError,
                    "mock disconnected".to_string(),
                ));
            }
            state.sent_commands.push(command.to_string());
            Ok(())
        }

        fn receive(&mut self) -> VastResult<String> {
            let mut state = self.state.lock().unwrap();
            state.responses.pop_front().unwrap_or_else(|| {
                Err(VastError::new(
                    VastErrorType::TcpReadError,
                    "no mocked response registered".to_string(),
                ))
            })
        }

        fn disconnect(&mut self) {
            self.state.lock().unwrap().connected = false;
        }

        fn is_connected(&mut self) -> bool {
            self.state.lock().unwrap().connected
        }
    }

    fn build_client(responses: &[&str]) -> (OnStepClient, Arc<Mutex<MockState>>) {
        let state = Arc::new(Mutex::new(MockState {
            connected: true,
            sent_commands: Vec::new(),
            responses: responses
                .iter()
                .map(|response| Ok((*response).to_string()))
                .collect(),
        }));
        let connection = MockConnection::new_with_state(Arc::clone(&state));
        (OnStepClient::new(Box::new(connection), false), state)
    }

    #[test]
    fn send_command_trims_hash_terminator() {
        let (mut client, state) = build_client(&["1#"]);

        let response = client.send_command(":GVN#");

        assert_eq!(response, "1");
        assert_eq!(state.lock().unwrap().sent_commands, vec![":GVN#"]);
    }

    #[test]
    fn update_status_parses_gu_response() {
        let (mut client, _) = build_client(&["PNHGSRET127#"]);

        client.update_status();

        assert!(!client.is_slewing);
        assert!(client.is_tracking);
        assert!(client.is_parked);
        assert_eq!(client.parking_status, "Parked");
        assert_eq!(client.mount_type, "Equatorial");
        assert!(client.is_home);
        assert_eq!(client.pier_side, "East");
        assert!(client.pec_recorded);
        assert_eq!(client.guide_status, "Guide pulse active");
        assert!(client.pps);
        assert_eq!(client.pulse_guide_rate, "1");
        assert_eq!(client.guide_rate, "2");
        assert_eq!(client.general_error, 7);
    }

    #[test]
    fn slew_equ_maps_known_response_code() {
        let (mut client, state) = build_client(&["5#"]);

        let response = client.slew_equ();

        assert_eq!(response, ("5".to_string(), "Goto in progress".to_string()));
        assert_eq!(state.lock().unwrap().sent_commands, vec![":MS#"]);
    }

    #[test]
    fn get_ra_refreshes_status_before_query() {
        let (mut client, state) = build_client(&["N#", "12:34:56#"]);

        let ra = client.get_ra(None);

        assert_eq!(ra, "12:34:56");
        assert_eq!(state.lock().unwrap().sent_commands, vec![":GU#", ":GR#"]);
    }

    #[test]
    fn set_horizon_limit_rejects_invalid_value_without_io() {
        let (mut client, state) = build_client(&[]);

        let result = client.set_horizon_limit(31.0);

        assert!(!result);
        assert!(state.lock().unwrap().sent_commands.is_empty());
    }

    #[test]
    fn onstep_client_is_send() {
        assert_send::<OnStepClient>();
    }
}
