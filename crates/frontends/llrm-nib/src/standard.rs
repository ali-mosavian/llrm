//! The modules the compiler supplies under the `std` and `abi` import roots
//! (section 14), built into it as the prelude is. `std.os` is the
//! runtime's own DOS layer.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether a library for BASIC calls the llrm runtime, whose entries take the target's own convention
/// (regparm3) instead of BASIC's (pascal16): `--basic-runtime=llrm`.
static LLRM_RUNTIME: AtomicBool = AtomicBool::new(false);

/// Says which runtime a library for BASIC is built for: `qb45` (the default) or `llrm`.
pub fn set_basic_runtime(runtime: &str) -> Result<(), String> {
    match runtime {
        "qb45" => LLRM_RUNTIME.store(false, Ordering::Relaxed),
        "llrm" => LLRM_RUNTIME.store(true, Ordering::Relaxed),
        other => return Err(format!("--basic-runtime takes qb45 or llrm, not {other}")),
    }
    Ok(())
}

/// QB 4.5's adapters for the llrm runtime: the same, but its string copy is called as the target calls any
/// function.
fn qb45_for_llrm() -> &'static str {
    static TEXT: OnceLock<String> = OnceLock::new();
    TEXT.get_or_init(|| {
        include_str!("abi/qb45.nib")
            .replace("@extern(\"pascal16\", name=\"B$SCPY\")", "@extern(\"regparm3\", name=\"B$SCPY\")")
    })
}

/// Whether `module` is under a root the compiler supplies.
pub fn supplied(module: &str) -> bool {
    module.starts_with("std.") || module.starts_with("abi.")
}

/// The source of `module`, when it is one of these.
pub fn source(module: &str) -> Option<&'static str> {
    match module {
        "std.io" => Some(include_str!("std/io.nib")),
        "std.dos" => Some(include_str!("std/dos.nib")),
        "std.sort" => Some(include_str!("std/sort.nib")),
        "abi.basic" => Some(include_str!("abi/basic.nib")),
        "abi.qb45" if LLRM_RUNTIME.load(Ordering::Relaxed) => Some(qb45_for_llrm()),
        "abi.qb45" => Some(include_str!("abi/qb45.nib")),
        "abi.pds71" => Some(include_str!("abi/pds71.nib")),
        "abi.vbdos" => Some(include_str!("abi/vbdos.nib")),
        _ => None,
    }
}

/// The source of `module` for a target whose OS layer is `os`: `std.os` and
/// `os` are that layer, the rest as `source` has them.
pub fn source_for(
    os: &str,
    module: &str,
) -> Option<String> {
    match module {
        "std.os" | "os" => Some(os.to_owned()),
        _ => source(module).map(str::to_owned),
    }
}
