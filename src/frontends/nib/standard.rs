//! The modules the compiler supplies under the `std` and `abi` import roots
//! (section 14), built into it as the prelude is. `std.os` is the
//! runtime's own DOS layer.

/// Whether `module` is under a root the compiler supplies.
pub fn supplied(module: &str) -> bool {
    module.starts_with("std.") || module.starts_with("abi.")
}

/// The source of `module`, when it is one of these.
pub fn source(module: &str) -> Option<&'static str> {
    match module {
        "std.io" => Some(include_str!("std/io.nbl")),
        "std.dos" => Some(include_str!("std/dos.nbl")),
        "std.os" => Some(include_str!("../../../runtime/nib/os.nbl")),
        "abi.basic" => Some(include_str!("abi/basic.nbl")),
        "abi.qb45" => Some(include_str!("abi/qb45.nbl")),
        "abi.pds71" => Some(include_str!("abi/pds71.nbl")),
        "abi.vbdos" => Some(include_str!("abi/vbdos.nbl")),
        _ => None,
    }
}

/// The source of `module` as a std module imports it: another of these,
/// or the runtime's operating-system layer, which is not beside the program.
pub fn imported(module: &str) -> Option<&'static str> {
    match module {
        "os" => Some(include_str!("../../../runtime/nib/os.nbl")),
        _ => source(module),
    }
}
