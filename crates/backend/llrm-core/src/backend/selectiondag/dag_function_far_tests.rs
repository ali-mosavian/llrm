//! Far and huge pointers at m16: selectors, segments, ES, phis of far pointers.

use super::dag_function::selected;
use super::dag_function_tests::FAR_CALL;
use crate::model::ir::Operation;

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
    let module = llrm_mir::parse::module(FAR_CALL).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let sites: Vec<(i64, String)> = done.calls.iter().map(|(at, name)| (*at, name.clone())).collect();
    assert_eq!(sites.len(), 2, "{sites:?}");
    let far: Vec<&str> = sites.iter().filter(|(at, _)| done.far.contains(at)).map(|(_, name)| name.as_str()).collect();
    assert_eq!(far, ["far"], "{sites:?} far={:?}", done.far);
}

const VARIABLE_MOVE: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
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
    let module = llrm_mir::parse::module(VARIABLE_MOVE).expect("parses");
    let libcalls = [("memmove".to_owned(), 0, "_memmove@3".to_owned())];
    let done =
        selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &libcalls)
            .unwrap_or_else(|e| panic!("{}", e.0));
    let sites: Vec<(&i64, &String)> = done.calls.iter().collect();
    assert_eq!(sites.len(), 1, "{sites:?}");
    assert!(done.far.contains(sites[0].0), "{sites:?} far={:?}", done.far);
}

const M16_FAR_LOOP: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
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

const M16_FAR_LOAD: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
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

const M16_ZERO_16: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
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

const M16_FILL: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
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

const FAR_INDIRECT_ARGUMENT: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
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
    let module = llrm_mir::parse::module(FAR_INDIRECT_ARGUMENT).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let call = insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.op == crate::model::ir::Operation::Call));
    assert!(call.is_some_and(|call| !call.requires.is_empty()), "{insns:?}");
}

const FAR_GLOBAL_ELEMENT: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
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
    let module = llrm_mir::parse::module(FAR_GLOBAL_ELEMENT).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const STRING_OP_ON_ELEMENT: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
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
    let module = llrm_mir::parse::module(STRING_OP_ON_ELEMENT).expect("parses");
    selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
}

const M16_FAR_AND_DOUBLE: &str =
    "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32\"
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
    "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32\"
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

const M16_HUGE_MEMSET: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
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
    format!(
        "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
define cc 1005 void @f(ptr addrspace({dst}) %d, ptr addrspace({src}) %s, i32 %n) addrspace(1) {{
entry:
  call void @llvm.{name}.p{dst}.p{src}.i32(ptr addrspace({dst}) %d, ptr addrspace({src}) %s, i32 %n, i1 false){metadata}
  ret void
}}
declare void @llvm.{name}.p{dst}.p{src}.i32(ptr addrspace({dst}), ptr addrspace({src}), i32, i1)
"
    )
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
    let ir = M16_HUGE_MEMSET.to_owned();
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

const VARSEG: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
@x = internal global i16 0
define i16 @f() {
entry:
  ret i16 ptrtoint (ptr addrspace(2) addrspacecast (ptr @x to ptr addrspace(2)) to i16)
}
";

/// `VARSEG(x)` is a near pointer cast to the segment space: the selector of
/// its segment. The DAG path took the offset (`mov b$seg, offset X%`), so
/// `POKE` after `DEF SEG = VARSEG(x)` wrote 0 bytes from the segment and
/// `far_poke_*` printed garbage.
#[test]
fn a_near_pointer_cast_to_the_segment_space_is_its_segment_selector() {
    let module = llrm_mir::parse::module(VARSEG).expect("parses");
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

const FAR_COPY: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
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
    let module = llrm_mir::parse::module(FAR_COPY).expect("parses");
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

const M16_FAR_COPY: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
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

const FAR_MEMSET: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
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
    let module = llrm_mir::parse::module(FAR_MEMSET).expect("parses");
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
