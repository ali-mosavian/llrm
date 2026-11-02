use llrm_mir::interpret::{Val, run};

const TEXT: &str = "declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)\n\
    define i16 @f(i16 %n) {\n  %a = alloca [8 x i8]\n  %b = alloca [8 x i8]\n\
    %p0 = getelementptr inbounds i8, ptr %a, i16 0\n  store i8 11, ptr %p0\n\
    %p1 = getelementptr inbounds i8, ptr %a, i16 1\n  store i8 22, ptr %p1\n\
    %p2 = getelementptr inbounds i8, ptr %a, i16 2\n  store i8 33, ptr %p2\n\
    %q0 = getelementptr inbounds i8, ptr %b, i16 0\n  store i8 1, ptr %q0\n\
    %q2 = getelementptr inbounds i8, ptr %b, i16 2\n  store i8 3, ptr %q2\n\
    call void @llvm.memcpy.p0.p0.i16(ptr %b, ptr %a, i16 %n, i1 false)\n\
    %x = load i8, ptr %q0\n  %y = load i8, ptr %q2\n  %xw = zext i8 %x to i16\n  %yw = zext i8 %y to i16\n\
    %r = shl i16 %xw, 8\n  %s = or i16 %r, %yw\n  ret i16 %s\n}\n";

fn answer(
    module: &llrm_mir::module::Module,
    n: u128,
) -> Val {
    run(module, "f", vec![Val::Int { bits: n, width: 16 }], 10_000).unwrap_or_else(|trap| panic!("{trap:?}"))
}

fn calls_in(module: &llrm_mir::module::Module) -> usize {
    llrm_mir::print::module(module).matches("call void @llvm.memcpy").count()
}

/// The loop copies the bytes the intrinsic copies and leaves the rest alone:
/// byte 2 is 33 once the length reaches it and 3 before, and a length of zero
/// runs the loop no time (which the do-while shape would, wrapping the count).
#[test]
fn the_loop_copies_what_the_intrinsic_copies() {
    let before = llrm_mir::parse::module(TEXT).expect("parses");
    let mut after = before.clone();
    crate::memloops::expanded(&mut after, &|_| true).expect("expands");
    assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{}", llrm_mir::print::module(&after));
    assert_eq!(calls_in(&after), 0, "the call is gone");
    for n in [0, 1, 2, 3, 8] {
        assert_eq!(answer(&after, n), answer(&before, n), "length {n}");
    }
}

/// Only the functions asked for are expanded, and a volatile copy is none of
/// them.
#[test]
fn a_function_not_asked_for_and_a_volatile_copy_keep_their_call() {
    let before = llrm_mir::parse::module(TEXT).expect("parses");
    assert_eq!(crate::memloops::candidates(&before), vec!["f".to_owned()]);
    let mut kept = before.clone();
    crate::memloops::expanded(&mut kept, &|_| false).expect("expands");
    assert_eq!(calls_in(&kept), 1);
    let mut volatile = llrm_mir::parse::module(&TEXT.replace("i16 %n, i1 false", "i16 %n, i1 true")).expect("parses");
    assert!(crate::memloops::candidates(&volatile).is_empty());
    crate::memloops::expanded(&mut volatile, &|_| true).expect("expands");
    assert_eq!(calls_in(&volatile), 1);
}

/// A copy of a constant length is the selector's (moves up to its limit): no
/// candidate, so a function with one is not priced as a loop.
#[test]
fn a_copy_of_a_constant_length_is_no_candidate() {
    let constant = llrm_mir::parse::module(&TEXT.replace("i16 %n, i1 false", "i16 4, i1 false")).expect("parses");
    assert!(crate::memloops::candidates(&constant).is_empty());
    let mut after = constant.clone();
    crate::memloops::expanded(&mut after, &|_| true).expect("expands");
    assert_eq!(calls_in(&after), 1);
}
