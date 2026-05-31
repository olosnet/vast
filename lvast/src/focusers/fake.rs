//! Fake focuser wrapper exposing in-memory focuser behavior through focuser module.

use crate::{
    base::{connections::Connection, errors::VastResult},
    drivers::native::fake_focuser::driver::FakeFocuser as NativeFakeFocuser,
    types::{common::TemperatureUnit, focuser::VastFocuser},
};

pub use crate::drivers::native::fake_focuser::driver::FakeFocuser;

impl VastFocuser for NativeFakeFocuser {
    fn new() -> Self {
        Self::new()
    }

    fn connect(&mut self, connection: Box<dyn Connection>) -> VastResult<()> {
        self.connect_inner(connection)
    }

    fn current_position(&self) -> VastResult<u32> {
        self.current_position_inner()
    }

    fn move_to(&mut self, position: u32) -> VastResult<()> {
        self.move_to_inner(position)
    }

    fn current_temperature(&self) -> VastResult<f64> {
        self.current_temperature_inner()
    }

    fn temperature_supported(&self) -> bool {
        self.temperature_supported_inner()
    }

    fn current_temperature_unit(&self) -> TemperatureUnit {
        self.current_temperature_unit_inner()
    }

    fn set_temperature_unit(&mut self, unit: TemperatureUnit) -> VastResult<()> {
        self.set_temperature_unit_inner(unit)
    }

    fn disconnect(&mut self) -> VastResult<()> {
        self.disconnect_inner()
    }
}
