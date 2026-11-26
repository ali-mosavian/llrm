//! LLVM IR to ca65 text: `llc`'s job over `opt`'s. Clang's front end alone
//! makes the IR; the shared mid-end optimizes it; this backend selects,
//! allocates and prints it. The runtime (`runtime/mos_rt.s`) is linked, not
//! printed.

use std::rc::Rc;

pub use llrm_codegen::driver::flags::apply_pass_flag;
use llrm_mir::program::Program;
use llrm_target::Target;
use llrm_transforms::pipeline::{Applied, Options};

use crate::alloc::Tuning;
use crate::dagisel::function::select_function;
use crate::dagisel::hooks::Cpu;
use crate::emit::equates;
use crate::platform::{Platform, SIM65};
use crate::select::module_text;
use crate::target::Mos;

/// How far the mid-end goes; `-Os` and `-Oz` also price a byte at the size
/// level's clocks (`Tuning::SIZE`).
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
        Options { backward_threading: false, ..self.mid_end() }
    }

    fn mid_end(self) -> Options {
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

    pub(crate) fn tuning(self) -> Tuning {
        match self {
            Self::Os | Self::Oz => Tuning::SIZE,
            _ => Tuning::SPEED,
        }
    }
}

/// The platform a row of `ROWS` names, or sim65.
pub fn platform_named(name: &str) -> Result<Platform, String> {
    let rows = std::iter::once(SIM65).chain(crate::platform::ROWS);
    rows.filter_map(|text| Platform::parse(text).ok())
        .find(|one| one.name == name)
        .ok_or_else(|| format!("no platform {name}"))
}

/// Everything a compile is told besides the IR.
pub struct Request {
    pub platform: Platform,
    pub cpu: Cpu,
    pub level: Level,
    /// `-f[no-]PASS` switches, applied to the mid-end.
    pub passes: Vec<String>,
    /// `-g`: `.dbg line` directives for ld65's debug file.
    pub debug: bool,
}

impl Request {
    pub fn new(level: Level) -> Self {
        Self {
            platform: Platform::parse(SIM65).expect("sim65 parses"),
            cpu: Cpu::Mos6502,
            level,
            passes: Vec::new(),
            debug: false,
        }
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

/// The words ca65 reads as a register, an opcode or one of our `rcN` equates
/// wherever they stand as an operand or a label.
const RESERVED: &[&str] = &[
    "a", "f", "x", "y", "s", "z", "adc", "and", "asl", "bbr", "bbs", "bcc", "bcs", "beq", "bit", "bmi", "bne", "bpl",
    "bra", "brk", "bvc", "bvs", "clc", "cld", "cli", "clv", "cmp", "cpx", "cpy", "dea", "dec", "dex", "dey", "eor",
    "ina", "inc", "inx", "iny", "jmp", "jsr", "lda", "ldx", "ldy", "lsr", "nop", "ora", "pha", "php", "phx", "phy",
    "pla", "plp", "plx", "ply", "rmb", "rol", "ror", "rti", "rts", "sbc", "sec", "sed", "sei", "smb", "sta", "stp",
    "stx", "sty", "stz", "tax", "tay", "trb", "tsb", "tsx", "txa", "txs", "tya", "wai",
];

/// Whether ca65 would read the symbol `name` as something else.
fn reserved(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let numbered =
        |stem: &str| lower.strip_prefix(stem).is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    RESERVED.contains(&lower.as_str())
        || numbered("rc")
        || numbered("bbr")
        || numbered("bbs")
        || numbered("rmb")
        || numbered("smb")
}

/// `text` with every `@name` that `reserved` prefixed by `__r_`, a name C code
/// cannot spell, so the assembly names the symbol and not the opcode; where
/// the platform has a `c_symbol_prefix`, every C symbol takes that instead
/// (intrinsics, `llvm.*`, and the private `.str` names are not C symbols). A
/// string that is not a symbol's quoted name (`c"user@x"`) is copied as it is.
fn without_reserved_symbols(
    text: &str,
    c_symbol_prefix: &str,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(['@', '"']) {
        let (before, here) = rest.split_at(at);
        out.push_str(before);
        let quoted_name = here.starts_with('@') && here[1..].starts_with('"');
        if here.starts_with('"') {
            let close = here[1..].find('"').map_or(here.len(), |n| n + 2);
            out.push_str(&here[..close]);
            rest = &here[close..];
            continue;
        }
        out.push('@');
        let body = &here[1..];
        let (quote, body) = if quoted_name { (true, &body[1..]) } else { (false, body) };
        let end = if quote {
            body.find('"').unwrap_or(body.len())
        } else {
            body.find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '$' | '-')))
                .unwrap_or(body.len())
        };
        if quote {
            out.push('"');
        }
        let name = &body[..end];
        if !c_symbol_prefix.is_empty() && !name.starts_with("llvm.") && !name.starts_with('.') {
            out.push_str(c_symbol_prefix);
        } else if reserved(name) {
            out.push_str("__r_");
        }
        rest = body;
    }
    out.push_str(rest);
    out
}

/// `-fwhole-program`: the module is the whole link but its runtime.
fn whole_program(request: &Request) -> bool {
    request.passes.iter().rev().find_map(|flag| match flag.as_str() {
        "-fwhole-program" => Some(true),
        "-fno-whole-program" => Some(false),
        _ => None,
    }) == Some(true)
}

/// What LTO does to a whole program before its optimizer runs:
/// everything but `main` becomes internal (`Internalize`), so `GlobalOpt` can
/// make a global nothing stores to a constant and fold its loads.
fn whole_program_internalized(
    program: &mut Program,
    main: &str,
) -> Result<(), String> {
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add_program(llrm_transforms::internalize::Internalize::new([main.to_owned()]));
    manager.run(program).map(|_| ())
}

/// What a linker's `--gc-sections` does for a whole program: what nothing
/// live names is deleted (`GlobalDCE`), after the mid-end.
fn whole_program_swept(program: &mut Program) -> Result<(), String> {
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add_program(llrm_transforms::globaldce::GlobalDce);
    manager.run(program).map(|_| ())
}

/// `text` through the mid-end at the request's level.
fn program(
    text: &str,
    request: &Request,
) -> Result<Program, String> {
    // a private function takes arguments in rc16..rc25 (`calling.toml`
    // `[mos_private]`), where the platform has them
    crate::abi::allow_private_row(request.platform.zero_page >= crate::alloc::MOST_CELLS);
    MID_ENDS.with(|count| count.set(count.get() + 1));
    let mut module =
        llrm_mir::parse::module(&without_reserved_symbols(&without_triple(text), &request.platform.c_symbol_prefix))
            .map_err(|error| error.to_string())?;
    module.refuse_aliases("the 6502")?;
    llrm_mir::parse::upgrade_lifetimes(&mut module);
    let layout = Mos.layout().datalayout;
    match module.datalayout.as_deref() {
        Some(stated) if stated == layout => {}
        Some(stated) => return Err(format!("the module's datalayout `{stated}` is not the 6502's `{layout}`")),
        None => return Err(format!("the module states no datalayout; the 6502's is `{layout}`")),
    }
    let mut program = Program::new(vec![module], machine())?;
    let main = format!("{}main", request.platform.c_symbol_prefix);
    if whole_program(request) {
        whole_program_internalized(&mut program, &main)?;
    }
    if request.level != Level::O0 {
        let dump = std::env::var_os("LLRM_MIR_STAGES").filter(|one| !one.is_empty()).map(Into::into);
        let mut options = request.level.options();
        options.second_inline = !request.passes.iter().any(|flag| flag == "-fno-second-inline")
            && std::env::var_os("LLRM_6502_NO_ROUND2").is_none();
        for flag in &request.passes {
            apply_pass_flag(&mut options, flag);
        }
        llrm_transforms::pipeline::applied(&mut program, &Applied { options, only: None, dump })?;
        // the 6502 has no scaled or indexed 16-bit address: a stepping pointer,
        // not `base + i * size` every trip
        program.modules.iter_mut().try_for_each(llrm_transforms::memsetpattern::expanded_stepping)?;
        let static_locals = !request.passes.iter().any(|flag| flag == "-fno-static-locals")
            && std::env::var_os("LLRM_6502_NO_STATIC_LOCALS").is_none();
        if static_locals {
            for module in &mut program.modules {
                crate::staticlocals::applied(module)?;
            }
        }
    }
    // The pin lowers `llvm.is.constant` and `llvm.objectsize` at every level.
    program.modules.iter_mut().try_for_each(llrm_transforms::lowerconstantintrinsics::module_lowered)?;
    if whole_program(request) {
        whole_program_swept(&mut program)?;
    }
    Ok(program)
}

