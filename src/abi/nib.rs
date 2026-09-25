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
pub const TEXT_COMPARE: &str = "N$TCMP";

pub const VIEW_COPY: &str = "N$VCPY";
pub const VIEW_COMPARE: &str = "N$VCMP";

pub const DICT_RESERVE: &str = "N$DRES";

pub const FILE_OPEN: &str = "N$OOPN";
pub const FILE_CREATE: &str = "N$OCRE";
pub const FILE_READ: &str = "N$OREA";
pub const FILE_WRITE: &str = "N$OWRI";
pub const FILE_CLOSE: &str = "N$OCLO";

pub const ERROR_BOUNDS: &str = "N$EBND";
pub const ERROR_SHIFT: &str = "N$ESHF";
pub const ERROR_CONVERT: &str = "N$ECNV";
pub const ERROR_KEY: &str = "N$EKEY";
