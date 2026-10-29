//! Far and huge pointers at m16: selectors, segments, ES, phis of far pointers.

use super::dag_function::selected;
use super::dag_function_tests::{FAR_CALL, select_with_contracts};
use crate::model::ir::Operation;
use crate::testing::m16_ir;

const LOOP: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %n) {
entry:
  br label %test
test:
  %s = phi i32 [ 0, %entry ], [ %s1, %body ]
  %i = phi i32 [ 0, %entry ], [ %i1, %body ]
  %more = icmp slt i32 %i, %n
  br i1 %more, label %body, label %done
body:
  %s1 = add i32 %s, %i
  %i1 = add i32 %i, 1
  br label %test
done:
  ret i32 %s
}
";

/// A counted loop is four LIR blocks: the header holds one phi per loop
/// variable with an input from the entry and one from the body, the header
/// branches to the body and falls into `done`, the body jumps back. A phi
/// with one input, or a branch to a block that is not there, would be wrong
/// code.
#[test]
fn a_loop_is_blocks_with_phis_and_branches() {
    let module = llrm_mir::parse::module(LOOP).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let blocks = &done.body.blocks;
    assert_eq!(blocks.len(), 4, "{blocks:#?}");
    let addresses: Vec<i64> = blocks.iter().map(|b| b.at).collect();
    let header = &blocks[1];
    assert_eq!(header.phis.len(), 2, "{header:#?}");
    for phi in header.phis.iter() {
        let mut from: Vec<i64> = phi.incoming.iter().map(|(at, _)| *at).collect();
        from.sort();
        assert_eq!(from, [addresses[0], addresses[2]], "entry and body feed the phi");
    }
    for block in blocks.iter() {
        assert!(block.succ.iter().all(|at| addresses.contains(at)), "{block:#?}");
    }
    // The builder swaps a branch whose true edge is the next block: it jumps to
    // `done` and falls into the body.
    assert_eq!(header.succ.iter().copied().collect::<Vec<_>>(), [addresses[3], addresses[2]]);
    assert_eq!(blocks[2].succ.iter().copied().collect::<Vec<_>>(), [addresses[1]]);
    assert!(blocks[3].succ.is_empty());
}

/// A call of far code is a far call: the call sites are marked in `far`, as
/// the old selector marks them. The DAG path left the set empty, so every
/// call was a near `call` of a far routine and the program crashed at once
/// (99 real-mode programs under `LLRM_ISEL_REAL`).
#[test]
fn a_call_of_far_code_is_marked_far() {
    let module = llrm_mir::parse::module(m16_ir(FAR_CALL)).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let sites: Vec<(i64, String)> = done.calls.iter().map(|(at, name)| (*at, name.clone())).collect();
    assert_eq!(sites.len(), 2, "{sites:?}");
    let far: Vec<&str> = sites.iter().filter(|(at, _)| done.far.contains(at)).map(|(_, name)| name.as_str()).collect();
    assert_eq!(far, ["far"], "{sites:?} far={:?}", done.far);
}

const VARIABLE_MOVE: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
declare void @llvm.memmove.p0.p0.i16(ptr, ptr, i16, i1)
define void @f(ptr %d, ptr %s, i16 %n) {
entry:
  call void @llvm.memmove.p0.p0.i16(ptr %d, ptr %s, i16 %n, i1 false)
  ret void
}
";

/// A call of the runtime's `memmove` at m16 is a far call: its routine is a far
/// procedure in another code segment. It was a near `call`, and the program
/// crashed at once (the memmove_overlap run test under `LLRM_ISEL_REAL`).
#[test]
fn a_runtime_routine_is_called_far_at_m16() {
    let module = llrm_mir::parse::module(m16_ir(VARIABLE_MOVE)).expect("parses");
    let libcalls = [("memmove".to_owned(), 0, "_memmove@3".to_owned())];
    let done =
        selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &libcalls)
            .unwrap_or_else(|e| panic!("{}", e.0));
    let sites: Vec<(&i64, &String)> = done.calls.iter().collect();
    assert_eq!(sites.len(), 1, "{sites:?}");
    assert!(done.far.contains(sites[0].0), "{sites:?} far={:?}", done.far);
}

const M16_FAR_LOOP: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
declare cc 1005 ptr addrspace(1) @g()
declare cc 1005 void @use(ptr addrspace(1))
define cc 1005 i16 @f(i16 %n) addrspace(1) {
entry:
  %p = call cc 1005 ptr addrspace(1) @g()
  br label %loop
loop:
  %q = phi ptr addrspace(1) [ %p, %entry ], [ %q2, %loop ]
  %i = phi i16 [ 0, %entry ], [ %i2, %loop ]
  %q2 = getelementptr i8, ptr addrspace(1) %q, i16 4
  %i2 = add i16 %i, 1
  call cc 1005 void @use(ptr addrspace(1) %q)
  %c = icmp ult i16 %i2, %n
  br i1 %c, label %loop, label %done
done:
  ret i16 %i2
}
";

/// A phi of a two-register value (a far pointer) has an entry for each part;
/// both went to the first register, so the second part was read and never
/// defined and the allocator's liveness failed ("a unique value") in 3 QCport
/// modules once float compares let those functions through.
#[test]
fn every_part_of_a_far_phi_is_defined() {
    let module = llrm_mir::parse::module(M16_FAR_LOOP).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let mut defined: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    for block in done.body.blocks.iter() {
        defined.extend(block.phis.iter().map(|phi| phi.result));
        for insn in block.insns.iter() {
            defined.extend(insn.defines.iter().copied());
        }
    }
    for insn in done.body.insns().iter() {
        for used in &insn.uses {
            assert!(defined.contains(used), "value {used} is read and never defined: {:?}", insn.what);
        }
    }
}

const M16_FAR_LOAD: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
define cc 1005 i16 @f(ptr addrspace(1) %p) addrspace(1) {
entry:
  %v = load i16, ptr addrspace(1) %p
  ret i16 %v
}
";