/// The assembly of every function `text` defines: the platform's register
/// equates, then each function in order.
pub fn assembly(
    text: &str,
    request: &Request,
) -> Result<String, String> {
    crate::dagisel::function::take_step_sites();
    let kept = assembly_flow(text, request)?;
    let steps = crate::dagisel::function::take_step_sites();
    long_step_trial(kept, steps, request).map(|done| done.text)
}

fn assembly_flow(
    text: &str,
    request: &Request,
) -> Result<Assembled, String> {
    crate::alloc::take_save_drops();
    let kept = flow_once(text, request)?;
    save_drop_trial(kept, crate::alloc::take_save_drops(), request)
}

/// The backend alone on the program a flow kept: the mid-end and its trials
/// are not run again (`save_drop_trial`, `long_step_trial`).
fn backend_flow(
    kept: &Assembled,
    request: &Request,
) -> Result<Assembled, String> {
    crate::alloc::take_save_drops();
    let done = tail(kept.source.clone(), request, false, false)?;
    save_drop_trial(done, crate::alloc::take_save_drops(), request)
}

/// DEVIATE 6502-SAVE-DROP-TRIAL (owner llrm-target-6502, exit 2026-12-06): a
/// module whose carry saves were dropped before allocation (`drops`) has its
/// kept program written again with them dropped after it, and the text with
/// fewer bytes is kept (a tie keeps the first). The pin has one form. Reason:
/// the free `a` helps some modules and defeats the outliner in others, whose
/// sites of one chain allocate differently (wide_shift_by_one -O2 843 B after,
/// 916 B before; call_keeps -O2 1,402 B before, 1,409 B after).
///
/// Compile time: only a module with two such saves runs the backend twice; the
/// mid-end and its trials run once (`backend_flow`).
fn save_drop_trial(
    kept: Assembled,
    drops: usize,
    request: &Request,
) -> Result<Assembled, String> {
    // a chain the outliner shares needs two sites
    if drops < 2 || request.level == Level::O0 || std::env::var_os("LLRM_6502_NO_SAVE_TRIAL").is_some() {
        return Ok(kept);
    }
    let Ok(other) = crate::alloc::with_early_save_drop(false, || tail(kept.source.clone(), request, false, false))
    else {
        return Ok(kept);
    };
    crate::alloc::take_save_drops();
    let (kept_bytes, other_bytes) = (image_bytes_of(&kept.text, request)?, image_bytes_of(&other.text, request)?);
    if std::env::var_os("LLRM_DEBUG_TRIAL").is_some() {
        eprintln!("trial save drop: {drops} drops; {other_bytes} B after allocation against {kept_bytes} B before");
    }
    Ok(if other_bytes < kept_bytes { other } else { kept })
}

fn flow_once(
    text: &str,
    request: &Request,
) -> Result<Assembled, String> {
    llrm_transforms::trial::take_speed_unrolls();
    let built = program(text, request)?;
    let made = llrm_transforms::trial::take_speed_unrolls().made;
    let size = instructions(&built);
    let kept = assembly_trials(built, text, request)?;
    speed_unroll_trial(kept, made, size, text, request)
}

/// DEVIATE 6502-LONG-STEP-TRIAL (owner llrm-target-6502, exit 2026-12-06): a
/// module that has a long plus or minus one stepped with an early exit
/// (`INC32`/`DEC32`, llvm-mos `G_INC`/`G_DEC`) or a long equality tested byte
/// by byte (`CMPZ32`, llvm-mos `G_CMPZ`) has its kept program written again
/// with the carry chain and the word compare, and the text with fewer bytes is
/// kept. The pin has one
/// form. Reason: those forms have labels and branches, which the outliner (the
/// pin's `MachineOutliner`) does not share, and the chain of many sites is one
/// `jsr` each (div_constants_long -O1: 13,520 B with the chains, 14,464
/// stepped, 15,431 with the byte compares), while one loop counter is 14 B
/// stepped against 22 B as a chain (licm_graph_callee_footprint: 306 B, 284 B).
///
/// Compile time: only a module with such a site runs the backend twice; the
/// mid-end and its trials run once (`backend_flow`).
fn long_step_trial(
    kept: Assembled,
    steps: usize,
    request: &Request,
) -> Result<Assembled, String> {
    if steps == 0 || request.level == Level::O0 || std::env::var_os("LLRM_6502_NO_STEP_TRIAL").is_some() {
        return Ok(kept);
    }
    let debug = std::env::var_os("LLRM_DEBUG_TRIAL").is_some();
    let Ok(other) = crate::dagisel::function::with_long_steps(false, || backend_flow(&kept, request)) else {
        return Ok(kept);
    };
    crate::dagisel::function::take_step_sites();
    let (kept_bytes, other_bytes) = (image_bytes_of(&kept.text, request)?, image_bytes_of(&other.text, request)?);
    if debug {
        eprintln!("trial long step: {steps} steps; {other_bytes} B carry chain against {kept_bytes} B stepped");
    }
    Ok(if other_bytes < kept_bytes { other } else { kept })
}

fn assembly_trials(
    built: Program,
    text: &str,
    request: &Request,
) -> Result<Assembled, String> {
    let at_speed = matches!(request.level, Level::O2 | Level::O3)
        && built.target.tti().is_some_and(|tti| tti.speed_inline_finished_trial());
    if at_speed {
        return judged_inlines(built, text, request);
    }
    let (built, inlined, assembled, joint) = trial_inlined(built, text, request)?;
    llrm_transforms::trial::with_inlined(&inlined, || trial_unrolled(built, assembled, joint, false, text, request))
}

/// DEVIATE UNROLL-SPEED-TRIAL (owner 6502, exit 2026-11-24; the z80's
/// `compile::effective`): at -O2 and -O3 a module whose mid-end made a full
/// unroll that `NoGrow` would not (`made`) is compiled again with only the
/// unrolls that add no bytes, and that text is kept if it is fewer bytes. The
/// pin's early unroll takes a loop of an inlinable callee unpriced, and the
/// inliner then declines the body it made (memmove_overlap -O3: `fill`, 32
/// trips, 14 B rolled, 161 B unrolled).
///
/// Compile time: only a module with such an unroll runs the second mid-end,
/// and its backend runs only if that mid-end has fewer instructions than the
/// kept one (`size`): a variant with as many or more cannot be fewer bytes
/// (write_through_parameter's folded 43 B against 249 B).
fn speed_unroll_trial(
    kept: Assembled,
    made: usize,
    size: usize,
    text: &str,
    request: &Request,
) -> Result<Assembled, String> {
    use llrm_transforms::trial::{SpeedUnroll, with_speed_unroll};
    let tried = matches!(request.level, Level::O2 | Level::O3)
        && made > 0
        && machine().tti().is_some_and(|tti| tti.unroll_speed_trial())
        && std::env::var_os("LLRM_6502_NO_SPEED_UNROLL_TRIAL").is_none();
    if !tried {
        return Ok(kept);
    }
    let debug = std::env::var_os("LLRM_DEBUG_TRIAL").is_some();
    let variant = |text: &str| -> Result<Option<Assembled>, String> {
        let built = program(text, request)?;
        if debug {
            eprintln!("trial speed unroll: {} instructions against {size}", instructions(&built));
        }
        if instructions(&built) >= size {
            if debug {
                eprintln!(
                    "trial speed unroll: {made} unrolls; {} instructions against {size}: not tried",
                    instructions(&built)
                );
            }
            return Ok(None);
        }
        tail(built, request, false, false).map(Some)
    };
    let Ok(Some(other)) = with_speed_unroll(Some(SpeedUnroll::NoGrow), || variant(text)) else {
        return Ok(kept);
    };
    let bytes = |assembly: &Assembled| -> Result<i64, String> { image_bytes_of(&assembly.text, request) };
    let (kept_bytes, other_bytes) = (bytes(&kept)?, bytes(&other)?);
    if debug {
        eprintln!("trial speed unroll: {made} unrolls; {other_bytes} B no-growth against {kept_bytes} B kept");
    }
    Ok(if other_bytes < kept_bytes { other } else { kept })
}

