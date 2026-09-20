//! QB-family source dialect selection.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dialect {
    QBasic11,
    QuickBasic45,
    Pds71,
    VbDos,
}

impl Dialect {
    pub fn parse(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "qbasic11" | "qbasic" => Some(Self::QBasic11),
            "qb45" | "quickbasic45" => Some(Self::QuickBasic45),
            "pds71" | "pds" => Some(Self::Pds71),
            "vbdos" | "vb" => Some(Self::VbDos),
            _ => None,
        }
    }
}
