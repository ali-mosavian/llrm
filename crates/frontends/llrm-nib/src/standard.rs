//! The modules the compiler supplies under the `std` and `abi` import roots
//! (section 14), built into it as the prelude is. `std.os` is the
//! runtime's own DOS layer.

/// `std.os` as an editor shows it, which is for the language's first target: a build reads the
/// OS layer of the target it is for (`Frontend::os`).
static OS: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| crate::Frontend::default().os.module);

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
        "std.os" => Some(OS.as_str()),
        "abi.basic" => Some(include_str!("abi/basic.nib")),
        "abi.qb45" => Some(include_str!("abi/qb45.nib")),
        "abi.pds71" => Some(include_str!("abi/pds71.nib")),
        "abi.vbdos" => Some(include_str!("abi/vbdos.nib")),
        _ => None,
    }
}

/// The source of `module` for a target whose OS layer is `os`: `std.os` and `os` are that layer, the
/// rest as `source` has them.
pub fn source_for(os: &str, module: &str) -> Option<String> {
    match module {
        "std.os" | "os" => Some(os.to_owned()),
        _ => source(module).map(str::to_owned),
    }
}

/// The source of `module` as a std module imports it: another of these,
/// or the runtime's operating-system layer, which is not beside the program.
pub fn imported(module: &str) -> Option<&'static str> {
    match module {
        "os" => Some(OS.as_str()),
        _ => source(module),
    }
}