/// The bytes of a program's assembly: its code and data, and the runtime
/// routines it links.
fn image_bytes_of(
    assembly: &str,
    request: &Request,
) -> Result<i64, String> {
    let names: Vec<String> = assembly
        .lines()
        .filter(|line| line.ends_with(':') && !line.starts_with(|c: char| c.is_whitespace() || c == '.' || c == '@'))
        .map(|line| line.trim_end_matches(':').to_owned())
        .collect();
    let code = i64::from(crate::timing::estimate(request.cpu, assembly)?.bytes);
    let routines = crate::mulimage::image_of_texts(&names, &[assembly.to_owned()]);
    Ok(code + data_bytes(assembly) + crate::mulimage::image_bytes(&routines))
}

/// DEVIATE OS-INLINE-TRIAL (owner 6502-c, exit 2026-12-05): the copies the -Os
/// byte model refused within its error, inlined in a second compile and kept
/// per chain (caller and callees) if the backend writes those functions in
/// fewer bytes. The model is within 25% of the written bytes for two functions
/// in three, so a refusal that close is the backend's to decide.
///
/// Compile time: a program with no such refusal compiles once. One with some
/// runs the mid-end twice and the backend on the functions whose bodies differ
/// twice (`trial::differing`), never on the whole program; a chain set that is
/// only partly kept compiles the mid-end a third time.
fn trial_inlined(
    built: Program,
    text: &str,
    request: &Request,
) -> Result<(Program, Vec<llrm_transforms::trial::Site>, Option<Assembled>, bool), String> {
    use llrm_transforms::trial;
    let sites = trial::take_refused();
    let limit = built
        .target
        .tti()
        .and_then(|tti| tti.opt_size_inline_trial_limits())
        .map_or(0, |(_, instructions)| instructions);
    if !matches!(request.level, Level::O1 | Level::Os) || sites.is_empty() {
        return Ok((built, Vec::new(), None, false));
    }
    if instructions(&built) > limit {
        if std::env::var_os("LLRM_DEBUG_TRIAL").is_some() {
            eprintln!(
                "trial inline: skipped, {} sites in {} instructions (limit {limit})",
                sites.len(),
                instructions(&built)
            );
        }
        return Ok((built, Vec::new(), None, false));
    }
    let kept_unroll = trial::take();
    let debug = std::env::var_os("LLRM_DEBUG_TRIAL").is_some();
    let chain = |program: &Program,
                 cut: &std::collections::BTreeSet<String>|
     -> Option<std::collections::BTreeMap<String, i64>> {
        let mut only = program.clone();
        only.modules[0] = trial::only(&program.modules[0], cut);
        let rows = tail(only, request, false, true).ok()?.functions;
        Some(rows.into_iter().filter(|(name, _)| cut.contains(name)).collect())
    };
    let Ok(other) = trial::with_inlined(&sites, || program(text, request)) else {
        trial::restore(kept_unroll);
        return Ok((built, Vec::new(), None, false));
    };
    let other_unroll = trial::take();
    // The loops the inlined copies hold that the threshold refused: the unroll
    // trial may take them with the pin's inlines made (`trial_unrolled`).
    let joint = request.level == Level::Os && !other_unroll.0.is_empty();
    let cut = trial::differing(&built.modules[0], &other.modules[0]);
    // The kept side is priced from its own assembly, which the result carries
    // to the unroll trial and the output; only the second compile's chain
    // is assembled for the trial.
    let Ok(kept_all) = tail(built.clone(), request, true, true) else {
        trial::restore(kept_unroll);
        return Ok((built, Vec::new(), None, false));
    };
    let of = |cut: &std::collections::BTreeSet<String>| -> std::collections::BTreeMap<String, i64> {
        kept_all.functions.iter().filter(|(name, _)| cut.contains(name)).cloned().collect()
    };
    let (kept_bytes, Some(other_bytes)) = (of(&cut), chain(&other, &cut)) else {
        trial::restore(kept_unroll);
        return Ok((built, Vec::new(), None, false));
    };
    let flags = trial::accepted(&sites, &kept_bytes, &other_bytes);
    if debug {
        eprintln!(
            "trial inline: {} sites, {} kept; {} functions differ; {} instructions",
            sites.len(),
            flags.iter().filter(|one| **one).count(),
            cut.len(),
            instructions(&built)
        );
    }
    if flags.iter().all(|one| !one) {
        trial::restore(kept_unroll);
        return Ok((built, Vec::new(), Some(kept_all), joint));
    }
    // Every inline kept: the pair (the pin's inlines with the loops they bring
    // unrolled in one mid-end) is still priced if the unroll trial puts the
    // unroll back (loop_fusion -O2 with byte-index loops).
    if flags.iter().all(|one| *one) {
        trial::restore(other_unroll);
        return Ok((other, sites, None, joint));
    }
    let chosen: Vec<_> = sites.into_iter().zip(&flags).filter(|(_, kept)| **kept).map(|(site, _)| site).collect();
    let Ok(third) = trial::with_inlined(&chosen, || program(text, request)) else {
        trial::restore(kept_unroll);
        return Ok((built, Vec::new(), Some(kept_all), false));
    };
    let third_unroll = trial::take();
    let cut = trial::differing(&built.modules[0], &third.modules[0]);
    let pays = chain(&third, &cut).is_some_and(|third| third.values().sum::<i64>() < of(&cut).values().sum::<i64>());
    if pays {
        trial::restore(third_unroll);
        Ok((third, chosen, None, joint))
    } else {
        trial::restore(kept_unroll);
        Ok((built, Vec::new(), Some(kept_all), joint))
    }
}

/// DEVIATE OS-INLINE-TRIAL at speed (6502-INLINE-RATE; owner w11, exit
/// 2026-12-20): at -O2 and -O3 the copies the byte rate refused are priced on
/// finished programs (unrolled, outlined, runtime linked), clocks against
/// bytes (`inline_pays`). Sites that share a function are one group, tried on
/// top of the groups already kept. Neither earlier price will do: bytes before
/// the outliner refused two copied loops it then shares (+71 B counted, -33 B
/// linked), and clocks before the unroll trial refused a copy whose loop that
/// trial then takes (+191 B for no clocks; finished, +228 B for 12,843).
///
/// Compile time: a program with no refused site compiles once; one with some
/// runs the mid-end, the unroll trial and the backend once more a group.
fn judged_inlines(
    built: Program,
    text: &str,
    request: &Request,
) -> Result<Assembled, String> {
    use llrm_transforms::trial;
    let sites = trial::take_refused();
    let limit = built
        .target
        .tti()
        .and_then(|tti| tti.opt_size_inline_trial_limits())
        .map_or(0, |(_, instructions)| instructions);
    let debug = std::env::var_os("LLRM_DEBUG_TRIAL").is_some();
    if sites.is_empty() || instructions(&built) > limit {
        if debug && !sites.is_empty() {
            eprintln!("trial inline: skipped, {} sites past {limit} instructions", sites.len());
        }
        return trial_unrolled(built, None, false, false, text, request);
    }
    let kept_unroll = trial::take();
    let Ok(all) = trial::with_inlined(&sites, || program(text, request)) else {
        trial::restore(kept_unroll);
        return trial_unrolled(built, None, false, false, text, request);
    };
    let all_unroll = trial::take();
    // The loops the copies hold that the threshold refused: the unroll trial
    // may take them with the pin's inlines made.
    let joint = !all_unroll.0.is_empty();
    let finished = |built: Program, noted, inlined: &[trial::Site]| {
        trial::restore(noted);
        trial::with_inlined(inlined, || trial_unrolled(built, None, joint, true, text, request))
    };
    let mut best = finished(built, kept_unroll, &[])?;
    let mut chosen: Vec<trial::Site> = Vec::new();
    let groups = trial::groups(&sites, &[]);
    let mut all = Some((all, all_unroll));
    for group in &groups {
        let mut tried = chosen.clone();
        tried.extend(group.refused.iter().map(|&one| sites[one].clone()));
        let candidate = match all.take().filter(|_| groups.len() == 1) {
            Some(done) => Ok(done),
            None => trial::with_inlined(&tried, || program(text, request)).map(|other| (other, trial::take())),
        };
        let Ok((other, noted)) = candidate else {
            trial::take();
            continue;
        };
        let Ok(priced) = finished(other, noted, &tried) else {
            continue;
        };
        let callers: std::collections::BTreeSet<&str> =
            group.refused.iter().map(|&one| sites[one].caller.as_str()).collect();
        let (saved, added) = (best.clocks - priced.clocks, priced.bytes - best.bytes);
        let pays = inline_pays(saved, added, callers.len());
        if debug {
            eprintln!(
                "trial inline: {} sites, {added:+} bytes, {saved:+} clocks saved: {}",
                group.refused.len(),
                if pays { "kept" } else { "put back" }
            );
        }
        if pays {
            (best, chosen) = (priced, tried);
        }
    }
    Ok(best)
}