/// A far pointer loaded from memory is its offset and, beside it, its selector
/// (the two word loads `farload` joins into a `les`); the cell through ES
/// takes that selector. `LES16rm` was 328 of 494 refusals at m16.
#[test]
fn a_far_load_reads_the_pointer_and_goes_through_its_selector() {
    use crate::model::ir::{Loc, Operation, Space};
    let module = llrm_mir::parse::module(M16_FAR_LOAD).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let whats: Vec<_> = insns.iter().filter_map(|i| i.what.as_ref()).collect();
    let words = whats
        .iter()
        .filter(|w| {
            w.op == Operation::Move
                && matches!(
                    &w.sources[..],
                    [Loc::Mem(m)] if m.width == 2 && m.selector.is_none()
                )
        })
        .count();
    assert!(words >= 2, "the offset and the selector are two word loads: {whats:?}");
    assert!(
        whats
            .iter()
            .flat_map(|w| w.sources.iter())
            .any(|l| matches!(l, Loc::Mem(m) if m.selector.is_some() && m.addr.is_some_and(|a| a.space == Space::Far))),
        "the access goes through the selector"
    );
}

const M16_ZERO_16: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
@buf = global [16 x i8] zeroinitializer
declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
define cc 1005 void @f() addrspace(1) {
entry:
  call void @llvm.memset.p0.i16(ptr @buf, i8 0, i16 16, i1 false)
  ret void
}
";

/// `getOptimalMemOpType` is `i32` in every mode: 16 zero bytes at m16 were
/// eight word stores (48 bytes of code), the pin's are four dword stores
/// (rep_count: 486 -> 1046 bytes against the old selector).
#[test]
fn an_m16_zero_fill_of_sixteen_bytes_is_four_dword_stores() {
    let module = llrm_mir::parse::module(M16_ZERO_16).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let stores = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter(|w| w.name.as_deref() == Some("mov") && matches!(w.dests.as_slice(), [crate::model::ir::Loc::Mem(_)]))
        .count();
    assert_eq!(stores, 4);
}

const M16_FILL: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
@buf = global [100 x i8] zeroinitializer
declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
define cc 1005 void @f() addrspace(1) {
entry:
  call void @llvm.memset.p0.i16(ptr @buf, i8 7, i16 100, i1 false)
  ret void
}
";

/// At real mode `rep stosb` names the segment its pointer is in (a value: the
/// group's selector for a global); `REP_STOSB_16` was a refusal in 9 functions.
#[test]
fn an_m16_fill_names_its_segment() {
    use crate::model::ir::Operation;
    let module = llrm_mir::parse::module(M16_FILL).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let fill = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| w.op == Operation::Fill && w.name.as_deref() == Some("stosb"))
        .expect("a stosb");
    assert_eq!(fill.sources.len(), 4, "value, count, destination and its segment: {fill:?}");
}

const FAR_INDIRECT_ARGUMENT: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
define void @f(ptr addrspace(1) %p) {
entry:
  call watcallcc addrspace(1) void %p(i32 7)
  ret void
}
";

/// The argument of a call through a far pointer reaches the call in its
/// register: `CALL_FAR` was not a call to the emitter, so the copy to `eax`
/// was dropped and `table[i](7 + i)` ran with garbage (function_table -O2:
/// want 7, got 478).
#[test]
fn a_call_through_a_far_pointer_receives_its_arguments() {
    let module = llrm_mir::parse::module(m16_ir(FAR_INDIRECT_ARGUMENT)).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let call = insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.op == crate::model::ir::Operation::Call));
    assert!(call.is_some_and(|call| !call.requires.is_empty()), "{insns:?}");
}

const FAR_GLOBAL_ELEMENT: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
@cells = addrspace(1) global [4 x i16] zeroinitializer
define i16 @f() {
entry:
  %p = getelementptr [4 x i16], ptr addrspace(1) @cells, i16 0, i16 3
  %v = load i16, ptr addrspace(1) %p
  ret i16 %v
}
";

