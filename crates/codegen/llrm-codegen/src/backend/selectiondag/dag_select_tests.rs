use llrm_dagisel::generated::x86_i486::{MACHINE_OPCODES, TABLE};
use llrm_dagisel::interp::Selector;
use llrm_dagisel::x86_hooks::{Cpu, Options, X86Hooks};
use llrm_lir::registers::RegId;
use llrm_mir::datalayout::DataLayout;
use llrm_support::hash::HashMap;
use llrm_target::Target;

use super::actions::{Features, TargetLowering};
use super::builder::{BuilderTarget, build_block};
use super::dag_select::select_block;
use super::emit::{EmitContext, FrameMap};
use super::function_lowering::FunctionLoweringInfo;
use super::mvt::MVT;
use super::opcodes::OpcodeMap;
use super::schedule::SchedInfo;
use super::x86_lowering::X86Lowering;
use crate::backend::isel::Parameter;
use crate::model::ir::{Addr, Mem, Space};

struct NoGlobals;

impl super::emit::GlobalMap for NoGlobals {
    fn segment(
        &self,
        _symbol: Option<u32>,
    ) -> crate::model::ir::Addr {
        crate::model::ir::Addr::new(crate::model::ir::Space::Group, 0)
    }

    fn address(
        &self,
        symbol: u32,
        offset: i64,
    ) -> Addr {
        Addr { index: i64::from(symbol), ..Addr::new(Space::Segment, offset) }
    }
}

struct NoTemps;

impl super::x86_preprocess::FrameObjects for NoTemps {
    fn objects(&self) -> i32 {
        0
    }

    fn create_stack_temporary(
        &mut self,
        _size: u32,
        _align: u32,
    ) -> i32 {
        panic!("this test makes no stack temporary")
    }
}

struct Info;

impl SchedInfo for Info {
    fn num_defs(
        &self,
        opcode: u32,
    ) -> usize {
        llrm_dagisel::instr::desc(opcode as u16).map_or(0, |d| usize::from(d.num_defs))
    }

    fn implicit_defs(
        &self,
        opcode: u32,
    ) -> &[u16] {
        llrm_dagisel::instr::desc(opcode as u16).map_or(&[], |d| d.implicit_defs)
    }

    fn is_call(
        &self,
        opcode: u32,
    ) -> bool {
        llrm_dagisel::instr::desc(opcode as u16).is_some_and(|d| d.is_call())
    }
}

/// Fixed object `-k` is the k-th stack-passed parameter.
struct Frame(Vec<i64>);

impl FrameMap for Frame {
    fn cell(
        &self,
        index: i32,
        width: u32,
    ) -> Option<Mem> {
        let disp = *self.0.get(usize::try_from(-index - 1).ok()?)?;
        Some(Mem { through: RegId::BP, disp_width: 2, ..Mem::new(Some(Addr::new(Space::Frame, disp)), width) })
    }
}

fn hooks() -> X86Hooks {
    X86Hooks::new(Options {
        cpu: Cpu::I486,
        opt_none: false,
        opt_for_size: false,
        min_size: false,
        has_frame_pointer: true,
        multiply_early_out: false,
        address_mode: llrm_dagisel::x86_addr::AddrMode::Flat32,
    })
}

/// The DAG path's result for each block of `@f`, or why it refused.
fn through(text: &str) -> Result<Vec<Vec<String>>, String> {
    let module = llrm_mir::parse::module(text).expect("parses");
    let layout =
        module.datalayout.as_deref().map_or_else(DataLayout::default, |t| DataLayout::parse(t).expect("layout"));
    let target = BuilderTarget::m32();
    let machine = llrm_x86_m32::M32;
    let map = OpcodeMap::new(llrm_target::instr::parse(&machine.forms_text(), machine.dialect()).expect("x86.instr"));
    let names: HashMap<u16, &str> = MACHINE_OPCODES.iter().copied().collect();
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let selector = Selector::new(TABLE);
    let (id, _, function) = module.functions().next().expect("a function");
    let convention = crate::backend::isel::convention(&module, &layout, id, &machine).expect("a convention");
    let cells =
        convention.parameters.iter().filter_map(|p| if let Parameter::Cell(disp) = p { Some(*disp) } else { None });
    let frame = Frame(cells.collect());
    let ctx = EmitContext {
        tables: &[],
        caller_saved: &Default::default(),
        conventions: &super::emit::NoConventions,
        constants: &super::emit::NoConstants,
        stack_pointer: RegId::SP,
        strings: None,
        registers: llrm_target::Target::registers(&machine),
        globals: &NoGlobals,
        block_at: &HashMap::default(),
        frame: &frame,
        address_mode: llrm_dagisel::x86_addr::AddrMode::Flat32,
        opt_size: false,
        call_alignment: 1,
        map: &map,
        names: &names,
    };
    let mut info = FunctionLoweringInfo::new(function);
    let mut out = Vec::new();
    for &block in function.layout() {
        let mut built = build_block(&module, function, &layout, &target, &machine, &convention, &mut info, block)
            .map_err(|r| r.0.to_owned())?;
        let before = built.dag.dump();
        let emitted = select_block(
            &mut built.dag,
            &tli,
            &mut X86Lowering { cmov: false, sse2: false, min_size: false },
            &selector,
            &mut hooks(),
            &Info,
            &ctx,
            &mut super::emit::EmitState::default(),
            0,
            &machine,
            &mut NoTemps,
        )
        .map_err(|r| format!("{r:?}\nbuilt:\n{before}\nafter:\n{}", built.dag.dump()))?;
        for b in emitted.blocks {
            out.push(
                b.insns
                    .iter()
                    .map(|i| {
                        let w = i.what.as_ref().unwrap();
                        format!("{:?} := {} {:?}", w.dests, w.name.as_deref().unwrap_or(""), w.sources)
                    })
                    .collect(),
            );
        }
    }
    Ok(out)
}

