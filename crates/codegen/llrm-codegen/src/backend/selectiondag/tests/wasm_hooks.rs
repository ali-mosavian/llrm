//! The hooks a target with no x86 conventions sets (wasm's): each is called
//! when set and invisible when not.

use std::sync::Mutex;

use llrm_lir::registers::RegId;
use llrm_mir::datalayout::DataLayout;
use llrm_target::BuilderArch;

use super::super::builder::{BlockDag, BuilderTarget, TargetNodes, build_block};
use super::super::function_lowering::FunctionLoweringInfo;
use super::super::target_calling::{FixedObject, In, LiveIn, Out, TargetCalling};
use super::*;

/// A target with no x86 description (wasm): the builder asks its hooks, not
/// x86's string instructions.
struct Narrow;

impl BuilderArch for Narrow {
    fn registers(&self) -> &'static llrm_lir::registers::Info {
        llrm_target::Target::registers(&llrm_x86_m32::M32)
    }
    fn register_named(
        &self,
        name: &str,
    ) -> RegId {
        llrm_target::Target::register_named(&llrm_x86_m32::M32, name)
    }
    fn stack_slot_bytes(&self) -> i64 {
        4
    }
    fn stack_alignment(&self) -> i64 {
        16
    }
    fn roles(&self) -> llrm_mir::spaces::Spaces {
        BuilderArch::roles(&llrm_x86_m32::M32)
    }
    fn full(&self) -> Option<&dyn llrm_target::Target> {
        None
    }
}

/// What the builder asks of the target, and which hooks it answers.
#[derive(Debug, Default)]
struct Probe {
    seen: Mutex<Vec<String>>,
    in_place: bool,
    buffer: bool,
    mem: bool,
}

impl TargetCalling for Probe {
    fn lower_call(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        callee: SDValue,
        _outs: &[Out],
        ins: &[In],
    ) -> (SDValue, Vec<SDValue>) {
        self.seen.lock().unwrap().push("call".into());
        let call = dag.get_node_multi(super::super::x86isd::CALL, &[EVT::OTHER, EVT::GLUE], &[chain, callee]);
        let values = ins.iter().map(|one| dag.get_constant(0, one.vt)).collect();
        (SDValue { node: call.node, res: 0 }, values)
    }

    fn lower_formal_arguments(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        ins: &[In],
        _fixed: FixedObject,
        live_in: LiveIn,
    ) -> Vec<SDValue> {
        ins.iter().map(|one| dag.get_copy_from_reg(chain, live_in(RegId::AL), one.vt)).collect()
    }

    fn lower_return(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        _outs: &[Out],
    ) -> SDValue {
        dag.get_node(super::super::x86isd::RET_GLUE, EVT::OTHER, &[chain])
    }

    fn passes_in_place(
        &self,
        _attribute: &str,
    ) -> bool {
        self.in_place
    }

    fn lower_call_with_frame(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        callee: SDValue,
        outs: &[Out],
        ins: &[In],
        variadic: bool,
        frame: &mut dyn FnMut(u32, u32) -> i32,
    ) -> (SDValue, Vec<SDValue>) {
        self.seen.lock().unwrap().push(format!("frame {} variadic {variadic}", frame(8, 4)));
        self.lower_call(dag, chain, callee, outs, ins)
    }

    fn variadic_buffer(
        &self,
        dag: &mut SelectionDAG,
        fixed: usize,
    ) -> Option<SDValue> {
        self.seen.lock().unwrap().push(format!("buffer after {fixed}"));
        self.buffer.then(|| dag.get_constant(0, EVT::I32))
    }

    fn emit_memcpy(
        &self,
        _dag: &mut SelectionDAG,
        chain: SDValue,
        _dst: SDValue,
        _src: SDValue,
        _size: SDValue,
    ) -> Option<SDValue> {
        self.seen.lock().unwrap().push("memcpy".into());
        self.mem.then_some(chain)
    }

    fn emit_memset(
        &self,
        _dag: &mut SelectionDAG,
        chain: SDValue,
        _dst: SDValue,
        _value: SDValue,
        _size: SDValue,
    ) -> Option<SDValue> {
        self.seen.lock().unwrap().push("memset".into());
        self.mem.then_some(chain)
    }
}

