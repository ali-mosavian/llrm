//! Parse each given `.nib` file with the LALR spike and the hand-written
//! parser, and report where they disagree.

use llrm_nib::lexer::lex;

fn main() {
    let (mut both, mut neither, mut only_lalr, mut only_hand) = (0, 0, Vec::new(), Vec::new());
    for path in std::env::args().skip(1) {
        let source = std::fs::read_to_string(&path).unwrap();
        let Ok(tokens) = lex(&source) else { continue };
        let hand = llrm_nib::parser::parse(tokens.clone()).is_ok();
        let lalr = lalrspike::parse_nib(&tokens);
        match (hand, &lalr) {
            (true, Ok(_)) => both += 1,
            (false, Err(_)) => neither += 1,
            (true, Err(error)) => only_hand.push(format!("{path}: {error}")),
            (false, Ok(_)) => only_lalr.push(path.clone()),
        }
    }
    println!("accepted by both: {both}, rejected by both: {neither}");
    println!("hand-written only: {}", only_hand.len());
    only_hand.iter().for_each(|line| println!("  {line}"));
    println!("LALR only: {}", only_lalr.len());
    only_lalr.iter().for_each(|line| println!("  {line}"));
}
