use crc::{Crc, CRC_16_IBM_SDLC};
use enumn::N;
use std::time::SystemTime;
pub const CRC: Crc<u16> = Crc::<u16>::new(&CRC_16_IBM_SDLC);

#[derive(Debug, PartialEq, Copy, Clone, N)]
#[repr(u8)]
pub enum Command {
    Empty,
    Enter,
    Text,
    Damaged,
    Retry,
    Exit,
    Error,
}

impl Command {
    pub fn to_code(self) -> u8 {
        self as u8
    }
    pub fn from_code(code: u8) -> Self {
        println!("{}", code);
        Command::n(code).unwrap_or(Command::Error)
    }
}

#[derive(Debug)]
pub struct Message {
    pub id: u32,
    checksum: u16,
    pub command: Command,
    pub data: Vec<u8>,
}
impl Message {
    pub fn new(command: Command, data: Vec<u8>) -> Self {
        let id = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;
        let checksum = CRC.checksum(&data);
        Message {
            id,
            checksum,
            command,
            data,
        }
    }
    pub fn empty() -> Self {
        Message {
            id: 0,
            checksum: 0,
            command: Command::Empty,
            data: [].to_vec(),
        }
    }
    pub fn enter(name: &str) -> Self {
        Message::new(Command::Enter, be_u8_from_str(name))
    }
    pub fn exit() -> Self {
        Message::new(Command::Exit, [].to_vec())
    }
    pub fn text(text: &str) -> Self {
        Message::new(
            Command::Text,
            be_u8_from_str(
                text.to_owned()
                    .chars()
                    .filter(|c| !c.is_control())
                    .collect::<String>()
                    .as_ref(),
            ),
        )
    }
    pub fn from_be_bytes(bytes: &[u8]) -> Option<Self> {
        let id: u32 = u32::from_be_bytes([
            *bytes.get(0)?,
            *bytes.get(1)?,
            *bytes.get(2)?,
            *bytes.get(3)?,
        ]);
        let checksum = u16::from_be_bytes([*bytes.get(4)?, *bytes.get(5)?]);
        let command = Command::from_code(u8::from_be_bytes([*bytes.get(6)?]));
        let data = match bytes.len() {
            0..=7 => [].to_vec(),
            _ => bytes[7..].to_owned(),
        };
        println!("MESSAGE: {:?}\n{:?}\n{:?}", id, command, data);
        if checksum == CRC.checksum(&data) {
            Some(Message {
                id,
                checksum,
                command,
                data,
            })
        } else {
            Some(Message::empty())
        }
    }

    pub fn to_be_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::<u8>::new();
        bytes.extend(self.id.to_be_bytes());
        bytes.extend(self.checksum.to_be_bytes());
        bytes.extend(self.command.to_code().to_be_bytes());
        bytes.extend(self.data.to_owned());

        bytes
    }
    pub fn read_text(&self) -> String {
        string_from_be_u8(&self.data)
    }
}

// fn is_valid_len(bytes: &[u8]) -> bool {
//     bytes.len() >= std::mem::size_of::<Message>()
// }

fn string_from_be_u8(bytes: &[u8]) -> String {
    // std::str::from_utf8(&bytes.iter().map(|b| u8::from_be(*b)).collect::<Vec<u8>>())
    std::str::from_utf8(bytes).unwrap_or("UNKNOWN").to_string()
}

fn be_u8_from_str(text: &str) -> Vec<u8> {
    text.trim().as_bytes().to_owned()
    // .iter()
    // .map(|c| u8::to_be(*c))
    // .collect::<Vec<u8>>()
}
