//! An IR `unreachable` emits nothing in the pin; the machine block has no
//! successors. Here a block with no terminator fell into the next one, so the
//! allocator saw the `noreturn` call's block reach code that reads its values.

use llrm_m68k::dagisel::function::select_function;
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::mfunc::emit;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
@tbl = external global [8 x ptr]
declare void @err() noreturn
declare i8 @peek()
define void @f() {
entry:
  br label %loop
loop:
  %c = call i8 @peek()
  %i = zext i8 %c to i32
  %g = getelementptr [8 x ptr], ptr @tbl, i32 %i
  %p = load ptr, ptr %g, align 4
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
    let blocks = select_function(&module, "f", Cpu::M68000).expect("selects");
    let function = emit("f", &blocks).expect("emits");
    let successors = llrm_spike_mf::regalloc::successors(&function);
    let ends = successors.iter().filter(|to| to.is_empty()).count();
    // `f` never returns: the `err` block is the only block that ends it.
    assert_eq!(ends, 1, "{successors:?}");
}
