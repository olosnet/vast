use crate::{
    base::{
        connections::{Connection, ConnectionParams},
        errors::VastResult,
    },
    focusers::fake::FakeFocuser,
    types::{common::TemperatureUnit, focuser::VastFocuser},
};

struct MockConnection {
    connected: bool,
}

impl Connection for MockConnection {
    fn new(_params: &ConnectionParams) -> VastResult<Self>
    where
        Self: Sized,
    {
        Ok(Self { connected: true })
    }

    fn send(&mut self, _command: &str) -> VastResult<()> {
        Ok(())
    }

    fn receive(&mut self) -> VastResult<String> {
        Ok(String::new())
    }

    fn disconnect(&mut self) {
        self.connected = false;
    }

    fn is_connected(&mut self) -> bool {
        self.connected
    }
}

#[test]
fn fake_focuser_moves_and_reports_temperature_units() {
    let mut focuser = FakeFocuser::new();
    focuser.connect(Box::new(MockConnection { connected: true })).unwrap();
    focuser.set_temperature_celsius(10.0);
    focuser.move_to(42_000).unwrap();
    focuser.set_temperature_unit(TemperatureUnit::Kelvin).unwrap();

    assert_eq!(focuser.current_position().unwrap(), 42_000);
    assert!((focuser.current_temperature().unwrap() - 283.15).abs() < 1e-6);
}

#[test]
fn fake_focuser_rejects_out_of_range_position() {
    let mut focuser = FakeFocuser::new();
    focuser.connect(Box::new(MockConnection { connected: true })).unwrap();

    assert!(focuser.move_to(200_001).is_err());
}
