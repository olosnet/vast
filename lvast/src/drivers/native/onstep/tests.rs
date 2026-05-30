use crate::base::{
    connections::{Connection, ConnectionParams},
    errors::{VastError, VastErrorType, VastResult},
};
use crate::drivers::native::onstep::driver::OnStepClient;
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

    client.update_status().unwrap();

    assert!(!client.is_slewing());
    assert!(client.is_tracking());
    assert!(client.is_parked());
    assert_eq!(client.pier_side(), Some("East"));
}

#[test]
fn slew_equ_maps_known_response_code() {
    let (mut client, state) = build_client(&["5#"]);

    let response = client.slew_equ();

    assert_eq!(
        response.unwrap(),
        ("5".to_string(), "Goto in progress".to_string())
    );
    assert_eq!(state.lock().unwrap().sent_commands, vec![":MS#"]);
}

#[test]
fn get_ra_refreshes_status_before_query() {
    let (mut client, state) = build_client(&["N#", "12:34:56#"]);

    let ra = client.get_ra(None);

    assert_eq!(ra.unwrap(), "12:34:56");
    assert_eq!(state.lock().unwrap().sent_commands, vec![":GU#", ":GR#"]);
}

#[test]
fn set_horizon_limit_rejects_invalid_value_without_io() {
    let (mut client, state) = build_client(&[]);

    let result = client.set_horizon_limit(31.0);

    assert!(matches!(
        result,
        Err(VastError {
            error_type: VastErrorType::InvalidInput,
            ..
        })
    ));
    assert!(state.lock().unwrap().sent_commands.is_empty());
}

#[test]
fn utc_offset_minutes_round_trip_uses_onstep_sign_and_fractional_format() {
    let (mut client, state) = build_client(&["1#", "+5:30#"]);

    let set_result = client.set_utc_offset_minutes(-330);
    let get_result = client.get_utc_offset_minutes();

    assert_eq!(set_result.unwrap(), true);
    assert_eq!(get_result.unwrap(), -330);
    assert_eq!(
        state.lock().unwrap().sent_commands,
        vec![":SG+05:30#", ":GG#"]
    );
}

#[test]
fn utc_offset_minutes_rejects_unsupported_fraction_without_io() {
    let (mut client, state) = build_client(&[]);

    let result = client.set_utc_offset_minutes(15);

    assert!(matches!(
        result,
        Err(VastError {
            error_type: VastErrorType::InvalidInput,
            ..
        })
    ));
    assert!(state.lock().unwrap().sent_commands.is_empty());
}

#[test]
fn onstep_client_is_send() {
    assert_send::<OnStepClient>();
}
