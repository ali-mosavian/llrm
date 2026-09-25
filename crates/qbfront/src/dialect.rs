//! QB-family source dialect selection.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dialect {
    QBasic11,
    QuickBasic45,
    Pds71,
    VbDos,
    /// QuickrBASIC: VBDOS plus sized integers and mandatory declarations.
    Quickr,
}

impl Dialect {
    pub fn parse(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "qbasic11" | "qbasic" => Some(Self::QBasic11),
            "qb45" | "quickbasic45" => Some(Self::QuickBasic45),
            "pds71" | "pds" => Some(Self::Pds71),
            "vbdos" | "vb" => Some(Self::VbDos),
            "quickr" | "quickrbasic" => Some(Self::Quickr),
            _ => None,
        }
    }

    /// `BYTE`, `SIGNED` and `UNSIGNED` integer types.
    pub fn sized_integers(self) -> bool {
        self == Self::Quickr
    }

    /// `f"…"` is a format string, not the name `f` before a string.
    pub fn format_strings(self) -> bool {
        self == Self::Quickr
    }

    /// A procedure's locals start at zero through explicit stores, not
    /// through the runtime's frame.
    pub fn zeroes_locals(self) -> bool {
        self == Self::Quickr
    }

    /// `PRIVATE SUB` and `PRIVATE FUNCTION`.
    pub fn private_procedures(self) -> bool {
        self == Self::Quickr
    }

    /// `x += e` and the other augmented assignments.
    pub fn augmented_assignment(self) -> bool {
        self == Self::Quickr
    }

    /// `BREAK` and `CONTINUE` act on the innermost loop.
    pub fn loop_control(self) -> bool {
        self == Self::Quickr
    }

    /// `FOR EACH item IN iterable`.
    pub fn for_each(self) -> bool {
        self == Self::Quickr
    }

    /// Every array dimension starts at 0.
    pub fn zero_based_arrays(self) -> bool {
        self == Self::Quickr
    }

    /// `OPTION EXPLICIT` holds for every module.
    pub fn explicit_declarations(self) -> bool {
        self == Self::Quickr
    }
}