#[test]
fn a_sum_of_two_arguments_goes_through() {
    let text = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\ndefine i32 @f(i32 %a, i32 %b) {\nentry:\n  %s = add i32 %a, %b\n  ret i32 %s\n}\n";
    let blocks = through(text).unwrap_or_else(|e| panic!("refused: {e}"));
    assert_eq!(blocks.len(), 1, "{blocks:#?}");
    let names: Vec<&str> =
        blocks[0].iter().map(|line| line.split(" := ").nth(1).unwrap().split(' ').next().unwrap()).collect();
    assert_eq!(names, ["mov", "add", ""], "{blocks:#?}");
    assert!(blocks[0][1].contains("Mem"), "the second load folds into the add: {blocks:#?}");
}

/// `x * 5` reaches selection with no `MUL` node: the combiner ran between
/// the legalizers. Without the `Combine` calls the multiply by a constant
/// is selected as `IMUL32rmi`, which the pin turns into a `lea`.
#[test]
fn the_combiner_runs_between_the_legalizers() {
    use super::isd::ISD;
    let text = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\ndefine i32 @f(i32 %a) {\nentry:\n  %m = mul i32 %a, 5\n  ret i32 %m\n}\n";
    let module = llrm_mir::parse::module(text).expect("parses");
    let layout =
        module.datalayout.as_deref().map_or_else(DataLayout::default, |t| DataLayout::parse(t).expect("layout"));
    let machine = llrm_x86_m32::M32;
    let (id, _, function) = module.functions().next().expect("a function");
    let convention = crate::backend::isel::convention(&module, &layout, id, &machine).expect("a convention");
    let mut info = FunctionLoweringInfo::new(function);
    let block = function.layout()[0];
    let mut built =
        build_block(&module, function, &layout, &BuilderTarget::m32(), &machine, &convention, &mut info, block)
            .expect("builds");
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    super::dag_select::legalized(
        &mut built.dag,
        &tli,
        &mut X86Lowering { cmov: false, sse2: false, min_size: false },
        &machine,
    )
    .expect("legalizes");
    let root = built.dag.get_root();
    let mut seen = llrm_support::hash::HashSet::default();
    let mut work = vec![root.node];
    while let Some(n) = work.pop() {
        if seen.insert(n) {
            let node = built.dag.node(super::SDValue { node: n, res: 0 });
            assert_ne!(node.opcode, ISD::Mul, "the multiply by 5 was combined");
            work.extend(node.ops.iter().map(|op| op.node));
        }
    }
}

/// The lowering makes the `Wrapper` of a global after the load that reads it,
/// so reverse creation order selected the wrapper first (`mov r, $g`) and the
/// load read through `r`: `mov eax, offset g; mov eax, [eax]` for `return g`.
#[test]
fn a_load_of_a_global_is_one_instruction_on_the_symbol() {
    let text = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\n@g = global i32 5\ndefine i32 @f() {\nentry:\n  %v = load i32, ptr @g\n  ret i32 %v\n}\n";
    let blocks = through(text).unwrap_or_else(|e| panic!("refused: {e}"));
    let names: Vec<&str> =
        blocks[0].iter().map(|line| line.split(" := ").nth(1).unwrap().split(' ').next().unwrap()).collect();
    assert_eq!(names, ["mov", ""], "the load, the return that requires it in EAX: {blocks:#?}");
    assert!(blocks[0][0].contains("addr: Some"), "the load reads the symbol's cell: {blocks:#?}");
}

