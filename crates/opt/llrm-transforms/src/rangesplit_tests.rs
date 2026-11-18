use llrm_mir::interpret::{Val, run};

/// `c >= 'a' && c <= 'z'` joined as `(c - 97) u< 26`, with `c` read again in
/// the taken arm (the shape of `toupper`).
const JOINED: &str = "define i8 @f(i8 %c) {
entry:
  %s = add i8 %c, -97
  %t = icmp ult i8 %s, 26
  br i1 %t, label %in, label %out
in:
  %u = add i8 %c, -32
  br label %out
out:
  %r = phi i8 [ %u, %in ], [ %c, %entry ]
  ret i8 %r
}
";

fn answer(
    module: &llrm_mir::module::Module,
    c: u128,
) -> Val {
    run(module, "f", vec![Val::Int { bits: c, width: 8 }], 1_000).unwrap_or_else(|trap| panic!("{trap:?}"))
}

/// Two compares answer as the joined one for every byte, and the join is gone.
#[test]
fn two_compares_answer_as_the_joined_range_test() {
    let before = llrm_mir::parse::module(JOINED).expect("parses");
    let mut after = before.clone();
    assert_eq!(crate::rangesplit::unmerged(&mut after).expect("rewrites"), vec!["f".to_owned()]);
    assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{}", llrm_mir::print::module(&after));
    let printed = llrm_mir::print::module(&after);
    assert!(printed.contains("icmp uge i8 %c, 97") && printed.contains("icmp ult i8 %c, 123"), "{printed}");
    for c in 0..=255 {
        assert_eq!(answer(&after, c), answer(&before, c), "byte {c}");
    }
}

/// A test whose value is read nowhere else keeps the one compare: nothing is
/// copied, so nothing is gained.
#[test]
fn a_range_test_that_reads_nothing_else_is_left_joined() {
    let text = "define i8 @g(i8 %c) {
entry:
  %s = add i8 %c, -97
  %t = icmp ult i8 %s, 26
  %r = zext i1 %t to i8
  ret i8 %r
}
";
    let mut module = llrm_mir::parse::module(text).expect("parses");
    assert_eq!(crate::rangesplit::unmerged(&mut module).expect("runs"), Vec::<String>::new());
}
