use super::*;
use crate::{
    algos::convert::{datetime_to_julian_day, j2000_to_jnow},
    base::{
        connections::{Connection, ConnectionParams},
        errors::{VastError, VastErrorType, VastResult},
    },
    types::consts,
};
use chrono::{TimeZone, Utc};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

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

fn build_mount(responses: &[&str]) -> (OnStepVastMount, Arc<Mutex<MockState>>) {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        sent_commands: Vec::new(),
        responses: responses
            .iter()
            .map(|response| Ok((*response).to_string()))
            .collect(),
    }));
    let connection = MockConnection::new_with_state(Arc::clone(&state));

    (
        OnStepVastMount {
            client: Some(OnStepClient::new(Box::new(connection), false)),
            current_settings: VastMountSettings::new(
                false,
                VastTrackingMode::Off,
                0,
                Utc::now(),
                0,
                0.0,
                0.0,
            ),
        },
        state,
    )
}

fn sent_commands(state: &Arc<Mutex<MockState>>) -> Vec<String> {
    state.lock().unwrap().sent_commands.clone()
}

fn ra_error_arcsec(lhs_hours: f64, rhs_hours: f64, dec_deg: f64) -> f64 {
    let delta_degrees =
        ((lhs_hours - rhs_hours) * consts::HOURS_TO_DEGREES + 540.0).rem_euclid(360.0) - 180.0;

    delta_degrees.abs() * 3600.0 * dec_deg.to_radians().cos()
}

#[test]
fn sexagesimal_parser_accepts_signed_values() {
    assert!(
        (OnStepVastMount::parse_ra_hours("12:30:15").unwrap() - 12.504_166_666_666_666).abs()
            < 1e-12
    );
    assert!(
        (OnStepVastMount::parse_signed_degrees("-45:15:30").unwrap() + 45.258_333_333_333_33).abs()
            < 1e-12
    );
    assert!(
        (OnStepVastMount::parse_signed_degrees("+00:00:30").unwrap() - 30.0 / 3600.0).abs() < 1e-12
    );
}

#[test]
fn sexagesimal_parser_rejects_invalid_values() {
    let error = OnStepVastMount::parse_sexagesimal("12:34").unwrap_err();

    assert_eq!(error.error_type, VastErrorType::InvalidInput);
    assert!(error.message.contains("Invalid sexagesimal value: 12:34"));
}

#[test]
fn sexagesimal_formatters_wrap_and_sign_values() {
    assert_eq!(OnStepVastMount::format_ra_hours(24.0), "00:00:00");
    assert_eq!(OnStepVastMount::format_ra_hours(-1.0 / 3600.0), "23:59:59");
    assert_eq!(OnStepVastMount::format_signed_degrees(-12.5), "-12:30:00");
    assert_eq!(OnStepVastMount::format_signed_degrees(0.0), "+00:00:00");
}

#[test]
fn tracking_rate_mapping_matches_known_modes() {
    assert_eq!(
        OnStepVastMount::parse_mount_tracking_mode(1.0),
        VastTrackingMode::Sidereal
    );
    assert_eq!(
        OnStepVastMount::parse_mount_tracking_mode(0.997_269_6),
        VastTrackingMode::Solar
    );
    assert_eq!(
        OnStepVastMount::parse_mount_tracking_mode(1.035_05),
        VastTrackingMode::Lunar
    );
    assert_eq!(
        OnStepVastMount::parse_mount_tracking_mode(1.2),
        VastTrackingMode::Custom
    );
}

#[test]
fn goto_formats_jnow_targets_and_accepts_in_progress_code() {
    let coords = EquatorialDegrees::from_ra_hours_dec_degrees(5.5, -20.25);
    let jd = datetime_to_julian_day(Utc::now());
    let (expected_ra, expected_dec) = {
        let (ra_jnow, dec_jnow) = j2000_to_jnow(5.5, -20.25, jd);
        (
            OnStepVastMount::format_ra_hours(ra_jnow),
            OnStepVastMount::format_signed_degrees(dec_jnow),
        )
    };
    let (mut mount, state) = build_mount(&["1#", "1#", "5#"]);

    mount.goto(coords).unwrap();

    assert_eq!(
        sent_commands(&state),
        vec![
            format!(":Sr{expected_ra}#"),
            format!(":Sd{expected_dec}#"),
            ":MS#".to_string(),
        ]
    );
}

