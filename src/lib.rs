use byteorder::{ReadBytesExt, WriteBytesExt, BE};
use std::error::Error;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// AMF version used by the Sol.
pub trait AmfVer {
    /// The id used to identify this AMF version in the Sol.
    const ID: u8;
    /// Type used to represent values of this AMF version.
    type Value;
}

#[derive(Debug)]
pub enum Amf0Value {
    Num(f64),
    Bool(bool),
    String(String),
}

pub type Amf3Value = amf::Amf3Value;

/// AMF version 0
#[derive(Debug)]
pub enum Amf0 {}

/// AMF version 3
pub enum Amf3 {}

impl AmfVer for Amf0 {
    const ID: u8 = 0;
    type Value = Amf0Value;
}

impl AmfVer for Amf3 {
    const ID: u8 = 3;
    type Value = Amf3Value;
}

pub struct Sol<Ver: AmfVer> {
    pub len: u32,
    pub root_name: String,
    /// A list of key-value pairs. The values are all of the same AMF version.
    /// There is no .sol file that has mixed AMF0 and AMF0.
    /// Instead, the AMF version is stated upfront in a special field in the .sol file.
    pub amf: Vec<Pair<Ver::Value>>,
}

impl<Ver: AmfVer> Sol<Ver> {
    pub fn new(root_name: String, amf: Vec<Pair<Ver::Value>>) -> Self {
        Self {
            len: 0,
            root_name,
            amf,
        }
    }
}

impl<Ver: AmfVer> Sol<Ver>
where
    Self: AmfWrite,
{
    pub fn write<W: Write + Seek>(&self, mut w: W) -> Result<(), Box<dyn Error>> {
        w.write_all(&BF_MAGIC)?;
        let len_pos = w.seek(SeekFrom::Current(0))?;
        w.write_u32::<BE>(0)?;
        w.write_all(&TCSO_MAGIC)?;
        w.write_all(&TAIL_MAGIC)?;
        w.write_u16::<BE>(self.root_name.len() as u16)?;
        w.write_all(&self.root_name.as_bytes())?;
        // Assume they are all zeroed, frick it.
        for _ in 0..3 {
            w.write_u8(0)?;
        }
        w.write_u8(Ver::ID)?;
        let (mut w, len) = self.write_amf(w)?;
        w.seek(SeekFrom::Start(len_pos))?;
        w.write_u32::<BE>(len as u32)?;
        Ok(())
    }
    pub fn write_to_file(&self, filename: &Path) -> Result<(), Box<dyn Error>> {
        let f = std::fs::File::create(filename)?;
        self.write(f)
    }
}

pub trait AmfWrite {
    fn write_amf<W: Write + Seek>(&self, w: W) -> Result<(W, u64), Box<dyn Error>>;
}

impl AmfWrite for Sol<Amf3> {
    fn write_amf<W: Write + Seek>(&self, w: W) -> Result<(W, u64), Box<dyn Error>> {
        let mut encoder = amf::amf3::Encoder::new(w);
        for Pair { key, value } in &self.amf {
            encoder.encode_utf8(key)?;
            encoder.encode(value)?;
            encoder.inner().write_u8(0).unwrap();
        }
        let end_pos = encoder.inner().seek(SeekFrom::Current(0))?;
        Ok((encoder.into_inner(), end_pos - 6))
    }
}

impl<Ver: AmfVer> std::fmt::Debug for Sol<Ver>
where
    Ver::Value: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Sol, version {}, len: {}, root name: {}, amf: {:#?}",
            Ver::ID,
            self.len,
            self.root_name,
            self.amf
        )
    }
}

pub type Pair<T> = amf::Pair<String, T>;

pub enum SolVariant {
    Amf0(Sol<Amf0>),
    Amf3(Sol<Amf3>),
}

impl SolVariant {
    pub fn root_name(&self) -> &str {
        match self {
            Self::Amf0(sol) => &sol.root_name,
            Self::Amf3(sol) => &sol.root_name,
        }
    }
}

pub fn read_from_file(path: &Path) -> Result<SolVariant, Box<dyn Error>> {
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
    assert_eq!(blob[0], 0);
    assert_eq!(blob[1], 0);
    assert_eq!(blob[2], 0);
    cursor.read_exact(&mut blob).unwrap();
    let amf_ver = match amf_ver_spec(blob) {
        Some(ver) => ver,
        None => panic!("Unknown AMF version"),
    };
    match amf_ver {
        AmfVerSpec::Amf0 => Ok(SolVariant::Amf0(Sol {
            len,
            root_name,
            amf: read_amf0(cursor, len as u64)?,
        })),
        AmfVerSpec::Amf3 => Ok(SolVariant::Amf3(Sol {
            len,
            root_name,
            amf: read_amf3(cursor, len as u64)?,
        })),
    }
}

const BF_MAGIC: [u8; 2] = [0x00, 0xBF];
const TCSO_MAGIC: [u8; 4] = *b"TCSO";
const TAIL_MAGIC: [u8; 6] = [0x00, 0x04, 0x00, 0x00, 0x00, 0x00];

enum AmfVerSpec {
    Amf0,
    Amf3,
}

fn amf_ver_spec(blob: [u8; 4]) -> Option<AmfVerSpec> {
    match blob[3] {
        Amf0::ID => Some(AmfVerSpec::Amf0),
        Amf3::ID => Some(AmfVerSpec::Amf3),
        _ => None,
    }
}

fn read_amf0(
    mut cursor: std::io::Cursor<Vec<u8>>,
    len: u64,
) -> Result<Vec<Pair<Amf0Value>>, Box<dyn Error>> {
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
        kvpairs.push(Pair { key, value });
        let _padding = cursor.read_u8().unwrap();
    }
}

fn read_amf3(
    cursor: std::io::Cursor<Vec<u8>>,
    len: u64,
) -> Result<Vec<Pair<amf::Amf3Value>>, Box<dyn Error>> {
    let mut kvpairs = Vec::new();
    let mut decoder = amf::amf3::Decoder::new(cursor);
    loop {
        if decoder.inner().position() - 6 == len {
            return Ok(kvpairs);
        }
        let key = decoder.decode_utf8().unwrap();
        let value = decoder.decode().unwrap();
        let _padding = decoder.inner().read_u8().unwrap();
        kvpairs.push(Pair { key, value });
    }
}
