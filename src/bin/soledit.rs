use std::env;

fn main() {
    let path = env::args_os().nth(1).expect("Need file path as argument");
    let sol = soledit::read_from_file(path.as_ref()).unwrap();
    println!("{:#?}", sol);
}
