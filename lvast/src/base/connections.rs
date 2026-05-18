use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::thread::sleep;
use std::time::Duration;

use crate::base::errors::{VastError, VastErrorType, VastResult};

#[derive(Debug)]
pub struct ConnectionParams {
    pub host: String,
    pub port: u16,
    pub baud: u32,
}

pub trait Connection {
    fn new(params: &ConnectionParams) -> VastResult<Self>
    where
        Self: Sized;
    fn send(&mut self, command: &str) -> VastResult<()>;
    fn receive(&mut self) -> VastResult<String>;
    fn disconnect(&mut self);

    fn is_connected(&mut self) -> bool;
}

pub struct SerialConnection {
    connection: Box<dyn serialport::SerialPort>,
    connected: bool,
}

impl Connection for SerialConnection {
    fn new(params: &ConnectionParams) -> VastResult<Self> {
        let connection = serialport::new(&params.host, params.baud)
            .timeout(Duration::from_millis(10))
            .open()
            .map_err(|err| {
                VastError::new(
                    VastErrorType::SerialConnectionRefused,
                    format!("Failed to open serial port '{}': {}", params.host, err),
                )
            })?;

        Ok(SerialConnection {
            connection,
            connected: true,
        })
    }

    fn send(&mut self, command: &str) -> VastResult<()> {
        if !self.connected {
            return Err(VastError::new(
                VastErrorType::SerialGenericError,
                "Not connected".to_string(),
            ));
        }

        self.connection
            .write_all(command.as_bytes())
            .map_err(|err| {
                // self.connected = false;
                VastError::new(
                    VastErrorType::SerialWriteError,
                    format!("Serial write failed: {}", err),
                )
            })?;

        Ok(())
    }

    fn receive(&mut self) -> VastResult<String> {
        let mut buffer: Vec<u8> = vec![0; 1024]; // buffer di lettura
        let mut bytes_read = 0;

        // Quando va in errore di timeout può aver già letto dei dati
        // di conseguenza non è un errore vero e proprio
        match self.connection.read(&mut buffer) {
            Ok(n) => {
                bytes_read += n;
            }
            Err(err) if std::io::ErrorKind::TimedOut == err.kind() => {}
            Err(err) => {
                return Err(VastError::new(
                    VastErrorType::SerialReadError,
                    format!("Serial read failed: {}", err),
                ));
            }
        }

        if bytes_read == 0 {
            return Err(VastError::new(
                VastErrorType::SerialReadError,
                format!("Read 0 bytes!"),
            ));
        }

        // Tentativo di decodificare i dati in una stringa
        match String::from_utf8(buffer[..bytes_read].to_vec()) {
            Ok(decoded_string) => Ok(decoded_string),
            Err(_) => Err(VastError::new(
                VastErrorType::SerialReadError,
                format!("Invalid UTF-8 data!"),
            )),
        }
    }

    fn is_connected(&mut self) -> bool {
        self.connected
    }

    fn disconnect(&mut self) {
        self.connected = false;
    }
}

pub struct TcpConnection {
    connection: TcpStream,
    connected: bool,
    conn_string: String,
    max_reconn_retries: u8,
}

impl TcpConnection {
    fn connect(conn_string: &str) -> VastResult<TcpStream> {
        let mut addrs = conn_string.to_socket_addrs().map_err(|err| {
            VastError::new(
                VastErrorType::InvalidInput,
                format!("Invalid TCP address: {}", err),
            )
        })?;

        let addr = addrs.next().ok_or_else(|| {
            VastError::new(
                VastErrorType::InvalidInput,
                format!("Invalid TCP address: {}", conn_string),
            )
        })?;

        let connection =
            TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|err| {
                VastError::new(
                    VastErrorType::TcpConnectionRefused,
                    format!(
                        "Failed to connect to TCP endpoint '{}': {}",
                        conn_string, err
                    ),
                )
            })?;
        let sock_ref = socket2::SockRef::from(&connection);