/// An element of a far global is a cell through ES whose displacement is the
/// global's symbol and offset. The emitter wanted a number there ("a
/// non-constant address part": far_return, es_across_copy, farstruct,
/// wide_globals at m16).
#[test]
fn an_element_of_a_far_global_is_a_cell_through_es() {
    let module = llrm_mir::parse::module(m16_ir(FAR_GLOBAL_ELEMENT)).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const STRING_OP_ON_ELEMENT: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
@buf = global [64 x i8] zeroinitializer
declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
define void @f(i16 %i, i16 %n) {
entry:
  %p = getelementptr [64 x i8], ptr @buf, i16 0, i16 %i
  call void @llvm.memset.p0.i16(ptr %p, i8 0, i16 %n, i1 false)
  ret void
}
";

/// A string operation on `&buf[i]` (an `lea` of the global and an index) takes
/// the global's segment: it refused "a pointer of no known segment" (idioms,
/// os_more, rep_count at m16).
#[test]
fn a_string_operation_on_an_element_of_a_global_knows_its_segment() {
    let module = llrm_mir::parse::module(m16_ir(STRING_OP_ON_ELEMENT)).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const M16_FAR_AND_DOUBLE: &str =
    "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-f80:16-n8:16:32\"
define double @f(ptr addrspace(1) %p, double %x) addrspace(1) {
entry:
  store i16 1, ptr addrspace(1) %p
  %y = fadd double %x, 1.000000e+00
  ret double %y
}
";

/// A far cell and a double in one function were refused as a pair ('floating
/// values with far cells'); neither refusal is left.
#[test]
fn a_double_not_live_across_a_far_cell_is_selected() {
    let module = llrm_mir::parse::module(M16_FAR_AND_DOUBLE).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const M16_FAR_NULL: &str =
    "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-f80:16-n8:16:32\"
define i16 @f(ptr addrspace(1) %p) addrspace(1) {
entry:
  %c = icmp eq ptr addrspace(1) %p, null
  %r = zext i1 %c to i16
  ret i16 %r
}
";

/// A far null pointer is two zero words. `get_constant` of the far type
/// asserted 'integer constant of Far' and stopped 35 QCport functions.
#[test]
fn a_far_null_is_two_zero_words() {
    let module = llrm_mir::parse::module(M16_FAR_NULL).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const M16_HUGE_MEMSET: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
define cc 1005 void @f(ptr addrspace(3) %p, i32 %n) addrspace(1) {
entry:
  call void @llvm.memset.p3.i32(ptr addrspace(3) %p, i8 1, i32 %n, i1 false)
  ret void
}
declare void @llvm.memset.p3.i32(ptr addrspace(3), i8, i32, i1)
";

fn huge_ir(
    name: &str,
    dst: &str,
    src: &str,
    metadata: &str,
) -> String {
    m16_ir(&format!(
        "target datalayout = \"@m16-datalayout@-ni:2:3\"
define cc 1005 void @f(ptr addrspace({dst}) %d, ptr addrspace({src}) %s, i32 %n) addrspace(1) {{
entry:
  call void @llvm.{name}.p{dst}.p{src}.i32(ptr addrspace({dst}) %d, ptr addrspace({src}) %s, i32 %n, i1 false){metadata}
  ret void
}}
declare void @llvm.{name}.p{dst}.p{src}.i32(ptr addrspace({dst}), ptr addrspace({src}), i32, i1)
"
    ))
    .to_owned()
}

fn select_f(ir: &str) -> Result<crate::backend::isel::Selected, String> {
    let module = llrm_mir::parse::module(ir).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .map_err(|e| e.0)
}

fn mnemonics(done: &crate::backend::isel::Selected) -> Vec<String> {
    done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.as_deref().map(str::to_owned)).collect()
}

/// A fill of `n` bytes through a huge pointer crosses segment ends and `n`
/// outgrows `cx`: a `rep stosb` filled only the first 64K (huge_runs gave a
/// wrong sum). The target's stepped routine is called inline instead, with
/// its operands in the registers it reads (`huge_runs` was the one c program
/// the DAG path refused).
#[test]
fn a_memset_through_a_huge_pointer_calls_the_stepped_routine() {
    let ir = m16_ir(M16_HUGE_MEMSET).to_owned();
    let done = select_f(&ir).unwrap_or_else(|why| panic!("{why}"));
    let routine = llrm_target::Target::huge_fill_routine(&llrm_x86_m16::M16, 1).expect("a fill routine");
    assert!(done.calls.values().any(|name| name == routine.name), "{:?}", done.calls);
    assert!(done.inline.values().any(|code| *code == routine.code), "its bytes are laid down at the call");
    assert!(!mnemonics(&done).iter().any(|m| m.starts_with("rep")), "{:?}", mnemonics(&done));
}

/// `memcpy` between huge pointers, and a `memmove` a pass proved forward or
/// backward, call the copy routine; a `memmove` of no proven direction, or
/// with a near pointer beside the huge one, is refused.
#[test]
fn a_copy_through_huge_pointers_calls_the_stepped_routine() {
    let forward = llrm_target::Target::huge_copy_routine(&llrm_x86_m16::M16, false).expect("a copy routine");
    let done = select_f(&huge_ir("memcpy", "3", "3", "")).unwrap_or_else(|why| panic!("{why}"));
    assert!(done.calls.values().any(|name| name == forward.name), "{:?}", done.calls);
    let refused = select_f(&huge_ir("memmove", "3", "3", "")).err().expect("no direction proved");
    assert!(refused.contains("direction"), "{refused}");
    let refused = select_f(&huge_ir("memcpy", "3", "0", "")).err().expect("a near source");
    assert!(refused.contains("near"), "{refused}");
}

const VARSEG: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
@x = internal global i16 0
define i16 @f() {
entry:
  ret i16 ptrtoint (ptr addrspace(2) addrspacecast (ptr @x to ptr addrspace(2)) to i16)
}
";

/// `m16_ir(VARSEG)(x)` is a near pointer cast to the segment space: the
/// selector of its segment. The DAG path took the offset (`mov b$seg, offset
/// X%`), so `POKE` after `DEF SEG = m16_ir(VARSEG)(x)` wrote 0 bytes from the
/// segment and `far_poke_*` printed garbage.
#[test]
fn a_near_pointer_cast_to_the_segment_space_is_its_segment_selector() {
    let module = llrm_mir::parse::module(m16_ir(VARSEG)).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let spaces: Vec<_> = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.clone())
        .flat_map(|w| w.sources)
        .filter_map(|loc| match loc {
            crate::model::ir::Loc::Imm(imm) => imm.address.map(|a| a.space),
            _ => None,
        })
        .collect();
    assert_eq!(spaces, [crate::model::ir::Space::Group], "{spaces:?}");
}

const FAR_COPY: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
define void @f(ptr %d, ptr addrspace(2) %s, i16 %i, i16 %n) addrspace(1) {
entry:
  %far = addrspacecast ptr addrspace(2) %s to ptr addrspace(4)
  %p = getelementptr i8, ptr addrspace(4) %far, i16 %i
  call void @llvm.memcpy.p0.p4.i16(ptr %d, ptr addrspace(4) %p, i16 %n, i1 false)
  ret void
}
declare void @llvm.memcpy.p0.p4.i16(ptr, ptr addrspace(4), i16, i1)
";

/// A far pointer is `offset | (selector << 16)`; the matcher turns an `or` of
/// disjoint values into an `add` (`ADD32rr_DB`), and a string operation then
/// took the pointer for a near one: ES/DS = DGROUP. c `idiom_edges` copied
/// from the wrong segment under `--isel=dag` (8de7ce51a).
#[test]
fn a_copy_from_a_far_pointer_reads_through_its_selector() {
    use crate::model::ir::{Loc, Operation};
    let module = llrm_mir::parse::module(m16_ir(FAR_COPY)).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
    let insns = done.body.insns();
    let copy = insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.op == Operation::Copy)).expect("a copy");
    let sources = &copy.what.as_ref().expect("semantics").sources;
    let Some(Loc::Held(from)) = sources.get(sources.len() - 2).cloned() else { panic!("{copy:?}") };
    let from_a_constant = insns
        .iter()
        .any(
            |i| i.what.as_ref().is_some_and(|w| {
                w.dests.contains(&Loc::Held(from)) && w.sources.iter().all(|source| matches!(source, Loc::Imm(_)))
            }),
        );
    assert!(!from_a_constant, "DS takes a constant (DGROUP), not the pointer's selector");
}

