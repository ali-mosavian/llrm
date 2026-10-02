//! The raise's memory: programs run by `llrm_mir::interpret` against their
//! BASIC meaning, and what `llrm_analysis` proves apart.

use llrm_analysis::alias::annotated;
use llrm_analysis::memory::{MemRef, Unit};
use llrm_analysis::regions::overlapping;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::interpret::{self, Val};
use llrm_mir::{CastOp, Constant, ConstantKind, GlobalKind, GlobalVariable, InstId, Linkage, Module, Opcode, Operand, Position, Type};

fn raised(fixture: &str) -> Module {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/inputs/omf").join(fixture);
    let found = llrm_omf::module::load(&path).expect("reads").expect("an object");
    let module = llrm_bc::raise(&found, &llrm_x86_code16::machine::BUILT_IN).unwrap_or_else(|refusal| panic!("{refusal}")).module;
    let errors = llrm_mir::verify::verify(&module);
    assert!(errors.is_empty(), "{errors:#?}\n{}", llrm_mir::print::module(&module));
    module
}

fn global(module: &mut Module, name: &str) -> Operand {
    let id = module.named(name).unwrap_or_else(|| panic!("no @{name}"));
    Operand::Constant(module.reference(id))
}

/// Runs `main` with every runtime routine a stub answering zero: each
/// `B$PEI2` stores its word to the next of `@printed`, and `B$DDIM` points
/// the descriptor at `@descriptor` to `@heap`, the offset fields 0 and 10
/// zero. Answers what was printed.
fn printed(mut module: Module, descriptor: &str, count: usize) -> Vec<i64> {
    let word = module.context.types.int(16);
    let row = module.context.types.intern(Type::Array { element: word, count: 64 });
    let heap = module.context.types.intern(Type::Array { element: word, count: 1024 });
    for (name, ty) in [("printed", row), ("heap", heap), ("count", word)] {
        let zero = module.context.constant(Constant { ty, kind: ConstantKind::Zero });
        module.add_variable(name, GlobalVariable { ty, constant: false, initializer: Some(zero), align: None }, Linkage::Internal).expect("free");
    }
    let (printed, count_cell, heap, descriptor) = (global(&mut module, "printed"), global(&mut module, "count"), global(&mut module, "heap"), global(&mut module, descriptor));
    let routines: Vec<_> = module
        .functions()
        .filter(|(_, one, function)| function.is_declaration() && one.name.as_deref().is_some_and(|name| name.starts_with(llrm_bc::RUNTIME)))
        .map(|(id, one, _)| (id, one.name.clone().unwrap()))
        .collect();
    for (id, name) in routines {
        let returns = match &module.globals[id.0 as usize].kind {
            GlobalKind::Function(function) => module.signature(function.ty).0,
            _ => unreachable!(),
        };
        let mut b = module.builder(id);
        let entry = b.block("entry");
        b.position(entry);
        match name.trim_start_matches(llrm_bc::RUNTIME) {
            "B$PEI2" => {
                let at = b.load(word, count_cell, false, "");
                let slot = b.gep(word, printed, &[at], Default::default(), "");
                let value = b.parameter(0);
                b.store(value, slot, false);
                let one = b.int(16, 1);
                let next = b.binary(llrm_mir::BinaryOp::Add, at, one, Default::default(), "");
                b.store(next, count_cell, false);
            }
            "B$DDIM" => {
                let selector = b.cast(CastOp::PtrToInt, heap, word, "");
                for (offset, value) in [(0, None), (2, Some(selector)), (10, None)] {
                    let index = b.int(16, offset);
                    let byte = b.context.types.int(8);
                    let field = b.gep(byte, descriptor, &[index], Default::default(), "");
                    let value = value.unwrap_or_else(|| b.int(16, 0));
                    b.store(value, field, false);
                }
            }
            _ => {}
        }
        let void = b.context.types.void();
        let answer = (returns != void).then(|| Operand::Constant(b.context.constant(Constant { ty: returns, kind: ConstantKind::Zero })));
        b.ret(answer);
    }
    // The program ends in B$CENP, which does not return.
    let (context, main) = module.function_mut("main").expect("a main");
    let void = context.types.void();
    let ends: Vec<_> = main.layout().iter().filter_map(|&block| main.terminator(block).filter(|&inst| main.instruction(inst).opcode == Opcode::Unreachable).map(|inst| (block, inst))).collect();
    for (block, inst) in ends {
        main.erase(inst).expect("a terminator");
        let ret = main.create_instruction(Opcode::Ret, void, vec![], Default::default(), None);
        main.insert(ret, Position::End(block)).expect("placed");
    }
    // What main leaves in @printed, read back by a probe that runs it.
    let long = module.context.types.int(16);
    (0..count)
        .map(|at| {
            let fn_ty = module.context.types.intern(Type::Function { returns: long, parameters: vec![], variadic: false });
            let name = format!("probe{at}");
            let id = module.add_function(&name, fn_ty, Linkage::External).expect("free");
            let main = global(&mut module, "main");
            let main_ty = module.context.types.intern(Type::Function { returns: void, parameters: vec![], variadic: false });
            let mut b = module.builder(id);
            let entry = b.block("entry");
            b.position(entry);
            b.call(main_ty, main, &[], "");
            let index = b.int(16, at as i128);
            let slot = b.gep(long, printed, &[index], Default::default(), "");
            let value = b.load(long, slot, false, "");
            b.ret(Some(value));
            match interpret::run(&module, &name, vec![], 10_000_000).unwrap_or_else(|trap| panic!("{trap:?}\n{}", llrm_mir::print::module(&module))) {
                Val::Int { bits, .. } => bits as u16 as i16 as i64,
                other => panic!("{other:?}"),
            }
        })
        .collect()
}

