//! LLVM IR to selected machine nodes: clang's front end alone makes the IR, the
//! shared mid-end optimizes it, this backend selects it. Allocation and
//! emission are not here yet.

use std::rc::Rc;

pub use llrm_codegen::driver::flags::apply_pass_flag;
use llrm_mir::program::Program;
use llrm_target::Target;
use llrm_transforms::pipeline::{Applied, Options};

use crate::dagisel::function::{Selection, select_function_with};
use crate::dagisel::hooks::Cpu;
use crate::target::Z80;

/// How far the mid-end goes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    O0,
    O1,
    O2,
    O3,
    /// -Omax: `Options::aggressive()`.
    Omax,
    Os,
    Oz,
}

impl Level {
    pub fn parse(text: &str) -> Result<Self, String> {
        use llrm_transforms::pipeline::LevelName as Name;
        Ok(match Name::parse(text)? {
            Name::O0 => Self::O0,
            Name::O1 => Self::O1,
            Name::O2 => Self::O2,
            Name::O3 => Self::O3,
            Name::Omax => Self::Omax,
            Name::Os => Self::Os,
            Name::Oz => Self::Oz,
        })
    }

    fn options(self) -> Options {
        match self {
            Self::O0 => Options::none(),
            Self::O1 => Options::basic(),
            Self::O2 => Options::default(),
            Self::O3 => Options::speed(),
            Self::Omax => Options::aggressive(),
            Self::Os => Options::size(),
            Self::Oz => Options::min_size(),
        }
    }
}

/// Everything a compile is told besides the IR.
#[derive(Clone)]
pub struct Request {
    pub cpu: Cpu,
    pub level: Level,
    /// `-f[no-]PASS` switches, applied to the mid-end.
    pub passes: Vec<String>,
    /// `-g`: SDCC's `C$file$line$..` labels ahead of the instructions.
    pub debug: bool,
    /// DEVIATE Z80-SLOT-CONFLICT (owner z80, exit 2026-11-20): the spill slot
    /// sent back to the frame, where the others cannot all be pushes and pops,
    /// is the one in the most load conflicts, not the one loaded. The pin has
    /// no such walk (StackSlotColoring.cpp only shares frame slots). On by
    /// default: a third candidate the allocator prices beside the first
    /// violation's two, so it is taken only where cheaper.
    pub slot_by_conflicts: bool,
    /// The pin's block frequencies as they are, a loop of 10 trips weighing as
    /// a loop of 41 (DEVIATE Z80-BFI-TRIPS, `frequency.rs`, is the default).
    pub flat_loop_trips: bool,
    /// DEVIATE z80-oz-best-of (owner z80, exit 2026-12-15): at -Oz the module is compiled as -Oz, as -Os and as -Os
    /// without `minsize` (`optsize_only`) and the smallest is kept (module bytes, the runtime it reaches included; a tie keeps -Oz). The pin's -Oz is not
    /// smaller than its -Os here: 41 of 109 corpus rows grow (+4681 B), because header duplication is off at -Oz
    /// (LoopRotation.cpp#L80) and the inline threshold is 5 (inlinecost.rs `OPT_MIN_SIZE_THRESHOLD`), and the
    /// pin has no Z80 byte cost to say where either pays. The rotation that wins depends on what LSR and the unroll
    /// do after it, which no local price sees (three single-compile rules measured, none without regressions).
    /// Cost: -Oz compiles three times (the second compile measured median +34 ms, p90 +212 ms, max +6.6 s over the corpus). The real fix is the
    /// rotation/LSR interplay; the exit date is the revisit.
    pub oz_best_of: bool,
    /// DEVIATE Z80-SP-FRAME (owner z80, exit 2026-11-24): a function whose frame is addressed from SP is no larger and no
    /// slower than with IX (`sp_frame.rs`). Off, every framed function keeps IX.
    pub sp_frame: bool,
    /// The functions' `minsize` is dropped before the mid-end: the -Os compile `oz_best_of` weighs -Oz against. The
    /// attribute names a per-function level of its own (no peeled switch case, no balanced tree), so an -Os compile
    /// of a module clang built at -Oz was not -Os, and the best of the two never saw what -Os makes.
    pub optsize_only: bool,
}

impl Request {
    pub fn new(level: Level) -> Self {
        Self { cpu: Cpu::Z80, level, passes: Vec::new(), debug: false, slot_by_conflicts: true, flat_loop_trips: false, oz_best_of: true, sp_frame: true, optsize_only: false }
    }
}

