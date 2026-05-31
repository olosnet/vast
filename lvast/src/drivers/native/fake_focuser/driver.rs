use crate::{
    base::{
        connections::Connection,
        errors::{VastError, VastErrorType, VastResult},
    },
    types::common::TemperatureUnit,
};

const MAX_POSITION: u32 = 200_000;
const DEFAULT_TEMPERATURE_C: f64 = 12.5;

fn connection_error(message: impl Into<String>) -> VastError {
    VastError::new(VastErrorType::ConnectionError, message.into())
}

fn invalid_input(message: impl Into<String>) -> VastError {
    VastError::new(VastErrorType::InvalidInput, message.into())
}

pub struct FakeFocuser {
    connection: Option<Box<dyn Connection>>,
    position: u32,
    temperature_celsius: f64,
    temperature_unit: TemperatureUnit,
}

impl FakeFocuser {
    pub fn new() -> Self {
        Self {
            connection: None,
            position: 10_000,
            temperature_celsius: DEFAULT_TEMPERATURE_C,
            temperature_unit: TemperatureUnit::Celsius,
        }
    }

    fn ensure_connected(&mut self) -> VastResult<()> {
        let connected = self
            .connection
            .as_mut()
            .map(|connection| connection.is_connected())
            .unwrap_or(false);
        if connected {
            Ok(())
        } else {
            Err(connection_error("Fake focuser is not connected"))
        }
    }

    pub fn set_temperature_celsius(&mut self, temperature_celsius: f64) {
        self.temperature_celsius = temperature_celsius;
    }

    fn convert_temperature(&self) -> f64 {
        match self.temperature_unit {
            TemperatureUnit::Celsius => self.temperature_celsius,
            TemperatureUnit::Fahrenheit => self.temperature_celsius * 9.0 / 5.0 + 32.0,
            TemperatureUnit::Kelvin => self.temperature_celsius + 273.15,
        }
    }
}

impl Default for FakeFocuser {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeFocuser {
    pub(crate) fn connect_inner(&mut self, connection: Box<dyn Connection>) -> VastResult<()> {
        self.connection = Some(connection);
        Ok(())
    }

    pub(crate) fn current_position_inner(&self) -> VastResult<u32> {
        self.connection
            .as_ref()
            .ok_or_else(|| connection_error("Fake focuser is not connected"))?;
        Ok(self.position)
    }

    pub(crate) fn move_to_inner(&mut self, position: u32) -> VastResult<()> {
        self.ensure_connected()?;
        if position > MAX_POSITION {
            return Err(invalid_input(format!(
                "Fake focuser position {position} exceeds max {MAX_POSITION}"
            )));
        }
        self.position = position;
        Ok(())
    }

    pub(crate) fn current_temperature_inner(&self) -> VastResult<f64> {
        self.connection
            .as_ref()
            .ok_or_else(|| connection_error("Fake focuser is not connected"))?;
        Ok(self.convert_temperature())
    }

    pub(crate) fn temperature_supported_inner(&self) -> bool {
        true
    }

    pub(crate) fn current_temperature_unit_inner(&self) -> TemperatureUnit {
        match self.temperature_unit {
            TemperatureUnit::Celsius => TemperatureUnit::Celsius,
            TemperatureUnit::Fahrenheit => TemperatureUnit::Fahrenheit,
            TemperatureUnit::Kelvin => TemperatureUnit::Kelvin,
        }
    }

    pub(crate) fn set_temperature_unit_inner(&mut self, unit: TemperatureUnit) -> VastResult<()> {
        self.temperature_unit = unit;
        Ok(())
    }

    pub(crate) fn disconnect_inner(&mut self) -> VastResult<()> {
        if let Some(connection) = self.connection.as_mut() {
            connection.disconnect();
        }
        self.connection = None;
        Ok(())
    }
}