/// Whether an inline that saves `saved` clocks for `added` bytes, in `callers`
/// functions, is kept at -O2 and -O3: the judge of those levels, 3 clocks a
/// byte (user ruling 2026-11-26), so a smaller copy may cost the clocks its
/// bytes repay. A growth stays under the bounds of `trial_pays`.
pub fn inline_pays(
    saved: i64,
    added: i64,
    callers: usize,
) -> bool {
    added <= TRIAL_GROWTH_LIMIT.min(TRIAL_BYTES_PER_FUNCTION * callers.max(1) as i64)
        && i128::from(saved) * 1000 >= i128::from(added) * i128::from(TRIAL_MILLICLOCKS_PER_BYTE)
}

pub use llrm_transforms::walk::SizeRow;

/// The size audit of one program: per function, the bytes `function_bytes` says
/// it will be against the bytes it is written as. The instrument
/// `tests/size_calibration.rs` brackets: the SZ2 inline trial decides by the
/// first.
pub fn size_audit(
    text: &str,
    request: &Request,
) -> Result<Vec<SizeRow>, String> {
    let built = program(text, request)?;
    let costs = built.target.trial_costs();
    let module = &built.modules[0];
    let mut rows: Vec<SizeRow> = module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration())
        .map(|(_, global, function)| SizeRow {
            name: global.name.as_deref().unwrap_or("?").to_owned(),
            model: llrm_transforms::walk::function_bytes(module, &built.layout, &costs, function),
            written: 0,
            counts: llrm_transforms::walk::operation_counts(module, &built.layout, function, &costs),
        })
        .collect();
    for (name, bytes) in tail(built, request, false, true)?.functions {
        if let Some(row) = rows.iter_mut().find(|row| row.name == name) {
            row.written = bytes;
        }
    }
    Ok(rows)
}

