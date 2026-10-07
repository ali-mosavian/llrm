//! `cvshape FILE.OBJ`: what CodeView says of an OMF object.
fn main() {
    for path in std::env::args().skip(1) {
        let records = llrm_omf::omf::parse(&std::fs::read(&path).expect("reads")).expect("parses");
        for line in llrm_omf::cvinfo::parse(&records).shape() {
            println!("{line}");
        }
    }
}