const SEGMENT_LOADED_IN_TWO_BLOCKS: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
define cc 1000 void @f() addrspace(1) {
b1:
  %0 = load volatile i8, ptr addrspace(1) getelementptr (i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 64 to ptr addrspace(2)) to ptr addrspace(1)), i16 108)
  br label %b3
b3:
  %1 = load volatile i8, ptr addrspace(1) getelementptr (i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 64 to ptr addrspace(2)) to ptr addrspace(1)), i16 108)
  %2 = icmp ne i8 %0, %1
  br i1 %2, label %b4, label %b3
b4:
  ret void
}
";

const M16_FAR_COPY: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
@far = addrspace(1) global [100 x i16] zeroinitializer
@near = global [100 x i16] zeroinitializer
declare void @llvm.memcpy.p0.p1.i16(ptr, ptr addrspace(1), i16, i1)
define void @f(i16 %n) {
entry:
  %bytes = shl i16 %n, 1
  call void @llvm.memcpy.p0.p1.i16(ptr @near, ptr addrspace(1) @far, i16 %bytes, i1 false)
  ret void
}
";

/// A segment constant put in ES in two blocks was one value defined twice
/// ('value N is defined 2 times in a body that should be in SSA': QB's
/// `t = PEEK(&H6C): DO: LOOP UNTIL PEEK(&H6C) <> t`, 59 tests of llrm-qb and
/// llrm-nib under the DAG default).
#[test]
fn a_selector_put_in_es_in_two_blocks_is_two_values() {
    let selected = select_with_contracts(SEGMENT_LOADED_IN_TWO_BLOCKS).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(crate::backend::verify::verify(&selected.body, true), Vec::<String>::new());
}

/// A copy from a far global took DGROUP as the source's segment, because the
/// far pointer was built with `add` and only `or` was recognised:
/// `idiom_edges.c` at m16 summed the near array's old contents.
#[test]
fn a_copy_from_a_far_global_loads_the_globals_segment() {
    let module = llrm_mir::parse::module(M16_FAR_COPY).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let copy = insns.iter().filter_map(|i| i.what.as_ref()).find(|w| w.op == Operation::Copy).expect("a copy");
    let crate::model::ir::Loc::Held(from) = copy.sources[3] else { panic!("{copy:?}") };
    let loaded = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| w.dests.iter().any(|d| matches!(d, crate::model::ir::Loc::Held(h) if h.value == from.value)))
        .expect("the source segment is loaded");
    let crate::model::ir::Loc::Imm(imm) = &loaded.sources[0] else { panic!("{loaded:?}") };
    let group = imm.address.as_ref().map(|address| address.index);
    assert_ne!(group, Some(0), "DGROUP, not the global's own segment: {loaded:?}");
}

const FAR_MEMSET: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
define void @f(ptr addrspace(2) %s, i16 %n) addrspace(1) {
entry:
  %p = addrspacecast ptr addrspace(2) %s to ptr addrspace(4)
  call void @llvm.memset.p4.i16(ptr addrspace(4) %p, i8 0, i16 %n, i1 false)
  ret void
}
declare void @llvm.memset.p4.i16(ptr addrspace(4), i8, i16, i1)
";

/// `rep stosb` stores at ES:DI. A far pointer whose offset folded to zero
/// (`shl edi, 16` of the selector, no `or`) was taken for a near pointer and
/// ES was set to `DGROUP`: qb `sieve.bas` cleared its data segment instead of
/// its array and hung under `--isel=dag`. The selector is the high word.
#[test]
fn a_fill_through_a_far_pointer_stores_through_its_selector() {
    use crate::model::ir::{Loc, Operation};
    let module = llrm_mir::parse::module(m16_ir(FAR_MEMSET)).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
    let insns = done.body.insns();
    let fill = insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.op == Operation::Fill)).expect("a fill");
    let Some(Loc::Held(segment)) = fill.what.as_ref().and_then(|w| w.sources.last().cloned()) else {
        panic!("{fill:?}")
    };
    let from_a_constant = insns
        .iter()
        .any(
            |i| i.what.as_ref().is_some_and(|w| {
                w.dests.contains(&Loc::Held(segment)) && w.sources.iter().all(|source| matches!(source, Loc::Imm(_)))
            }),
        );
    assert!(!from_a_constant, "ES takes a constant (DGROUP), not the pointer's selector");
}

const TWO_FAR_LOADS: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
define cc 1000 i32 @f(i16 %0, i16 %1) addrspace(1) {
b1:
  %2 = inttoptr i16 %0 to ptr addrspace(2)
  %3 = addrspacecast ptr addrspace(2) %2 to ptr addrspace(1)
  %4 = load i16, ptr addrspace(1) %3
  %5 = sext i16 %4 to i32
  %6 = inttoptr i16 %1 to ptr addrspace(2)
  %7 = addrspacecast ptr addrspace(2) %6 to ptr addrspace(1)
  %8 = load i16, ptr addrspace(1) %7
  %9 = sext i16 %8 to i32
  %10 = mul nsw i32 %9, %5
  ret i32 %10
}
";