/// harr: `m(r, c) = r + c`, then `t = t + m(r, c)`, r and c 1 to 10, in a
/// dynamic 21 x 21 array: T=1100. Its B$DDIM pops what its two dimensions
/// pushed, and the element's address reads the descriptor through SI.
#[test]
fn a_dynamic_array_sums_what_it_stored() {
    for fixture in ["harr-q-o.obj", "harr-p-g2.obj", "harr-v-g3.obj"] {
        assert_eq!(printed(raised(fixture), "BC_DATA.0006", 1), vec![1100], "{fixture}");
    }
}

/// segld: five times `a(i) = i`, `t = t + a(i)`, i 1 to 20: T=1050.
#[test]
fn a_dynamic_array_in_a_nested_loop() {
    assert_eq!(printed(raised("segld-q-o.obj"), "BC_DATA.0006", 1), vec![1050]);
}

/// The accesses of `main`, each with what alias analysis makes of it.
fn accesses(module: &Module) -> Vec<(InstId, MemRef)> {
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a layout")).expect("parses");
    let function = module.global(module.named("main").expect("a main")).function().expect("a function");
    let references = annotated(&Unit::of(module, &layout, function)).expect("annotates");
    references.into_iter().collect()
}

/// An access of `main` to the global `name`, through its GEPs.
fn access<'a>(module: &Module, all: &'a [(InstId, MemRef)], name: &str) -> &'a MemRef {
    let function = module.global(module.named("main").expect("a main")).function().expect("a function");
    let target = module.named(name).unwrap_or_else(|| panic!("no @{name}"));
    let root = |mut pointer: Operand| loop {
        let Operand::Value(value) = pointer else { return pointer };
        let llrm_mir::ValueDef::Instruction(inst) = function.value(value).def else { return pointer };
        let one = function.instruction(inst);
        if !matches!(one.opcode, Opcode::GetElementPtr { .. }) {
            return pointer;
        }
        pointer = one.operands[0];
    };
    all.iter()
        .find(|(inst, _)| {
            let one = function.instruction(*inst);
            let pointer = match one.opcode {
                Opcode::Load { .. } => one.operands[0],
                Opcode::Store { .. } => one.operands[1],
                _ => return false,
            };
            matches!(root(pointer), Operand::Constant(id) if module.context.get(id).kind == ConstantKind::Global(target))
        })
        .map(|(_, reference)| reference)
        .unwrap_or_else(|| panic!("no access to @{name}"))
}

