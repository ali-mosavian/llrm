//! An IR `unreachable` emits nothing in the pin; the machine block has no
//! successors. Here a block with no terminator falls into the next one, so a
//! value the next block reads was kept live across the `noreturn` call (a
//! `push bc` / `pop bc` around `call _err`).

use llrm_z80::compile::machine_function;
use llrm_z80::driver::{Level, Request};

const IR: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"
@tbl = external global [8 x ptr]
declare void @err() noreturn
declare i8 @peek()
define void @f() {
entry:
  br label %loop
loop:
  %c = call i8 @peek()
  %i = zext i8 %c to i16
  %g = getelementptr [8 x ptr], ptr @tbl, i16 %i
  %p = load ptr, ptr %g, align 2
  %null = icmp eq ptr %p, null
  br i1 %null, label %bad, label %ok
bad:
  call void @err()
  unreachable
ok:
  call void %p()
  br label %loop
}
";

#[test]
fn a_block_that_ends_in_unreachable_has_no_successor() {
    let function = machine_function(IR, "f", &Request::new(Level::O1)).expect("compiles");
    let successors = llrm_spike_mf::regalloc::successors(&function);
    let ends = successors.iter().filter(|to| to.is_empty()).count();
    // `f` never returns: the `err` block is the only block that ends it.
    assert_eq!(ends, 1, "{successors:?}");
}