/// `end_of(t) - t` after inlining is `sub (sym+12), sym` of constant addresses:
/// the DAG selects `neg` of the symbol and an `lea` of `sym+12` with the
/// negated register as its index; `emit` named the cell by its symbol alone
/// (`AddressRef` has no values), the register dropped: the answer
/// was the address, 1639644 for 12 (`--isel=dag`).
#[test]
fn a_symbol_difference_keeps_the_negated_register_in_its_lea() {
    let text = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\n@b = private constant [13 x i8] c\"strlen idiom\\00\"\ndefine i32 @f() {\nentry:\n  %d = sub i32 ptrtoint (ptr getelementptr inbounds nuw (i8, ptr @b, i32 12) to i32), ptrtoint (ptr @b to i32)\n  ret i32 %d\n}\n";
    let blocks = through(text).unwrap_or_else(|e| panic!("refused: {e}"));
    let names: Vec<&str> =
        blocks[0].iter().map(|line| line.split(" := ").nth(1).unwrap().split(' ').next().unwrap()).collect();
    let lea = blocks[0].iter().find(|line| line.contains(" lea "));
    // Either the difference is a `sub`, or an `lea` that reads the register
    // `neg` made.
    assert!(names.contains(&"sub") || lea.is_some_and(|line| line.contains("index: Some")), "{blocks:#?}");
}

/// `x << (32 - c)`: the shift hook rewrites the count (`32 - c` is `-c` to the
/// hardware's mask), making new nodes during selection. They stayed generic
/// (`Truncate`, `Sub`) and the emitter refused the function ("operand Truncate
/// before its definition", shift_int64_masked_amount).
#[test]
fn nodes_a_selection_makes_are_selected_too() {
    let text = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\ndefine i32 @f(i32 %v, i32 %c) {\nentry:\n  %n = sub i32 32, %c\n  %r = shl i32 %v, %n\n  ret i32 %r\n}\n";
    let blocks = through(text).unwrap_or_else(|e| panic!("refused: {e}"));
    assert!(blocks[0].iter().any(|line| line.contains("shl")), "{blocks:#?}");
}

/// A target other than [`X86Target`] drives the same path: `select_units` asks
/// it to preprocess once, after legalizing and before any selection, and the
/// units it returns are the ones x86's own wrapper gets.
#[test]
fn a_dag_target_is_preprocessed_once_before_it_selects() {
    use super::dag_select::{X86Chooser, X86Target};
    use super::dag_target::{DagTarget, Phases, select_units};

    struct Recording<'a> {
        inner: X86Target<'a>,
        log: Vec<&'static str>,
    }
    impl DagTarget for Recording<'_> {
        type Lowering = X86Lowering;

        fn phases(&mut self) -> Phases<'_, X86Lowering> {
            self.log.push("legalize");
            self.inner.phases()
        }

        fn selection(&mut self) -> (&dyn super::dagisel_adapter::PinRegisters, &mut dyn super::dag_target::Select) {
            self.log.push("select");
            self.inner.selection()
        }

        fn preprocess(
            &mut self,
            dag: &mut super::SelectionDAG,
            temporaries: &mut dyn super::x86_preprocess::FrameObjects,
        ) {
            self.log.push("preprocess");
            self.inner.preprocess(dag, temporaries);
        }
    }

    let text = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\ndefine i32 @f(i32 %a, i32 %b) {\nentry:\n  %s = add i32 %a, %b\n  ret i32 %s\n}\n";
    let module = llrm_mir::parse::module(text).expect("parses");
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).expect("layout");
    let machine = llrm_x86_m32::M32;
    let (id, _, function) = module.functions().next().expect("a function");
    let convention = crate::backend::isel::convention(&module, &layout, id, &machine).expect("a convention");
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let selector = Selector::new(TABLE);
    let mut units = Vec::new();
    for through_trait in [false, true] {
        let mut info = FunctionLoweringInfo::new(function);
        let mut built = build_block(
            &module,
            function,
            &layout,
            &BuilderTarget::m32(),
            &machine,
            &convention,
            &mut info,
            function.layout()[0],
        )
        .expect("builds");
        let (mut lowering, mut hooks) = (X86Lowering { cmov: false, sse2: false, min_size: false }, hooks());
        let x86 = X86Target {
            tli: &tli,
            lowering: &mut lowering,
            chooser: X86Chooser { selector: &selector, hooks: &mut hooks },
            arch: &machine,
        };
        let made = if through_trait {
            let mut target = Recording { inner: x86, log: Vec::new() };
            let made = select_units(&mut built.dag, &mut target, &Info, &mut NoTemps).expect("selects");
            assert_eq!(target.log, ["legalize", "preprocess", "select"]);
            made
        } else {
            let mut x86 = x86;
            select_units(&mut built.dag, &mut x86, &Info, &mut NoTemps).expect("selects")
        };
        units.push(made.len());
    }
    assert_eq!(units[0], units[1]);
}
