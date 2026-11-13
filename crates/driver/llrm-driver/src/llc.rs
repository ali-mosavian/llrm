//! `llrm-llc [options] FILE.ll`: LLVM IR from a front end that only parses
//! (clang's `-O2 -Xclang -disable-llvm-passes -emit-llvm`) through llrm's whole
//! pipeline and backend to an object. The target is the one whose `llvm_triple`
//! the module's `target triple` names, or the `-m` flag's.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use llrm_codegen::backend::{masm, objbuild};
use llrm_codegen::driver::flags::{Flags, write_output};
use llrm_target::Target;

use std::borrow::Cow;

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
        let line = if line.contains(" dso_local") || line.contains(CLANG_PASCAL) {
            Cow::Owned(line.replace(" dso_local", "").replace(CLANG_PASCAL, " cc1000 "))
        } else {
            Cow::Borrowed(line)
        };
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
fn port_intrinsic(line: &str) -> Cow<'_, str> {
    for (borland, intrinsic, writes) in PORTS {
        // Most lines name none: no string is made to look.
        if !line.contains(borland) {
            continue;
        }
        let Some(at) = line.find(&format!("@{borland}(")) else { continue };
        let width = intrinsic.rsplit('.').next().unwrap_or("i16");
        let parameters = if writes { format!("i16, {width}") } else { "i16".to_owned() };
        if line.starts_with("declare") {
            let result = if writes { "void" } else { width };
            return format!("declare {result} @{intrinsic}({parameters})").into();
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
        }
        .into();
    }
    Cow::Borrowed(line)
}