/// `body` built for a 32-bit target whose calling hooks are `probe`: whether it
/// built, the DAG, what the hooks saw and the lowering info.
fn built(
    body: &str,
    probe: Probe,
    options: impl FnOnce(&mut BuilderTarget),
) -> (Result<BlockDag, String>, Vec<String>, FunctionLoweringInfo) {
    let text = format!("target datalayout = \"e-p:32:32-n8:16:32\"\n{body}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let (id, _, function) = module.functions().find(|(_, _, function)| !function.is_declaration()).expect("f");
    let convention = crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
    let mut info = FunctionLoweringInfo::new(function);
    let probe: &'static Probe = Box::leak(Box::new(probe));
    let mut target = BuilderTarget {
        target_nodes: Some(TargetNodes {
            call: super::super::x86isd::CALL,
            ret: super::super::x86isd::RET_GLUE,
            stack_pointer: "esp",
            big_endian: false,
            calling: Some(probe),
        }),
        ..BuilderTarget::m32()
    };
    options(&mut target);
    let done =
        build_block(&module, function, &layout, &target, &Narrow, &convention, &mut info, function.entry().unwrap());
    (done.map_err(|why| why.0.to_string()), probe.seen.lock().unwrap().clone(), info)
}

fn has(
    block: &BlockDag,
    opcode: ISD,
) -> bool {
    block.dag.nodes().iter().any(|node| node.opcode == opcode) || block.extra.iter().any(|(_, one)| has(one, opcode))
}

const SRET: &str = "define void @f(ptr sret(i32) align 4 %p) {\n  ret void\n}\n";

/// A target whose `sret` is an ordinary pointer (wasm) said so with
/// `passes_in_place`; the builder refused the function whatever the target
/// said.
#[test]
fn a_target_passes_an_sret_pointer_in_place_when_it_says_so() {
    let (default, ..) = built(SRET, Probe::default(), |_| {});
    assert!(default.is_err(), "the default hook moves sret: the builder refuses");
    let (set, ..) = built(SRET, Probe { in_place: true, ..Probe::default() }, |_| {});
    assert!(set.is_ok(), "{:?}", set.err());
}

const VARIADIC_CALL: &str =
    "declare i32 @v(i32, ...)\ndefine i32 @f() {\n  %r = call i32 (i32, ...) @v(i32 1)\n  ret i32 %r\n}\n";

/// `LowerCall` of a target that makes frame objects of its own or needs the
/// callee's variadic flag: the builder called the plain `lower_call` and
/// dropped both.
#[test]
fn a_call_reaches_the_targets_frame_hook_with_the_variadic_flag() {
    let (done, seen, _) = built(VARIADIC_CALL, Probe::default(), |_| {});
    assert!(done.is_ok(), "{:?}", done.err());
    assert_eq!(seen, ["frame 0 variadic true", "call"]);
}

const VARIADIC_BODY: &str = "define i32 @f(i32 %n, ...) {\n  ret i32 %n\n}\n";

/// A variadic function's extra buffer parameter, after the fixed ones: the
/// target gave none, so `va_start` had nothing to read.
#[test]
fn a_variadic_function_asks_the_target_for_its_buffer() {
    let (_, seen, none) = built(VARIADIC_BODY, Probe::default(), |_| {});
    assert_eq!((seen, none.variadic_buffer), (vec!["buffer after 1".to_owned()], None));
    let (_, _, some) = built(VARIADIC_BODY, Probe { buffer: true, ..Probe::default() }, |_| {});
    assert!(some.variadic_buffer.is_some());
}

const MEM: &str = "declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)\ndeclare void @llvm.memset.p0.i32(ptr, i8, i32, i1)\n\
    define void @f(ptr %d, ptr %s, i32 %n) {\n  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr %s, i32 %n, i1 false)\n  \
    call void @llvm.memset.p0.i32(ptr %d, i8 0, i32 %n, i1 false)\n  ret void\n}\n";

/// A copy or fill of a size not known asks the target for its own node before
/// the library call.
#[test]
fn a_memcpy_and_a_memset_of_unknown_size_ask_the_target_first() {
    let asked = |seen: &[String]| seen.iter().filter(|one| matches!(one.as_str(), "memcpy" | "memset")).count();
    let (done, seen, _) = built(MEM, Probe::default(), |_| {});
    assert!(done.is_ok(), "{:?}", done.err());
    assert_eq!((asked(&seen), seen.iter().filter(|one| one.starts_with("frame")).count()), (2, 2), "{seen:?}");
    let (done, taken, _) = built(MEM, Probe { mem: true, ..Probe::default() }, |_| {});
    assert!(done.is_ok(), "{:?}", done.err());
    assert_eq!(taken, ["memcpy", "memset"], "the target's nodes replaced the library calls");
}

const UNREACHABLE: &str = "define void @f() {\n  unreachable\n}\n";

/// `TargetOptions::TrapUnreachable`: control that reaches an `unreachable`
/// stops with a trap (wasm).
#[test]
fn an_unreachable_traps_only_for_a_target_that_traps() {
    let (off, ..) = built(UNREACHABLE, Probe::default(), |_| {});
    assert!(!has(&off.unwrap(), ISD::Trap));
    let (on, ..) = built(UNREACHABLE, Probe::default(), |target| target.trap_unreachable = true);
    assert!(has(&on.unwrap(), ISD::Trap));
}

const SHIFT: &str = "define i32 @f(i32 %a) {\n  %r = shl i32 %a, 3\n  ret i32 %r\n}\n";

/// `getScalarShiftAmountTy` of wasm is the value's type: a constant amount was
/// always an `i8`.
#[test]
fn a_shift_amount_takes_the_type_the_target_names() {
    let amount = |block: &BlockDag| {
        let dag = &block.dag;
        let shl = dag.nodes().iter().find(|node| node.opcode == ISD::Shl).expect("a shift");
        dag.value_type(shl.ops[1])
    };
    let (default, ..) = built(SHIFT, Probe::default(), |_| {});
    assert_eq!(amount(&default.unwrap()), EVT::I8);
    let (by_value, ..) = built(SHIFT, Probe::default(), |target| target.shift_amount_of = Some(|vt| vt));
    assert_eq!(amount(&by_value.unwrap()), EVT::I32);
}

const SWITCH: &str = "define i32 @f(i32 %x) {\nentry:\n  switch i32 %x, label %d [ i32 0, label %a i32 1, label %b i32 2, label %c i32 3, label %e ]\n\
    a:\n  ret i32 10\nb:\n  ret i32 11\nc:\n  ret i32 12\ne:\n  ret i32 13\nd:\n  ret i32 0\n}\n";

/// A jump table needs `min_jump_table_entries` cases and an optimizing target;
/// the floor was fixed at 4 and the search always on.
#[test]
fn the_jump_table_floor_and_the_optimizer_switch_are_the_targets() {
    let table = |options: fn(&mut BuilderTarget)| has(&built(SWITCH, Probe::default(), options).0.unwrap(), ISD::BrJT);
    assert!(table(|_| {}), "the control: four dense cases make a table");
    assert!(!table(|target| target.min_jump_table_entries = 8));
}

const SPARSE: &str = "define i32 @f(i32 %x) {\nentry:\n  switch i32 %x, label %d [ i32 10, label %a i32 30, label %b i32 50, label %c i32 70, label %e i32 90, label %g ]\n\
    a:\n  ret i32 1\nb:\n  ret i32 2\nc:\n  ret i32 3\ne:\n  ret i32 4\ng:\n  ret i32 5\nd:\n  ret i32 0\n}\n";

/// `getOptLevel() == None`: five sparse cases are tested one after another; an
/// optimizing target splits them into a binary search.
#[test]
fn an_unoptimized_switch_is_a_chain_not_a_binary_search() {
    let dump = |optimize: bool| {
        let (done, ..) = built(SPARSE, Probe::default(), |target| target.switch_optimize = optimize);
        let block = done.unwrap();
        std::iter::once(&block)
            .chain(block.extra.iter().map(|(_, one)| one))
            .map(|one| one.dag.dump())
            .collect::<String>()
    };
    assert_ne!(dump(true), dump(false));
}

/// `BuildSDIVPow2`'s default under `isIntDivCheap`: an `sdiv` by a power of two
/// stays an `sdiv` (wasm); it was always turned into shifts.
#[test]
fn an_sdiv_by_a_power_of_two_stays_when_the_target_says_so() {
    use super::super::dag::DagConfig;
    use super::super::dag_combiner::{CombineLevel, NoCombines, combine};
    let root = |stays: bool| {
        let mut dag = SelectionDAG::new(DagConfig { sdiv_pow2_stays: stays, ..DagConfig::default() });
        let x = dag.get_register(EVT::I32, 5);
        let four = dag.get_constant(4, EVT::I32);
        let div = dag.get_node(ISD::SDiv, EVT::I32, &[x, four]);
        dag.set_root(div);
        combine(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes);
        let root = dag.get_root();
        dag.opcode(root)
    };
    assert_ne!(root(false), ISD::SDiv);
    assert_eq!(root(true), ISD::SDiv);
}
