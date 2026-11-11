//! A word returned in `a` and `x` was joined into them for its whole range, and
//! a reload of a char that the `cmp` needs in `a`, `x` or `y` found no register
//! while the word held two of them: "term: greedy could not allocate", the
//! function refused (basic.c at -Os and -Oz). The pin joins only reserved
//! registers; the allocation is made again without the pinned joins.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

const IR: &str = r#"@.str = private constant [3 x i8] c"EE\00"

declare zeroext i8 @peek()
declare void @err(ptr)
declare ptr @var_ref()

define i16 @expr() {
  %a = call i16 @arith()
  br label %loop
loop:
  br label %loop
}

define i16 @arith() {
  br label %loop
loop:
  %t = call i16 @term()
  br label %loop
}

define i16 @term() {
  %w = alloca i16, align 2
  %c = alloca i8, align 1
  br label %top
top:
  %p = call zeroext i8 @peek()
  store i8 %p, ptr %c, align 1
  %f = call i16 @factor()
  store i16 %f, ptr %w, align 2
  %v = load i8, ptr %c, align 1
  %z = zext i8 %v to i16
  %m = icmp eq i16 %z, 42
  br i1 %m, label %top, label %rest
rest:
  %x = load i16, ptr %w, align 2
  %nz = icmp ne i16 %x, 0
  br i1 %nz, label %top, label %bad
bad:
  call void @err(ptr @.str)
  br label %top
}

define i16 @factor() {
  %r = alloca i16, align 2
  %c = alloca i8, align 1
  %p = call zeroext i8 @peek()
  store i8 %p, ptr %c, align 1
  %v = load i8, ptr %c, align 1
  %z = zext i8 %v to i16
  %m = icmp eq i16 %z, 40
  br i1 %m, label %paren, label %var
paren:
  %e = call i16 @expr()
  br label %done
var:
  %q = call ptr @var_ref()
  %l = load i16, ptr %q, align 2
  store i16 %l, ptr %r, align 2
  br label %done
done:
  %o = load i16, ptr %r, align 2
  ret i16 %o
}
"#;

#[test]
fn a_char_compared_while_a_word_is_live_is_allocated() {
    let ir = format!("target datalayout = \"{}\"\n{IR}", datalayout());
    for level in [Level::Os, Level::Oz] {
        let out = assembly(&ir, &Request::new(level)).unwrap_or_else(|why| panic!("{level:?}: {why}"));
        assert!(out.contains("term:"), "{out}");
    }
}
