//! LLVM IR to wasm assembly text: `llc -O0 -fast-isel=false`'s job for the
//! functions of a module, one after another.

use llrm_codegen::backend::masm::Datum;
use crate::Width;
use crate::dag::{Options, compile_function};
use crate::print;

/// The width `-mtriple=` names.
pub fn width_of(triple: &str) -> Result<Width, String> {
    match triple {
        "wasm32" | "wasm32-unknown-unknown" => Ok(Width::W32),
        "wasm64" | "wasm64-unknown-unknown" => Ok(Width::W64),
        _ => Err(format!("unknown wasm triple `{triple}`; choose wasm32 or wasm64")),
    }
}

/// The pin clang's datalayout for `--target=wasm32` and `wasm64`.
pub fn datalayout(width: Width) -> &'static str {
    match width {
        Width::W32 => "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20",
        Width::W64 => "e-m:e-p:64:64-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20",
    }
}

/// `text`, LLVM IR, as a module: the lines and words MIR's reader leaves out of
/// LLVM dropped.
pub fn module(
    text: &str,
    width: Width,
) -> Result<llrm_mir::module::Module, String> {
    let mut kept = String::with_capacity(text.len());
    for line in text.lines() {
        if line.starts_with("target triple") || line.starts_with(';') {
            continue;
        }
        kept.push_str(&line.replace(" dso_local", ""));
        kept.push('\n');
    }
    let mut module = llrm_mir::parse::module(&kept).map_err(|error| error.to_string())?;
    llrm_mir::parse::upgrade_lifetimes(&mut module);
    // `llc` takes the target's layout when the module states none.
    match module.datalayout.as_deref() {
        Some(stated) if stated != datalayout(width) => {
            return Err(format!("the module's datalayout `{stated}` is not wasm's `{}`", datalayout(width)));
        }
        Some(_) => {}
        None => module.datalayout = Some(datalayout(width).to_owned()),
    }
    Ok(module)
}

/// Each defined function of `text`: its label and instructions.
pub fn assembly(
    text: &str,
    width: Width,
    options: Options,
) -> Result<String, String> {
    let module = module(text, width)?;
    let mut out = String::new();
    for (_, global, function) in module.functions() {
        if function.is_declaration() {
            continue;
        }
        let name = global.name.owned().ok_or("an unnamed function")?;
        let f = compile_function(&module, &name, width, options).map_err(|why| format!("@{name}: {why}"))?;
        out.push_str(&print::function(&f));
    }
    Ok(out)
}

/// The relocatable object of each defined function of `text`: `llc
/// -filetype=obj`.
pub fn object(
    text: &str,
    width: Width,
    options: Options,
) -> Result<Vec<u8>, String> {
    let module = module(text, width)?;
    let mut functions = Vec::new();
    for (_, global, function) in module.functions() {
        if function.is_declaration() {
            continue;
        }
        let name = global.name.owned().ok_or("an unnamed function")?;
        let f = compile_function(&module, &name, width, options).map_err(|why| format!("@{name}: {why}"))?;
        functions.push(crate::object::encode_function(&f));
    }
    Ok(crate::object::write(&functions, &data(&module)?))
}

/// The defined variables of `module` as data segments, in module order.
fn data(module: &llrm_mir::module::Module) -> Result<Vec<crate::object::DataObject>, String> {
    use crate::object::{DataKind, DataObject};
    use llrm_codegen::backend::globals;
    use llrm_codegen::objectfile::module::Space;
    use llrm_mir::module::{GlobalKind, Linkage};

    let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().ok_or("no datalayout")?)?;
    let mut out = Vec::new();
    for (index, global) in module.globals.iter().enumerate() {
        let id = llrm_mir::GlobalId(index as u32);
        let GlobalKind::Variable(variable) = &global.kind else { continue };
        if variable.initializer.is_none() {
            continue;
        }
        let name = global.name.owned().ok_or("an unnamed variable")?;
        if global.linkage != Linkage::External {
            return Err(format!("@{name}: {:?} linkage is not supported yet", global.linkage));
        }
        let mut names = llrm_codegen::support::hash::IndexMap::default();
        names.insert((Space::Segment, i64::from(id.0)), name.clone());
        let mut bytes = Vec::new();
        for datum in globals::datums(module, id, &names)? {
            match datum {
                Datum::Label(_) => {}
                Datum::Bytes(more) => bytes.extend(more),
                Datum::Fill(fill) => bytes.resize(bytes.len() + fill.size as usize, fill.byte.unwrap_or(0)),
                other => return Err(format!("@{name}: {other:?} in an initializer is not supported yet")),
            }
        }
        let align = globals::alignment(module, Some(&layout), variable, false, false).unwrap_or(1);
        let kind = match (variable.constant, bytes.iter().all(|&b| b == 0)) {
            (false, true) => DataKind::Bss,
            (true, _) => DataKind::Rodata,
            _ => DataKind::Data,
        };
        out.push(DataObject { name, kind, align: align.trailing_zeros(), bytes });
    }
    Ok(out)
}