/// `text` without the lines MIR's reader leaves out of LLVM (`target triple`,
/// comments) and `dso_local`.
fn without_triple(text: &str) -> String {
    text.lines()
        .filter(|line| !line.starts_with("target triple") && !line.starts_with(';'))
        .map(|line| line.replace(" dso_local", ""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `text` through the mid-end at the request's level.
pub fn program(
    text: &str,
    request: &Request,
) -> Result<Program, String> {
    let mut module = llrm_mir::parse::module(&without_triple(text)).map_err(|error| error.to_string())?;
    module.refuse_aliases("the z80")?;
    llrm_mir::parse::upgrade_lifetimes(&mut module);
    let layout = Z80.layout().datalayout;
    match module.datalayout.as_deref() {
        Some(stated) if stated == layout => {}
        Some(stated) => return Err(format!("the module's datalayout `{stated}` is not the Z80's `{layout}`")),
        None => return Err(format!("the module states no datalayout; the Z80's is `{layout}`")),
    }
    if request.optsize_only {
        for global in &mut module.globals {
            if let llrm_mir::module::GlobalKind::Function(function) = &mut global.kind {
                function.attrs.retain(|attr| !matches!(attr, llrm_mir::opcode::Attribute::Flag(one) if one == "minsize"));
            }
        }
    }
    if request.debug {
        llrm_mir::dbgimport::types_and_globals(&mut module);
    }
    let mut program = Program::new(vec![module], machine())?;
    if request.level != Level::O0 {
        let dump = std::env::var_os("LLRM_MIR_STAGES").filter(|one| !one.is_empty()).map(Into::into);
        let mut options = request.level.options();
        // No zero- or sign-extending load: the compare of a loaded byte stays a
        // byte compare (`cp #255`), not a word compare of its extension.
        options.extload = false;
        // The DAG builder plans the switch (clusters, table header), as the pin does.
        // DEVIATE Z80-SWITCH-ROWS (owner z80, exit 2026-11-15): against the
        // mid-end lowering, switch_jump_table Os +4 B, switch_table_holes Os
        // +2 B and Oz +82k T, compound_assignment_types Os/Oz +2 B. Layout
        // and allocation noise after the pin's own plan; no pass is wrong.
        options.lower_switches = false;
        // The runtime's `strlen` (`cpir`): LoopIdiomRecognize may form a call.
        // and `memcmp` (SDCC's libc has no `bcmp`).
        options.library = llrm_mir::libfunc::Routine::stated(["strlen", "memcmp"], |name| format!("_{name}"), 0);
        for flag in &request.passes {
            apply_pass_flag(&mut options, flag);
        }
        llrm_transforms::pipeline::applied(&mut program, &Applied { options, only: None, dump })?;
        program.modules.iter_mut().try_for_each(llrm_transforms::memsetpattern::expanded_stepping)?;
        program.modules.iter_mut().for_each(crate::prepare::mark_long_multiplies);
        program.modules.iter_mut().for_each(crate::prepare::mark_constant_fills);
        program.modules.iter_mut().for_each(crate::prepare::mark_wide_shifts);
        program.modules.iter_mut().for_each(crate::prepare::mark_wide_divides);
        if matches!(request.level, Level::O1 | Level::Os | Level::Oz) {
            program.modules.iter_mut().for_each(crate::prepare::mark_word_chains);
        }
    }
    // The pin lowers `llvm.is.constant` at every level; -O0 reaches here with the call.
    program.modules.iter_mut().try_for_each(llrm_transforms::lowerconstantintrinsics::module_lowered)?;
    Ok(program)
}

/// Every function `text` defines, selected: its name and the result, or why it
/// was refused.
pub fn selections(
    text: &str,
    request: &Request,
) -> Result<Vec<(String, Result<Selection, String>)>, String> {
    Ok(selections_both(text, request)?.into_iter().map(|(name, source, _)| (name, source)).collect())
}

/// [`selections`] with the DAG scheduled by `Source` and again by `ILP`
/// (DEVIATE Z80S1, in `dagisel/pipeline.rs`): the mid-end runs once.
pub fn selections_both(
    text: &str,
    request: &Request,
) -> Result<Vec<Selected>, String> {
    Ok(selections_with_alternatives(text, request)?.0)
}

/// A function's name and its selection scheduled by `Source` and by `ILP`.
pub type Selected = (String, Result<Selection, String>, Result<Selection, String>);

/// [`selections_both`], and again the functions that call `__mulsi3` for
/// their long multiplies, selected with the multiplies by `2^k +- 1` left as
/// shifts and an add: the caller keeps whichever form is smaller in the
/// allocated function (`compile::lines_in`).
pub fn selections_with_alternatives(
    text: &str,
    request: &Request,
) -> Result<(Vec<Selected>, Vec<Selected>), String> {
    let mut program = program(text, request)?;
    program.modules.iter_mut().for_each(crate::prepare::split_phi_edges);
    let module = &program.modules[0];
    let names: Vec<String> = module
        .globals
        .iter()
        .filter(|global| global.function().is_some_and(|one| !one.is_declaration()))
        .filter_map(|global| global.name.as_deref().map(str::to_owned))
        .collect();
    let select_form = |name: &str, ilp: bool| {
        crate::dagisel::pipeline::ILP.with(|one| one.set(ilp));
        crate::dagisel::function::FLAT_LOOP_TRIPS.with(|one| one.set(request.flat_loop_trips));
        crate::dagisel::pipeline::STRICT_LEVEL.with(|one| one.set(matches!(request.level, Level::O1 | Level::Os | Level::Oz)));
        let one = select_function_with(module, name, request.cpu, request.level != Level::O0);
        crate::dagisel::pipeline::STRICT_LEVEL.with(|one| one.set(false));
        crate::dagisel::pipeline::ILP.with(|one| one.set(false));
        one
    };
    // A form whose long adds became carry ops is also selected with them left to the generic expansion, kept on the
    // selection (`Selection::generic`): the allocated function is the cheaper of the two (`compile::lines_of`).
    // DEVIATE Z80-CARRY-PRICE (owner z80, exit 2026-11-24): the pin's `ExpandIntRes_ADDSUB` takes the carry ops
    // whenever `UADDO_CARRY` is legal and prices nothing; `adc hl, rr` ties each sum to `hl`, which costs spills
    // where several longs are live (register_upper_half -Os +28 B; call_keeps -O1 all-calls form +65 B).
    let select = |name: &str, ilp: bool| {
        crate::dagisel::pipeline::CARRY_OPS.with(|count| count.set(0));
        let mut one = select_form(name, ilp);
        if crate::dagisel::pipeline::CARRY_OPS.with(std::cell::Cell::get) > 0 {
            crate::dagisel::pipeline::GENERIC_CARRY.with(|flag| flag.set(true));
            let generic = select_form(name, ilp);
            crate::dagisel::pipeline::GENERIC_CARRY.with(|flag| flag.set(false));
            if let (Ok(one), Ok(generic)) = (&mut one, generic) {
                one.generic = Some(Box::new(generic));
            }
        }
        one
    };
    // A function with the routine for its long multiplies (`prepare::mark_long_multiplies`) is also selected the
    // other way: a sized one with every multiply inline (`DECOMPOSE_MULS`), an unsized one (at -O1 only) with every multiply a
    // call (`CALL_MULS`); the caller prices the two (`compile::decomposed_functions`).
    let calls_the_routine = |name: &str| -> Option<bool> {
        module.globals.iter().find_map(|global| {
            let one = global.function().filter(|_| global.name.as_deref() == Some(name))?;
            llrm_mir::memory::has(&one.attrs, crate::prepare::LONG_MULTIPLY).then(|| {
                ["optsize", "minsize"].into_iter().any(|attr| llrm_mir::memory::has(&one.attrs, attr))
            })
        })
    };
    // A function with a constant-count `memset` (`prepare::mark_constant_fills`) is also selected with each such
    // fill an inline `ldir` fill (`INLINE_FILLS`); the caller prices the two with the routine linked once.
    let has_fills = |name: &str| -> bool {
        module.globals.iter().any(|global| {
            global.function().is_some_and(|one| {
                global.name.as_deref() == Some(name) && llrm_mir::memory::has(&one.attrs, crate::prepare::CONSTANT_FILL)
            })
        })
    };
    // A function with a variable 64-bit shift (`prepare::mark_wide_shifts`) is
    // also selected with each such shift an in-place loop
    // (`INLINE_SHIFTS`); the caller prices the two with the routine linked
    // once.
    let has_shifts = |name: &str| -> bool {
        module.globals.iter().any(|global| {
            global.function().is_some_and(|one| {
                global.name.as_deref() == Some(name) && llrm_mir::memory::has(&one.attrs, crate::prepare::WIDE_SHIFT)
            })
        })
    };
    // A function with a long divide (`prepare::mark_wide_divides`) is also selected with the speed routines in a
    // function tuned for size (`SPEED_ROUTINES`).
    let has_divides = |name: &str| -> bool {
        module.globals.iter().any(|global| {
            global.function().is_some_and(|one| {
                global.name.as_deref() == Some(name) && llrm_mir::memory::has(&one.attrs, crate::prepare::WIDE_DIVIDE)
            })
        })
    };
    let main: Vec<Selected> = names.iter().map(|name| (name.clone(), select(name, false), select(name, true))).collect();
    let alternatives: Vec<Selected> = names
        .iter()
        .filter_map(|name| {
            // The all-calls form is priced by relative regret at -O1 only. Above it the rate (1 B = 3 T) needs the
            // clocks in absolute terms, and the static loop weights run about 5x under the measured ones (call_keeps:
            // 11124 priced, 57813 run), which made -368 B for +5970 T look like a gain.
            let muls = calls_the_routine(name).filter(|sized| *sized || request.level == Level::O1);
            let fills = has_fills(name);
            let shifts = has_shifts(name);
            let divides = has_divides(name);
            if muls.is_none() && !fills && !shifts && !divides {
                return None;
            }
            let switch = muls.map(|sized| {
                if sized { &crate::dagisel::pipeline::DECOMPOSE_MULS } else { &crate::dagisel::pipeline::CALL_MULS }
            });
            if let Some(switch) = switch {
                switch.with(|one| one.set(true));
            }
            crate::dagisel::pipeline::INLINE_FILLS.with(|one| one.set(fills));
            crate::dagisel::pipeline::INLINE_SHIFTS.with(|one| one.set(shifts));
            // the alternative is one switch for every reason a function has one: the speed routines ride along only where
            // nothing else is switched, or the form that decomposes a multiply would also carry the divide's price
            crate::dagisel::pipeline::SPEED_ROUTINES.with(|one| one.set(divides && muls.is_none() && !fills && !shifts));
            let selected = (name.clone(), select(name, false), select(name, true));
            crate::dagisel::pipeline::INLINE_FILLS.with(|one| one.set(false));
            crate::dagisel::pipeline::INLINE_SHIFTS.with(|one| one.set(false));
            crate::dagisel::pipeline::SPEED_ROUTINES.with(|one| one.set(false));
            if let Some(switch) = switch {
                switch.with(|one| one.set(false));
            }
            Some(selected)
        })
        .collect();
    Ok((main, alternatives))
}

/// The cost model the mid-end reads: the pin's `opt` with no target, whose
/// `Basic` TargetTransformInfo prices the loop-idiom, inline and
/// strength-reduction decisions.
pub fn machine() -> Rc<dyn llrm_mir::target::Machine> {
    Rc::new(Z80Machine)
}

/// `opcosts.txt`.
pub static DESCRIPTION: std::sync::LazyLock<llrm_target::opcosts::Description> = std::sync::LazyLock::new(|| {
    llrm_target::opcosts::Description::parse(include_str!("opcosts.txt")).expect("opcosts.txt parses")
});

struct Z80Machine;

impl llrm_mir::target::Machine for Z80Machine {
    /// The Z80 shifts by one bit per instruction: a variable count is `__ashlhi3`.
    fn variable_shift_legal(&self) -> bool {
        false
    }

    fn foreign_span(
        &self,
        selectors: (i64, i64),
        offsets: (i64, i64),
        width: i64,
    ) -> Option<(i64, i64)> {
        llrm_mir::target::Neutral.foreign_span(selectors, offsets, width)
    }

    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        llrm_mir::target::Neutral.spaces()
    }

    fn costs(&self) -> llrm_mir::target::OperationCosts {
        crate::clocks::sheet()
    }

    /// One unit an operation, as `costs()` was before it stated T-states: the pin's thresholds read instruction
    /// units, and the size decisions defaulted to `costs()` (-Os grew copy_runs +362 B reading T-states as bytes).
    fn size_costs(&self) -> llrm_mir::target::OperationCosts {
        llrm_mir::target::OperationCosts { jump_table: 4, ..llrm_mir::target::Neutral.costs() }
    }

    /// Real code bytes for the decisions weighed at a rate (`opcosts.txt`).
    fn byte_costs(&self) -> llrm_mir::target::OperationCosts {
        DESCRIPTION.size_costs()
    }

    fn registers(&self) -> i64 {
        llrm_mir::target::Neutral.registers()
    }

    fn call_registers(&self) -> i64 {
        llrm_mir::target::Neutral.call_registers()
    }

    fn address_forms(&self) -> Vec<llrm_mir::target::AddressForm> {
        llrm_mir::target::Neutral.address_forms()
    }

    fn tti(&self) -> Option<Rc<dyn llrm_mir::tti::Tti>> {
        Some(Rc::new(crate::tti::Z80Tti::new()))
    }

    fn kept_stack_align(&self) -> Option<u64> {
        Some(crate::KEPT_STACK_ALIGN)
    }

    fn max_alloca_align(&self) -> Option<u64> {
        Some(crate::MAX_ALLOCA_ALIGN)
    }

    fn tuning(&self) -> llrm_mir::target::Tuning {
        llrm_mir::target::Tuning {
            max_stores_per_memcpy: MEMCPY_STORES,
            max_stores_per_memcpy_opt_size: MEMCPY_STORES,
            max_stores_per_memset: memset_stores(llrm_mir::target::Tuning::PIN.max_stores_per_memset),
            max_stores_per_memset_opt_size: memset_stores(llrm_mir::target::Tuning::PIN.max_stores_per_memset_opt_size),
            ..llrm_mir::target::Tuning::PIN
        }
    }
}

/// `MaxStoresPerMemcpy` (and `...OptSize`): the word copies a constant `memcpy` may be before it is an `ldir`.
///
/// DEVIATE Z80-MEMCPY-LIMIT (owner z80-argpush, exit 2026-11-30): the pin's x86 default is 8 at speed and 4 at size
/// (`Tuning::PIN`, `TargetLoweringBase`), where a copied word is a load and a store with a displacement. Here a word
/// is 8 B / 52 T through `hl` and `de` and 12 B / 76 T between frame slots, against an `ldir` block of 11 B to 20 B
/// and 21 T a byte: one word is cheaper in both bytes and clocks, two are dearer in both (`tests/memcpy_limit.rs`
/// prices it). Reason: variadic copied a 12 B struct argument twice as 6 word pairs, 72 B each. Exit: none; the number
/// is the target's price, as m16's `Dos::tuning` is.
const MEMCPY_STORES: usize = 1;

/// `MaxStoresPerMemset` where the level allows no trade between bytes and clocks (-O1, -Os, -Oz): one store of the widest
/// width, then the fill is an `ldir`. The pin's x86 limits (16 at speed, 8 at size) count 4-byte stores, which are two
/// word stores of 3 B each through IX here: a 34 B zero fill was 102 B against an `ldir` fill of 13 B for 1.1% of its
/// clocks (huffman_decode -O1 +70 B over the -Os form). Where bytes are priced at 3 T (-O2, -O3) the two forms are within
/// the noise of that rate and the pin's limit stays.
///
/// DEVIATE Z80-MEMSET-LIMIT (owner z80-pf, exit 2026-11-24), as Z80-MEMCPY-LIMIT.
const MEMSET_STORES: usize = 1;

fn memset_stores(pin: usize) -> usize {
    if crate::dagisel::pipeline::STRICT_LEVEL.with(std::cell::Cell::get) { MEMSET_STORES } else { pin }
}

thread_local! {
    /// Parses of a module's text for its source file since the last
    /// [`take_source_parses`].
    static SOURCE_PARSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The parses made by [`source_of`] and [`sources_of`] on this thread since
/// the last call.
pub fn take_source_parses() -> usize {
    SOURCE_PARSES.with(|one| one.replace(0))
}

/// The source file of the unit's `!DICompileUnit` and the line function `name`
/// starts at (its `!DISubprogram`'s `scopeLine`), if `text` has them.
pub fn source_of(
    text: &str,
    name: &str,
) -> Option<(String, Option<u32>)> {
    SOURCE_PARSES.with(|one| one.set(one.get() + 1));
    let module = llrm_mir::parse::module(&without_triple(text)).ok()?;
    let (file, _) = llrm_mir::dbgimport::unit_file(&module)?;
    let line = module.named(name).and_then(|id| module.subprogram_line(module.global(id).function()?));
    Some((file, line))
}

/// [`source_of`] for every function of the module, from one parse.
pub fn sources_of(text: &str) -> llrm_support::hash::HashMap<String, (String, Option<u32>)> {
    SOURCE_PARSES.with(|one| one.set(one.get() + 1));
    let mut out = llrm_support::hash::HashMap::default();
    let Ok(module) = llrm_mir::parse::module(&without_triple(text)) else { return out };
    let Some((file, _)) = llrm_mir::dbgimport::unit_file(&module) else { return out };
    for global in &module.globals {
        let Some(name) = global.name.as_deref() else { continue };
        let Some(function) = global.function() else { continue };
        out.insert(name.to_owned(), (file.clone(), module.subprogram_line(function)));
    }
    out
}

/// SDCC's size and spelling of a debug type, as the `.adb` writes it: the class
/// and then the sign (`SI:S` int, `SI:U` unsigned int, `SC:U` char, `DG,SC:U`
/// pointer to char); none for one it has no spelling for.
pub(crate) fn sdcc_type(
    module: &llrm_mir::module::Module,
    id: llrm_mir::module::MetadataId,
) -> Option<(i64, String)> {
    use llrm_mir::debuginfo::{Kind, Scalar, read_type};
    let one = read_type(module, id)?;
    match one.kind {
        Kind::Scalar => {
            let (size, class, sign) = match Scalar::from_value(&one.name)? {
                Scalar::Void => (0, "SV", "S"),
                // DW_ATE_signed_char says signed, whatever SDCC's plain char is
                Scalar::Char | Scalar::Int8 => (1, "SC", "S"),
                Scalar::UInt8 => (1, "SC", "U"),
                Scalar::Int16 => (2, "SI", "S"),
                Scalar::UInt16 => (2, "SI", "U"),
                Scalar::Int32 => (4, "SL", "S"),
                Scalar::UInt32 => (4, "SL", "U"),
                Scalar::Float32 => (4, "SF", "S"),
                _ => return None,
            };
            Some((size, format!("{class}:{sign}")))
        }
        Kind::Pointer => Some((2, format!("DG,{}", sdcc_type(module, one.target?)?.1))),
        _ => None,
    }
}

/// The module's `.adb` (ASxxxx debug records, which sdld merges into the
/// `.cdb`): the module, then each defined function with its result type.
/// None without debug info. The lines come from the `C$` symbols.
pub fn adb(
    text: &str,
    request: &Request,
) -> Option<String> {
    use llrm_spike_mf::VariablePlace;
    let mut program = program(text, request).ok()?;
    program.modules.iter_mut().for_each(crate::prepare::split_phi_edges);
    let module = &program.modules[0];
    let (file, _) = llrm_mir::dbgimport::unit_file(module)?;
    let stem = file.rsplit_once('.').map_or(file.as_str(), |(stem, _)| stem);
    let mut out = format!("M:{stem}\n");
    for one in llrm_mir::debuginfo::functions(module) {
        let procedure = llrm_mir::debuginfo::read_type(module, one.r#type)?;
        let result = match procedure.target {
            Some(result) => sdcc_type(module, result)?.1,
            None => "SV:S".to_owned(),
        };
        let size = 2;
        out.push_str(&format!("F:G${}$0_0$0({{{size}}}DF,{result}),C,0,0,0,0,0\n", one.function));
        out.push_str(&format!("S:G${}$0_0$0({{{size}}}DF,{result}),C,0,0\n", one.function));
        let Ok(selection) = select_function_with(module, &one.function, request.cpu, request.level != Level::O0) else {
            continue;
        };
        // Frame objects sit below IX, the locals first (`expand::layout`).
        let mut below = 0;
        let ends: Vec<i64> = selection
            .locals
            .iter()
            .map(|&bytes| {
                below += bytes;
                below
            })
            .collect();
        for (at, variable) in selection.variables.iter().enumerate() {
            let Some((size, spelling)) = sdcc_type(module, variable.r#type) else { continue };
            let place = match variable.place {
                VariablePlace::Register(register) => format!("R,0,0,{}", crate::expand::register_list(register)),
                VariablePlace::Frame { index, offset } => {
                    format!("B,1,{}", offset - ends.get(index as usize).copied().unwrap_or(0))
                }
            };
            out.push_str(&format!(
                "S:L{stem}.{}${}$1_0${at}({{{size}}}{spelling}),{place}\n",
                one.function, variable.name
            ));
        }
    }
    // A function the module calls but does not define: a symbol of its own.
    for one in llrm_mir::debuginfo::declarations(module) {
        let procedure = llrm_mir::debuginfo::read_type(module, one.r#type)?;
        let result = match procedure.target {
            Some(result) => sdcc_type(module, result)?.1,
            None => "SV:S".to_owned(),
        };
        out.push_str(&format!("S:G${}$0_0$0({{2}}DF,{result}),C,0,0\n", one.function));
    }
    Some(out)
}
