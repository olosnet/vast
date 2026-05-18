use serialport::{SerialPortInfo, SerialPortType};

/// Rileva le porte seriali disponibili sul sistema corrente.
///
/// Implementazione multipiattaforma basata su `serialport::available_ports()`,
/// che supporta Linux, macOS e Windows.
pub fn detect_serial_ports() -> serialport::Result<Vec<SerialPortInfo>> {
    serialport::available_ports()
}

/// Rileva porte seriali e bluetooth usabili su Linux/Windows.
///
/// - Linux: include porte enumerabili (`ttyUSB`, `ttyACM`, `rfcomm`) e tipi riconosciuti
///   dalla libreria. Le porte legacy/platform (`ttyS`, `ttyAMA`, `ttyTHS`) vengono escluse
///   perché possono esistere anche senza un device collegato.
/// - Windows: include porte `COM*` e tipi seriali/bluetooth riconosciuti.
pub fn detect_serial_and_bluetooth_ports() -> serialport::Result<Vec<SerialPortInfo>> {
    let ports = serialport::available_ports()?;

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        Ok(ports
            .into_iter()
            .filter(is_serial_or_bluetooth_port)
            .collect())
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Ok(ports)
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn is_serial_or_bluetooth_port(port: &SerialPortInfo) -> bool {
    match port.port_type {
        SerialPortType::BluetoothPort => true,
        SerialPortType::UsbPort(_) | SerialPortType::PciPort => true,
        SerialPortType::Unknown => {
            #[cfg(target_os = "linux")]
            {
                let name = port.port_name.as_str();
                return name.starts_with("/dev/ttyUSB")
                    || name.starts_with("/dev/ttyACM")
                    || name.starts_with("/dev/rfcomm");
            }

            #[cfg(target_os = "windows")]
            {
                return port.port_name.to_ascii_uppercase().starts_with("COM");
            }

            #[allow(unreachable_code)]
            false
        }
    }
}
