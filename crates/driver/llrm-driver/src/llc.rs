//! `llrm-llc [options] FILE.ll`: LLVM IR from a front end that only parses
//! (clang's `-O2 -Xclang -disable-llvm-passes -emit-llvm`) through llrm's whole
//! pipeline and backend to an object. The target is the one whose `llvm_triple`
//! the module's `target triple` names, or the `-m` flag's.

use std::path::{Path, PathBuf};
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
        // Most lines carry none of the words below: they are copied as they
        // are.
        if !line.contains(" dso_local") && !line.contains(CLANG_PASCAL) && !line.contains('@') {
            kept.push_str(line);
            kept.push('\n');
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

/// `setFunctionAttributes` of `CommandFlags.cpp`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/CommandFlags.cpp#L664>):
/// `-frame-pointer=` gives each function without a `"frame-pointer"` attribute
/// that policy.
fn with_frame_pointer(
    module: &mut llrm_mir::module::Module,
    policy: &str,
) {
    use llrm_mir::opcode::Attribute;
    for global in &mut module.globals {
        let llrm_mir::module::GlobalKind::Function(function) = &mut global.kind else { continue };
        if function.is_declaration()
            || function.attrs.iter().any(|one| matches!(one, Attribute::Str(key, _) if key == "frame-pointer"))
        {
            continue;
        }
        function.attrs.push(Attribute::Str("frame-pointer".to_owned(), Some(policy.to_owned())));
    }
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
    let mut dump = None;
    let mut at = 0;
    while at < argv.len() {
        if let Some(value) = argv[at].strip_prefix("--dump=") {
            dump = Some(PathBuf::from(value));
            at += 1;
            continue;
        }
        if argv[at] == "--dump" {
            at += 1;
            let Some(value) = argv.get(at) else {
                eprintln!("llrm-llc: --dump: expected one argument");
                return 2;
            };
            dump = Some(PathBuf::from(value));
            at += 1;
            continue;
        }
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
    match compile(&flags, Path::new(&source), dump.as_deref()) {
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
) -> Vec<(String, String)> {
    let mut spellings = vec![];
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
            if global.function().is_some() {
                spellings.push((before, spelled.clone()));
            }
            global.name = Some(spelled).into();
        }
    }
    spellings
}

/// Convention 0 is LLVM's `ccc`, which clang writes for every function it gave
/// no convention: here the one `-mabi` chose, in the functions and in their
/// calls. LLVM's and llrm's own intrinsics stay as they are.
///
/// With `c_library` the library's functions (`cos`, `strlen`) keep their C
/// convention, as the Borland library the program links has them.
///
/// With `regcall_default` the front end marked the unmarked ones
/// `x86_regcallcc` itself, so those take the ABI's convention and `ccc` is an
/// explicit `__cdecl`, which stays C's. A variadic function stays on the ABI's:
/// clang never marks one, and Open Watcom spells it as its own.
fn with_default_convention(
    module: &mut llrm_mir::module::Module,
    default: &llrm_target::calling::Convention,
    regcall_default: bool,
    c_library: bool,
) {
    let Some(number) = llrm_target::calling::Calling::number_of(default.cc.as_deref()).filter(|number| *number != 0)
    else {
        return;
    };
    let takes = |convention: u32, variadic: bool| match (regcall_default, convention) {
        (false, 0) => true,
        (true, llrm_mir::opcode::REGCALL) => true,
        (true, 0) => variadic,
        _ => false,
    };
    // Under the register-size convention `main` is Watcom's, `main_`: both
    // its start-up and the repo's (`LL_MAIN`) call that.
    let main_is_watcom = default.cc.as_deref() == Some("regparm3");
    // Where the ABI is the register-size one, the library's functions are Open
    // Watcom's whichever way the front end marks the rest.
    let library_is_watcom = (!regcall_default || main_is_watcom) && !c_library;
    // clang gives the library functions it knows (`strlen`, `atan`) their C
    // convention whatever the default is, so `_strlen` was called where the
    // library has `strlen_`: a declaration or a definition of one (the float
    // routines of floatm.c) takes the default too.
    let library: llrm_mir::hash::HashSet<llrm_mir::context::GlobalId> = {
        module
            .globals
            .iter()
            .enumerate()
            .filter(|(_, global)| {
                matches!(
                    &global.kind,
                    llrm_mir::module::GlobalKind::Function(function) if function.calling_convention == 0
                )
            })
            .filter(|(_, global)| {
                global
                    .name
                    .as_deref()
                    .is_some_and(|name| llrm_mir::libfunc::TargetLibraryInfo::get_lib_func_by_name(name).is_some())
            })
            .map(|(at, _)| llrm_mir::context::GlobalId(at as u32))
            .collect()
    };
    let types = &module.context.types;
    let is_variadic = |ty| matches!(
        types.get(ty),
        llrm_mir::types::Type::Function { variadic: true, .. }
    );
    for (at, global) in module.globals.iter_mut().enumerate() {
        if global.name.as_deref().is_some_and(|name| name.starts_with("llvm.") || name.starts_with("llrm.")) {
            continue;
        }
        let llrm_mir::module::GlobalKind::Function(function) = &mut global.kind else { continue };
        let kept_c = c_library && library.contains(&llrm_mir::context::GlobalId(at as u32));
        // `main` is the ABI's, as the start-up calls it; clang leaves it `ccc`.
        let entry = global.name.as_deref() == Some("main");
        // A function the front end marked `regcall` where the default is not
        // is the Watcom register convention (`__watcall` of its headers).
        let watcom = (entry && main_is_watcom)
            || (!regcall_default && function.calling_convention == llrm_mir::opcode::REGCALL)
            || (library_is_watcom && library.contains(&llrm_mir::context::GlobalId(at as u32)));
        if !kept_c
            && (watcom
                || takes(
                    function.calling_convention,
                    is_variadic(function.ty) || entry || library.contains(&llrm_mir::context::GlobalId(at as u32)),
                ))
        {
            // clang spells a regcall function `__regcall3__NAME`.
            if function.calling_convention == llrm_mir::opcode::REGCALL {
                if let Some(plain) = global.name.as_deref().and_then(|name| name.strip_prefix("__regcall3__")) {
                    let plain = plain.to_owned();
                    if let Some(section) = module.sections.remove(global.name.as_deref().unwrap_or_default()) {
                        module.sections.insert(plain.clone(), section);
                    }
                    global.name = Some(plain).into();
                }
            }
            function.calling_convention = if watcom { llrm_mir::opcode::WATCALL } else { number };
        }
        if library_is_watcom {
            let watcom: Vec<_> = function
                .walk()
                .filter_map(|(_, inst)| match &function.instruction(inst).opcode {
                    llrm_mir::opcode::Opcode::Call(info) | llrm_mir::opcode::Opcode::Invoke(info)
                        if (!regcall_default && info.calling_convention == llrm_mir::opcode::REGCALL)
                            || (info.calling_convention == 0
                                && llrm_mir::memory::callee(&module.context, function, inst)
                                    .is_some_and(|one| library.contains(&one))) =>
                    {
                        Some(inst)
                    }
                    _ => None,
                })
                .collect();
            for inst in watcom {
                function.set_call_convention(inst, llrm_mir::opcode::WATCALL);
            }
        }
        let calls: Vec<_> = function
            .walk()
            .filter_map(|(_, inst)| match &function.instruction(inst).opcode {
                llrm_mir::opcode::Opcode::Call(info) | llrm_mir::opcode::Opcode::Invoke(info)
                    if !(c_library
                        && llrm_mir::memory::callee(&module.context, function, inst)
                            .is_some_and(|one| library.contains(&one)))
                        && takes(
                            info.calling_convention,
                            is_variadic(info.function_type)
                                || (regcall_default
                                    && llrm_mir::memory::callee(&module.context, function, inst)
                                        .is_some_and(|one| library.contains(&one))),
                        ) =>
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
    compiled_dumping(flags, text, name, None)
}

/// `compiled`, writing each stage to `dump` (llrm-c's `--dump DIR`) or where
/// `LLRM_MIR_STAGES` names.
pub fn compiled_dumping(
    flags: &Flags,
    text: &str,
    name: &str,
    dump: Option<&Path>,
) -> Result<(masm::Module, llrm_target::object::Format), String> {
    let (triple, text) = without_triple(text);
    let bound = bound(flags, triple.as_deref())?;
    let mut module = llrm_mir::parse::module(&text).map_err(|error| format!("{name}: {error}"))?;
    if llrm_codegen::support::debug::enabled("mir") {
        // The size of the program as read, before any pass: what the other
        // front ends report as they link their MIR.
        let bodies =
            || module.globals.iter().filter_map(|global| global.function()).filter(|one| !one.is_declaration());
        let instructions: usize =
            bodies().map(|one| one.layout().iter().map(|&block| one.block(block).len()).sum::<usize>()).sum();
        eprintln!("[mir] functions {} instructions {}", bodies().count(), instructions);
    }
    let machine = flags.machine(&*bound.target, bound.target.machine())?;
    let format = flags.format(&*bound.target)?;
    let default = flags.convention(&*bound.target)?;
    refuse_debug_format(flags, format.name(), bound.target.object().bitness)?;
    refuse_weak(&module, format.name())?;
    refuse_foreign_conventions(&module)?;
    allow_near_calls(&mut module);
    if let Some(policy) = &flags.frame_pointer {
        with_frame_pointer(&mut module, policy);
    }
    with_default_convention(&mut module, default, flags.regcall_default, flags.c_library);
    crate::coerce::small_structs_as_scalars(&mut module, bound.target.calling())?;
    let spellings = decorate(&mut module, bound.target.calling(), default, format.name());
    // After the rename: the debug info names the symbols the object has.
    llrm_mir::dbgimport::types_and_globals(&mut module);
    let mut options = options_for(&bound, flags, machine, format.name(), default);
    // `-fno-inline=NAME` takes a function as the source names it, whatever
    // the convention spells it as; a name already spelled stays.
    for asked in &mut options.no_inline {
        if let Some((_, spelled)) = spellings.iter().find(|(source, _)| source == asked) {
            *asked = spelled.clone();
        }
    }
    if let Some(dump) = dump {
        std::fs::create_dir_all(dump).map_err(|error| format!("{}: {error}", dump.display()))?;
        options.dump = Some(dump.to_path_buf());
    }
    // The Watcom library is the register convention where the default is
    // regparm3.
    let library = default
        .cc
        .as_deref()
        .filter(|cc| *cc == "regparm3")
        .and_then(|_| bound.target.calling().by_number(llrm_mir::opcode::WATCALL))
        .unwrap_or(default);
    options.libcalls.extend(libm_calls(library, format.name()));
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

/// `-gcodeview` and `-gtd` where the object format cannot carry them, with the
/// words the debug writers use: an object that holds another flavor than the
/// one asked for must not be written.
fn refuse_debug_format(
    flags: &Flags,
    format: &str,
    bitness: u32,
) -> Result<(), String> {
    use llrm_object::debug::Format;
    if !flags.debug {
        return Ok(());
    }
    match flags.debug_format {
        Format::CodeView if !matches!(format, "omf" | "coff") => {
            Err(format!("-gcodeview: the {format} object format cannot carry CodeView"))
        }
        Format::TurboDebugger if format != "omf" => {
            Err(format!("-gtd: the {format} object format cannot carry Turbo Debugger's information"))
        }
        Format::TurboDebugger if bitness != 16 => {
            Err("-gtd: Borland's records are 16-bit; use a 16-bit target".to_owned())
        }
        _ => Ok(()),
    }
}

/// A C `static` function may be entered by a near call wherever only its own
/// module calls it, as the other frontends allow (`nearcode`); the pass
/// that moves it checks the calls.
fn allow_near_calls(module: &mut llrm_mir::module::Module) {
    use llrm_mir::module::{GlobalKind, Linkage};
    for global in &mut module.globals {
        if !matches!(global.linkage, Linkage::Internal | Linkage::Private) {
            continue;
        }
        let GlobalKind::Function(function) = &mut global.kind else { continue };
        if !function.is_declaration() {
            function.attrs.push(llrm_mir::opcode::Attribute::Flag(llrm_mir::callgraph::NEAR_CODE.to_owned()));
        }
    }
}

/// Refuses `__stdcall` and `__fastcall` (clang's `x86_stdcallcc`,
/// `x86_fastcallcc`): no target here has either, and the backend would
/// say only "calling convention 64".
fn refuse_foreign_conventions(module: &llrm_mir::module::Module) -> Result<(), String> {
    for global in &module.globals {
        let llrm_mir::module::GlobalKind::Function(function) = &global.kind else { continue };
        let Some((name, _)) = llrm_mir::opcode::CONVENTIONS.iter().find(|(name, number)| {
            *number == function.calling_convention && matches!(*name, "x86_stdcallcc" | "x86_fastcallcc")
        }) else {
            continue;
        };
        let function_name = global.name.as_deref().unwrap_or("?");
        return Err(format!(
            "@{function_name}: {} is not a calling convention this target has; use __cdecl or the target's own",
            name.trim_start_matches("x86_").trim_end_matches("cc")
        ));
    }
    Ok(())
}

/// The object formats that cannot say a symbol is weak or merged: a definition
/// would link as a strong one and change what the program means.
const NO_WEAK_SYMBOLS: [&str; 1] = ["omf"];

/// Refuses a weak, `linkonce` or `extern_weak` symbol where the object format
/// has no way to say so (`__attribute__((weak))`, `#pragma weak`), naming it.
fn refuse_weak(
    module: &llrm_mir::module::Module,
    format: &str,
) -> Result<(), String> {
    use llrm_mir::module::Linkage;
    if !NO_WEAK_SYMBOLS.contains(&format) {
        return Ok(());
    }
    for global in &module.globals {
        let kind = match global.linkage {
            Linkage::Weak | Linkage::WeakOdr => "weak",
            Linkage::LinkOnce | Linkage::LinkOnceOdr => "linkonce",
            Linkage::ExternWeak => "extern_weak",
            _ => continue,
        };
        let name = global.name.as_deref().unwrap_or("?");
        return Err(format!(
            "@{name}: {kind} linkage cannot be written in the {format} object format; the symbol would link as a strong one"
        ));
    }
    Ok(())
}

/// The driver's options for IR from the pinned clang: what the Watcom front
/// end set for C, which `llrm-llc` now serves. The pin's float spills round to
/// the type; the C runtime's library routines are called (`memcpy` as the
/// convention spells it) and `strlen` is known where the convention is System
/// V's (DEVIATE D11 of `pipeline::Options::library`).
fn options_for(
    bound: &Bound,
    flags: &Flags,
    machine: llrm_codegen::abi::machine::Machine,
    format: &'static str,
    default: &llrm_target::calling::Convention,
) -> llrm_codegen::driver::Options {
    let mut options = llrm_codegen::driver::Options {
        object_format: format,
        rounds_float_spills: true,
        ..bound.options(flags, machine)
    };
    let number = llrm_target::calling::Calling::number_of(default.cc.as_deref());
    let pattern = default.symbol.get(format).map_or("*", String::as_str);
    if let (Some(number), Some(description)) = (number, bound.target.runtime("c")) {
        let names = description
            .table()
            .ok()
            .and_then(|table| table.get("libcalls").and_then(|one| one.as_array()).cloned())
            .unwrap_or_default();
        options.libcalls = names
            .iter()
            .filter_map(|one| one.as_str())
            .map(|name| (name.to_owned(), number, pattern.replace('*', name)))
            .collect();
        if default.cc.as_deref() == Some("sysv") && default.symbol.contains_key(format) {
            options.pipeline.library = vec![llrm_mir::libfunc::Routine {
                standard: "strlen",
                symbol: pattern.replace('*', "strlen"),
                calling_convention: number,
            }];
        }
    }
    options
}

#[cfg(test)]
mod options_tests {
    use super::*;

    fn options(
        arguments: &[&str],
        format: &'static str,
    ) -> llrm_codegen::driver::Options {
        let argv: Vec<String> = arguments.iter().map(|one| one.to_string()).collect();
        let mut flags = Flags::default();
        let mut at = 0;
        while at < argv.len() {
            assert!(flags.take(&argv, &mut at).unwrap());
            at += 1;
        }
        let bound = bound(&flags, None).unwrap();
        let machine = flags.machine(&*bound.target, bound.target.machine()).unwrap();
        let default = flags.convention(&*bound.target).unwrap();
        options_for(&bound, &flags, machine, format, default)
    }

    /// `llrm-cc --dump DIR` reached the backend as an input file: "cannot
    /// specify -o with several inputs", and no stage was written.
    #[test]
    fn dump_writes_the_stages() {
        let dir = std::env::temp_dir().join(format!("llrm-llc-dump-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                    target triple = \"x86_64-unknown-linux-gnu\"\n\
                    define i32 @f(i32 %a) {\n  ret i32 %a\n}\n";
        compiled_dumping(&Flags::default(), text, "f", Some(&dir)).unwrap();
        let written = std::fs::read_dir(&dir).unwrap().count();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(written > 0, "no stage written");
    }

    /// `llrm-cc` ran the backend with none of what the Watcom front end set
    /// for C: float spills kept 80 bits, `memcpy` stayed `rep movs`, and no
    /// loop became `strlen`.
    #[test]
    fn the_backend_serves_c_as_the_watcom_front_end_did() {
        let watcom = options(&["-m32", "-mabi=watcom"], "omf");
        assert!(watcom.rounds_float_spills);
        assert!(
            watcom.libcalls.iter().any(|(name, _, symbol)| name == "memcpy" && symbol == "memcpy_"),
            "{:?}",
            watcom.libcalls
        );
        assert!(watcom.pipeline.library.is_empty());
        let sysv = options(&["-m32", "-mabi=sysv"], "omf");
        assert!(
            sysv.libcalls.iter().any(|(name, _, symbol)| name == "memcpy" && symbol == "_memcpy"),
            "{:?}",
            sysv.libcalls
        );
        assert_eq!(sysv.pipeline.library.len(), 1);
        assert_eq!(sysv.pipeline.library[0].symbol, "_strlen");
    }
}

fn compile(
    flags: &Flags,
    source: &Path,
    dump: Option<&Path>,
) -> Result<(), String> {
    let text = std::fs::read_to_string(source).map_err(|error| format!("{}: {error}", source.display()))?;
    let name = source.file_stem().and_then(|one| one.to_str()).unwrap_or("module");
    let (built, format) = compiled_dumping(flags, &text, name, dump)?;
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

#[cfg(test)]
mod text_tests {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    use super::without_triple;

    thread_local! {
        static COUNTING: Cell<Option<usize>> = const { Cell::new(None) };
    }

    struct Counting;

    // SAFETY: every call is `System`'s; the counter is a thread-local `Cell`.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(
            &self,
            layout: Layout,
        ) -> *mut u8 {
            let _ = COUNTING.try_with(|count| count.set(count.get().map(|n| n + 1)));
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(
            &self,
            pointer: *mut u8,
            layout: Layout,
        ) {
            unsafe { System.dealloc(pointer, layout) }
        }
    }

    #[global_allocator]
    static ALLOCATOR: Counting = Counting;

    /// Every line of the text was rewritten into fresh strings (two `replace`s,
    /// a `format!` per port name, a copy per marker): 5,300 lines of a
    /// jpeg_fixed.ll cost 23,000 string searches and as many allocations. A
    /// line none of the rewrites touches costs none.
    #[test]
    fn a_plain_line_is_copied_once_into_the_text() {
        let line = "  %5 = add nsw i32 %3, %4\n";
        let text = line.repeat(5000);
        COUNTING.with(|count| count.set(Some(0)));
        let (_, kept) = without_triple(&text);
        let made = COUNTING.with(|count| count.take()).expect("counting");
        assert_eq!(kept, text);
        assert!(made < 50, "{made} allocations for 5000 untouched lines");
    }

    #[test]
    fn the_rewrites_still_apply() {
        let text = "target triple = \"i386\"\n; c\n@x = dso_local global i32 0\ndeclare void @llvm.lifetime.start.p0(ptr)\n  %1 = call i16 @__inportw__(i16 %0)\n";
        let (triple, kept) = without_triple(text);
        assert_eq!(triple.as_deref(), Some("i386"));
        assert_eq!(
            kept,
            "@x = global i32 0\ndeclare void @llvm.lifetime.start.p0(i64 immarg, ptr)\n  %1 = call i16 @llvm.ia16.in.i16(i16 %0)\n"
                .replace("llvm.ia16", "llrm.ia16")
        );
    }
}
