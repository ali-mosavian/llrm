use llrm_mir::interpret::{Val, run};

/// A buffer of 600 bytes filled with its own index, copied `n` bytes to a
/// second, whose last byte copied is the answer. The copy steps two pointers;
/// the lengths below cross the page 0, 1 and 2 boundaries.
const TEXT: &str = "define i16 @f(i16 %n) {
entry:
  %a = alloca [600 x i8]
  %b = alloca [600 x i8]
  br label %fill
fill:
  %i = phi i16 [ 0, %entry ], [ %i2, %fill ]
  %p = getelementptr inbounds i8, ptr %a, i16 %i
  %v = trunc i16 %i to i8
  store i8 %v, ptr %p
  %i2 = add i16 %i, 1
  %c = icmp ult i16 %i2, 600
  br i1 %c, label %fill, label %copy
copy:
  %sp = phi ptr [ %a, %fill ], [ %sp2, %copy ]
  %dp = phi ptr [ %b, %fill ], [ %dp2, %copy ]
  %k = phi i16 [ %n, %fill ], [ %k2, %copy ]
  %x = load i8, ptr %sp
  store i8 %x, ptr %dp
  %sp2 = getelementptr inbounds i8, ptr %sp, i16 1
  %dp2 = getelementptr inbounds i8, ptr %dp, i16 1
  %k2 = sub i16 %k, 1
  %d = icmp ne i16 %k2, 0
  br i1 %d, label %copy, label %done
done:
  %last = getelementptr inbounds i8, ptr %dp, i16 0
  %y = load i8, ptr %last
  %w = zext i8 %y to i16
  %r = add i16 %w, %n
  ret i16 %r
}
";

fn answer(
    module: &llrm_mir::module::Module,
    n: u128,
) -> Val {
    run(module, "f", vec![Val::Int { bits: n, width: 16 }], 100_000).unwrap_or_else(|trap| panic!("{trap:?}"))
}

/// The copy runs as it did across the page boundaries: the index wraps at 256
/// and the bases move on by 256, so the last byte is the same for every length.
#[test]
fn the_walk_by_page_copies_what_the_pointer_walk_copies() {
    let before = llrm_mir::parse::module(TEXT).expect("parses");
    let mut after = before.clone();
    let changed = crate::pageindex::indexed(&mut after, 8).expect("rewrites");
    assert_eq!(changed, vec!["f".to_owned()]);
    assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{}", llrm_mir::print::module(&after));
    let printed = llrm_mir::print::module(&after);
    assert!(printed.contains("phi i8"), "no byte index:\n{printed}");
    assert!(!printed.contains("%sp2"), "the pointer still steps:\n{printed}");
    for n in [1, 2, 255, 256, 257, 300, 511, 512, 513, 599] {
        assert_eq!(answer(&after, n), answer(&before, n), "length {n}");
    }
}

/// A pointer compared with another is read as a value, not only as an address:
/// the loop keeps it.
#[test]
fn a_pointer_that_is_compared_keeps_its_step() {
    let text = "define i8 @g(ptr %s, ptr %e) {
entry:
  br label %loop
loop:
  %p = phi ptr [ %s, %entry ], [ %q, %loop ]
  %x = load i8, ptr %p
  %q = getelementptr inbounds i8, ptr %p, i16 1
  %c = icmp ne ptr %q, %e
  br i1 %c, label %loop, label %done
done:
  ret i8 %x
}
";
    let mut module = llrm_mir::parse::module(text).expect("parses");
    assert_eq!(crate::pageindex::indexed(&mut module, 8).expect("runs"), Vec::<String>::new());
}

/// A loop rotated to read the next byte before it tests (the shape of a
/// `strcmp`) reads the pointer's next value in the latch. The step moved after
/// that read left the pointer unconverted and the loop with its two steps; or
/// read the next byte through the old base after the index wrapped, and the
/// answer differed from the pointer walk's past the first page.
#[test]
fn a_loop_that_reads_the_next_byte_ahead_walks_by_page() {
    let text = "define i16 @f(i16 %n) {
entry:
  %a = alloca [600 x i8]
  %b = alloca [600 x i8]
  br label %fill
fill:
  %i = phi i16 [ 0, %entry ], [ %i2, %fill ]
  %p = getelementptr inbounds i8, ptr %a, i16 %i
  %v = trunc i16 %i to i8
  %o = or i8 %v, 1
  store i8 %o, ptr %p
  %i2 = add i16 %i, 1
  %c = icmp ult i16 %i2, 600
  br i1 %c, label %fill, label %start
start:
  %first = load i8, ptr %a
  br label %copy
copy:
  %sp = phi ptr [ %a, %start ], [ %sq, %copy ]
  %dp = phi ptr [ %b, %start ], [ %dq, %copy ]
  %k = phi i16 [ %n, %start ], [ %k2, %copy ]
  %cur = phi i8 [ %first, %start ], [ %nx, %copy ]
  store i8 %cur, ptr %dp
  %sq = getelementptr inbounds i8, ptr %sp, i16 1
  %nx = load i8, ptr %sq
  %dq = getelementptr inbounds i8, ptr %dp, i16 1
  %k2 = sub i16 %k, 1
  %d = icmp ne i16 %k2, 0
  br i1 %d, label %copy, label %done
done:
  %y = load i8, ptr %dp
  %w = zext i8 %y to i16
  %z = zext i8 %nx to i16
  %r = add i16 %w, %z
  ret i16 %r
}
";
    let before = llrm_mir::parse::module(text).expect("parses");
    let mut after = before.clone();
    let changed = crate::pageindex::indexed(&mut after, 8).expect("rewrites");
    assert_eq!(changed, vec!["f".to_owned()]);
    let printed = llrm_mir::print::module(&after);
    assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{printed}");
    assert!(!printed.contains("%sq ="), "the pointer still steps:\n{printed}");
    for n in [1, 2, 255, 256, 257, 300, 511, 512, 513, 598] {
        assert_eq!(answer(&after, n), answer(&before, n), "length {n}");
    }
}

/// The index is as wide as the target says (`Machine::index_register_bits`),
/// not a byte: a 4-bit index wraps every 16 bytes and the bases move by 16.
/// With a byte hard-coded the pass made an `i8` index whatever the target
/// asked.
#[test]
fn the_page_is_as_wide_as_the_targets_index() {
    let before = llrm_mir::parse::module(TEXT).expect("parses");
    let mut after = before.clone();
    assert_eq!(crate::pageindex::indexed(&mut after, 4).expect("rewrites"), vec!["f".to_owned()]);
    let printed = llrm_mir::print::module(&after);
    assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{printed}");
    assert!(printed.contains("phi i4"), "no 4-bit index:\n{printed}");
    for n in [1, 15, 16, 17, 31, 32, 33, 100] {
        assert_eq!(answer(&after, n), answer(&before, n), "length {n}");
    }
}