/// Two loads through ES from two segments read the same one: both
/// `CopyToReg ES` came before both loads, and a load took the last selector
/// set (QB's `CLNG(a(i, k)) * b(k, j)` read `b * b`: matmul printed 54349 for
/// 353712).
#[test]
fn two_loads_through_es_read_their_own_selectors() {
    use crate::model::ir::Loc;
    let module = llrm_mir::parse::module(TWO_FAR_LOADS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
    let selectors: Vec<_> = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .flat_map(|w| w.sources.iter())
        .filter_map(|source| if let Loc::Mem(cell) = source { cell.selector } else { None })
        .collect();
    assert_eq!(selectors.len(), 2, "{selectors:?}");
    assert_ne!(selectors[0], selectors[1], "both loads take one selector");
}

/// A far pointer made from a `long` call result (`(unsigned char far *)
/// qglSfAccessWrEx(..)`), then cleared by a loop: QCport `part.c` stopped at
/// "a string operation on a pointer of no known segment".
const FAR_FROM_A_CALL_RESULT: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
define i16 @f(i32 %sf) addrspace(1) {
b1:
  %r = call addrspace(1) i32 @acc(i32 %sf, i16 0, i16 1)
  %p = inttoptr i32 %r to ptr addrspace(1)
  %n = ptrtoint ptr addrspace(1) %p to i32
  %z = icmp eq i32 %n, 0
  br i1 %z, label %b4, label %b2
b2:
  %q = inttoptr i32 %r to ptr addrspace(1)
  call void @llvm.memset.p1.i16(ptr addrspace(1) align 1 %q, i8 0, i16 16384, i1 false)
  br label %b4
b4:
  %v = phi i16 [ 0, %b1 ], [ 1, %b2 ]
  ret i16 %v
}
declare i32 @acc(i32, i16, i16) addrspace(1)
declare void @llvm.memset.p1.i16(ptr addrspace(1) nocapture writeonly, i8, i16, i1 immarg)
";

#[test]
fn a_far_pointer_from_a_call_result_has_its_selector_for_a_fill() {
    let module = llrm_mir::parse::module(m16_ir(FAR_FROM_A_CALL_RESULT)).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
    // The selector is the high word of the result: `shr r, 16`.
    let insns = done.body.insns();
    let shifts_by_16 = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .any(
            |w| w.name.as_deref() == Some("shr")
                && matches!(
                    w.sources.last(),
                    Some(crate::model::ir::Loc::Imm(imm)) if imm.value == 16
                ),
        );
    assert!(shifts_by_16);
}

/// A run of `drow[px] = color` over the pointer a call returned (QCport
/// `part.c` at -O3, `part_raster` inlined): the fill's address is `offset +
/// (x & 0xFFFF0000)` for the dword `x` the call returned, and it stopped at
/// "a string operation on a pointer of no known segment".
const FILL_INTO_A_CALL_RESULT_IN_A_LOOP: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"

define cc 1005 void @\"_frame@3\"(i32 noundef %0, i32 noundef %1, i16 noundef %2) addrspace(1) \"llrm-fp-eval\"=\"x87-extended\" {
b1:
  %3 = tail call cc 1005 addrspace(1) i32 @\"_acc@3\"(i32 noundef %0, i16 noundef 0, i16 noundef 3)
  %4 = inttoptr i32 %3 to ptr addrspace(1)
  %5 = ptrtoint ptr addrspace(1) %4 to i32
  %6 = icmp eq i32 %5, 0
  br i1 %6, label %b4, label %7

7:
  %8 = icmp sgt i16 %2, 0
  br i1 %8, label %9, label %12

9:
  %10 = inttoptr i32 %3 to ptr addrspace(1)
  %11 = getelementptr i8, ptr addrspace(1) %10, i16 7
  br label %17

12:
  br label %13

13:
  %14 = phi i16 [ %16, %13 ], [ 0, %12 ]
  %15 = tail call cc 1005 addrspace(1) i32 @\"_acc@3\"(i32 noundef %1, i16 noundef %14, i16 noundef 1)
  %16 = add nuw nsw i16 %14, 1
  %exitcond2 = icmp eq i16 %16, 101
  br i1 %exitcond2, label %b4, label %13

17:
  %18 = phi i16 [ %59, %58 ], [ 0, %9 ]
  %19 = tail call cc 1005 addrspace(1) i32 @\"_acc@3\"(i32 noundef %1, i16 noundef %18, i16 noundef 1)
  %20 = inttoptr i32 %19 to ptr addrspace(1)
  %21 = ptrtoint ptr addrspace(1) %20 to i32
  %22 = icmp eq i32 %21, 0
  br i1 %22, label %58, label %23

23:
  br label %24

24:
  %25 = phi ptr addrspace(1) [ %11, %23 ], [ %57, %55 ]
  %26 = phi i16 [ %2, %23 ], [ %56, %55 ]
  %27 = getelementptr i8, ptr addrspace(1) %25, i16 -2
  %28 = load i8, ptr addrspace(1) %27, !tbaa !7
  %29 = zext i8 %28 to i16
  %30 = icmp samesign ult i16 %18, %29
  br i1 %30, label %55, label %31

31:
  %32 = getelementptr i8, ptr addrspace(1) %25, i16 -1
  %33 = load i8, ptr addrspace(1) %32, !tbaa !7
  %34 = zext i8 %33 to i16
  %35 = icmp samesign ugt i16 %18, %34
  br i1 %35, label %55, label %36

36:
  %37 = getelementptr i8, ptr addrspace(1) %25, i16 -7
  %38 = load i16, ptr addrspace(1) %37, !tbaa !9
  %39 = getelementptr i8, ptr addrspace(1) %25, i16 -3
  %40 = load i8, ptr addrspace(1) %39, !tbaa !7
  %41 = zext i8 %40 to i16
  %42 = add i16 %38, %41
  %43 = icmp sgt i16 %38, %42
  br i1 %43, label %55, label %44

44:
  %45 = inttoptr i32 %19 to ptr addrspace(1)
  %46 = load i8, ptr addrspace(1) %25, !tbaa !7
  %47 = getelementptr i8, ptr addrspace(1) %45, i16 %38
  %48 = sext i16 %42 to i32
  %49 = add nsw i32 %48, 1
  %50 = add i16 %38, 1
  %51 = sext i16 %50 to i32
  %52 = tail call i32 @llvm.smax.i32(i32 %49, i32 %51)
  %53 = trunc i32 %52 to i16
  %54 = sub i16 %53, %38
  tail call void @llvm.memset.p1.i16(ptr addrspace(1) align 1 %47, i8 %46, i16 %54, i1 false)
  br label %55

55:
  %56 = add i16 %26, -1
  %57 = getelementptr i8, ptr addrspace(1) %25, i16 8
  %exitcond = icmp eq i16 %56, 0
  br i1 %exitcond, label %58, label %24

58:
  %59 = add nuw nsw i16 %18, 1
  %exitcond1 = icmp eq i16 %59, 101
  br i1 %exitcond1, label %b4, label %17

b4:
  ret void
}

