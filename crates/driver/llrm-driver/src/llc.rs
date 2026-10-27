//! `llrm-llc [options] FILE.ll`: LLVM IR from a front end that only parses
//! (clang's `-O2 -Xclang -disable-llvm-passes -emit-llvm`) through llrm's whole
//! pipeline and backend to an object. The target is the one whose `llvm_triple`
//! the module's `target triple` names, or the `-m` flag's.

use std::path::Path;
use std::rc::Rc;

use llrm_codegen::backend::{masm, objbuild};
use llrm_codegen::driver::flags::{Flags, write_output};
use llrm_target::Target;

use crate::Bound;

/// How the i8086 clang writes llrm's BASIC convention (1000, `opcode::BASIC`):
/// clang keeps a convention in 8 bits.
const CLANG_PASCAL: &str = " cc232 ";

/// `text` without the lines and words MIR's reader leaves out of LLVM (`target
/// triple`, `dso_local`), and the triple the module stated.
fn without_triple(text: &str) -> (Option<String>, String) {
    let mut triple = None;
    let mut kept = String::with_capacity(text.len());
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("target triple") {
            triple = rest.split('"').nth(1).map(str::to_owned);
            continue;
        }
        if line.starts_with(';') {
            continue;
        }
        let line = line.replace(" dso_local", "").replace(CLANG_PASCAL, " cc1000 ");
        kept.push_str(&port_intrinsic(&lifetime_markers_with_a_size(&line)));
        kept.push('\n');
    }
    (triple, kept)
}

/// Borland's port functions and the intrinsics that are them: name, MIR's name,
/// whether it writes.
const PORTS: [(&str, &str, bool); 4] = [
    ("__inportb__", "llrm.ia16.in.i8", false),
    ("__inportw__", "llrm.ia16.in.i16", false),
    ("__outportb__", "llrm.ia16.out.i8", true),
    ("__outportw__", "llrm.ia16.out.i16", true),
];

/// `line` with a Borland port function written as its intrinsic. An `out`
/// answers the value written, which the C function returns and clang numbers
/// even when unused, so the numbering stays dense.
fn port_intrinsic(line: &str) -> String {
    for (borland, intrinsic, writes) in PORTS {
        let Some(at) = line.find(&format!("@{borland}(")) else { continue };
        let width = intrinsic.rsplit('.').next().unwrap_or("i16");
        let parameters = if writes { format!("i16, {width}") } else { "i16".to_owned() };
        if line.starts_with("declare") {
            let result = if writes { "void" } else { width };
            return format!("declare {result} @{intrinsic}({parameters})");
        }
        let arguments =
            line[at..].split_once('(').and_then(|(_, rest)| rest.split_once(')')).map_or("", |(inside, _)| inside);
        let typed: Vec<String> = arguments
            .split(',')
            .filter_map(|one| Some(format!("{} {}", one.split_whitespace().next()?, one.split_whitespace().last()?)))
            .collect();
        let call = typed.join(", ");
        let destination = line.split_once(" = ").map(|(name, _)| name.trim().to_owned());
        return match (writes, destination) {
            (true, Some(name)) => {
                let value = typed.last().and_then(|one| one.split_whitespace().last()).unwrap_or("0");
                format!("  call void @{intrinsic}({call})\n  {name} = add {width} {value}, 0")
            }
            (true, None) => format!("  call void @{intrinsic}({call})"),
            (false, Some(name)) => format!("  {name} = call {width} @{intrinsic}({call})"),
            (false, None) => format!("  call {width} @{intrinsic}({call})"),
        };
    }
    line.to_owned()
}

