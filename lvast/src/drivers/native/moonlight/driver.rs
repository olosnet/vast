use crate::base::connections::Connection;
use crate::base::errors::{VastError, VastErrorType, VastResult};
use crate::base::workers::{ConnectionWorker, ReceiveOptions};
use crate::types::common::TemperatureUnit;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::Duration;

const UNIT_CELSIUS: u8 = 0;
const UNIT_FAHRENHEIT: u8 = 1;
const UNIT_KELVIN: u8 = 2;

fn connection_worker_error(message: impl Into<String>) -> VastError {
    VastError::new(VastErrorType::ConnectionError, message.into())
}

/// Native MoonLite/Moonlight focuser client.
///
/// Transport I/O is serialized through dedicated worker thread so command/response ordering stays
/// correct even when shared across threads.
pub struct MoonlightFocuser {
    state: Mutex<Option<ConnectionWorker>>,
    temperature_unit: AtomicU8,
}

impl MoonlightFocuser {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(None),
            temperature_unit: AtomicU8::new(UNIT_CELSIUS),
        }
    }

    fn invalid_input(message: impl Into<String>) -> VastError {
        VastError::new(VastErrorType::InvalidInput, message.into())
    }

    fn with_worker<T>(
        &self,
        f: impl FnOnce(&ConnectionWorker) -> VastResult<T>,
    ) -> VastResult<T> {
        self.state
            .lock()
            .map_err(|_| connection_worker_error("Moonlight state lock poisoned"))?
            .as_ref()
            .ok_or_else(|| connection_worker_error("Moonlight focuser is not connected"))
            .and_then(f)
    }

    fn send(&self, command: &str) -> VastResult<()> {
        self.with_worker(|worker| worker.send(command))
    }

    fn send_receive(&self, command: &str, trim_hash: bool) -> VastResult<String> {
        self.with_worker(|worker| {
            worker.send_receive_with_options(
                command,
                ReceiveOptions {
                    delay: Duration::from_millis(0),
                    trim_suffix: trim_hash.then_some('#'),
                },
            )
        })
    }

    fn read_version_from_worker(worker: &ConnectionWorker) -> VastResult<String> {
        let response = worker.send_receive(":GV#")?;
        let trimmed = response.trim();
        if trimmed.len() < 2 {
            return Err(connection_worker_error(format!(
                "Invalid Moonlight firmware version response '{}'",
                response
            )));
        }

        Ok(format!("{}.{}", &trimmed[0..1], &trimmed[1..2]))
    }

    fn parse_hex_u32(value: &str, label: &str) -> VastResult<u32> {
        u32::from_str_radix(value.trim(), 16).map_err(|err| {
            Self::invalid_input(format!("Invalid Moonlight {label} value '{value}': {err}"))
        })
    }

    fn parse_hex_i16(value: &str, label: &str) -> VastResult<i16> {
        let raw = u16::from_str_radix(value.trim(), 16).map_err(|err| {
            Self::invalid_input(format!("Invalid Moonlight {label} value '{value}': {err}"))
        })?;
        Ok(raw as i16)
    }

    fn unit_from_raw(raw: u8) -> TemperatureUnit {
        match raw {
            UNIT_FAHRENHEIT => TemperatureUnit::Fahrenheit,
            UNIT_KELVIN => TemperatureUnit::Kelvin,
            _ => TemperatureUnit::Celsius,
        }
    }

    fn convert_temperature_from_celsius(&self, temperature_celsius: f64) -> f64 {
        match Self::unit_from_raw(self.temperature_unit.load(Ordering::Relaxed)) {
            TemperatureUnit::Celsius => temperature_celsius,
            TemperatureUnit::Fahrenheit => temperature_celsius * 9.0 / 5.0 + 32.0,
            TemperatureUnit::Kelvin => temperature_celsius + 273.15,
        }
    }

    pub fn read_version(&self) -> VastResult<String> {
        self.with_worker(Self::read_version_from_worker)
    }

    pub fn is_moving(&self) -> VastResult<bool> {
        let response = self.send_receive(":GI#", true)?;
        match response.trim() {
            "0" | "00" => Ok(false),
            "1" | "01" => Ok(true),
            value => Err(Self::invalid_input(format!(
                "Invalid Moonlight motion state '{value}'"
            ))),
        }
    }
}

impl Default for MoonlightFocuser {
    fn default() -> Self {
        Self::new()
    }
}

impl MoonlightFocuser {
    pub(crate) fn connect_inner(&mut self, connection: Box<dyn Connection>) -> VastResult<()> {
        let _ = self.disconnect_inner();

        let mut worker = ConnectionWorker::new("Moonlight", connection);

        let mut last_error = None;
        let mut version = None;
        for _ in 0..3 {
            match Self::read_version_from_worker(&worker) {
                Ok(found_version) => {
                    version = Some(found_version);
                    break;
                }
                Err(err) => {
                    last_error = Some(err);
                    #[cfg(not(test))]
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }

        let Some(version) = version else {
            worker.shutdown();
            return Err(last_error.unwrap_or_else(|| {
                connection_worker_error("Moonlight focuser handshake failed")
            }));
        };

        log::info!("Moonlight focuser online. Firmware version {}", version);

        let mut state = self
            .state
            .lock()
            .map_err(|_| connection_worker_error("Moonlight state lock poisoned"))?;
        *state = Some(worker);
        Ok(())
    }

    pub(crate) fn current_position_inner(&self) -> VastResult<u32> {
        let response = self.send_receive(":GP#", true)?;
        Self::parse_hex_u32(&response, "position")
    }

    pub(crate) fn move_to_inner(&mut self, position: u32) -> VastResult<()> {
        if position > 0xFFFF {
            return Err(Self::invalid_input(format!(
                "Moonlight position {} exceeds controller range 0xFFFF",
                position
            )));
        }

        self.send(&format!(":SN{position:04X}#"))?;
        self.send(":FG#")
    }

    pub(crate) fn current_temperature_inner(&self) -> VastResult<f64> {
        self.send(":C#")?;
        let response = self.send_receive(":GT#", true)?;
        let temperature_celsius = f64::from(Self::parse_hex_i16(&response, "temperature")?) / 2.0;
        Ok(self.convert_temperature_from_celsius(temperature_celsius))
    }

    pub(crate) fn temperature_supported_inner(&self) -> bool {
        true
    }

    pub(crate) fn current_temperature_unit_inner(&self) -> TemperatureUnit {
        Self::unit_from_raw(self.temperature_unit.load(Ordering::Relaxed))
    }

    pub(crate) fn set_temperature_unit_inner(&mut self, unit: TemperatureUnit) -> VastResult<()> {
        let raw = match unit {
            TemperatureUnit::Celsius => UNIT_CELSIUS,
            TemperatureUnit::Fahrenheit => UNIT_FAHRENHEIT,
            TemperatureUnit::Kelvin => UNIT_KELVIN,
        };
        self.temperature_unit.store(raw, Ordering::Relaxed);
        Ok(())
    }

    pub(crate) fn disconnect_inner(&mut self) -> VastResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| connection_worker_error("Moonlight state lock poisoned"))?;

        let Some(mut state) = state.take() else {
            return Ok(());
        };

        state.shutdown();

        Ok(())
    }
}
