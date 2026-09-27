//! The raise, checked by running what it produces: `llrm_mir::interpret`
//! against each program's BASIC meaning, derived by hand.

use llrm_mir::interpret::{self, Val};
use llrm_mir::{BinaryOp, CastOp, Constant, ConstantKind, GlobalKind, GlobalVariable, Linkage, Module, Opcode, Operand, Position, Type};

fn raised(fixture: &str) -> Module {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/omf").join(fixture);
    let found = llrm_omf::module::load(&path).expect("reads").expect("an object");
    let module = llrm_bc::raise(&found).unwrap_or_else(|refusal| panic!("{refusal}"));
    let errors = llrm_mir::verify::verify(&module);
    assert!(errors.is_empty(), "{errors:#?}\n{}", llrm_mir::print::module(&module));
    module
}

/// A global `@name` of `bits`, holding `value`.
fn cell(module: &mut Module, name: &str, bits: u32, value: i128) -> Operand {
    let ty = module.context.types.int(bits);
    let initializer = module.context.int(ty, value);
    let global = module.add_variable(name, GlobalVariable { ty, constant: false, initializer: Some(initializer), align: None }, Linkage::Internal).expect("free");
    Operand::Constant(module.reference(global))
}

/// Gives each runtime routine a body that returns nothing, and makes the
/// end of the program a return. Each number printed, `B$P?I2`, or
/// `B$P?I4` and `B$P?R4` (a LONG or a SINGLE's bits, pushed high word
/// first), goes to the next word of `@printed`.
fn stub_runtime(module: &mut Module) {
    let long = module.context.types.int(32);
    let row = module.context.types.intern(Type::Array { element: long, count: 64 });
    let zero = module.context.constant(Constant { ty: row, kind: ConstantKind::Zero });
    let printed = module.add_variable("printed", GlobalVariable { ty: row, constant: false, initializer: Some(zero), align: None }, Linkage::Internal).expect("free");
    let printed = Operand::Constant(module.reference(printed));
    let count = cell(module, "count", 16, 0);
    let routines: Vec<_> = module.functions().filter(|(_, global, function)| function.is_declaration() && global.name.as_deref().is_some_and(|name| name.starts_with(llrm_bc::RUNTIME))).map(|(id, global, _)| (id, global.name.clone().unwrap())).collect();
    for (id, name) in routines {
        let returns = match &module.globals[id.0 as usize].kind {
            GlobalKind::Function(function) => module.signature(function.ty).0,
            _ => unreachable!(),
        };
        let routine = name.trim_start_matches(llrm_bc::RUNTIME);
        let mut b = module.builder(id);
        let entry = b.block("entry");
        b.position(entry);
        let value = match (routine.starts_with("B$P") && routine.len() == 6, &routine[4.min(routine.len())..]) {
            (true, "I2") => {
                let word = b.parameter(0);
                Some(b.cast(CastOp::SExt, word, long, ""))
            }
            (true, "I4" | "R4") => {
                let (high, low) = (b.parameter(0), b.parameter(1));
                let high = b.cast(CastOp::ZExt, high, long, "");
                let low = b.cast(CastOp::ZExt, low, long, "");
                let sixteen = b.int(32, 16);
                let high = b.binary(BinaryOp::Shl, high, sixteen, Default::default(), "");
                Some(b.binary(BinaryOp::Or, high, low, Default::default(), ""))
            }
            _ => None,
        };
        if let Some(value) = value {
            let word = b.context.types.int(16);
            let at = b.load(word, count, false, "");
            let slot = b.gep(long, printed, &[at], Default::default(), "");
            b.store(value, slot, false);
            let one = b.int(16, 1);
            let next = b.binary(BinaryOp::Add, at, one, Default::default(), "");
            b.store(next, count, false);
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
}

/// Every integer `main` prints, stubbed as `stub_runtime` does.
fn printed(fixture: &str, count: usize) -> Vec<i64> {
    let mut module = raised(fixture);
    stub_runtime(&mut module);
    let (main, ty) = function(&mut module, "main");
    let row = module.named("printed").expect("stubbed");
    let row = Operand::Constant(module.reference(row));
    let word = module.context.types.int(16);
    let long = module.context.types.int(32);
    let fn_ty = module.context.types.intern(Type::Function { returns: long, parameters: vec![word], variadic: false });
    let id = module.add_function("probe", fn_ty, Linkage::External).expect("free");
    let mut b = module.builder(id);
    let entry = b.block("entry");
    b.position(entry);
    b.call(ty, Operand::Constant(main), &[], "");
    let slot = b.gep(long, row, &[b.parameter(0)], Default::default(), "");
    let value = b.load(long, slot, false, "");
    b.ret(Some(value));
    (0..count)
        .map(|at| {
            let answer = interpret::run(&module, "probe", vec![Val::Int { bits: at as u128, width: 16 }], 10_000_000).unwrap_or_else(|trap| panic!("{trap:?}\n{}", llrm_mir::print::module(&module)));
            int(&answer) as i32 as i64
        })
        .collect()
}

/// `@probe`, returning what `body` builds.
fn probe(module: &mut Module, returns: u32, body: impl FnOnce(&mut llrm_mir::build::Builder) -> Operand) -> Val {
    let ty = module.context.types.int(returns);
    let fn_ty = module.context.types.intern(Type::Function { returns: ty, parameters: vec![], variadic: false });
    let id = module.add_function("probe", fn_ty, Linkage::External).expect("free");
    let mut b = module.builder(id);
    let entry = b.block("entry");
    b.position(entry);
    let value = body(&mut b);
    b.ret(Some(value));
    interpret::run(module, "probe", vec![], 10_000_000).unwrap_or_else(|trap| panic!("{trap:?}\n{}", llrm_mir::print::module(module)))
}

/// A function's reference and type.
fn function(module: &mut Module, name: &str) -> (llrm_mir::ConstantId, llrm_mir::TypeId) {
    let id = module.named(name).expect("raised");
    let GlobalKind::Function(function) = &module.globals[id.0 as usize].kind else { unreachable!() };
    let ty = function.ty;
    (module.reference(id), ty)
}

fn int(value: &Val) -> i64 {
    match value {
        Val::Int { bits, width } => ((*bits as i64) << (64 - width)) >> (64 - width),
        other => panic!("{other:?}"),
    }
}

/// Each compiler's procs: QB, QB without CodeView (/Zd, whose FUNCTION
/// answers in what its callers read), VBDOS, and VBDOS /G3's 32-bit code.
const PROCS: [&str; 4] = ["procs-q-o.obj", "procs-q-o-zd.obj", "procs-v-g2.obj", "procs-v-g3.obj"];

/// `FUNCTION Twice& (n AS LONG)`: `n + n`, a carry crossing its words, BYREF.
#[test]
fn twice_adds_a_long_through_its_carry() {
    for (fixture, (n, expected)) in PROCS.iter().flat_map(|one| [(0x1234_5678i64, 0x2468_ACF0i64), (0xFFFF, 0x1_FFFE), (-3, -6)].map(|case| (one, case))) {
        let mut module = raised(fixture);
        let n = cell(&mut module, "n", 32, i128::from(n));
        let (reference, ty) = function(&mut module, "TWICE");
        let answer = probe(&mut module, 32, |b| {
            let word = b.context.types.int(16);
            let address = b.cast(CastOp::PtrToInt, n, word, "");
            b.call_as(llrm_mir::opcode::BASIC, ty, Operand::Constant(reference), &[address], "").expect("an answer")
        });
        assert_eq!(int(&answer) as i32 as i64, expected, "{fixture}: Twice({n:?})");
    }
}

/// `SUB Report` answers nothing, though its callers go on to write AX: a
/// write of AX is no read of EAX's high word, nor of AX.
#[test]
fn a_sub_answers_nothing() {
    for fixture in PROCS {
        let mut module = raised(fixture);
        let (_, ty) = function(&mut module, "REPORT");
        let void = module.context.types.void();
        assert_eq!(module.signature(ty).0, void, "{fixture}");
    }
}

/// rcflip's RAMP: nested loops, IFs on comparisons, `\\` and an array.
/// SUM=1769558, from the BASIC source by hand (`\\` truncates toward zero).
#[test]
fn ramp_prints_its_sum() {
    let mut module = raised("rcflip-q-o.obj");
    stub_runtime(&mut module);
    let (reference, ty) = function(&mut module, "RAMP");
    let row = module.named("printed").expect("stubbed");
    let row = Operand::Constant(module.reference(row));
    let answer = probe(&mut module, 32, |b| {
        b.call_as(llrm_mir::opcode::BASIC, ty, Operand::Constant(reference), &[], "");
        let long = b.context.types.int(32);
        b.load(long, row, false, "")
    });
    assert_eq!(int(&answer), 1_769_558);
}

/// cmpord: B$CPI4 on four pairs, each first below second, both ways round:
/// < <= > >= = <> give -1 0, -1 0, 0 -1, 0 -1, 0 0, -1 -1.
#[test]
fn long_comparisons_order_their_operands() {
    let pair = [-1, 0, -1, 0, 0, -1, 0, -1, 0, 0, -1, -1];
    let expected: Vec<i64> = pair.iter().copied().cycle().take(48).collect();
    assert_eq!(printed("cmpord-q-o.obj", 48), expected);
}

/// lngmix: ten times 100000 \ 7 + 100000 MOD 7 is 10 * (14285 + 5).
#[test]
fn long_divide_and_remainder_sum() {
    assert_eq!(printed("lngmix-q-o.obj", 1), vec![142_900]);
}

/// fpemu: the x87 stack as values, `B$FILD`, `B$FIST` and `B$FCMP`, from
/// the BASIC source: q = 1073741831 \\ 1024, a = q, b = 1024, s = q, t = 4.
#[test]
fn fpemu_prints_its_float_results() {
    let expected = vec![1_048_576, 1_049_600, 1_047_552, 7, 1_073_741_824, 1024, 5, 1024, 1_048_580, 4_194_304, 1_073_741_824, -1, 0];
    for fixture in ["fpemu-p-g2.obj", "fpemu-p-g2-zd.obj", "fpemu-v-g2.obj", "fpemu-v-g3.obj"] {
        assert_eq!(printed(fixture, expected.len()), expected, "{fixture}");
    }
}

/// fpdeep: `p(i) * p(i)` holds two values from one address, and `fdivp`
/// writes st(1) before the pop renumbers it. p = 12, 28, 60; k = 4; d = 12.
#[test]
fn fpdeep_keeps_two_values_on_the_stack() {
    let expected = vec![1, 144, 1, 6, 1, 512, 2, 784, 2, 14, 2, 768, 3, 3600, 3, 30, 3, 896, 144, 6];
    for fixture in ["fpdeep-q-o.obj", "fpdeep-q-noo.obj"] {
        assert_eq!(printed(fixture, expected.len()), expected, "{fixture}");
    }
}

/// fpcse: ten times (2 + 4) * 8 + (2 + 4) / 8 is S = 487.5, a SINGLE
/// printed by `B$PER4` as its bits.
#[test]
fn fpcse_prints_its_single_sum() {
    for fixture in ["fpcse-q-o.obj", "fpcse-p-g2.obj", "fpcse-v-g3.obj"] {
        assert_eq!(printed(fixture, 1), vec![i64::from(487.5f32.to_bits() as i32)], "{fixture}");
    }
}
