use byteorder::{ReadBytesExt, BE};
use std::error::Error;
use std::io::Read;
use std::path::Path;

#[derive(Debug)]
pub struct Sol<ValueType> {
    pub len: u32,
    pub root_name: String,
    pub amf: Vec<(String, ValueType)>,
}

#[derive(Debug)]
pub enum Amf0Value {
    Num(f64),
    Bool(bool),
    String(String),
}

pub type Amf3Value = amf::Amf3Value;

#[derive(Debug)]
pub enum SolReadResult {
    Amf0(Sol<Amf0Value>),
    Amf3(Sol<amf::Amf3Value>),
}

impl SolReadResult {
    pub fn root_name(&self) -> &str {
        match self {
            Self::Amf0(sol) => &sol.root_name,
            Self::Amf3(sol) => &sol.root_name,
        }
    }
}

pub fn read_from_file(path: &Path) -> Result<SolReadResult, Box<dyn Error>> {
    let data = std::fs::read(path).unwrap();
    let mut cursor = std::io::Cursor::new(data);
    let mut magic = [0; 2];
    cursor.read_exact(&mut magic).unwrap();
    if magic != BF_MAGIC {
        panic!("Unsupported format: {:X?}", magic);
    }
    let len = cursor.read_u32::<BE>().unwrap();
    let mut type_ = [0; 4];
    cursor.read_exact(&mut type_).unwrap();
    assert!(type_ == TCSO_MAGIC);
    let mut tail = [0; 6];
    cursor.read_exact(&mut tail).unwrap();
    assert!(tail == TAIL_MAGIC);
    let root_name_len = cursor.read_u16::<BE>().unwrap();
    let mut root_name = vec![0; root_name_len as usize];
    cursor.read_exact(&mut root_name).unwrap();
    let root_name = std::str::from_utf8(&root_name).unwrap().to_owned();
    let mut blob = [0; 4];
    cursor.read_exact(&mut blob).unwrap();
    let amf_ver = match amf_ver_spec(blob) {
        Some(ver) => ver,
        None => panic!("Unknown AMF version"),
    };
    match amf_ver {
        AmfVer::Amf0 => Ok(SolReadResult::Amf0(Sol {
            len,
            root_name,
            amf: read_amf0(cursor, len as u64)?,
        })),
        AmfVer::Amf3 => Ok(SolReadResult::Amf3(Sol {
            len,
            root_name,
            amf: read_amf3(cursor, len as u64)?,
        })),
    }
}

const BF_MAGIC: [u8; 2] = [0x00, 0xBF];
const TCSO_MAGIC: [u8; 4] = *b"TCSO";
const TAIL_MAGIC: [u8; 6] = [0x00, 0x04, 0x00, 0x00, 0x00, 0x00];
enum AmfVer {
    Amf0,
    Amf3,
}

fn amf_ver_spec(blob: [u8; 4]) -> Option<AmfVer> {
    match blob[3] {
        0 => Some(AmfVer::Amf0),
        3 => Some(AmfVer::Amf3),
        _ => None,
    }
}

fn read_amf0(
    mut cursor: std::io::Cursor<Vec<u8>>,
    len: u64,
) -> Result<Vec<(String, Amf0Value)>, Box<dyn Error>> {
    let mut kvpairs = Vec::new();
    loop {
        if cursor.position() - 6 == len {
            return Ok(kvpairs);
        }
        let key_len = cursor.read_u16::<BE>().unwrap();
        let mut key = vec![0; key_len as usize];
        cursor.read_exact(&mut key).unwrap();
        let key = std::str::from_utf8(&key).unwrap().to_owned();
        let type_ = cursor.read_u8().unwrap();
        let value = match type_ {
            0 => {
                let num = cursor.read_f64::<BE>().unwrap();
                Amf0Value::Num(num)
            }
            1 => {
                let bool_marker = cursor.read_u8().unwrap();
                Amf0Value::Bool(bool_marker != 0)
            }
            2 => {
                let len = cursor.read_u16::<BE>().unwrap();
                let mut buf = vec![0; len as usize];
                cursor.read_exact(&mut buf).unwrap();
                Amf0Value::String(std::str::from_utf8(&buf).unwrap().to_owned())
            }
            _ => panic!("Unexpected type: {:02X}", type_),
        };
        kvpairs.push((key, value));
        let _padding = cursor.read_u8().unwrap();
    }
}

fn read_amf3(
    cursor: std::io::Cursor<Vec<u8>>,
    len: u64,
) -> Result<Vec<(String, amf::Amf3Value)>, Box<dyn Error>> {
    let mut kvpairs = Vec::new();
    let mut decoder = amf::amf3::Decoder::new(cursor);
    loop {
        if decoder.inner().position() - 6 == len {
            return Ok(kvpairs);
        }
        let key = decoder.decode_utf8().unwrap();
        let value = decoder.decode().unwrap();
        let _padding = decoder.inner().read_u8().unwrap();
        kvpairs.push((key, value));
    }
}
