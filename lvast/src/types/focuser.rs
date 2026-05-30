use crate::{
    base::{connections::Connection, errors::VastResult},
    types::common::TemperatureUnit,
};

pub trait VastFocuser {
    fn new() -> Self;

    fn connect(&mut self, connection: Box<dyn Connection>) -> VastResult<()>;
    fn current_position(&self) -> VastResult<u32>;
    fn move_to(&mut self, position: u32) -> VastResult<()>;
    fn current_temperature(&self) -> VastResult<f64>;
    fn temperature_supported(&self) -> bool;
    fn current_temperature_unit(&self) -> TemperatureUnit;
    fn set_temperature_unit(&mut self, unit: TemperatureUnit) -> VastResult<()>;

    fn disconnect(&mut self) -> VastResult<()>;
}
