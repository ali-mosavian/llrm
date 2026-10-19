//! `llrm-llc [options] FILE.ll`: LLVM IR from a front end that only parses
//! (clang's `-O2 -Xclang -disable-llvm-passes -emit-llvm`) through llrm's whole
//! pipeline and backend to an object. The target is the one whose `llvm_triple`
//! the module's `target triple` names, or the `-m` flag's.

use std::path::Path;
use std::rc::Rc;

use llrm_core::backend::{masm, objbuild};
use llrm_core::driver::flags::{Flags, write_output};
use llrm_target::Target;

use crate::Bound;

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
        kept.push_str(&line.replace(" dso_local", ""));
        kept.push('\n');
    }
    (triple, kept)
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

/// `text`, LLVM IR, compiled for the target its `target triple` (or `flags`)
/// names.
pub fn compiled(
    flags: &Flags,
    text: &str,
    name: &str,
) -> Result<(masm::Module, llrm_target::object::Format), String> {
    let (triple, text) = without_triple(text);
    let bound = bound(flags, triple.as_deref())?;
    let module = llrm_mir::parse::module(&text).map_err(|error| format!("{name}: {error}"))?;
    let machine = flags.machine(&*bound.target, bound.target.machine())?;
    let format = flags.format(&*bound.target)?;
    let options = llrm_core::driver::Options { object_format: format.name(), ..bound.options(flags, machine) };
    Ok((llrm_core::driver::compiled_module(module, name, &options)?, format))
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
