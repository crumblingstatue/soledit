use byteorder::{ReadBytesExt, BE};
use custom_debug::CustomDebug;
use std::env;
use std::fmt;
use std::io::Read;

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

fn utf8_dump<T: AsRef<[u8]>>(buf: &T, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?}", String::from_utf8_lossy(buf.as_ref()))
}

fn main() {
    let path = env::args_os().nth(1).expect("Need file path as argument");
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
    assert!(&header.type_ == b"TCSO");
    let root_name_len = cursor.read_u16::<BE>().unwrap();
    let mut root_name = vec![0; root_name_len as usize];
    cursor.read_exact(&mut root_name).unwrap();
    println!("Root name: {}", String::from_utf8_lossy(&root_name));
    let mut padding = [0; 4];
    cursor.read_exact(&mut padding).unwrap();
    loop {
        if cursor.position() - 6 == header.len as u64 {
            break;
        }
        let key_len = cursor.read_u16::<BE>().unwrap();
        let mut key = vec![0; key_len as usize];
        cursor.read_exact(&mut key).unwrap();
        println!("Found key: {:?}", String::from_utf8_lossy(&key));
        let type_ = cursor.read_u8().unwrap();
        println!("Type: {}", type_);
        match type_ {
            0 => {
                let num = cursor.read_f64::<BE>().unwrap();
                println!("Numeric value: {}", num);
            }
            1 => {
                let bool_marker = cursor.read_u8().unwrap();
                println!("Bool marker: {}", bool_marker);
            }
            2 => {
                let len = cursor.read_u16::<BE>().unwrap();
                let mut buf = vec![0; len as usize];
                cursor.read_exact(&mut buf).unwrap();
                println!("UTF-8 String: {}", String::from_utf8_lossy(&buf));
            }
            _ => panic!("Unexpected type: {:02X}", type_),
        }
        let _padding = cursor.read_u8().unwrap();
    }
}