/// byref2: `PRINT Doubled(d)` reads the DOUBLE through the address the
/// FUNCTION answers, a register: that is its result cell, apart from `d`.
#[test]
fn an_address_in_a_register_keeps_its_object() {
    let module = raised("byref2-q-o.obj");
    let all = accesses(&module);
    let (result, d) = (access(&module, &all, "BC_DATA.0016"), access(&module, &all, "D#"));
    assert_eq!(overlapping(result, d, None, None, None), Ok(false));
}

/// harr: the element stored through the descriptor's selector is the
/// array's heap block, apart from `t` and from the descriptor, and `r`
/// and `c`, two carved variables, apart from each other.
#[test]
fn the_heap_element_is_apart_from_every_variable() {
    let module = raised("harr-q-o.obj");
    let all = accesses(&module);
    let element = all.iter().map(|(_, one)| one).find(|one| one.space == 1).expect("a far access");
    for variable in ["BC_DATA.0020", "BC_DATA.001c", "BC_DATA.0006"] {
        let other = access(&module, &all, variable);
        assert_eq!(overlapping(element, other, None, None, None), Ok(false), "the element and {variable}");
    }
    let (r, c) = (access(&module, &all, "BC_DATA.001c"), access(&module, &all, "BC_DATA.001e"));
    assert_eq!(overlapping(r, c, None, None, None), Ok(false));
}

/// qbdemo's RENDER: `DEF SEG = &HA000` is &HA000 stored where PEEK and
/// POKE read their segment, and GlobalsAA tracks that cell: B$ERAS writes
/// none of it, B$DSG0 does.
#[test]
fn def_seg_is_a_store_the_runtime_leaves_alone() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/inputs/omf/regressions/qbdemo-fil2.obj");
    let found = llrm_omf::module::load(&path).expect("reads").expect("an object");
    let raised = llrm_bc::raise_each(&found, &llrm_x86_code16::machine::BUILT_IN).expect("raises");
    assert!(raised.outcomes.iter().any(|(name, outcome)| name == "RENDER" && outcome.is_ok()));
    let program = llrm_mir::program::Program::new(vec![raised.module], std::rc::Rc::new(llrm_mir::target::Neutral)).and_then(|one| one.with_runtime(raised.runtime)).expect("links");
    let module = &program.modules[0];
    let cell = module.named("b$seg").expect("named");
    let render = module.global(module.named("RENDER").expect("raised")).function().expect("a function");
    let stored = render.walk().any(|(_, inst)| {
        let one = render.instruction(inst);
        matches!(one.opcode, Opcode::Store { .. })
            && matches!(one.operands[..], [Operand::Constant(value), Operand::Constant(pointer)]
                if module.context.get(value).kind == ConstantKind::Int(0xA000) && module.context.get(pointer).kind == ConstantKind::Global(cell))
    });
    assert!(stored, "{}", llrm_mir::print::module(module));
    let mut analyses = llrm_mir::passes::ModuleAnalyses::new(llrm_mir::program::ProgramAnalyses::default().proxy(&program, 0));
    let globals = llrm_analysis::globalsaa::analysis(module, &mut analyses).expect("analyzes");
    assert!(globals.tracked(cell));
    let writes = |routine: &str| globals.unsummarized(module.named(&format!("{}{routine}", llrm_bc::RUNTIME))).1.contains(&cell);
    assert!(!writes("B$ERAS") && writes("B$DSG0"));
}