#[test]
fn goto_returns_error_for_failed_slew() {
    let coords = EquatorialDegrees::from_ra_hours_dec_degrees(1.25, 10.5);
    let (mut mount, state) = build_mount(&["1#", "1#", "4#"]);

    let error = mount.goto(coords).unwrap_err();

    assert_eq!(error.error_type, VastErrorType::ConnectionError);
    assert!(
        error
            .message
            .contains("OnStep goto failed (4): mount is parked")
    );
    assert_eq!(sent_commands(&state).len(), 3);
}

#[test]
fn get_current_settings_maps_custom_tracking_rate() {
    let (mut mount, state) = build_mount(&["1.2345#", "1.2345#", "-3#", "11.25#", "45.5#"]);

    let settings = mount.get_current_settings().unwrap();

    assert_eq!(settings.tracking_mode(), VastTrackingMode::Custom);
    assert_eq!(settings.custom_tracking_value(), 1235);
    assert_eq!(settings.timezone_offset(), 3);
    assert_eq!(settings.longitude(), 11.25);
    assert_eq!(settings.latitude(), 45.5);
    assert_eq!(
        sent_commands(&state),
        vec![
            ":GT#".to_string(),
            ":GT#".to_string(),
            ":GG#".to_string(),
            ":Gg#".to_string(),
            ":Gt#".to_string(),
        ]
    );
}

#[test]
fn get_current_status_converts_coordinates_and_maps_fields() {
    let coords_j2000 = EquatorialDegrees::from_ra_hours_dec_degrees(5.25, 20.5);
    let jd = datetime_to_julian_day(Utc::now());
    let (ra_jnow, dec_jnow) = j2000_to_jnow(5.25, 20.5, jd);
    let ra_response = format!("{}#", OnStepVastMount::format_ra_hours(ra_jnow));
    let dec_response = format!("{}#", OnStepVastMount::format_signed_degrees(dec_jnow));
    let responses = [
        "1.0#",
        "0#",
        "11.5#",
        "45.0#",
        "PN#",
        &ra_response,
        "PN#",
        &dec_response,
        "PN#",
        "+45:30:00#",
        "PN#",
        "+120:15:30#",
    ];
    let (mut mount, state) = build_mount(&responses);

    let status = mount.get_current_status().unwrap();
    let (ra_hours, dec_deg) = status.coords_j2000().to_ra_hours_dec_degrees();

    assert!(status.is_tracking());
    assert!(!status.park_mode());
    assert!((status.altitude() - 45.5).abs() < 1e-12);
    assert!((status.azimuth() - 120.258_333_333_333_34).abs() < 1e-12);
    assert!(ra_error_arcsec(ra_hours, 5.25, 20.5) < 15.0);
    assert!((dec_deg - 20.5).abs() * 3600.0 < 2.0);
    assert_eq!(
        sent_commands(&state),
        vec![
            ":GT#".to_string(),
            ":GG#".to_string(),
            ":Gg#".to_string(),
            ":Gt#".to_string(),
            ":GU#".to_string(),
            ":GR#".to_string(),
            ":GU#".to_string(),
            ":GD#".to_string(),
            ":GU#".to_string(),
            ":GA#".to_string(),
            ":GU#".to_string(),
            ":GZ#".to_string(),
        ]
    );
    let (expected_ra, expected_dec) = coords_j2000.to_ra_hours_dec_degrees();
    assert!(ra_error_arcsec(ra_hours, expected_ra, expected_dec) < 15.0);
}

#[test]
fn set_settings_sends_expected_commands_for_sidereal_tracking() {
    let datetime = Utc
        .with_ymd_and_hms(2026, 5, 24, 12, 34, 56)
        .single()
        .unwrap();
    let settings = VastMountSettings::new(
        false,
        VastTrackingMode::Sidereal,
        0,
        datetime,
        2,
        11.5,
        45.25,
    );
    let (mut mount, state) = build_mount(&["1#", "1#", "1#", "1#", "1#", "1#"]);

    mount.set_settings(settings.clone()).unwrap();

    assert_eq!(mount.current_settings.datetime(), datetime);
    assert_eq!(
        mount.current_settings.tracking_mode(),
        VastTrackingMode::Sidereal
    );
    assert_eq!(
        sent_commands(&state),
        vec![
            ":SG2#".to_string(),
            ":SC05/24/2026#".to_string(),
            ":SL12:34:56#".to_string(),
            ":Sg11.5#".to_string(),
            ":St45.25#".to_string(),
            ":Te#".to_string(),
        ]
    );
}
