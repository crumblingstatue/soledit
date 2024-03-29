use byteorder::{ReadBytesExt, WriteBytesExt, BE};
use std::error::Error;
use std::fmt::Display;
use std::io::{self, Read, Seek, SeekFrom, Write};
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
    Object(Object<Amf0>),
}

impl Amf0Value {
    const NUM: u8 = 0;
    const BOOL: u8 = 1;
    const STRING: u8 = 2;
    const OBJECT: u8 = 3;
    pub fn type_(&self) -> u8 {
        match self {
            Self::Num(_) => Self::NUM,
            Self::Bool(_) => Self::BOOL,
            Self::String(_) => Self::STRING,
            Self::Object(_) => Self::OBJECT,
        }
    }
}

impl Display for Amf0Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Amf0Value::Num(n) => write!(f, "{}", n),
            Amf0Value::Bool(b) => write!(f, "{}", b),
            Amf0Value::String(s) => write!(f, "{}", s),
            Amf0Value::Object(obj) => write!(f, "{}", obj.display()),
        }
    }
}

impl<T: AsRef<[Pair<Amf0Value>]>> Amf0Obj for T {
    fn as_pairs(&self) -> &[Pair<Amf0Value>] {
        self.as_ref()
    }
}

pub trait Amf0Obj {
    fn as_pairs(&self) -> &[Pair<Amf0Value>];
    fn display(&self) -> Amf0ObjDisplay {
        Amf0ObjDisplay(self.as_pairs())
    }
}

pub struct Amf0ObjDisplay<'a>(&'a [Pair<Amf0Value>]);

impl<'a> Display for Amf0ObjDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{{")?;
        for pair in self.0 {
            writeln!(f, "\t{} => {}", pair.key, pair.value)?;
        }
        writeln!(f, "}}")?;
        Ok(())
    }
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

type Object<Ver> = Vec<Pair<<Ver as AmfVer>::Value>>;

pub struct Sol<Ver: AmfVer> {
    pub len: u32,
    pub root_name: String,
    /// A list of key-value pairs. The values are all of the same AMF version.
    /// There is no .sol file that has mixed AMF0 and AMF3.
    /// Instead, the AMF version is stated upfront in a special field in the .sol file.
    pub root_object: Object<Ver>,
}

impl<Ver: AmfVer> Sol<Ver> {
    pub fn new(root_name: String, amf: Vec<Pair<Ver::Value>>) -> Self {
        Self {
            len: 0,
            root_name,
            root_object: amf,
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
        w.write_all(self.root_name.as_bytes())?;
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
        for Pair { key, value } in &self.root_object {
            encoder.encode_utf8(key)?;
            encoder.encode(value)?;
            encoder.inner_mut().write_u8(0).unwrap();
        }
        let end_pos = encoder.inner_mut().stream_position()?;
        Ok((encoder.into_inner(), end_pos - 6))
    }
}

impl AmfWrite for Sol<Amf0> {
    fn write_amf<W: Write + Seek>(&self, mut w: W) -> Result<(W, u64), Box<dyn Error>> {
        for pair in &self.root_object {
            write_key_and_type(pair, &mut w)?;
            write_value(&pair.value, &mut w)?;
            w.write_u8(0)?;
        }
        let end_pos = w.stream_position()?;
        Ok((w, end_pos - 6))
    }
}

fn write_key_and_type(pair: &Pair<Amf0Value>, w: &mut impl Write) -> io::Result<()> {
    w.write_u16::<BE>(pair.key.len() as u16)?;
    w.write_all(pair.key.as_bytes())?;
    w.write_u8(pair.value.type_())
}

fn write_value(value: &Amf0Value, w: &mut impl Write) -> io::Result<()> {
    match value {
        Amf0Value::Num(n) => w.write_f64::<BE>(*n)?,
        Amf0Value::Bool(b) => w.write_u8(if *b { 1 } else { 0 })?,
        Amf0Value::String(s) => {
            w.write_u16::<BE>(s.len() as u16)?;
            w.write_all(s.as_bytes())?;
        }
        Amf0Value::Object(_o) => todo!(),
    }
    Ok(())
    /*let value = match type_ {
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
        3 => {
            let mut kvpairs = Vec::new();
            loop {
                let (key, type_) = read_key_and_type(cursor);
                if type_ == 9 {
                    return Amf0Value::Object(kvpairs);
                }
                let value = read_value(type_, cursor);
                kvpairs.push(Pair { key, value });
            }
        }
        _ => panic!("Unexpected type: {:02X}", type_),
    };
    value*/
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
            self.root_object
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
    pub fn write_to_file(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        match self {
            Self::Amf0(sol) => sol.write_to_file(path),
            Self::Amf3(sol) => sol.write_to_file(path),
        }
    }
}

pub fn read_from_file(path: &Path) -> Result<SolVariant, Box<dyn Error>> {
    let data = std::fs::read(path).unwrap();
    let mut cursor = std::io::Cursor::new(data);
    let mut magic = [0; 2];
    cursor.read_exact(&mut magic).unwrap();
    assert!(magic == BF_MAGIC, "Unsupported format: {:X?}", magic);
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
            root_object: read_amf0(cursor, len as u64)?,
        })),
        AmfVerSpec::Amf3 => Ok(SolVariant::Amf3(Sol {
            len,
            root_name,
            root_object: read_amf3(cursor, len as u64)?,
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
        let (key, type_) = read_key_and_type(&mut cursor);
        let value = read_value(type_, &mut cursor);
        kvpairs.push(Pair { key, value });
        let _padding = cursor.read_u8().unwrap();
    }
}

fn read_value(type_: u8, cursor: &mut std::io::Cursor<Vec<u8>>) -> Amf0Value {
    let value = match type_ {
        Amf0Value::NUM => {
            let num = cursor.read_f64::<BE>().unwrap();
            Amf0Value::Num(num)
        }
        Amf0Value::BOOL => {
            let bool_marker = cursor.read_u8().unwrap();
            Amf0Value::Bool(bool_marker != 0)
        }
        Amf0Value::STRING => {
            let len = cursor.read_u16::<BE>().unwrap();
            let mut buf = vec![0; len as usize];
            cursor.read_exact(&mut buf).unwrap();
            Amf0Value::String(std::str::from_utf8(&buf).unwrap().to_owned())
        }
        Amf0Value::OBJECT => {
            let mut kvpairs = Vec::new();
            loop {
                let (key, type_) = read_key_and_type(cursor);
                if type_ == 9 {
                    return Amf0Value::Object(kvpairs);
                }
                let value = read_value(type_, cursor);
                kvpairs.push(Pair { key, value });
            }
        }
        _ => panic!("Unexpected type: {:02X}", type_),
    };
    value
}

fn read_key_and_type(cursor: &mut std::io::Cursor<Vec<u8>>) -> (String, u8) {
    let key_len = cursor.read_u16::<BE>().unwrap();
    let mut key = vec![0; key_len as usize];
    cursor.read_exact(&mut key).unwrap();
    let key = std::str::from_utf8(&key).unwrap().to_owned();
    let type_ = cursor.read_u8().unwrap();
    (key, type_)
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
        let _padding = decoder.inner_mut().read_u8().unwrap();
        kvpairs.push(Pair { key, value });
    }
}
