//! The scheduler asks for the LLVM opcode of every instruction's form name;
//! the table was a `match` on the string, compared arm by arm (1.66 M `memcmp`
//! calls, 48 M of a 3.5 G compile).

use llrm_arm64::schedmodel::opcode_names;

#[test]
fn looking_up_the_last_form_name_costs_no_more_than_the_first() {
    let all = opcode_names();
    assert!(all.len() > 500, "the premise: a table of {} names", all.len());
    let (first, last) = (all[0], all[all.len() - 1]);
    let time = |name: &str| {
        let start = std::time::Instant::now();
        for _ in 0..20_000 {
            std::hint::black_box(llrm_arm64::schedmodel::opcode_of_name(std::hint::black_box(name)));
        }
        start.elapsed()
    };
    let (a, b) = (time(first), time(last));
    assert!(b < a * 8 + std::time::Duration::from_millis(5), "first {a:?}, last {b:?} for 20,000 lookups: a scan");
}