        // Imposta il timeout per la lettura/scrittura della connessione
        connection
            .set_read_timeout(Some(Duration::from_secs(1)))
            .map_err(|err| {
                VastError::new(
                    VastErrorType::TcpGenericError,
                    format!("Failed to set TCP read timeout: {}", err),
                )
            })?;
        connection.set_write_timeout(None).map_err(|err| {
            VastError::new(
                VastErrorType::TcpGenericError,
                format!("Failed to set TCP write timeout: {}", err),
            )
        })?;

        let mut ka = socket2::TcpKeepalive::new();
        ka = ka.with_time(Duration::from_secs(3));
        ka = ka.with_interval(Duration::from_secs(3));
        sock_ref.set_tcp_keepalive(&ka).map_err(|err| {
            VastError::new(
                VastErrorType::TcpGenericError,
                format!("Failed to set TCP keepalive: {}", err),
            )
        })?;

        Ok(connection)
    }
}

impl Connection for TcpConnection {
    fn new(params: &ConnectionParams) -> VastResult<Self> {
        let conn_string = format!("{}:{}", params.host, params.port);
        let connection = TcpConnection::connect(&conn_string)?;

        Ok(TcpConnection {
            connection,
            connected: true,
            conn_string,
            max_reconn_retries: 3,
        })
    }

    fn send(&mut self, command: &str) -> VastResult<()> {
        if !self.connected {
            return Err(VastError::new(
                VastErrorType::TcpGenericError,
                "Not connected".to_string(),
            ));
        }

        let bytes = command.as_bytes(); // Converte la stringa in un array di byte

        if self.connection.write_all(bytes).is_ok() {
            sleep(Duration::from_millis(30));
            return Ok(());
        }

        log::warn!("TCP write error. Trying to reconnect...");

        for attempt in 1..=self.max_reconn_retries {
            match TcpConnection::connect(&self.conn_string) {
                Ok(new_conn) => {
                    self.connection = new_conn;
                    self.connected = true;
                    log::info!(
                        "TCP reconnection ok (attempt {}/{})",
                        attempt,
                        self.max_reconn_retries
                    );

                    if let Err(err) = self.connection.write_all(bytes) {
                        log::warn!("TCP write after reconnect failed: {}", err);
                        continue;
                    }

                    sleep(Duration::from_millis(30));
                    return Ok(());
                }
                Err(err) => {
                    log::warn!(
                        "TCP reconnection attempt {}/{} failed: {}",
                        attempt,
                        self.max_reconn_retries,
                        err
                    );
                }
            }
        }

        self.connected = false;
        Err(VastError::new(
            VastErrorType::TcpWriteError,
            "TCP write failed after reconnection retries".to_string(),
        ))
    }

    // Funzione per leggere i dati dalla connessione TCP
    fn receive(&mut self) -> VastResult<String> {
        if !self.connected {
            return Err(VastError::new(
                VastErrorType::TcpGenericError,
                "Not connected".to_string(),
            ));
        }

        let mut buffer: Vec<u8> = vec![0; 1024]; // buffer di lettura
        let bytes_read = self.connection.read(&mut buffer).map_err(|err| {
            VastError::new(
                VastErrorType::TcpReadError,
                format!("TCP read failed: {}", err),
            )
        })?;

        if bytes_read == 0 {
            self.connected = false;
            return Err(VastError::new(
                VastErrorType::TcpReadError,
                "Read 0 bytes".to_string(),
            ));
        }

        // Tentativo di decodificare i dati in una stringa UTF-8
        match String::from_utf8(buffer[..bytes_read].to_vec()) {
            Ok(decoded_string) => Ok(decoded_string),
            Err(_) => Err(VastError::new(
                VastErrorType::TcpReadError,
                "Invalid UTF-8 data".to_string(),
            )),
        }
    }

    fn is_connected(&mut self) -> bool {
        self.connected
    }

    fn disconnect(&mut self) {
        self.connected = false;
    }
}