thread_local! {
    static SELECTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static MID_ENDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The mid-end runs this thread has made: a trial that only changes a switch
/// of the backend must not add one (`backend_flow`).
pub fn mid_ends() -> usize {
    MID_ENDS.with(std::cell::Cell::get)
}

/// The functions this thread has selected: the cost of a compile, which the
/// trial's pricing doubled and then tripled (2026-11-08: +16.9% at -O2).
pub fn selected() -> usize {
    SELECTED.with(std::cell::Cell::get)
}

/// The bytes the initialised data in `text` takes in the image: what a trial
/// that folds a table away saves, which the functions' own bytes leave out.
/// Zero-filled `.res` of the BSS is not in the image.
pub fn data_bytes(text: &str) -> i64 {
    let mut in_image = true;
    let mut total = 0;
    for line in text.lines().map(str::trim) {
        if let Some(segment) = line.strip_prefix(".segment") {
            in_image = !segment.contains("BSS");
        } else if in_image {
            let count = |rest: &str| rest.split(',').filter(|one| !one.trim().is_empty()).count() as i64;
            if let Some(rest) = line.strip_prefix(".byte") {
                total += count(rest);
            } else if let Some(rest) = line.strip_prefix(".word").or_else(|| line.strip_prefix(".addr")) {
                total += 2 * count(rest);
            } else if let Some(rest) = line.strip_prefix(".res") {
                total += rest.trim().split(',').next().and_then(|n| n.trim().parse::<i64>().ok()).unwrap_or(0);
            }
        }
    }
    total
}

/// A program's assembly and, when asked for, its price: the clocks and bytes
/// of the functions as the pass that wrote them estimates them.
struct Assembled {
    text: String,
    clocks: i64,
    bytes: i64,
    /// Each function's bytes, when asked for (`size_audit`).
    functions: Vec<(String, i64)>,
    /// The program as `tail` was given it: written again by the backend
    /// trials, which change a switch of the backend and not the mid-end.
    source: Program,
}

/// The assembly of `program`: every function selected (the copies as loops and
/// the paged walks where they price cheaper) and written out.
fn tail(
    mut program: Program,
    request: &Request,
    price: bool,
    audit: bool,
) -> Result<Assembled, String> {
    let source = program.clone();
    // The copies as loops, for the functions that call `memcpy`: priced
    // against the calls below.
    let copies = if request.level == Level::O0 {
        Vec::new()
    } else {
        llrm_transforms::memloops::candidates(&program.modules[0])
    };
    let looped = (!copies.is_empty()).then(|| {
        let mut looped = program.modules[0].clone();
        llrm_transforms::memloops::expanded(&mut looped, &|name| copies.iter().any(|one| one == name)).map(|()| {
            crate::prepare::split_phi_edges(&mut looped);
            looped
        })
    });
    let looped = looped.transpose()?;
    // The loops that walk a pointer by page, for the functions that have one:
    // priced against the loops as they are, below.
    let index_bits = llrm_mir::target::Machine::index_register_bits(&Mos6502Machine);
    let paged = index_bits
        .filter(|_| request.level != Level::O0)
        .map(|bits| {
            let mut paged = program.modules[0].clone();
            // Selection's DAG CSE merges the equal `gep`s LSR leaves (the
            // next pointer and an unfolded `p + 1`); the pass reads the IR.
            let mut cse = llrm_mir::passes::PassManager::default();
            cse.add(llrm_transforms::earlycse::EarlyCse { memory_ssa: false });
            cse.run_module(&mut paged, machine())?;
            llrm_transforms::pageindex::indexed(&mut paged, bits).map(|names| {
                crate::prepare::split_phi_edges(&mut paged);
                (names, paged)
            })
        })
        .transpose()?
        .filter(|(names, _)| !names.is_empty());
    // The same walks that also end a counted loop on the index and a page
    // count: priced against the walks above.
    let counted = paged
        .as_ref()
        .zip(index_bits)
        .map(|(_, bits)| {
            let mut counted = program.modules[0].clone();
            let mut cse = llrm_mir::passes::PassManager::default();
            cse.add(llrm_transforms::earlycse::EarlyCse { memory_ssa: false });
            cse.run_module(&mut counted, machine())?;
            llrm_transforms::pageindex::indexed_counting(&mut counted, bits).map(|names| {
                crate::prepare::split_phi_edges(&mut counted);
                (names, counted)
            })
        })
        .transpose()?
        .filter(|(names, _)| !names.is_empty());
    // The range tests that two compares do better than the joined one: priced
    // against it, below.
    let ranged =
        |source: &llrm_mir::module::Module| -> Result<Option<(Vec<String>, llrm_mir::module::Module)>, String> {
            if request.level == Level::O0 {
                return Ok(None);
            }
            let mut split = source.clone();
            let names = llrm_transforms::rangesplit::unmerged(&mut split)?;
            crate::prepare::split_phi_edges(&mut split);
            Ok((!names.is_empty()).then_some((names, split)))
        };
    let ranged_base = ranged(&program.modules[0])?;
    let ranged_paged = match &paged {
        Some((_, paged)) => ranged(paged)?,
        None => None,
    };
    let ranged_counted = match &counted {
        Some((_, counted)) => ranged(counted)?,
        None => None,
    };
    llrm_6502_prepare(&mut program);
    let module = &program.modules[0];
    // after the modules it keys texts by: dropped before them
    let _texts = crate::select::TextCache::new();
    let names: Vec<String> = module
        .globals
        .iter()
        .filter(|global| global.function().is_some_and(|one| !one.is_declaration()))
        .filter_map(|global| global.name.as_deref().map(str::to_owned))
        .collect();
    let data = data_text(module, &program.layout)?;
    crate::dagisel::function::set_widen_products(if matches!(request.level, Level::O2 | Level::O3 | Level::Omax) {
        crate::dagisel::function::Widen::Unsized
    } else {
        crate::dagisel::function::Widen::Off
    });
    let mut selections = Vec::new();
    let mut from_module = Vec::new();
    // The module each function was selected from: its blocks are the text's.
    let mut sources: Vec<Option<&llrm_mir::module::Module>> = Vec::new();
    for name in &names {
        SELECTED.with(|count| count.set(count.get() + 1));
        let mut selection = select_function(module, name, request.cpu)?;
        let mut as_called = true;
        let mut paged_chosen = false;
        let mut counted_chosen = false;
        let mut source = None;
        selection.zero_page = request.platform.zero_page;
        selection.tuning = request.level.tuning();
        if let Some(looped) = looped.as_ref().filter(|_| copies.contains(name)) {
            let mut other = select_function(looped, name, request.cpu)?;
            other.zero_page = request.platform.zero_page;
            other.tuning = request.level.tuning();
            if cheaper(module, name, &other, looped, &selection, request, false)? {
                selection = other;
                as_called = false;
                source = Some(looped);
            }
        }
        if let Some((_, paged)) = paged.as_ref().filter(|(names, _)| as_called && names.contains(name)) {
            let mut other = select_function(paged, name, request.cpu)?;
            other.zero_page = request.platform.zero_page;
            other.tuning = request.level.tuning();
            if cheaper(module, name, &other, paged, &selection, request, true)? {
                selection = other;
                paged_chosen = true;
                source = Some(paged);
            }
        }
        if let Some((_, counted)) = counted.as_ref().filter(|(names, _)| names.contains(name)) {
            let mut other = select_function(counted, name, request.cpu)?;
            other.zero_page = request.platform.zero_page;
            other.tuning = request.level.tuning();
            let kept = source.unwrap_or(module);
            if cheaper(kept, name, &other, counted, &selection, request, true)? {
                selection = other;
                counted_chosen = true;
                source = Some(counted);
            }
        }
        let mut range_chosen = false;
        if as_called || paged_chosen {
            let candidate = match (paged_chosen, counted_chosen) {
                (true, true) => ranged_counted.as_ref(),
                (true, false) => ranged_paged.as_ref(),
                _ => ranged_base.as_ref(),
            };
            if let Some((_, split)) = candidate.filter(|(names, _)| names.contains(name)) {
                let mut other = select_function(split, name, request.cpu)?;
                other.zero_page = request.platform.zero_page;
                other.tuning = request.level.tuning();
                let kept = source.unwrap_or(module);
                if cheaper(kept, name, &other, split, &selection, request, true)? {
                    selection = other;
                    range_chosen = true;
                    source = Some(split);
                }
            }
        }
        selections.push(selection);
        sources.push(source);
        from_module.push(as_called && !paged_chosen && !range_chosen);
    }
    crate::mulimage::widen_when_it_pays(module, &names, &mut selections, &from_module, request)?;
    crate::mulimage::expand_when_unshared(module, &names, &mut selections, &from_module, request)?;
    crate::mulimage::call_when_it_pays(module, &names, &mut selections, &from_module, request)?;
    // Set once the selection is final: pricing the alternatives sees none.
    let file = request.debug.then(|| llrm_mir::dbgimport::unit_file(module)).flatten().map(|(name, _)| name);
    for (name, selection) in names.iter().zip(selections.iter_mut()) {
        let scope = module.named(name).and_then(|id| module.subprogram_line(module.global(id).function()?));
        selection.source = file.clone().map(|file| (file, scope));
    }
    let parts: Vec<(&str, &_)> = names.iter().map(String::as_str).zip(&selections).collect();
    let pieces = module_text(module, &parts)?;
    let plain_pieces = crate::select::plain_pieces(&pieces);
    let functions = if audit {
        names
            .iter()
            .zip(&pieces)
            .filter_map(|(name, piece)| {
                crate::timing::estimate(request.cpu, piece).ok().map(|priced| (name.clone(), i64::from(priced.bytes)))
            })
            .collect()
    } else {
        Vec::new()
    };
    let (mut clocks, mut bytes) = (0i64, 0i64);
    if price {
        let weights = estimate_weights(module, &names, &sources, &program.layout, request.level);
        let (run, size) = crate::weights::run_cost(request.cpu, &names, &pieces, &weights)?;
        (clocks, bytes) = (run as i64, size as i64);
    }
    if price {
        bytes += data_bytes(&data.0);
    }
    let mut text = pieces.concat();
    for (name, label) in &data.1 {
        text = text.replace(name.as_str(), label);
    }
    let files = file.map_or_else(String::new, |name| format!(".dbg file, \"{name}\", 0, 0\n"));
    // `-fno-outline` is the kill switch, for a test of what the backend wrote;
    // `LLRM_6502_NO_OUTLINE=1` is the same for an A/B of one binary through
    // `llrm-cc` (which does not take the flag)
    let outlined = request.level != Level::O0
        && !request.passes.iter().any(|flag| flag == "-fno-outline")
        && std::env::var_os("LLRM_6502_NO_OUTLINE").is_none();
    // A place in a loop is given up for its clocks at -O1 and -Os: at -O2 and
    // -O3 that takes div_constants_long and div_constants_ulong out of the
    // bytes that beat the other compilers', and bytes come first (row
    // 6502-OUTLINE-CLOCKS). At -Os the loop is a loop the function's callers
    // make too (`outline::weights`).
    //
    // DEVIATE 6502-OUTLINE-OS-RATE (owner llrm-target-6502, review
    // 2027-01-31): the pin's `MachineOutliner` prices bytes alone, and the pass
    // was that at -Os. frameless -Os ran 54,218 clocks against llvm-mos's
    // 46,669: `add4`'s `d * 4` was two calls of a 4-instruction shift in a
    // function a loop calls. The rate is the one every size-reducing choice of
    // the target is made at (`alloc::Tuning`).
    let rate =
        matches!(request.level, Level::O1 | Level::Os | Level::Oz).then(|| request.level.tuning().clocks_per_byte);
    // at -O1 to -O3 a place is given up by the share of the module's clocks its
    // calls add against the share of its bytes they save (`[share.speed]
    // outline`)
    let share = matches!(request.level, Level::O1 | Level::O2 | Level::O3)
        .then(|| crate::routines::speed_share_percent("outline"));
    // The outliner is also tried on the module as the allocator wrote it
    // before the transfer of known constants, which that pass makes differ
    // between copies of one run; it then runs on the outlined text, and the
    // smaller module is kept.
    // `LLRM_6502_OUTLINE_PLAIN=0` drops only this variant, not the outliner
    // (see `LLRM_6502_NO_OUTLINE`)
    let plain = (outlined
        && crate::copyprop::enabled(std::env::var_os("LLRM_6502_OUTLINE_PLAIN").as_deref())
        && plain_pieces.iter().zip(&pieces).any(|(plain, piece)| plain != piece))
    .then(|| {
        let mut plain = plain_pieces.concat();
        for (name, label) in &data.1 {
            plain = plain.replace(name.as_str(), label);
        }
        plain
    });
    let after = |text: String| if request.level != Level::O0 { crate::peephole::after_outlining(&text) } else { text };
    let kept = after(if outlined { crate::outline::run_priced(&text, rate, share) } else { text });
    let text = match plain {
        Some(plain) => {
            let mut outlined = crate::asm::Asm::parse(&crate::outline::run_priced(&plain, rate, share));
            crate::peephole::drop_known_loads(&mut outlined);
            let plain = after(outlined.text());
            let bytes = |text: &str| {
                crate::timing::estimate(request.cpu, &crate::alloc::without_lines(text)).map(|priced| priced.bytes)
            };
            match (bytes(&kept), bytes(&plain)) {
                (Ok(kept_bytes), Ok(plain_bytes)) if plain_bytes < kept_bytes => plain,
                _ => kept,
            }
        }
        None => kept,
    };
    let text = match request.platform.static_zero_page {
        0 => text,
        // `LLRM_6502_STATIC_ZP=0` is the kill switch, for an A/B of one binary
        budget
            if whole_program(request)
                && crate::copyprop::enabled(std::env::var_os("LLRM_6502_STATIC_ZP").as_deref()) =>
        {
            let first = u16::from(request.platform.imag_base)
                + u16::from(request.platform.zero_page)
                + u16::from(request.platform.scratch_zero_page);
            crate::zerospill::static_cells_to_zero_page(&text, first, u16::from(budget))
        }
        _ => text,
    };
    // The bytes a trial prices are the finished text's: the outliner shares
    // the repeated sequences of an unrolled body (static_pointers: 26 B of
    // 174 modelled bytes, 16 of the rolled loop's 181), which the functions'
    // own bytes leave out. The runtime routines the text calls are linked
    // bytes too: a rolled loop's `jsr __mulsi3` costs the routine, the unrolled
    // constants none (tail_calls `framed`: kept at -6 B, then put back at +10 B
    // against a rolled copy 89 B larger once linked).
    // `LLRM_6502_MODEL_BYTES` is the kill switch, for an A/B of one binary
    if price && std::env::var_os("LLRM_6502_MODEL_BYTES").is_none() {
        if let Ok(finished) = image_bytes_of(&text, request) {
            bytes = finished + data_bytes(&data.0);
        }
    }
    Ok(Assembled { text: equates(&request.platform) + &files + &text + &data.0, clocks, bytes, functions, source })
}

/// DEVIATE UNROLL-TRIAL (owner llrm-target-6502, exit 2026-12-05): the program
/// compiled again with every loop under the target's trial size unrolled, kept
/// if its assembly is priced cheaper. The pin's threshold unrolls a loop by its
/// own units and by what the loop folds of itself (at most ten trips); an array
/// filled by one loop and read by the next folds only when both are gone. Only
/// a program with a loop the threshold refused is compiled twice, and each
/// build is written out once: the winner's text is the one priced.
///
/// DEVIATE UNROLL-TRIAL-TIME (owner llrm-target-6502, exit 2026-12-05): the
/// second compile costs a program with a candidate loop about 2.5x (the 43 of
/// 176 corpus programs that have one); the corpus total is +16.9% at -O2 and
/// +27.7% at -Os in user-mode instructions, the median program unchanged.
fn trial_unrolled(
    built: Program,
    assembled: Option<Assembled>,
    joint: bool,
    price_kept: bool,
    text: &str,
    request: &Request,
) -> Result<Assembled, String> {
    use llrm_transforms::loopunrollpass::trial;
    // `assembled`: `built` as the inline trial already wrote it (price and text
    // of `tail`)
    let kept_text = |built: Program, assembled: Option<Assembled>| match assembled {
        Some(done) => Ok(done),
        None => tail(built, request, price_kept, false),
    };
    let tti = built.target.tti();
    let Some(limit) = tti.as_ref().and_then(|tti| tti.unroll_trial_threshold()) else {
        return kept_text(built, assembled);
    };
    let (eligible, units) = trial::take();
    let level_tried = matches!(request.level, Level::O1 | Level::O2 | Level::O3 | Level::Os);
    if !level_tried || (eligible.is_empty() && !joint) {
        return kept_text(built, assembled);
    }
    let debug = std::env::var_os("LLRM_DEBUG_TRIAL").is_some();
    if debug {
        eprintln!("trial candidates: {} functions, {units} units", eligible.len());
    }
    // The program the trials are priced against, as the kept side is written.
    let size = instructions(&built);
    let modeled = if joint { modeled_bytes(&built) } else { 0 };
    // Kept whole where the joint compile below prices against it.
    let mut built = Some(built);
    let own = |program: &mut Option<Program>| if joint { program.clone() } else { program.take() };
    let mut best: Option<Assembled> = None;
    let mut kept = assembled;
    if !eligible.is_empty() {
        let other = trial::with(limit, || program(text, request))?;
        let growth = instructions(&other).saturating_sub(size);
        if !trial_fits(growth) {
            if debug {
                eprintln!("trial unroll: {growth} instructions more: put back unpriced");
            }
        } else {
            let priced = match kept.take() {
                Some(done) => done,
                None => tail(own(&mut built).expect("the program"), request, true, false)?,
            };
            let tried = tail(other, request, true, false)?;
            let (saved, added) = (priced.clocks - tried.clocks, tried.bytes - priced.bytes);
            let pays = trial_pays(request.level, saved, added, eligible.len());
            if debug {
                eprintln!(
                    "trial unroll: {added:+} bytes, {saved:+} clocks saved: {}",
                    if pays { "kept" } else { "put back" }
                );
            }
            if pays {
                best = Some(tried);
                kept = Some(priced);
            } else {
                best = Some(priced);
            }
        }
    }
    // An unroll the trial kept is the baseline the pair is priced against: at
    // -O2 and -O3 it may have grown the code (loop_fusion: +1,701 B kept, the
    // pair 131 B).
    if !joint {
        return match best {
            Some(done) => Ok(done),
            None => kept_text(built.take().expect("the program"), kept),
        };
    }
    // DEVIATE INLINE-UNROLL-TRIAL (owner 6502, exit 2026-11-24, row
    // 6502-INLINE-RATE): the pin's inlines all made and the loops they bring
    // unrolled. Neither trial above prices that pair: the inline trial prices
    // the chain rolled, the unroll trial the callees as calls (loop_fusion -O2:
    // 1,190 B and 92,367 clk kept, the pair 133 B and 2,825 clk). Kept if it
    // prices cheaper than the best of the above.
    let priced = match best {
        Some(done) => done,
        None => match kept {
            Some(done) => done,
            None => tail(built.take().expect("the program"), request, true, false)?,
        },
    };
    // -Os prices the -O2 pair too: its size rule refuses the callees whose
    // bodies the unrolled loops fold away, and the backend prices the result.
    let pair_request = Request {
        platform: request.platform.clone(),
        cpu: request.cpu,
        level: if request.level == Level::Os { Level::O2 } else { request.level },
        passes: request.passes.clone(),
        debug: request.debug,
    };
    let Ok(both) = llrm_transforms::trial::with_pin_inlines(|| trial::with(limit, || program(text, &pair_request)))
    else {
        return Ok(priced);
    };
    // Priced by the backend only where the byte model finds it smaller: the
    // pair wins by folding the program away, and a pair that does not is the
    // most compile time a trial spends for nothing.
    let model = modeled_bytes(&both);
    if debug {
        eprintln!("trial inline+unroll: model {model} B against {modeled} B");
    }
    if model >= modeled || !trial_fits(instructions(&both).saturating_sub(size)) {
        return Ok(priced);
    }
    let tried = tail(both, request, true, false)?;
    let (saved, added) = (priced.clocks - tried.clocks, tried.bytes - priced.bytes);
    let pays = trial_pays(request.level, saved, added, eligible.len().max(1));
    if debug {
        eprintln!(
            "trial inline+unroll: {added:+} bytes, {saved:+} clocks saved: {}",
            if pays { "kept" } else { "put back" }
        );
    }
    Ok(if pays { tried } else { priced })
}

/// The instructions a trial may add before it is priced: the byte limit below
/// at about eight bytes an instruction (the programs' trials cost 5 to 60).
/// Pricing selects every function, which is where the time goes: `fcmp_all`'s
/// trial grew 1,423 instructions to 4,231 and took 14 s to price, 85,508 bytes
/// more.
const TRIAL_GROWTH_INSTRUCTIONS: usize = 2048;

/// Whether a trial that adds `growth` instructions is priced at all.
pub fn trial_fits(growth: usize) -> bool {
    growth <= TRIAL_GROWTH_INSTRUCTIONS
}

/// The bytes `function_bytes` says `program` is, the model the inline trials
/// are priced by before the backend writes anything.
fn modeled_bytes(program: &Program) -> i64 {
    let costs = program.target.trial_costs();
    let module = &program.modules[0];
    module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration())
        .map(|(_, _, function)| llrm_transforms::walk::function_bytes(module, &program.layout, &costs, function))
        .sum()
}

