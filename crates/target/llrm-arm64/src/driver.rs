//! LLVM IR to an arm64 object: `llc`'s job over `opt`'s. Clang's front end
//! alone makes the IR (`-O2 -Xclang -disable-llvm-passes`); the shared mid-end
//! pipeline optimizes it for the machine; the arm64 backend selects, allocates
//! and encodes.

use std::rc::Rc;

use llrm_mir::program::Program;
use llrm_transforms::pipeline::Applied;

use crate::machine::{Arm64Machine, Platform};

/// `text`, LLVM IR, without the lines and words MIR's reader leaves out of
/// LLVM (`target triple`, comments, `dso_local`), and the triple it stated.
pub fn without_triple(text: &str) -> (Option<String>, String) {
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

/// The platform whose triple `triple` is.
pub fn platform_of(triple: &str) -> Option<Platform> {
    if triple.starts_with("arm64-apple-") || triple.starts_with("aarch64-apple-") {
        Some(Platform::Darwin)
    } else if triple.starts_with("aarch64-") && triple.contains("linux") {
        Some(Platform::Linux)
    } else {
        None
    }
}

/// `text` read as a program for `platform`, its datalayout the platform's.
pub fn program(
    text: &str,
    platform: Platform,
) -> Result<Program, String> {
    let (_, text) = without_triple(text);
    let module = llrm_mir::parse::module(&text).map_err(|error| error.to_string())?;
    match module.datalayout.as_deref() {
        Some(stated) if stated == platform.datalayout() => {}
        Some(stated) => {
            return Err(format!(
                "the module's datalayout `{stated}` is not {:?}'s `{}`",
                platform,
                platform.datalayout()
            ));
        }
        None => {
            return Err(format!("the module states no datalayout; {:?}'s is `{}`", platform, platform.datalayout()));
        }
    }
    Program::new(vec![module], Rc::new(Arm64Machine { platform }))
}

/// `program` through the shared mid-end at -O2.
pub fn optimized(program: &mut Program) -> Result<(), String> {
    llrm_transforms::pipeline::applied(program, &Applied::default())
}

/// Every defined function of `program`'s one module, selected, allocated and
/// lowered.
pub fn functions(program: &Program) -> Result<Vec<crate::emit::Function>, String> {
    let module = &program.modules[0];
    let platform = platform_of_machine(program);
    let mut out = Vec::new();
    for (id, global, function) in module.functions() {
        if function.is_declaration() {
            continue;
        }
        let name = global.name.clone().ok_or("an unnamed function")?;
        let mut machine =
            crate::isel::select(module, id, &program.layout, platform).map_err(|why| format!("@{name}: {why}"))?;
        let allocation = crate::regalloc::allocate(&mut machine);
        let blocks = crate::frame::lower(&machine, &allocation).map_err(|why| format!("@{name}: {why}"))?;
        let blocks = crate::layout::fold(blocks);
        out.push(crate::emit::Function {
            symbol: crate::data::symbol_name(platform, &name),
            public: !matches!(
                global.linkage,
                llrm_mir::Linkage::Internal | llrm_mir::Linkage::Private
            ),
            blocks,
        });
    }
    Ok(out)
}

fn platform_of_machine(program: &Program) -> Platform {
    // The machine is the one `program` was made with; its datalayout names the
    // platform.
    match program.modules[0].datalayout.as_deref() {
        Some(text) if text == Platform::Linux.datalayout() => Platform::Linux,
        _ => Platform::Darwin,
    }
}

/// `text`, LLVM IR for `platform`, as an object: through the mid-end and the
/// backend.
pub fn object(
    text: &str,
    platform: Platform,
    name: &str,
    optimize: bool,
) -> Result<llrm_object::Object, String> {
    let mut program = program(text, platform)?;
    if optimize {
        optimized(&mut program)?;
    }
    let lowered = functions(&program)?;
    crate::emit::object(&program.modules[0], &program.layout, platform, name, &lowered)
}

/// The assembly of the lowered functions, one instruction a line.
pub fn assembly(
    text: &str,
    platform: Platform,
    optimize: bool,
) -> Result<String, String> {
    let mut program = program(text, platform)?;
    if optimize {
        optimized(&mut program)?;
    }
    let mut out = String::new();
    for function in functions(&program)? {
        out.push_str(&format!("{}:\n", function.symbol));
        for block in &function.blocks {
            out.push_str(&format!("{}:\n", block.label));
            for inst in &block.insts {
                out.push_str(&format!("\t{inst}\n"));
            }
        }
    }
    Ok(out)
}

/// The object file for `platform`: Mach-O or ELF.
pub fn object_file(
    text: &str,
    platform: Platform,
    name: &str,
    optimize: bool,
) -> Result<Vec<u8>, String> {
    let object = object(text, platform, name, optimize)?;
    match platform {
        Platform::Darwin => llrm_macho::write(&object).map_err(|error| error.to_string()),
        Platform::Linux => llrm_elf_arm64::write(&object).map_err(|error| error.to_string()),
    }
}
