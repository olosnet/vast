use crate::{
    base::{
        connections::{Connection, ConnectionParams},
        errors::{VastError, VastErrorType, VastResult},
    },
    drivers::native::moonlight::driver::MoonlightFocuser,
    types::{common::TemperatureUnit, focuser::VastFocuser},
};
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

fn build_focuser(responses: &[&str]) -> (MoonlightFocuser, Arc<Mutex<MockState>>) {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        sent_commands: Vec::new(),
        responses: responses
            .iter()
            .map(|response| Ok((*response).to_string()))
            .collect(),
    }));
    let connection = MockConnection::new_with_state(Arc::clone(&state));
    let mut focuser = MoonlightFocuser::new();
    focuser.connect(Box::new(connection)).unwrap();
    (focuser, state)
}

#[test]
fn connect_retries_handshake_and_logs_version_read() {
    let (focuser, state) = build_focuser(&["12", "12"]);

    assert_eq!(focuser.read_version().unwrap(), "1.2");
    assert_eq!(state.lock().unwrap().sent_commands, vec![":GV#", ":GV#"]);
}

#[test]
fn current_position_reads_hex_value() {
    let (focuser, state) = build_focuser(&["12", "00FA#"]);

    let position = focuser.current_position().unwrap();

    assert_eq!(position, 250);
    assert_eq!(state.lock().unwrap().sent_commands, vec![":GV#", ":GP#"]);
}

#[test]
fn move_to_sends_target_and_start_commands() {
    let (mut focuser, state) = build_focuser(&["12"]);

    focuser.move_to(0x1A2B).unwrap();

    assert_eq!(
        state.lock().unwrap().sent_commands,
        vec![":GV#", ":SN1A2B#", ":FG#"]
    );
}

#[test]
fn current_temperature_applies_selected_unit() {
    let (mut focuser, state) = build_focuser(&["12", "001E#", "001E#"]);

    focuser
        .set_temperature_unit(TemperatureUnit::Kelvin)
        .unwrap();
    let kelvin = focuser.current_temperature().unwrap();
    focuser
        .set_temperature_unit(TemperatureUnit::Fahrenheit)
        .unwrap();
    let fahrenheit = focuser.current_temperature().unwrap();

    assert!((kelvin - 288.15).abs() < 1e-6);
    assert!((fahrenheit - 59.0).abs() < 1e-6);
    assert_eq!(
        state.lock().unwrap().sent_commands,
        vec![":GV#", ":C#", ":GT#", ":C#", ":GT#"]
    );
}

#[test]
fn is_moving_accepts_short_and_padded_values() {
    let (focuser, _) = build_focuser(&["12", "01#"]);

    assert!(focuser.is_moving().unwrap());
}

#[test]
fn move_to_rejects_out_of_range_value_without_io() {
    let (mut focuser, state) = build_focuser(&["12"]);

    let result = focuser.move_to(0x1_0000);

    assert!(matches!(
        result,
        Err(VastError {
            error_type: VastErrorType::InvalidInput,
            ..
        })
    ));
    assert_eq!(state.lock().unwrap().sent_commands, vec![":GV#"]);
}

#[test]
fn moonlight_focuser_is_send() {
    assert_send::<MoonlightFocuser>();
}