/// The pin's `llvm.lifetime.start.p0(ptr)` as MIR's `(i64 size, ptr)`: MIR's
/// markers still take the size `Intrinsics.td` dropped, and a marker it cannot
/// read is a call that makes its alloca escape (nothing is promoted). The size
/// is -1, which LangRef calls unknown.
fn lifetime_markers_with_a_size(line: &str) -> String {
    for marker in ["@llvm.lifetime.start.p0(", "@llvm.lifetime.end.p0("] {
        if let Some(at) = line.find(marker) {
            let (head, tail) = line.split_at(at + marker.len());
            if tail.starts_with("i64") {
                return line.to_owned();
            }
            let size = if line.starts_with("declare") { "i64 immarg, " } else { "i64 -1, " };
            return format!("{head}{size}{tail}");
        }
    }
    line.to_owned()
}

fn bound(
    flags: &Flags,
    triple: Option<&str>,
) -> Result<Bound, String> {
    let found: Rc<dyn Target> = match (flags.mode(), triple) {
        (None, Some(triple)) => crate::by_triple(triple).ok_or_else(|| format!("no target for `{triple}`"))?,
        (_, _) => return crate::target(flags, None),
    };
    flags.format(&*found)?;
    flags.convention(&*found)?;
    let selection =
        crate::selector(found.name()).ok_or_else(|| format!("no instruction selector for {}", found.name()))?;
    Ok(Bound { target: found, selection })
}

/// `llrm-llc [-O<n>] [-m<bits>] [-march=CPU] [-S] [-o OUT] FILE.ll`: exit
/// status 0, 1 on a refusal, 2 on bad arguments.
pub fn main(argv: &[String]) -> i32 {
    let mut flags = Flags::default();
    let mut source = None;
    let mut at = 0;
    while at < argv.len() {
        match flags.take(argv, &mut at) {
            Ok(true) => {}
            Ok(false) if !argv[at].starts_with('-') && source.is_none() => source = Some(argv[at].clone()),
            Ok(false) => {
                eprintln!("llrm-llc: unrecognized argument {}", argv[at]);
                return 2;
            }
            Err(why) => {
                eprintln!("llrm-llc: {why}");
                return 2;
            }
        }
        at += 1;
    }
    let Some(source) = source else {
        eprintln!("usage: llrm-llc [-O2] [-m64] [-S] [-o OUT] FILE.ll");
        return 2;
    };
    match compile(&flags, Path::new(&source)) {
        Ok(()) => 0,
        Err(why) => {
            eprintln!("llrm-llc: {why}");
            1
        }
    }
}

/// Every symbol as `format` spells it under its convention (`_main`, `main_`,
/// `MAIN`): a C front end for these targets records decorated names, and the
/// start-up and the OS layer call them so. A function takes its own
/// convention's spelling, a variable C's. Intrinsics keep
/// theirs.
fn decorate(
    module: &mut llrm_mir::module::Module,
    calling: &llrm_target::calling::Calling,
    default: &llrm_target::calling::Convention,
    format: &str,
) {
    for global in &mut module.globals {
        // A variable takes C's spelling (`_name`) under every ABI, Open
        // Watcom's register one included.
        let convention = match global.function() {
            Some(one) => calling.by_number(one.calling_convention).unwrap_or(default),
            None => calling.by_number(0).unwrap_or(default),
        };
        let Some(name) = global.name.as_deref().filter(|name| !name.starts_with("llvm.") && !name.starts_with("llrm."))
        else {
            continue;
        };
        // LLVM's `\01` marker (an asm label, `#pragma aux NAME "LINK"`): the
        // symbol as written.
        let before = name.to_owned();
        let spelled = match name.strip_prefix('\u{1}') {
            Some(exact) => Some(exact.to_owned()),
            None => convention.decorated(format, name),
        };
        if let Some(spelled) = spelled {
            if let Some(section) = module.sections.remove(&before) {
                module.sections.insert(spelled.clone(), section);
            }
            global.name = Some(spelled).into();
        }
    }
}