fn instructions(program: &Program) -> usize {
    program.modules[0].functions().map(|(_, _, function)| function.walk().count()).sum()
}

/// The most code a trial may add: a quarter of the 6502's 64 KiB address space,
/// which the stack, the data and the runtime share. `fcmp_all` unrolled to
/// 85,508 bytes more (2,008,446 clocks saved, 23 clk/B) and would not have
/// fitted.
const TRIAL_GROWTH_LIMIT: i64 = 16 * 1024;

/// Thousandths of a clock a byte at -O2 and -O3: the judge of those levels
/// (clocks + 3 * bytes). DEVIATE 6502-UNROLL-RATE (owner x87, exit 2026-12-15;
/// oversight ruling 2026-11-24, not yet confirmed by the user): the rate was
/// infinite there, bytes first (ruling 2026-11-14), which kept `dirty`'s
/// 96-trip store loop (long_double_constant_store -O3 2,490 clk against 979).
const TRIAL_MILLICLOCKS_PER_BYTE: i64 = 3000;

/// The most a priced trial may add for each function it unrolls in: the pin's
/// full-unroll threshold (`UnrollThreshold`, 300 instructions at -O3) at two
/// bytes an instruction. Priced alone the judge's rate kept loop_interchange
/// +13,418 B for -2.8% clocks and div_constants_short +5,912 B for -1.6%;
/// the pin's own bound is on size too.
const TRIAL_BYTES_PER_FUNCTION: i64 = 300 * 2;

