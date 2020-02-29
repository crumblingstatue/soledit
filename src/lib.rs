use byteorder::{ReadBytesExt, BE};
use custom_debug::CustomDebug;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::io::Read;
use std::path::Path;

#[derive(Debug)]
pub struct Sol<ValueType> {
    header: SolHeader,
    root_name: String,
    amf: HashMap<String, ValueType>,
}

#[derive(Debug)]
pub enum Amf0Value {
    Num(f64),
    Bool(bool),
    String(String),
}

#[derive(Debug)]
pub enum SolReadResult {
    Amf0(Sol<Amf0Value>),
    Amf3(Sol<amf::Amf3Value>),
}

pub fn read_from_file(path: &Path) -> Result<SolReadResult, Box<dyn Error>> {
    let data = std::fs::read(path).unwrap();
    let mut cursor = std::io::Cursor::new(data);
    let header = SolHeader {
        magic: {
            let mut buf = [0; 2];
            cursor.read_exact(&mut buf).unwrap();
            buf
        },
        len: cursor.read_u32::<BE>().unwrap(),
        type_: {
            let mut type_ = [0; 4];
            cursor.read_exact(&mut type_).unwrap();
            type_
        },
        tail: {
            let mut tail = [0; 6];
            cursor.read_exact(&mut tail).unwrap();
            tail
        },
    };
    if header.magic != BF_MAGIC {
        panic!("Unsupported format: {:X?}", header.magic);
    }
    assert!(header.type_ == *b"TCSO");
    assert!(header.tail == [0x00, 0x04, 0x00, 0x00, 0x00, 0x00]);
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
    let len = header.len as u64;
    match amf_ver {
        AmfVer::Amf0 => Ok(SolReadResult::Amf0(Sol {
            header,
            root_name,
            amf: read_amf0(cursor, len)?,
        })),
        AmfVer::Amf3 => Ok(SolReadResult::Amf3(Sol {
            header,
            root_name,
            amf: read_amf3(cursor, len)?,
        })),
    }
}

const BF_MAGIC: [u8; 2] = [0x00, 0xBF];

#[derive(CustomDebug)]
struct SolHeader {
    #[debug(format = "{:02X?}")]
    magic: [u8; 2],
    len: u32,
    #[debug(with = "utf8_dump")]
    type_: [u8; 4],
    #[debug(format = "{:02X?}")]
    tail: [u8; 6],
}

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

fn utf8_dump<T: AsRef<[u8]>>(buf: &T, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?}", String::from_utf8_lossy(buf.as_ref()))
}

fn read_amf0(
    mut cursor: std::io::Cursor<Vec<u8>>,
    len: u64,
) -> Result<HashMap<String, Amf0Value>, Box<dyn Error>> {
    let mut map = HashMap::new();
    loop {
        if cursor.position() - 6 == len {
            return Ok(map);
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
        map.insert(key, value);
        let _padding = cursor.read_u8().unwrap();
    }
}

fn read_amf3(
    cursor: std::io::Cursor<Vec<u8>>,
    len: u64,
) -> Result<HashMap<String, amf::Amf3Value>, Box<dyn Error>> {
    let mut map = HashMap::new();
    let mut decoder = amf::amf3::Decoder::new(cursor);
    loop {
        if decoder.inner().position() - 6 == len {
            return Ok(map);
        }
        let key = decoder.decode_utf8().unwrap();
        let value = decoder.decode().unwrap();
        let _padding = decoder.inner().read_u8().unwrap();
        map.insert(key, value);
    }
}
