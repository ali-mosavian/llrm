//! What a target says of its data: the layout string MIR is built under, and
//! how a frontend's near, far, segment, huge and fixed addresses are its
//! address spaces. Read from the target's `datalayout.toml`.

use std::collections::BTreeMap;

/// The address spaces of a target, by what a frontend calls them. Where two are
/// one space (`near` = `far`) the target is flat; one a target has no space for
/// is `None`, and a program using it is refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddressSpaces {
    pub near: u32,
    pub far: u32,
    /// Where the program's data is, for the passes that ask: its globals' and
    /// escaping locals' space.
    pub data: u32,
    /// Where the stack is.
    pub stack: u32,
    /// A selector alone.
    pub segment: Option<u32>,
    /// A far pointer whose offset carries into its selector.
    pub huge: Option<u32>,
    /// Memory no program object occupies.
    pub fixed: Option<u32>,
    /// What an unmarked pointer of a width, in bytes, is: `near` or `far`.
    pub unmarked: BTreeMap<i64, Kind>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Near,
    Far,
}

/// A target's data layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Layout {
    /// LLVM's datalayout string.
    pub datalayout: String,
    pub spaces: AddressSpaces,
}

impl AddressSpaces {
    /// Whether `far` is the `near` space: the target is flat, and a far or huge pointer is a near one.
    pub fn far_is_near(&self) -> bool {
        self.far == self.near
    }

    /// The selector-alone space, or why the target has none.
    pub fn segment_space(&self) -> Result<u32, String> {
        self.segment.ok_or_else(|| "this target has no selector address space".to_owned())
    }

    /// The huge-pointer space, or why the target has none.
    pub fn huge_space(&self) -> Result<u32, String> {
        self.huge.ok_or_else(|| "this target has no huge address space".to_owned())
    }

    /// The fixed-address space, or why the target has none.
    pub fn fixed_space(&self) -> Result<u32, String> {
        self.fixed.ok_or_else(|| "this target has no fixed address space".to_owned())
    }

    /// The space of an unmarked pointer `width` bytes wide.
    pub fn unmarked(&self, width: i64) -> Result<u32, String> {
        match self.unmarked.get(&width) {
            Some(Kind::Near) => Ok(self.near),
            Some(Kind::Far) => Ok(self.far),
            None => Err(format!("a {width}-byte pointer has no address space")),
        }
    }
}

impl Layout {
    /// `text`, a `datalayout.toml`: `datalayout`, `[spaces]` (`near`, `far`, and
    /// `data` and `stack`, the optional `segment`, `huge`, `fixed`) and `[pointers]` (a width in bytes
    /// to `"near"` or `"far"`).
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: toml::Table = text.parse().map_err(|error: toml::de::Error| error.to_string())?;
        let datalayout = value.get("datalayout").and_then(|one| one.as_str()).ok_or("datalayout is not a string")?.to_owned();
        let spaces = value.get("spaces").and_then(|one| one.as_table()).ok_or("[spaces] is missing")?;
        let number = |name: &str| -> Result<Option<u32>, String> {
            match spaces.get(name) {
                None => Ok(None),
                Some(one) => one.as_integer().and_then(|one| u32::try_from(one).ok()).map(Some).ok_or(format!("spaces.{name} is not an address space number")),
            }
        };
        let required = |name: &str| number(name)?.ok_or(format!("spaces.{name} is missing"));
        let mut unmarked = BTreeMap::new();
        for (width, kind) in value.get("pointers").and_then(|one| one.as_table()).ok_or("[pointers] is missing")? {
            let width: i64 = width.parse().map_err(|_| format!("pointers.{width} is not a width"))?;
            unmarked.insert(
                width,
                match kind.as_str() {
                    Some("near") => Kind::Near,
                    Some("far") => Kind::Far,
                    _ => return Err(format!("pointers.{width} is not \"near\" or \"far\"")),
                },
            );
        }
        Ok(Self {
            datalayout,
            spaces: AddressSpaces { near: required("near")?, far: required("far")?, data: required("data")?, stack: required("stack")?, segment: number("segment")?, huge: number("huge")?, fixed: number("fixed")?, unmarked },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLAT: &str = "datalayout = \"e-p:32:32\"\n[spaces]\nnear = 0\nfar = 0\ndata = 0\nstack = 0\n[pointers]\n4 = \"near\"\n";

    #[test]
    fn a_flat_layout_has_one_space_and_none_of_the_pair_kinds() {
        let flat = Layout::parse(FLAT).unwrap();
        assert_eq!((flat.spaces.data, flat.spaces.stack), (0, 0));
        assert_eq!((flat.spaces.near, flat.spaces.far, flat.spaces.segment, flat.spaces.huge, flat.spaces.fixed), (0, 0, None, None, None));
        assert_eq!(flat.spaces.unmarked(4), Ok(0));
        assert!(flat.spaces.unmarked(2).is_err());
    }

    #[test]
    fn a_malformed_layout_is_refused_with_what_is_wrong() {
        assert_eq!(Layout::parse("datalayout = \"e\"\n[pointers]\n").unwrap_err(), "[spaces] is missing");
        assert_eq!(Layout::parse(&FLAT.replace("near = 0", "near = \"x\"")).unwrap_err(), "spaces.near is not an address space number");
    }
}
