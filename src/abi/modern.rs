//! The modern language's runtime routines (section 13), named as BC's are:
//! `M$`, a letter for the group, then the operation; a print names the type
//! it prints. P prints and formats, B keeps a string's or vec's buffer, T
//! works on text, V on views, D on dicts, E stops with an error, and O is
//! the operating system, which only assembly reaches. The divide fault's
//! `M$EDIV` is called by start.asm, not by compiled code.

pub const PRINT_I1: &str = "M$PI1";
pub const PRINT_U1: &str = "M$PU1";
pub const PRINT_I2: &str = "M$PI2";
pub const PRINT_U2: &str = "M$PU2";
pub const PRINT_I4: &str = "M$PI4";
pub const PRINT_U4: &str = "M$PU4";
pub const PRINT_R4: &str = "M$PR4";
pub const PRINT_R8: &str = "M$PR8";
/// A fixed-point value: its raw storage, then its fraction bits.
pub const PRINT_Q2: &str = "M$PQ2";
pub const PRINT_Q4: &str = "M$PQ4";
pub const PRINT_BOOL: &str = "M$PB";
pub const PRINT_CHAR: &str = "M$PC";
pub const PRINT_STRING: &str = "M$PS";
/// A `&string`: its far data and length.
pub const PRINT_VIEW: &str = "M$PV";
pub const PRINT_NEWLINE: &str = "M$PN";
/// The width, radix, fill and alignment of the next value printed.
pub const PRINT_FIELD: &str = "M$PFLD";
/// Prints to a new string from here, until `PRINT_END` returns it.
pub const PRINT_BEGIN: &str = "M$PBEG";
pub const PRINT_END: &str = "M$PEND";

pub const BUFFER_RESERVE: &str = "M$BRES";
pub const BUFFER_DROP: &str = "M$BDRP";
pub const BUFFER_GROW: &str = "M$BGRW";
pub const BUFFER_SHRINK: &str = "M$BSHR";
pub const BUFFER_CLONE: &str = "M$BCLN";

pub const TEXT_CONCAT: &str = "M$TCAT";
pub const TEXT_APPEND: &str = "M$TAPP";
pub const TEXT_COMPARE: &str = "M$TCMP";

pub const VIEW_COPY: &str = "M$VCPY";
pub const VIEW_COMPARE: &str = "M$VCMP";

pub const DICT_RESERVE: &str = "M$DRES";

pub const FILE_OPEN: &str = "M$OOPN";
pub const FILE_CREATE: &str = "M$OCRE";
pub const FILE_READ: &str = "M$OREA";
pub const FILE_WRITE: &str = "M$OWRI";
pub const FILE_CLOSE: &str = "M$OCLO";

pub const ERROR_BOUNDS: &str = "M$EBND";
pub const ERROR_SHIFT: &str = "M$ESHF";
pub const ERROR_CONVERT: &str = "M$ECNV";
pub const ERROR_KEY: &str = "M$EKEY";
