//! An IR `unreachable` emits nothing in the pin; the machine block simply has
//! no successors. Here a block with no terminator falls into the next one, so
//! basic.c's `if (...) err("SYNTAX"); tbl[i]();` made the `err` block fall into
//! the call block: a pointer defined there looked live-in at the entry and the
//! allocator panicked (`hoistspill.rs` "the walk ends at the root's dominator",
//! `spliteditor.rs` "MBB must be dominated by the def") at -O1 to -Oz.

use llrm_6502::alloc::build_with_places;
use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::select::function_items_with_places;

const IR: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8\"
@tbl = internal constant [8 x ptr] [ptr @a, ptr @a, ptr @a, ptr @a, ptr @a, ptr @a, ptr @a, ptr null]
declare void @a()
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
    let module = llrm_mir::parse::module(IR).expect("parses");
    let selection = select_function(&module, "f", Cpu::Mos6502).expect("selects");
    let (items, _, places) = function_items_with_places(&module, "f", &selection).expect("items");
    let (function, _) = build_with_places("f", &items, places).expect("builds");
    let successors = llrm_spike_mf::regalloc::successors(&function);
    let ends = successors.iter().filter(|to| to.is_empty()).count();
    // `f` never returns: the `err` block is the only block that ends it.
    assert_eq!(ends, 1, "{successors:?}");
}