declare cc 1005 i32 @\"_acc@3\"(i32, i16, i16) addrspace(1)
declare i32 @llvm.smax.i32(i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare void @llvm.memset.p1.i16(ptr addrspace(1) nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!\"llrm hir\"}
!1 = !{!\"place\", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!\"allocation\", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!\"Simple C/C++ TBAA\"}
!6 = !{!\"omnipotent char\", !5, i64 0}
!7 = !{!6, !6, i64 0}
!8 = !{!\"int2\", !6, i64 0}
!9 = !{!8, !8, i64 0}
!10 = !{!\"int4\", !6, i64 0}
!11 = !{!10, !10, i64 0}
!12 = !{!\"int8\", !6, i64 0}
!13 = !{!12, !12, i64 0}
!14 = !{!\"float4\", !6, i64 0}
!15 = !{!14, !14, i64 0}
!16 = !{!\"float8\", !6, i64 0}
!17 = !{!16, !16, i64 0}
!18 = !{!\"float10\", !6, i64 0}
!19 = !{!18, !18, i64 0}
!20 = !{!\"pointer2\", !6, i64 0}
!21 = !{!20, !20, i64 0}
!22 = !{!\"pointer4\", !6, i64 0}
!23 = !{!22, !22, i64 0}";

#[test]
fn a_fill_at_an_offset_into_a_call_result_has_the_results_selector() {
    let module = llrm_mir::parse::module(m16_ir(FILL_INTO_A_CALL_RESULT_IN_A_LOOP)).expect("parses");
    let done =
        selected(&module, "_frame@3", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
            .unwrap_or_else(|why| panic!("{}", why.0));
    let shifts_by_16 = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .any(
            |w| w.name.as_deref() == Some("shr")
                && matches!(
                    w.sources.last(),
                    Some(crate::model::ir::Loc::Imm(imm)) if imm.value == 16
                ),
        );
    assert!(shifts_by_16);
}

/// A far pointer from a near one (`addrspacecast`) with a constant byte
/// offset added: the offset was built as a constant of the far type, which
/// has none (`nbody.bas`: "integer constant of Simple(Far)").
const FAR_PLUS_A_CONSTANT: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
define void @f(ptr %p) addrspace(1) {
b1:
  %w = load i16, ptr %p
  %n = inttoptr i16 %w to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %n to ptr addrspace(1)
  store double -1.250000e-02, ptr addrspace(1) %far
  %next = getelementptr i8, ptr addrspace(1) %far, i16 8
  store double 1.250000e-02, ptr addrspace(1) %next
  ret void
}
";

#[test]
fn a_constant_offset_from_a_far_pointer_is_an_offset_of_its_halves() {
    let module = llrm_mir::parse::module(m16_ir(FAR_PLUS_A_CONSTANT)).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
}

/// A far pointer to a frame object (`addrspacecast ptr to addrspace(5)`, the
/// selector SS) read through after another far access had set ES: the cell is
/// the frame's own. It stopped with "address component Reg(ES)" (bench
/// `lru.nib`), reduced from its `main`.
const FAR_FRAME_POINTER: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"

@$var_bord = internal global [6 x i8] zeroinitializer, align 2
@$var_bprev = internal global [6 x i8] zeroinitializer, align 2
@$var_bnext = internal global [6 x i8] zeroinitializer, align 2
@$var_bstamp = internal global [12 x i8] zeroinitializer, align 4

define cc 1005 i16 @f() addrspace(1) nearcode \"llrm-fp-eval\"=\"x87-extended\" memory(read, inaccessiblemem: readwrite) {
entry:
  %v1 = alloca [48 x i8]
  %v8 = getelementptr inbounds nuw i8, ptr %v1, i16 34
  %v9 = addrspacecast ptr %v8 to ptr addrspace(1)
  call void @llvm.memset.p1.i16(ptr addrspace(1) align 2 %v9, i8 -1, i16 6, i1 false)
  %v10 = addrspacecast ptr %v1 to ptr addrspace(5)
  %v11 = load i16, ptr addrspace(1) getelementptr (i8, ptr addrspace(1) addrspacecast (ptr @$var_bprev to ptr addrspace(1)), i16 2)
  %v12 = icmp sgt i16 %v11, -1
  %v13 = load i16, ptr addrspace(1) getelementptr (i8, ptr addrspace(1) addrspacecast (ptr @$var_bnext to ptr addrspace(1)), i16 2)
  %v14 = icmp sgt i16 %v13, -1
  %or.cond = select i1 %v12, i1 true, i1 %v14
  br i1 %or.cond, label %L32, label %L15

L15:
  %v16 = load i16, ptr addrspace(1) getelementptr (i8, ptr addrspace(1) addrspacecast (ptr @$var_bord to ptr addrspace(1)), i16 2)
  %v17 = getelementptr inbounds nuw i8, ptr addrspace(5) %v10, i16 20
  %v18 = shl i16 %v16, 1
  %v19 = getelementptr inbounds i8, ptr addrspace(5) %v17, i16 %v18
  %v20 = load i16, ptr addrspace(5) %v19
  %v21 = icmp eq i16 %v20, 1
  br i1 %v21, label %L32, label %L22

L22:
  %v23 = getelementptr inbounds nuw i8, ptr addrspace(5) %v10, i16 34
  %v24 = getelementptr inbounds i8, ptr addrspace(5) %v23, i16 %v18
  %v25 = load i16, ptr addrspace(5) %v24
  %v26 = icmp sgt i16 %v25, -1
  br i1 %v26, label %L27, label %L32

L27:
  br label %L32

L32:
  ret i16 0
}

declare void @llvm.memset.p1.i16(ptr addrspace(1) nocapture writeonly, i8, i16, i1 immarg)

";

#[test]
fn a_cell_through_a_far_pointer_to_the_frame_is_the_frames_own() {
    let module = llrm_mir::parse::module(m16_ir(FAR_FRAME_POINTER)).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
}

/// The far pointer a call's dword result is: the fill took the dword apart and
/// put it together again (`shr; shl; movzx; add`) to name `di`, where `di` is
/// the low word and `es` the high one.
#[test]
fn a_far_fill_through_a_call_result_does_not_rebuild_the_dword() {
    let module = llrm_mir::parse::module(m16_ir(FAR_FROM_A_CALL_RESULT)).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|why| panic!("{}", why.0));
    let insns = done.body.insns();
    let names: Vec<_> = insns.iter().filter_map(|i| i.what.as_ref().and_then(|w| w.name.as_deref())).collect();
    // One `shl` joins the call's two words for the null test; the fill needs
    // none.
    assert_eq!(names.iter().filter(|name| **name == "shl").count(), 1, "{names:?}");
}

/// `rep stosb` at real mode reads DI only, but the destination was the whole
/// joined pointer (`shl; movzx; add; mov edi, ..`: 3 extra instructions and 3
/// live values per far fill in `_part_init`). The offset word goes to DI as it
/// is.
#[test]
fn a_far_fill_takes_the_offset_word_not_the_joined_pointer() {
    let ir = m16_ir(FAR_MEMSET)
        .replace("  %p = addrspacecast ptr addrspace(2) %s to ptr addrspace(4)\n", "")
        .replace("ptr addrspace(2) %s", "ptr addrspace(4) %p");
    let done = select_f(&ir).unwrap_or_else(|why| panic!("{why}"));
    let insns = done.body.insns();
    let fill = insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.op == Operation::Fill)).expect("a fill");
    let sources = &fill.what.as_ref().expect("semantics").sources;
    let crate::model::ir::Loc::Held(destination) = &sources[2] else { panic!("{fill:?}") };
    assert_eq!(destination.width, 2, "the joined dword, not the offset word: {fill:?}");
}

/// `smin`, `smax`, `umin` and `umax` of an `i64` stopped the legalizer with
/// `Unsupported { op: SMin, ty: I64, what: "expand result" }`.
#[test]
fn an_i64_min_or_max_expands_to_its_halves() {
    for op in ["smin", "smax", "umin", "umax"] {
        let ir = m16_ir(&format!(
            "target datalayout = \"@m16-datalayout@-ni:2:3\"
declare i64 @llvm.{op}.i64(i64, i64)
define i64 @f(i64 %a, i64 %b) {{
entry:
  %r = call i64 @llvm.{op}.i64(i64 %a, i64 %b)
  ret i64 %r
}}
"
        ));
        let module = llrm_mir::parse::module(&ir).expect("parses");
        selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
            .unwrap_or_else(|e| panic!("{op}: {}", e.0));
    }
}

const SELECT_IN_CALL: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
@s = internal global [2 x i8] zeroinitializer
declare cc 1000 void @g(i32) addrspace(1)
declare cc 1000 void @h(ptr) addrspace(1)
define cc 1000 void @f(i16 %k, i32 %a) addrspace(1) {
  %c = icmp eq i16 %k, 1
  %r = select i1 %c, i32 %a, i32 524246911
  call cc 1000 addrspace(1) void @h(ptr @s)
  call cc 1000 addrspace(1) void @g(i32 %r)
  ret void
}
";

/// A select that splits the block between an argument area and its call left
/// the area open: `sub sp, 2; mov bx, sp; mov [bx], offset _s` where the
/// old selector pushed (+42 B in `range_power_digits`, +45 in `jumps`). The
/// area moves to the call and stays a push.
#[test]
fn a_select_between_an_argument_area_and_its_call_leaves_pushes() {
    let done = select_f(m16_ir(SELECT_IN_CALL)).unwrap_or_else(|why| panic!("{why}"));
    let names = mnemonics(&done);
    assert_eq!(names.iter().filter(|m| m.starts_with("push")).count(), 2, "{names:?}");
}

/// `freeze` of an `i64` at m16 stopped the legalizer with `Unsupported
/// { op: Freeze, ty: I64, what: "expand result" }` (vsgcc `x_ll_div`).
#[test]
fn a_frozen_i64_expands_to_two_frozen_halves() {
    let ir = m16_ir(
        "target datalayout = \"@m16-datalayout@-ni:2:3\"
define i64 @f(i64 %a) {
entry:
  %b = freeze i64 %a
  ret i64 %b
}
",
    );
    let module = llrm_mir::parse::module(ir).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const HUGE_STEP: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
define ptr addrspace(3) @f(ptr addrspace(3) %p) {
entry:
  %q = getelementptr i8, ptr addrspace(3) %p, i32 4
  ret ptr addrspace(3) %q
}
";

/// A constant step of a huge pointer moves the selector by at most one
/// stride, and the offset's wrap says whether. It went through the dword sum
/// and its high word (`add; sar 16; shl`, six instructions per step
/// in `huge_array`) instead of the wrap of the 16-bit add.
#[test]
fn a_constant_step_of_a_huge_pointer_is_the_offset_wrap() {
    let module = llrm_mir::parse::module(m16_ir(HUGE_STEP)).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let names = mnemonics(&done);
    assert!(!names.iter().any(|m| m == "sar"), "{names:?}");
}

/// The wrap of a constant step was a compare of the new offset against the
/// old one (`sub`), which kept the old offset live across the step
/// (`huge_copy.c` 54 -> 82 bytes); it is the carry flag of the add.
#[test]
fn a_constant_step_of_a_huge_pointer_wraps_by_the_carry_of_the_add() {
    let module = llrm_mir::parse::module(m16_ir(HUGE_STEP)).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let names = mnemonics(&done);
    assert!(!names.iter().any(|m| m == "sub"), "{names:?}");
}

/// A load through a huge pointer plus a constant read ES from the old
/// selector: the carry into it was computed and dropped (`hugelp.bas`
/// 264 bytes against the old selector's 303), wrong once the offset wraps.
#[test]
fn a_load_through_a_stepped_huge_pointer_reads_the_carried_selector() {
    let load = m16_ir(HUGE_STEP)
        .replace("define ptr addrspace(3) @f", "define i16 @f")
        .replace("ret ptr addrspace(3) %q", "%v = load i16, ptr addrspace(3) %q\n  ret i16 %v");
    let module = llrm_mir::parse::module(&load).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let named = |what: &str| insns.iter().rposition(|i| i.what.as_ref().and_then(|w| w.name.as_deref()) == Some(what));
    let (load, carried) = (named("mov").expect("the load"), named("add").expect("the selector sum"));
    assert!(insns[load].uses.iter().any(|v| insns[carried].defines.contains(v)), "the load reads the old selector");
}

/// `uadd.with.overflow` and `usub.with.overflow` returning the `i1` flag
/// stopped the legalizer with `Unsupported { op: UAddO, ty: I1, what: "promote
/// result" }` (any width), and at `i64` with `"expand result"`. An `i64` one is
/// `add; adc` (`sub; sbb`) and the carry out.
#[test]
fn an_unsigned_overflow_flag_is_the_carry_of_the_whole_sum() {
    for (op, width, carry) in [
        ("uadd", 16, None),
        ("usub", 16, None),
        ("uadd", 32, None),
        ("uadd", 64, Some("adc")),
        ("usub", 64, Some("sbb")),
    ] {
        let ir = m16_ir(&format!(
            "target datalayout = \"@m16-datalayout@-ni:2:3\"
declare {{i{width}, i1}} @llvm.{op}.with.overflow.i{width}(i{width}, i{width})
define i1 @f(i{width} %a, i{width} %b) {{
entry:
  %r = call {{i{width}, i1}} @llvm.{op}.with.overflow.i{width}(i{width} %a, i{width} %b)
  %o = extractvalue {{i{width}, i1}} %r, 1
  ret i1 %o
}}
"
        ));
        let module = llrm_mir::parse::module(&ir).expect("parses");
        let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
            .unwrap_or_else(|e| panic!("{op} i{width}: {}", e.0));
        let names = mnemonics(&done);
        if let Some(carry) = carry {
            assert!(names.iter().any(|m| m == carry), "{op} i{width}: {names:?}");
        }
    }
}

/// `llvm.{u,s}{add,sub}.sat` stopped the selector at "intrinsic call" (and the
/// MIR parser at the unknown name) at every width. They are the wrapped
/// operation and a select on its overflow, so each width's selection holds the
/// operation's own instruction (`adc`/`sbb` for the halves of a wide one).
#[test]
fn a_saturating_add_or_subtract_is_the_wrapped_operation_and_a_select() {
    for op in ["uadd", "usub", "sadd", "ssub"] {
        for width in [8, 16, 32, 64] {
            let ir = format!(
                "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"
declare i{width} @llvm.{op}.sat.i{width}(i{width}, i{width})
define i{width} @f(i{width} %a, i{width} %b) {{
entry:
  %r = call i{width} @llvm.{op}.sat.i{width}(i{width} %a, i{width} %b)
  ret i{width} %r
}}
"
            );
            let module = llrm_mir::parse::module(&ir).expect("parses");
            let done =
                selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
                    .unwrap_or_else(|e| panic!("{op}.sat i{width}: {}", e.0));
            let names = mnemonics(&done);
            let wrapped = if op.ends_with("add") { "add" } else { "sub" };
            assert!(names.iter().any(|m| m == wrapped), "{op}.sat i{width}: {names:?}");
            if width == 64 {
                let carry = if op.ends_with("add") { "adc" } else { "sbb" };
                assert!(names.iter().any(|m| m == carry), "{op}.sat i64: {names:?}");
            }
        }
    }
}

/// `llvm.umul.fix`, `llvm.{s,u}mul.fix.sat` and `llvm.{s,u}shl.sat` were
/// unknown names (the selector panicked in the assembler on the call to a
/// function nothing defines); `smul.fix` of an `i8`, `i16` or `i64` stopped the
/// legalizer with `Unsupported { op: SMulFix, what: "convert node to libcall"
/// }`. Each is a multiply (a shift) and selects.
#[test]
fn fixed_point_multiplies_and_saturating_shifts_select_at_every_width() {
    for width in [8, 16, 32, 64] {
        for (name, extra) in [
            ("smul.fix", ", i32 3"),
            ("umul.fix", ", i32 3"),
            ("smul.fix.sat", ", i32 3"),
            ("umul.fix.sat", ", i32 3"),
            ("smul.fix.sat", ", i32 0"),
            ("umul.fix.sat", ", i32 0"),
            ("sshl.sat", ""),
            ("ushl.sat", ""),
        ] {
            let scale = if extra.is_empty() { "" } else { ", i32" };
            let ir = format!(
                "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"
declare i{width} @llvm.{name}.i{width}(i{width}, i{width}{scale})
define i{width} @f(i{width} %a, i{width} %b) {{
entry:
  %r = call i{width} @llvm.{name}.i{width}(i{width} %a, i{width} %b{extra})
  ret i{width} %r
}}
"
            );
            let module = llrm_mir::parse::module(&ir).expect("parses");
            selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
                .unwrap_or_else(|e| panic!("{name}{extra} i{width}: {}", e.0));
        }
    }
}
