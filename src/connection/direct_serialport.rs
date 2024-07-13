use crate::connection::MavConnection;
use crate::error::MessageReadError;
use crate::{read_versioned_msg, write_versioned_msg, MavHeader, MavlinkVersion, Message};
use serialport::SerialPort;
use std::io;
use std::sync::Mutex;

pub struct SerialConnection {
    read_port: Mutex<Box<dyn SerialPort>>,
    write_port: Mutex<Box<dyn SerialPort>>,
    protocol_version: MavlinkVersion,
    sequence: Mutex<u8>,
}

pub fn open(settings: &str) -> io::Result<SerialConnection> {
    let settings_toks: Vec<&str> = settings.split(':').collect();
    if settings_toks.len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            "Incomplete port settings",
        ));
    }

    let baud_rate = match settings_toks[1].parse() {
        Ok(baud_rate) => baud_rate,
        Err(_e) => {
            return Err(io::Error::other("Invalid baud rate"));
        }
    };

    let port_name = settings_toks[0];
    let mut read_port = serialport::new(port_name, baud_rate).open()?;
    read_port.set_timeout(std::time::Duration::from_secs(60 * 60 * 24 * 365 * 100))?;
    let write_port = read_port.try_clone()?;

    Ok(SerialConnection {
        read_port: Mutex::new(read_port),
        write_port: Mutex::new(write_port),
        protocol_version: MavlinkVersion::V2,
        sequence: Mutex::new(0),
    })
}

impl<M: Message> MavConnection<M> for SerialConnection {
    fn recv(&self) -> Result<(MavHeader, M), MessageReadError> {
        let mut read_port = self.read_port.lock().unwrap();

        loop {
            match read_versioned_msg(&mut *read_port, self.protocol_version) {
                ok @ Ok(..) => {
                    return ok;
                }
                Err(MessageReadError::Io(e)) => {
                    if e.kind() == io::ErrorKind::UnexpectedEof {
                        return Err(MessageReadError::Io(e));
                    }
                }
                Err(MessageReadError::Parse(_)) => {}
            }
        }
    }
    fn send(&self, header: &MavHeader, data: &M) -> Result<usize, crate::error::MessageWriteError> {
        let mut write_port = self.write_port.lock().unwrap();
        let mut sequence = self.sequence.lock().unwrap();

        let header = MavHeader {
            sequence: *sequence,
            system_id: header.system_id,
            component_id: header.component_id,
        };

        *sequence = sequence.wrapping_add(1);

        write_versioned_msg(&mut *write_port, self.protocol_version, header, data)
    }
    fn set_protocol_version(&mut self, version: MavlinkVersion) {
        self.protocol_version = version;
    }
    fn get_protocol_version(&self) -> MavlinkVersion {
        self.protocol_version
    }
}
