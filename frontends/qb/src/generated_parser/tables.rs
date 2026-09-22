//! Build-time QBasic 1.1 parser tables and typed AST-action identities.

include!(concat!(env!("OUT_DIR"), "/qbasic_parser_tables.rs"));

pub fn token_id(name: &str) -> Option<u16> {
    TOKENS
        .iter()
        .find_map(|(candidate, _spelling, id, _flags)| (*candidate == name).then_some(*id))
}

pub fn token_spelling(id: u16) -> Option<&'static str> {
    TOKENS
        .get(usize::from(id))
        .map(|(_name, spelling, _id, _flags)| *spelling)
}