/// The pin's `llvm.lifetime.start.p0(ptr)` as MIR's `(i64 size, ptr)`: MIR's
/// markers still take the size `Intrinsics.td` dropped, and a marker it cannot
/// read is a call that makes its alloca escape (nothing is promoted). The size
/// is -1, which LangRef calls unknown.
fn lifetime_markers_with_a_size(line: &str) -> Cow<'_, str> {
    for marker in ["@llvm.lifetime.start.p0(", "@llvm.lifetime.end.p0("] {
        if let Some(at) = line.find(marker) {
            let (head, tail) = line.split_at(at + marker.len());
            if tail.starts_with("i64") {
                return Cow::Borrowed(line);
            }
            let size = if line.starts_with("declare") { "i64 immarg, " } else { "i64 -1, " };
            return format!("{head}{size}{tail}").into();
        }
    }
    Cow::Borrowed(line)
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
        function.attrs.push(Attribute::Str("frame-pointer".into(), Some(policy.to_owned())));
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
    // An alias is spelled as what it names is.
    for at in 0..module.aliases.len() {
        let aliasee = module.aliases[at].aliasee;
        let convention = match module.global(aliasee).function() {
            Some(one) => calling.by_number(one.calling_convention).unwrap_or(default),
            None => calling.by_number(0).unwrap_or(default),
        };
        let mut name = module.aliases[at].name.clone();
        // clang spells a regcall alias `__regcall3__NAME`, as its function; the
        // function lost the prefix above when it took the ABI's convention.
        if let Some(plain) = name.strip_prefix("__regcall3__")
            && module.global(aliasee).function().is_some_and(|one| one.calling_convention != llrm_mir::opcode::REGCALL)
        {
            name = plain.to_owned();
        }
        let spelled = match name.strip_prefix('\u{1}') {
            Some(exact) => Some(exact.to_owned()),
            None => convention.decorated(format, &name),
        };
        module.aliases[at].name = spelled.unwrap_or(name);
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
    watcom_library: bool,
) {
    let Some(number) = llrm_target::calling::Calling::number_of(default.cc.as_deref()).filter(|number| *number != 0)
    else {
        return;
    };
    let served: Vec<u32> =
        default.serves.iter().filter_map(|name| llrm_target::calling::Calling::number_of(Some(name))).collect();
    let takes = |convention: u32, variadic: bool| match (regcall_default, convention) {
        (_, other) if served.contains(&other) => true,
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
    let library_is_watcom = watcom_library && (!regcall_default || main_is_watcom) && !c_library;
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

/// `compiled`, writing each stage to `dump` (llrm-cc's `--dump DIR`) or where
/// `LLRM_MIR_STAGES` names.
pub fn compiled_dumping(
    flags: &Flags,
    text: &str,
    name: &str,
    dump: Option<&Path>,
) -> Result<(masm::Module, llrm_target::object::Format), String> {
    let (triple, text) = without_triple(text);
    let bound = bound(flags, triple.as_deref())?;
    // The system the module's triple names writes its own object format unless
    // `-fobject-format=` says otherwise.
    let system = bound
        .target
        .layout()
        .platforms
        .into_iter()
        .find(|one| triple.as_deref().is_some_and(|stated| crate::same_system(&one.triple, stated)));
    if let Some(why) = system.as_ref().and_then(|one| one.pending.as_ref()).filter(|_| !flags.allow_pending_abi) {
        return Err(format!("{}: {why}; -fallow-pending-abi emits it anyway", triple.as_deref().unwrap_or_default()));
    }
    let mut flags = flags.clone();
    if let Some(system) = system {
        if flags.object_format.is_none() {
            flags.object_format = Some(llrm_target::object::Format::parse(&system.format)?);
        }
        if flags.abi.is_none() {
            flags.abi = system.abi;
        }
    }
    let flags = &flags;
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
    // Open Watcom's library is the DOS one: the other systems' own libc keeps
    // C's convention.
    with_default_convention(&mut module, default, flags.regcall_default, flags.c_library, format.name() == "omf");
    crate::coerce::small_structs_as_scalars(&mut module, bound.target.calling())?;
    crate::coerce::small_struct_results_as_scalars(&mut module, bound.target.calling())?;
    crate::coerce::far_result_pointers(&mut module, bound.target.calling(), bound.target.layout().spaces.roles.clone())?;
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
    // The soft-float routines are C compiled under the program's own
    // convention, not the Watcom library's.
    options.libcalls.extend(soft_float_calls(default, format.name()));
    let objects = bound.target.objects();
    let bytes = |built: &masm::Module| {
        objbuild::written_in(built, name, layout_of(flags, format), format, objects).ok().map(|object| object.len() as i64)
    };
    // A bytes-strict target's trials compare the code the instructions fill (`Tti::unroll_trial_bytes_strict`).
    let flat = options.cpu().ok().and_then(|cpu| cpu.target().tti()).is_some_and(|tti| tti.unroll_trial_bytes_strict());
    let unpadded = |built: &masm::Module| {
        objbuild::unpadded_split(built, name, layout_of(flags, format), format, objects, flat)
            .ok()
            .map(|(code, data)| llrm_codegen::driver::trial::Sizes { code, data })
    };
    Ok((llrm_codegen::driver::trial::compiled_module_trying(module, name, &options, &bytes, &unpadded)?, format))
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

/// The soft-float routines, spelled as the C compiler spells a definition of
/// them: what a function with no x87 calls for a float operation. The runtime
/// (`runtime/compiler-rt`) is C, so its objects carry the convention's
/// spelling.
fn soft_float_calls(
    default: &llrm_target::calling::Convention,
    format: &str,
) -> Vec<(String, u32, String)> {
    let number = llrm_target::calling::Calling::number_of(default.cc.as_deref()).unwrap_or(0);
    let names = llrm_codegen::backend::selectiondag::libcalls::names().map(str::to_owned);
    names
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
    use llrm_debug::Format;
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
            function.attrs.push(llrm_mir::opcode::Attribute::Flag(llrm_mir::callgraph::NEAR_CODE.into()));
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
        // `strlen` where the convention is System V's (DEVIATE D11 of
        // `pipeline::Options::library`); the compare routines wherever the
        // object is ELF, whose C library is a hosted one (`bcmp` is not in
        // Watcom's or DOS's).
        let stated: Option<Vec<&'static str>> = match default.cc.as_deref() {
            Some("sysv") if default.symbol.contains_key(format) => {
                Some(["strlen"].into_iter().chain(llrm_mir::libfunc::HOSTED_COMPARE).collect())
            }
            _ if format == "elf" => Some(llrm_mir::libfunc::HOSTED_COMPARE.to_vec()),
            _ => None,
        };
        // DEVIATE DIV64-LIBCALL (owner x86-m32, exit 2026-11-20): a hosted
        // 32-bit object calls libgcc's i64 divide routines, as the pin does;
        // the inline routine stays where the runtime lists none (DOS).
        if format == "elf" && bound.target.layout().mode == 32 {
            options.libcalls.extend(
                ["__divdi3", "__udivdi3", "__moddi3", "__umoddi3"]
                    .map(|name| (name.to_owned(), number, pattern.replace('*', name))),
            );
        }
        if let Some(names) = stated {
            options.pipeline.library =
                llrm_mir::libfunc::Routine::stated(names, |name| pattern.replace('*', name), number);
        }
    }
    // The pin's clang IR at 16 and 32 bits has the forwarding of a global's
    // store the write-through `Promote` stands in for at -O1 (lru -29 B, -16
    // clk at m16; -11 clk, -4 B at m32, the only m32 cell it moves). Nib and
    // BASIC build their options by `m16_options` and keep it.
    if matches!(bound.target.layout().mode, 16 | 32) {
        options.pipeline.write_through = false;
    }
    // At -O1 the forwarding made jpeg_fixed/jpeg_single 345-348 B larger at m64
    // (the pin's -O1 forwards it in InstCombine, not here); -Og, which has no
    // GVN either, is smaller with it (row WT1, owner x86-m64, exit 2026-11-14).
    if bound.target.layout().mode == 64 && flags.level == llrm_codegen::driver::flags::Level::O1 {
        options.pipeline.write_through = false;
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

    /// The write-through `Promote` ran on the pin's clang IR at m16 and m32 -O1
    /// and made `lru` larger (29 B at m16); the frontends that build for
    /// m16 alone (Nib, BASIC) have no GVN there and keep it.
    #[test]
    fn clang_ir_at_m16_and_m32_has_no_write_through_and_the_m16_frontends_keep_it() {
        assert!(!options(&["-m16"], "omf").pipeline.write_through);
        assert!(!options(&["-m32"], "omf").pipeline.write_through);
        assert!(crate::m16_options(crate::m16_machine()).pipeline.write_through);
    }

    /// The write-through `Promote` made jpeg_fixed -O1 348 B and jpeg_single
    /// -O1 345 B larger at m64; it stays on at -Og and at the levels with GVN
    /// `pre` (where it does not run); m16 and m32 never have it.
    #[test]
    fn clang_ir_at_o1_has_no_write_through_and_og_keeps_it() {
        assert!(!options(&["-m64", "-O1"], "elf").pipeline.write_through);
        assert!(!options(&["-m32", "-O1"], "elf").pipeline.write_through);
        assert!(options(&["-m64", "-Og"], "elf").pipeline.write_through);
        assert!(options(&["-m64", "-O2"], "elf").pipeline.write_through);
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

    /// A Windows module's `fastcc` function took System V's registers
    /// (`edi`, `esi`) while its `ccc` ones took Win64's: `hanoi`'s recursion
    /// called with `edi` and `esi` and read its own arguments from `ecx` and
    /// `edx`. On a Win64 subtarget every convention is `CC_X86_Win64_C`.
    #[test]
    fn a_windows_fastcc_function_and_its_call_take_the_win64_registers() {
        let text = "target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                    target triple = \"x86_64-pc-windows-msvc\"\n\
                    define internal fastcc i32 @sub(i32 %a, i32 %b) noinline {\n  %r = sub i32 %a, %b\n  ret i32 %r\n}\n\
                    define i32 @f(i32 %x, i32 %y) {\n  %r = call fastcc i32 @sub(i32 %y, i32 %x)\n  ret i32 %r\n}\n";
        let mut flags = Flags::default();
        flags.allow_pending_abi = true;
        let (built, _) = compiled_dumping(&flags, text, "w", None).unwrap();
        let assembly = masm::text(&built).unwrap();
        let (callee, caller) = assembly.split_once("f proc near").expect("both functions");
        assert!(callee.contains("sub eax, edx"), "the callee reads ecx and edx:\n{assembly}");
        assert!(caller.contains("mov ecx, edx") && !caller.contains("edi") && !caller.contains("esi"), "{assembly}");
    }

    /// Every Win64 call moved the stack pointer around its 32 bytes of home
    /// area (`sub rsp, 32` / `add rsp, 32` per call, `hanoi`'s loop included);
    /// the pin reserves the area in the prologue, the only place the stack
    /// pointer may change on Win64.
    #[test]
    fn a_windows_call_reserves_its_home_area_in_the_prologue() {
        let text = "target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                    target triple = \"x86_64-pc-windows-msvc\"\n\
                    declare i32 @g(i32)\n\
                    define i32 @f(i32 %x) {\n  %a = call i32 @g(i32 %x)\n  %b = call i32 @g(i32 %a)\n  %c = add i32 %a, %b\n  ret i32 %c\n}\n";
        let mut flags = Flags::default();
        flags.allow_pending_abi = true;
        let (built, _) = compiled_dumping(&flags, text, "w", None).unwrap();
        let assembly = masm::text(&built).unwrap();
        assert_eq!((assembly.matches("sub rsp").count(), assembly.matches("add rsp").count()), (1, 1), "{assembly}");
    }

    /// Windows turned a libc function's call into Open Watcom's `watcallcc`,
    /// which the Win64 convention does not describe: `printf` of a double was
    /// refused ("call the convention does not describe"). The Watcom library
    /// is the DOS one (an OMF object); and a variadic double goes in its
    /// position's XMM and its shadow GPR.
    #[test]
    fn a_windows_printf_of_a_double_keeps_win64_and_copies_it_to_the_shadow_register() {
        let text = "target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n\
                    target triple = \"x86_64-pc-windows-msvc\"\n\
                    declare i32 @printf(ptr, ...)\n\
                    @fmt = private constant [4 x i8] c\"%f\\0A\\00\"\n\
                    define i32 @f(double %d, i32 %i) {\n  %r = call i32 (ptr, ...) @printf(ptr @fmt, double %d, i32 %i)\n  %s = add i32 %r, 1\n  ret i32 %s\n}\n";
        let mut flags = Flags::default();
        flags.allow_pending_abi = true;
        let (built, _) = compiled_dumping(&flags, text, "w", None).unwrap();
        let assembly = masm::text(&built).unwrap();
        assert!(assembly.contains("movq rdx, xmm0") && assembly.contains("mov r8d, edx"), "{assembly}");
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
        let symbols: Vec<_> = sysv.pipeline.library.iter().map(|routine| routine.symbol.as_str()).collect();
        assert_eq!(symbols, ["_strlen", "_memcmp", "_bcmp"]);
        // x86-64 Linux: `memcmp` and `bcmp` (`memcmp(p, q, n) == 0` stayed a
        // `memcmp` call there); its `strlen` is not stated. A DOS object states
        // none: Watcom's library has no `bcmp`.
        let long = options(&["-m64", "-mabi=sysv"], "elf");
        let symbols: Vec<_> = long.pipeline.library.iter().map(|routine| routine.symbol.as_str()).collect();
        assert_eq!(symbols, ["memcmp", "bcmp"]);
    }
}

pub(crate) fn compile(
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
        let objects = bound(flags, without_triple(&text).0.as_deref())?.target.objects();
        let bytes = objbuild::written_in(&built, file, layout_of(flags, format), format, objects)
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

    /// A line with an `@` and none of the rewrites' words still went through
    /// two `replace`s, four `format!`s and two copies: 2.6 M of a 185 M -O0
    /// compile of 3.9 K lines, most of them calls and globals.
    #[test]
    fn a_line_with_a_symbol_and_no_rewrite_is_copied_once() {
        let line = "  %5 = call i32 @foo(i32 %3, ptr @bar)\n";
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

/// One COMDAT per function where `-ffunction-sections` asks for it and the
/// format has them (OMF); one segment for the module's code otherwise.
fn layout_of(
    flags: &Flags,
    format: llrm_target::object::Format,
) -> objbuild::CodeLayout {
    if flags.function_sections && format == llrm_target::object::Format::Omf {
        objbuild::CodeLayout::PerProcedure
    } else {
        objbuild::CodeLayout::OneSegment
    }
}
