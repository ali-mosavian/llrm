use qbfront::{compile, parse, Dialect};
fn main() {
    let src = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    let m = parse(&src, Dialect::QuickBasic45).unwrap();
    match compile(&m, "probe", Dialect::QuickBasic45, "qb45") {
        Ok(h) => println!("{h}"),
        Err(e) => println!("ERR {e:?}"),
    }
}