/// Convention 0 is LLVM's `ccc`, which clang writes for every function it gave
/// no convention: here the one `-mabi` chose, in the functions and in their
/// calls. LLVM's and llrm's own intrinsics stay as they are.
fn with_default_convention(
    module: &mut llrm_mir::module::Module,
    default: &llrm_target::calling::Convention,
) {
    let Some(number) = llrm_target::calling::Calling::number_of(default.cc.as_deref()).filter(|number| *number != 0)
    else {
        return;
    };
    for global in &mut module.globals {
        if global.name.as_deref().is_some_and(|name| name.starts_with("llvm.") || name.starts_with("llrm.")) {
            continue;
        }
        let llrm_mir::module::GlobalKind::Function(function) = &mut global.kind else { continue };
        if function.calling_convention == 0 {
            function.calling_convention = number;
        }
        let calls: Vec<_> = function
            .walk()
            .filter_map(|(_, inst)| match &function.instruction(inst).opcode {
                llrm_mir::opcode::Opcode::Call(info) | llrm_mir::opcode::Opcode::Invoke(info)
                    if info.calling_convention == 0 =>
                {
                    Some(inst)
                }
                _ => None,
            })
            .collect();
        for inst in calls {
            function.set_call_convention(inst, number);
        }
    }
}

/// `text`, LLVM IR, compiled for the target its `target triple` (or `flags`)
/// names.
pub fn compiled(
    flags: &Flags,
    text: &str,
    name: &str,
) -> Result<(masm::Module, llrm_target::object::Format), String> {
    let (triple, text) = without_triple(text);
    let bound = bound(flags, triple.as_deref())?;
    let mut module = llrm_mir::parse::module(&text).map_err(|error| format!("{name}: {error}"))?;
    let machine = flags.machine(&*bound.target, bound.target.machine())?;
    let format = flags.format(&*bound.target)?;
    let default = flags.convention(&*bound.target)?;
    with_default_convention(&mut module, default);
    decorate(&mut module, bound.target.calling(), default, format.name());
    let mut options = llrm_codegen::driver::Options { object_format: format.name(), ..bound.options(flags, machine) };
    options.libcalls.extend(libm_calls(default, format.name()));
    Ok((llrm_codegen::driver::compiled_module(module, name, &options)?, format))
}

/// The C library's rounding routines, in the convention and spelling a call
/// the source declared would have: what `llvm.floor` and its family become
/// where the target has no instruction for them. A C program that made one
/// called the routine, so its runtime has it.
fn libm_calls(
    default: &llrm_target::calling::Convention,
    format: &str,
) -> Vec<(String, u32, String)> {
    let number = llrm_target::calling::Calling::number_of(default.cc.as_deref()).unwrap_or(0);
    ["floor", "ceil", "trunc", "round", "roundeven", "nearbyint"]
        .iter()
        .flat_map(|op| ["", "f", "l"].map(|suffix| format!("{op}{suffix}")))
        .map(|name| {
            let symbol = default.decorated(format, &name).unwrap_or_else(|| name.clone());
            (name, number, symbol)
        })
        .collect()
}

fn compile(
    flags: &Flags,
    source: &Path,
) -> Result<(), String> {
    let text = std::fs::read_to_string(source).map_err(|error| format!("{}: {error}", source.display()))?;
    let name = source.file_stem().and_then(|one| one.to_str()).unwrap_or("module");
    let (built, format) = compiled(flags, &text, name)?;
    let output = flags.output.clone().unwrap_or_else(|| source.with_extension(if flags.assembly { "s" } else { "o" }));
    let file = source.file_name().and_then(|one| one.to_str()).unwrap_or_default();
    if flags.assembly {
        write_output(&output, masm::text(&built).map_err(|error| error.to_string())?)
    } else {
        let bytes = objbuild::written_in(&built, file, objbuild::CodeLayout::OneSegment, format)
            .map_err(|error| error.to_string())?;
        write_output(&output, bytes)
    }
    .map_err(|error| error.to_string())
}
