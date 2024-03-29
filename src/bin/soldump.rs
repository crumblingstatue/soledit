use std::env;

use soledit::Amf0Obj;

fn main() {
    let path = env::args_os().nth(1).expect("Need file path as argument");
    let sol = soledit::read_from_file(path.as_ref()).unwrap();
    let mut indent_level = 0;
    print!("{} ", sol.root_name());
    match sol {
        soledit::SolVariant::Amf0(sol) => dump_amf0(&sol.root_object),
        soledit::SolVariant::Amf3(sol) => dump_amf3(&sol.root_object, &mut indent_level, false),
    }
}

macro_rules! printindent {
    ($inline:expr, $level:expr, $($arg:tt)*) => {{
        if !$inline {
            indent($level);
        }
        print!($($arg)*)
    }};
}

fn dump_amf0(amf: &[soledit::Pair<soledit::Amf0Value>]) {
    println!("{}", amf.display());
}

fn dump_amf3(amf: &[soledit::Pair<soledit::Amf3Value>], indent_level: &mut u32, inline: bool) {
    printindent!(inline, *indent_level, "{{\n");
    *indent_level += 1;
    for pair in amf {
        printindent!(false, *indent_level, "{} = ", pair.key);
        dump_amf3_value(&pair.value, indent_level, true);
    }
    *indent_level -= 1;
    printindent!(false, *indent_level, "}}\n");
}

fn indent(level: u32) {
    for _ in 0..level {
        print!("  ");
    }
}

fn dump_amf3_value(value: &soledit::Amf3Value, indent_level: &mut u32, inline: bool) {
    use soledit::Amf3Value as Value;
    match value {
        Value::Object {
            class_name,
            sealed_count: _,
            entries,
        } => {
            if let Some(name) = class_name {
                print!("{} ", name);
            }
            dump_amf3(entries, indent_level, inline);
        }
        Value::String(s) => printindent!(inline, *indent_level, "\"{}\"\n", s),
        Value::Boolean(b) => printindent!(inline, *indent_level, "{}\n", b),
        Value::Integer(n) => printindent!(inline, *indent_level, "{}\n", n),
        Value::Double(n) => printindent!(inline, *indent_level, "{}\n", n),
        Value::Array {
            assoc_entries,
            dense_entries,
        } => {
            println!("[");
            *indent_level += 1;
            if !assoc_entries.is_empty() {
                println!("ASSOC WAS NOT EMPTY WHOAH");
                dump_amf3(assoc_entries, indent_level, inline);
            }
            for v in dense_entries {
                dump_amf3_value(v, indent_level, false);
            }
            *indent_level -= 1;
            printindent!(false, *indent_level, "]\n");
        }
        Value::Null => printindent!(inline, *indent_level, "<null>\n"),
        Value::Date { unix_time } => printindent!(
            inline,
            *indent_level,
            "<date: {}s {}ns>\n",
            unix_time.as_secs(),
            unix_time.subsec_nanos()
        ),
        _ => todo!("Unimplemented item: {:?}", value),
    }
}
