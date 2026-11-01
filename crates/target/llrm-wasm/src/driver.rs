//! LLVM IR to wasm assembly text: `llc -O0 -fast-isel=false`'s job for the
//! functions of a module, one after another.

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

/// The pin clang's datalayout for `--target=wasm32` and `wasm64`, from the
/// machine description.
pub fn datalayout(width: Width) -> String {
    crate::layout(width).datalayout
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
    // No `atomics` feature: the pin's `LowerAtomicPass` makes the accesses
    // plain.
    llrm_mir::atomic::lowered_single_thread(&mut module)?;
    // `llc` takes the target's layout when the module states none.
    match module.datalayout.as_deref() {
        Some(stated) if *stated != datalayout(width) => {
            return Err(format!("the module's datalayout `{stated}` is not wasm's `{}`", datalayout(width)));
        }
        Some(_) => {}
        None => module.datalayout = Some(datalayout(width)),
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
    let mut labels = print::Labels::default();
    for (_, global, function) in module.functions() {
        if function.is_declaration() {
            continue;
        }
        let name = global.name.owned().ok_or("an unnamed function")?;
        let f = compile_function(&module, &name, width, options).map_err(|why| format!("@{name}: {why}"))?;
        out.push_str(&print::function_in(&f, &mut labels));
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
    Ok(crate::object::write(
        &functions,
        &data(&module, width)?,
        options.exception_model == crate::dag::ExceptionModel::Wasm,
    ))
}

/// The assembler's name for `global`: `.L` ahead of a private one (`Mangler`
/// with the private prefix `.L`).
pub fn symbol_name(global: &llrm_mir::module::GlobalValue) -> String {
    let name = global.name.as_deref().unwrap_or("");
    match global.linkage {
        llrm_mir::module::Linkage::Private => format!(".L{name}"),
        _ => name.to_string(),
    }
}

/// The variables of `module`: the defined ones as data segments, in module
/// order, and the names of the declared ones.
fn data(
    module: &llrm_mir::module::Module,
    width: Width,
) -> Result<crate::object::Data, String> {
    use llrm_codegen::backend::{globals, masm::Datum};
    use llrm_mir::module::{GlobalKind, Linkage, UnnamedAddr};

    use crate::object::{Data, DataKind, DataObject, DataReloc};

    let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().ok_or("no datalayout")?)?;
    let mut names = llrm_codegen::support::hash::IndexMap::default();
    for (index, global) in module.globals.iter().enumerate() {
        let id = llrm_mir::GlobalId(index as u32);
        names.insert((globals::space(module, id), i64::from(id.0)), symbol_name(global));
    }
    let mut out = Data::default();
    for (index, global) in module.globals.iter().enumerate() {
        let GlobalKind::Variable(variable) = &global.kind else { continue };
        let id = llrm_mir::GlobalId(index as u32);
        let name = symbol_name(global);
        let Some(initializer) = variable.initializer else {
            out.undefined.push(name);
            continue;
        };
        let constructors = name == "llvm.global_ctors" && global.linkage == Linkage::Appending;
        if !constructors
            && !matches!(
                global.linkage,
                Linkage::External
                    | Linkage::Internal
                    | Linkage::Private
                    | Linkage::Weak
                    | Linkage::WeakOdr
                    | Linkage::LinkOnce
                    | Linkage::LinkOnceOdr
            )
        {
            return Err(format!("@{name}: {:?} linkage is not supported yet", global.linkage));
        }
        let mut bytes = Vec::new();
        let mut relocs = Vec::new();
        for datum in globals::datums(module, id, &names)? {
            match datum {
                Datum::Label(_) => {}
                Datum::Bytes(more) => bytes.extend(more),
                Datum::Align(align) => bytes.resize(bytes.len().next_multiple_of(align.to as usize), 0),
                Datum::Fill(fill) => bytes.resize(bytes.len() + fill.size as usize, fill.byte.unwrap_or(0)),
                Datum::Pointer(pointer) if !pointer.far && pointer.bytes == 4 => {
                    relocs.push(DataReloc { offset: bytes.len(), symbol: pointer.name, addend: pointer.offset });
                    bytes.extend([0; 4]);
                }
                other => return Err(format!("@{name}: {other:?} in an initializer is not supported yet")),
            }
        }
        for reloc in &relocs {
            let declared = out.functions.iter().any(|(one, ..)| *one == reloc.symbol);
            if !declared && module.named(&reloc.symbol).is_some_and(|id| module.global(id).function().is_some()) {
                let (params, results) = crate::dag::signature(module, &reloc.symbol, width)?;
                out.functions.push((reloc.symbol.clone(), params, results));
            }
        }
        if constructors {
            // The shared reader's order and its drop of an entry without a
            // function.
            out.init_funcs = globals::constructor_entries(module, &layout, initializer)?
                .into_iter()
                .map(|(priority, function, _)| (priority as u16, symbol_name(module.global(function))))
                .collect();
            continue;
        }
        let align = globals::alignment(module, Some(&layout), variable, false, false).unwrap_or(1);
        let kind = match (variable.constant, bytes.iter().all(|&b| b == 0) && relocs.is_empty()) {
            (false, true) => DataKind::Bss,
            (true, _) => DataKind::Rodata,
            _ => DataKind::Data,
        };
        // `getKindForGlobal`: a constant whose address does not matter, and
        // that is one C string, merges.
        let is_string = matches!(
            module.context.get(initializer).kind,
            llrm_mir::ConstantKind::Bytes(_)
        )
            && bytes.last() == Some(&0)
            && !bytes[..bytes.len() - 1].contains(&0);
        out.objects.push(DataObject {
            name,
            kind,
            align: align.trailing_zeros(),
            bytes,
            binding: crate::mir::Binding::of(global.linkage),
            strings: variable.constant && global.unnamed_addr == UnnamedAddr::Global && is_string,
            relocs,
        });
    }
    Ok(out)
}