/// Whether a trial that saved `saved` clocks for `added` bytes in `functions`
/// functions is kept. -Os and -O1: bytes first (user ruling 2026-11-14), so one
/// that adds none, and saves no fewer clocks, or one that removes some. -O2 and
/// -O3: one that adds bytes is kept too where the clocks it saves repay them at
/// the judge's rate (`keeps_at_o1`, the rule of every target's priced trial)
/// and it stays under the pin's size bound.
pub fn trial_pays(
    level: Level,
    saved: i64,
    added: i64,
    functions: usize,
) -> bool {
    if added > TRIAL_GROWTH_LIMIT {
        return false;
    }
    match level {
        // strict: no more bytes and no more clocks (6502-UNROLL-O1)
        Level::O1 => llrm_transforms::loopunrollpass::trial::keeps_at_o1(saved, added, 0),
        Level::O2 | Level::O3 => {
            added <= TRIAL_BYTES_PER_FUNCTION * functions.max(1) as i64
                && llrm_transforms::loopunrollpass::trial::keeps_at_o1(saved, added, TRIAL_MILLICLOCKS_PER_BYTE)
        }
        _ => added < 0 || (added == 0 && saved >= 0),
    }
}

/// Whether `name` as the loops in `alt` costs less than the calls in
/// `selected`: fewer bytes, the clocks (a trip through a loop weighs ten) only
/// breaking a tie. A function either can't be priced for keeps its calls.
fn cheaper(
    module: &llrm_mir::module::Module,
    name: &str,
    alt: &crate::dagisel::function::Selection,
    alt_module: &llrm_mir::module::Module,
    selected: &crate::dagisel::function::Selection,
    request: &Request,
    strict_at_o1: bool,
) -> Result<bool, String> {
    let prices = crate::routines::copy_prices();
    let routines = [("memcpy", prices.call_clocks), ("memset", prices.call_clocks)];
    let original = module;
    let cost = |module: &llrm_mir::module::Module, selection| -> Option<(u64, u64)> {
        let text = crate::select::function_text(module, name, selection).ok()?;
        let depths = llrm_transforms::memloops::loop_depths(module.global(module.named(name)?).function()?);
        let depth_of = |block: u32| depths.get(block as usize).copied().unwrap_or(0);
        // loop depth guesses ten trips a level; a branch with weights (the
        // page walk's wrap) says how often a block runs against its loop
        let function = module.global(module.named(name)?).function()?;
        let mut weights: Vec<u64> =
            (0..function.block_capacity() as u32).map(|block| 10u64.pow(depth_of(block).min(3))).collect();
        crate::weights::by_branch_weights(module, function, &mut weights);
        let weight_of = |block: u32| weights.get(block as usize).copied().unwrap_or(1);
        let priced = crate::timing::estimate_weighted(request.cpu, &text, &routines, Some(&weight_of)).ok()?;
        // a library routine the text calls is linked once for its callers: this
        // function pays its share
        let mut called: Vec<&str> = text
            .lines()
            .filter_map(|line| line.trim().strip_prefix("jsr ").or_else(|| line.trim().strip_prefix("jmp ")))
            .collect();
        called.sort_unstable();
        called.dedup();
        let library: u64 = called
            .iter()
            .filter(|routine| crate::RUNTIME.contains(&format!("\n{routine}:")))
            .map(|routine| {
                u64::from(crate::mulimage::library_bytes(routine))
                    / llrm_transforms::memloops::callers(original, routine).max(1) as u64
            })
            .sum();
        Some((priced.cycles, u64::from(priced.bytes) + library))
    };
    let (alt_cost, kept_cost) = (cost(alt_module, alt), cost(module, selected));
    if std::env::var_os("LLRM_DEBUG_TRIAL").is_some() {
        eprintln!("cheaper {name}: alternative (clocks, bytes) {alt_cost:?} against kept {kept_cost:?}");
    }
    Ok(match (alt_cost, kept_cost) {
        // -O1 keeps no more bytes and no more clocks (6502-UNROLL-O1)
        (Some(alt), Some(kept)) if strict_at_o1 && request.level == Level::O1 => {
            alt.0 <= kept.0 && alt.1 <= kept.1 && alt != kept
        }
        // bytes first at every level, clocks break a tie (user ruling
        // 2026-11-14)
        (Some(alt), Some(kept)) => (alt.1, alt.0) < (kept.1, kept.0),
        _ => false,
    })
}

fn llrm_6502_prepare(program: &mut Program) {
    program.modules.iter_mut().for_each(crate::prepare::split_phi_edges);
}

/// The cost model the mid-end reads: the pin's `opt` with no target, whose
/// `Basic` TargetTransformInfo prices the loop-idiom, inline and
/// strength-reduction decisions. Without it (`tti() = None`) those passes price
/// by generic rules and the recognizer of CRC loops never runs. Everything else
/// is `Neutral`'s: no register pressure, unit prices.
pub fn machine() -> Rc<dyn llrm_mir::target::Machine> {
    Rc::new(Mos6502Machine)
}

/// DEVIATE 6502-MEM-LIMITS (owner 6502, exit 2026-11-24): bytes first at every
/// level (6502-UNROLL-RATE). The pin's `MaxStoresPerMemcpy` and
/// `MaxStoresPerMemmove` are 8 at speed and 4 tuned for size
/// (X86ISelLowering.cpp); a constant copy of more than four pieces is a call
/// at -Os (memmove_overlap's 27 bytes: about 14 word pairs inline against a
/// 6-byte call) and stays inline at -O1..-O3. This target takes the size
/// limits at every level.
pub(crate) fn bytes_first_tuning() -> llrm_mir::target::Tuning {
    let pin = llrm_mir::target::Tuning::PIN;
    llrm_mir::target::Tuning {
        max_stores_per_memcpy: pin.max_stores_per_memcpy_opt_size,
        max_stores_per_memmove: pin.max_stores_per_memmove_opt_size,
        ..pin
    }
}

struct Mos6502Machine;

impl llrm_mir::target::Machine for Mos6502Machine {
    /// DEVIATE 6502-ALLOCA-ALIGN (owner llrm-target-6502, exit 2026-12-15): the
    /// soft stack is not realigned and the frame keeps `STACK_ALIGN` (1);
    /// an `align 2` local from the front end is clamped to it at pipeline
    /// entry, so `ptr & 1` is no longer folded to 0 for an object on an odd
    /// frame. The pin states what a frame keeps the same
    /// way (`TargetFrameLowering::getStackAlignment`). Exit: even frames and
    /// argument areas, then 2.
    fn kept_stack_align(&self) -> Option<u64> {
        Some(crate::frame::STACK_ALIGN as u64)
    }

    fn max_alloca_align(&self) -> Option<u64> {
        Some(crate::frame::MAX_ALLOCA_ALIGN)
    }

