//! Nib's runtime routines (section 13), named as BC's are:
//! `M$`, a letter for the group, then the operation; a print names the type
//! it prints. P prints and formats, B keeps a string's or vec's buffer, T
//! works on text, V on views, D on dicts, E stops with an error, and O is
//! the operating system, which only assembly reaches. The divide fault's
//! `N$EDIV` is called by start.asm, not by compiled code.

pub const PRINT_I1: &str = "N$PI1";
pub const PRINT_U1: &str = "N$PU1";
pub const PRINT_I2: &str = "N$PI2";
pub const PRINT_U2: &str = "N$PU2";
pub const PRINT_I4: &str = "N$PI4";
pub const PRINT_U4: &str = "N$PU4";
pub const PRINT_R4: &str = "N$PR4";
pub const PRINT_R8: &str = "N$PR8";
/// A fixed-point value: its raw storage, then its fraction bits.
pub const PRINT_Q2: &str = "N$PQ2";
pub const PRINT_Q4: &str = "N$PQ4";
pub const PRINT_BOOL: &str = "N$PB";
pub const PRINT_CHAR: &str = "N$PC";
pub const PRINT_STRING: &str = "N$PS";
/// A `&string`: its far data and length.
pub const PRINT_VIEW: &str = "N$PV";
pub const PRINT_NEWLINE: &str = "N$PN";
/// The width, radix, fill and alignment of the next value printed.
pub const PRINT_FIELD: &str = "N$PFLD";
/// Prints to a new string from here, until `PRINT_END` returns it.
pub const PRINT_BEGIN: &str = "N$PBEG";
pub const PRINT_END: &str = "N$PEND";

pub const BUFFER_RESERVE: &str = "N$BRES";
pub const BUFFER_DROP: &str = "N$BDRP";
pub const BUFFER_GROW: &str = "N$BGRW";
pub const BUFFER_SHRINK: &str = "N$BSHR";
pub const BUFFER_CLONE: &str = "N$BCLN";

pub const TEXT_CONCAT: &str = "N$TCAT";
pub const TEXT_APPEND: &str = "N$TAPP";

pub const VIEW_COPY: &str = "N$VCPY";
pub const VIEW_COMPARE: &str = "N$VCMP";

pub const DICT_RESERVE: &str = "N$DRES";

/// The OS layer's operation a runtime symbol implements, by its name in the
/// interface, when it is a file call (`open`, `create`, `read`, `write_file`,
/// `close`) or a console one.
pub fn os_operation(symbol: &str) -> Option<&'static str> {
    static INTERFACE: std::sync::LazyLock<llrm_target::os::Interface> =
        std::sync::LazyLock::new(llrm_target::os::Interface::shipped);
    let files = ["open", "create", "read", "write_file", "close"];
    INTERFACE
        .ops
        .iter()
        .find(|op| INTERFACE.symbol(op) == symbol && (op.group == "console" || files.contains(&op.name.as_str())))
        .map(|op| op.name.as_str())
}

/// The number of the standard handle `name` (`stdin`, `stdout`, `stderr`): the
/// operating system's fact.
pub fn standard_handle(name: &str) -> usize {
    let facts: toml::Table = llrm_x86::DOS_FACTS.parse().expect("the OS facts parse");
    usize::try_from(facts[name].as_integer().expect("a standard handle is a number")).expect("a handle is not negative")
}

/// The interface's code of the error condition `name`.
pub fn error_code(name: &str) -> i16 {
    static INTERFACE: std::sync::LazyLock<llrm_target::os::Interface> =
        std::sync::LazyLock::new(llrm_target::os::Interface::shipped);
    i16::try_from(INTERFACE.errors[name]).expect("an error code is an i16")
}

pub const ERROR_BOUNDS: &str = "N$EBND";
pub const ERROR_SHIFT: &str = "N$ESHF";
pub const ERROR_CONVERT: &str = "N$ECNV";
pub const ERROR_KEY: &str = "N$EKEY";
pub const ERROR_DIVIDE: &str = "N$EDIV";

/// The routines that end the program, touching only memory its caller cannot
/// name.
pub const TERMINATING: [&str; 5] = [ERROR_BOUNDS, ERROR_CONVERT, ERROR_DIVIDE, ERROR_KEY, ERROR_SHIFT];

/// The routines that take values and touch only the runtime's own state,
/// memory the program cannot name: each print of a number, a bool or a
/// char, a field's format, a newline, and the start of a print to a string.
pub const RUNTIME_STATE_ONLY: [&str; 15] = [
    PRINT_I1,
    PRINT_U1,
    PRINT_I2,
    PRINT_U2,
    PRINT_I4,
    PRINT_U4,
    PRINT_R4,
    PRINT_R8,
    PRINT_Q2,
    PRINT_Q4,
    PRINT_BOOL,
    PRINT_CHAR,
    PRINT_NEWLINE,
    PRINT_FIELD,
    PRINT_BEGIN,
];

/// The routines that only read the memory their arguments point to.
pub const READ_ONLY: [&str; 1] = [VIEW_COMPARE];
