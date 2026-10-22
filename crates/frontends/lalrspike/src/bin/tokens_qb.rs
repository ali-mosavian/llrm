fn main() {
    for arg in std::env::args().skip(1) {
        let id: usize = arg.parse().unwrap();
        println!("{id}: {:?}", qbfront::generated_parser::tables::TOKENS[id]);
    }
}