    /// `lda (zp),y`: the index is `y`.
    fn index_register_bits(&self) -> Option<u32> {
        Some(8)
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

    fn tuning(&self) -> llrm_mir::target::Tuning {
        bytes_first_tuning()
    }

    /// Clocks and bytes of the sequences the backend emits, per native byte
    /// (`crate::opcosts`); `byte_costs` falls back to the bytes.
    fn costs(&self) -> llrm_mir::target::OperationCosts {
        static CLOCKS: std::sync::OnceLock<llrm_mir::target::OperationCosts> = std::sync::OnceLock::new();
        CLOCKS.get_or_init(|| crate::opcosts::clocks(crate::dagisel::hooks::Cpu::Mos6502)).clone()
    }

    fn size_costs(&self) -> llrm_mir::target::OperationCosts {
        static BYTES: std::sync::OnceLock<llrm_mir::target::OperationCosts> = std::sync::OnceLock::new();
        BYTES.get_or_init(|| crate::opcosts::bytes(crate::dagisel::hooks::Cpu::Mos6502)).clone()
    }

    fn trial_costs(&self) -> llrm_mir::target::OperationCosts {
        static BYTES: std::sync::OnceLock<llrm_mir::target::OperationCosts> = std::sync::OnceLock::new();
        BYTES.get_or_init(|| crate::opcosts::trial_bytes(crate::dagisel::hooks::Cpu::Mos6502)).clone()
    }

    /// The bytes the allocator can put a value in on the smallest zero page
    /// (`a`, `x`, `y`, `rc2`..`rc15`). 0 left every pressure estimate off: no
    /// loop was ever too wide for the registers.
    ///
    /// DEVIATE (6502-OPCOSTS): `Room` counts values, not bytes, so a wide value
    /// takes one of these. Exit: residents weighed by `OperationCosts::words`
    /// (2026-11-20).
    fn registers(&self) -> i64 {
        crate::alloc::register_file(0).0
    }

    /// The bytes a call leaves: the callee-kept cells `rc8`..`rc15`.
    fn call_registers(&self) -> i64 {
        crate::alloc::register_file(0).1
    }

    fn address_forms(&self) -> Vec<llrm_mir::target::AddressForm> {
        llrm_mir::target::Neutral.address_forms()
    }

    fn tti(&self) -> Option<Rc<dyn llrm_mir::tti::Tti>> {
        Some(Rc::new(crate::tti::Mos6502::new()))
    }

    fn lowering(&self) -> Option<Rc<dyn llrm_mir::lowering::Lowering>> {
        Some(Rc::new(crate::ir_lowering::MosIrLowering))
    }
}

/// The module's defined variables as ca65 data, and the renames of the symbols
/// ca65 cannot spell (`.crctable`, the table `LoopIdiomRecognize` makes) to the
/// labels that stand for them in the code.
///
/// DEVIATE: integers, pointers, arrays, structs and zero fills; a float
/// initializer is refused. Reason: the corpus has none. Exit: floats, with the
/// softfloat path. A word holding the address of a global (plus a constant
/// offset) is a `.word` for ca65 to relocate.
fn data_text(
    module: &llrm_mir::module::Module,
    layout: &llrm_mir::datalayout::DataLayout,
) -> Result<(String, Vec<(String, String)>), String> {
    use llrm_mir::module::GlobalKind;
    let mut out = String::new();
    let mut renames = Vec::new();
    for global in &module.globals {
        let GlobalKind::Variable(variable) = &global.kind else { continue };
        let Some(initializer) = variable.initializer else { continue };
        let name = global.name.owned().ok_or("an unnamed variable")?;
        let mut bytes = vec![0u8; layout.alloc_size(&module.context.types, variable.ty) as usize];
        let mut words = Vec::new();
        write_constant(module, layout, initializer, variable.ty, &mut bytes, 0, &mut words)?;
        let label = data_label(&name);
        if label != name {
            renames.push((name.clone(), label.clone()));
        }
        if crate::select::exported(global.linkage) && label == name {
            out.push_str(&format!(".export {label}\n"));
        }
        // a variable of zeros is BSS, which the platform's startup clears: it
        // costs no bytes in the image
        if !variable.constant && words.is_empty() && bytes.iter().all(|&b| b == 0) {
            out.push_str(&format!(".segment \"BSS\"\n{label}: .res {}\n.segment \"CODE\"\n", bytes.len().max(1)));
            continue;
        }
        out.push_str(&format!(".segment \"{}\"\n{label}:\n", if variable.constant { "RODATA" } else { "DATA" }));
        let mut at = 0;
        for (word_at, symbol, addend) in words {
            for chunk in bytes[at..word_at].chunks(16) {
                out.push_str(&format!("  .byte {}\n", chunk.iter().map(u8::to_string).collect::<Vec<_>>().join(",")));
            }
            out.push_str(&format!(
                "  .word {symbol}{}\n",
                if addend == 0 { String::new() } else { format!("{addend:+}") }
            ));
            at = word_at + 2;
        }
        for chunk in bytes[at..].chunks(16) {
            out.push_str(&format!("  .byte {}\n", chunk.iter().map(u8::to_string).collect::<Vec<_>>().join(",")));
        }
        out.push_str(".segment \"CODE\"\n");
    }
    renames.sort_by_key(|(name, _)| std::cmp::Reverse(name.len()));
    Ok((out, renames))
}

/// The label `name` has in the data: itself, or `data` and its letters and
/// digits where ca65 cannot spell it (`.str.1`).
fn data_label(name: &str) -> String {
    if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        name.to_owned()
    } else {
        let label: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
        format!("data{label}")
    }
}

/// The symbol and byte offset the constant `id` is the address of: a global,
/// a constant GEP of one, or either through a cast that keeps the bits.
fn address_of(
    module: &llrm_mir::module::Module,
    layout: &llrm_mir::datalayout::DataLayout,
    id: llrm_mir::ConstantId,
) -> Result<Option<(String, i64)>, String> {
    use llrm_mir::module::GlobalKind;
    use llrm_mir::opcode::CastOp;
    use llrm_mir::{ConstantExpr, ConstantKind};
    match &module.context.get(id).kind {
        ConstantKind::Global(global) => {
            let global = module.global(*global);
            let name = global.name.owned().ok_or("the address of an unnamed global")?;
            Ok(Some((if matches!(global.kind, GlobalKind::Variable(_)) { data_label(&name) } else { name }, 0)))
        }
        ConstantKind::Expr(ConstantExpr::GetElementPtr { source, operands, .. }) => {
            let Some((symbol, base)) = address_of(module, layout, operands[0])? else { return Ok(None) };
            let offset = llrm_codegen::backend::globals::constant_offset(module, layout, *source, &operands[1..])?;
            Ok(Some((symbol, base + offset)))
        }
        ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::PtrToInt | CastOp::IntToPtr | CastOp::BitCast, value }) => {
            address_of(module, layout, *value)
        }
        _ => Ok(None),
    }
}

fn write_constant(
    module: &llrm_mir::module::Module,
    layout: &llrm_mir::datalayout::DataLayout,
    id: llrm_mir::ConstantId,
    ty: llrm_mir::TypeId,
    out: &mut [u8],
    at: usize,
    words: &mut Vec<(usize, String, i64)>,
) -> Result<(), String> {
    use llrm_mir::opcode::CastOp;
    use llrm_mir::{ConstantExpr, ConstantKind, Type};
    let context = &module.context;
    let types = &context.types;
    if let Some((symbol, addend)) = address_of(module, layout, id)? {
        if layout.alloc_size(types, ty) != 2 {
            return Err("an address that is not 16 bits wide in a global initializer".into());
        }
        words.push((at, symbol, addend));
        return Ok(());
    }
    match (&context.get(id).kind, types.get(ty)) {
        (ConstantKind::Zero | ConstantKind::Null | ConstantKind::Undef | ConstantKind::Poison, _) => Ok(()),
        (ConstantKind::Int(value), Type::Int(bits)) => {
            let bytes = (*bits as usize).div_ceil(8);
            out[at..at + bytes].copy_from_slice(&value.to_le_bytes()[..bytes]);
            Ok(())
        }
        (ConstantKind::Float(bits), Type::Float(_)) => {
            // the soft-float image: the float's bits, little-endian
            let bytes = layout.alloc_size(types, ty) as usize;
            out[at..at + bytes].copy_from_slice(&bits.to_le_bytes()[..bytes]);
            Ok(())
        }
        // `inttoptr (i16 n)`: the number itself (an expanded block address).
        (
            ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::PtrToInt | CastOp::IntToPtr | CastOp::BitCast, value }),
            _,
        ) => write_constant(module, layout, *value, context.get(*value).ty, out, at, words),
        (ConstantKind::Bytes(data), _) => {
            out[at..at + data.len()].copy_from_slice(data);
            Ok(())
        }
        (ConstantKind::Aggregate(members), Type::Array { element, .. }) => {
            let stride = layout.alloc_size(types, *element) as usize;
            members.iter().enumerate().try_for_each(|(index, member)| {
                write_constant(module, layout, *member, *element, out, at + index * stride, words)
            })
        }
        (ConstantKind::Aggregate(members), _) if types.fields(ty).is_some() => {
            let (_, offsets) = layout.struct_layout(types, ty);
            let fields = types.fields(ty).expect("a struct").to_vec();
            members.iter().enumerate().try_for_each(|(index, member)| {
                write_constant(module, layout, *member, fields[index], out, at + offsets[index] as usize, words)
            })
        }
        _ => Err("a global initializer that is not an integer, pointer, array or struct".into()),
    }
}

/// The trips each block of each function runs, from the module a function was
/// selected from.
fn estimate_weights(
    module: &llrm_mir::module::Module,
    names: &[String],
    sources: &[Option<&llrm_mir::module::Module>],
    layout: &llrm_mir::datalayout::DataLayout,
    level: Level,
) -> Vec<Vec<u64>> {
    names
        .iter()
        .zip(sources)
        .map(|(name, source)| {
            let from = source.unwrap_or(module);
            // -O2 and -O3 judge clocks; -O1 and -Os break ties by them
            if matches!(level, Level::O2 | Level::O3) {
                return crate::weights::block_weights_at_sites(from, name, layout);
            }
            let function = from.named(name).and_then(|id| from.global(id).function());
            function.map_or_else(Vec::new, |function| crate::weights::block_weights(from, function, layout))
        })
        .collect()
}
