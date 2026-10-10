//! What a target says of its data: the layout string MIR is built under, and
//! how a frontend's near, far, segment, huge and fixed addresses are its
//! address spaces. Read from the target's `datalayout.toml`.

use std::collections::BTreeMap;

use llrm_mir::spaces::Spaces;

/// A target's address spaces: the roles the passes ask
/// (`llrm_mir::spaces::Spaces`, reached through `Deref`) and what an unmarked
/// pointer is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddressSpaces {
    pub roles: Spaces,
    /// What an unmarked pointer of a width, in bytes, is: `near` or `far`.
    pub unmarked: BTreeMap<i64, Kind>,
}

impl std::ops::Deref for AddressSpaces {
    type Target = Spaces;

    fn deref(&self) -> &Spaces {
        &self.roles
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Near,
    Far,
}

/// A target's data layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Layout {
    /// gcc's `-m` number that names this target: `-m16`, `-m32`, `-m64`.
    pub mode: u32,
    /// LLVM's datalayout string.
    pub datalayout: String,
    pub spaces: AddressSpaces,
    /// The operations (`select`, `fptoui.i64`) the machine has no instruction
    /// for, which the compiler expands before selection: LLVM's
    /// `setOperationAction(..., Expand)`. An operation not listed is native.
    pub expand: Vec<String>,
}

impl AddressSpaces {
    /// The space of an unmarked pointer `width` bytes wide.
    pub fn unmarked(
        &self,
        width: i64,
    ) -> Result<u32, String> {
        match self.unmarked.get(&width) {
            Some(Kind::Near) => Ok(self.near),
            Some(Kind::Far) => Ok(self.far),
            None => Err(format!("a {width}-byte pointer has no address space")),
        }
    }
}

impl Layout {
    /// Whether the machine lacks an instruction for `operation`, so the
    /// compiler expands it (`expand` in the description).
    pub fn expands(
        &self,
        operation: &str,
    ) -> bool {
        self.expand.iter().any(|one| one == operation)
    }

    /// The most bytes a data segment holds; none where segments are not.
    pub fn segment_bytes(&self) -> Option<usize> {
        self.spaces.segment_bytes.map(|bytes| bytes as usize)
    }

    /// `text`, a `datalayout.toml`: `datalayout`, `[spaces]` (`near`, `far`,
    /// and `data` and `stack`, the optional `segment`, `huge`, `fixed`,
    /// `segment_bytes`) and `[pointers]` (a width in bytes to `"near"` or
    /// `"far"`).
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: toml::Table = text.parse().map_err(|error: toml::de::Error| error.to_string())?;
        let mode = value
            .get("mode")
            .and_then(|one| one.as_integer())
            .and_then(|one| u32::try_from(one).ok())
            .filter(|one| *one > 0)
            .ok_or("mode is not a positive number")?;
        let datalayout =
            value.get("datalayout").and_then(|one| one.as_str()).ok_or("datalayout is not a string")?.to_owned();
        let spaces = value.get("spaces").and_then(|one| one.as_table()).ok_or("[spaces] is missing")?;
        let number = |name: &str| -> Result<Option<u32>, String> {
            match spaces.get(name) {
                None => Ok(None),
                Some(one) => one
                    .as_integer()
                    .and_then(|one| u32::try_from(one).ok())
                    .map(Some)
                    .ok_or(format!("spaces.{name} is not an address space number")),
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
        let segment_bytes = match spaces.get("segment_bytes") {
            None => None,
            Some(one) => Some(
                one.as_integer()
                    .and_then(|one| u64::try_from(one).ok())
                    .filter(|one| *one > 0)
                    .ok_or("spaces.segment_bytes is not a positive size")?,
            ),
        };
        let real_mode_base = match spaces.get("real_mode_base") {
            None => None,
            Some(one) => Some(
                one.as_integer().and_then(|one| u64::try_from(one).ok()).ok_or("spaces.real_mode_base is not an address")?,
            ),
        };
        let expand = match value.get("expand") {
            None => Vec::new(),
            Some(list) => list
                .as_array()
                .and_then(|list| list.iter().map(|one| one.as_str().map(str::to_owned)).collect())
                .ok_or("expand is not a list of operation names")?,
        };
        Ok(Self {
            mode,
            datalayout,
            expand,
            spaces: AddressSpaces {
                roles: Spaces {
                    near: required("near")?,
                    far: required("far")?,
                    data: required("data")?,
                    stack: required("stack")?,
                    segment: number("segment")?,
                    huge: number("huge")?,
                    fixed: number("fixed")?,
                    segment_bytes,
                    real_mode_base,
                },
                unmarked,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLAT: &str = "mode = 32\ndatalayout = \"e-p:32:32\"\n[spaces]\nnear = 0\nfar = 0\ndata = 0\nstack = 0\n[pointers]\n4 = \"near\"\n";

    #[test]
    fn a_flat_layout_has_one_space_and_none_of_the_pair_kinds() {
        let flat = Layout::parse(FLAT).unwrap();
        assert_eq!((flat.spaces.data, flat.spaces.stack), (0, 0));
        assert_eq!(
            (flat.spaces.near, flat.spaces.far, flat.spaces.segment, flat.spaces.huge, flat.spaces.fixed),
            (0, 0, None, None, None)
        );
        assert_eq!(flat.spaces.unmarked(4), Ok(0));
        assert!(flat.spaces.unmarked(2).is_err());
    }

    /// The segment size was the compiler's own 64K, cutting a flat program's
    /// large array in two.
    #[test]
    fn a_segment_size_is_the_layouts_and_a_flat_one_has_none() {
        assert_eq!(Layout::parse(FLAT).unwrap().segment_bytes(), None);
        assert_eq!(
            Layout::parse(&FLAT.replace("near = 0\n", "near = 0\nsegment_bytes = 65536\n")).unwrap().segment_bytes(),
            Some(65536)
        );
        assert!(
            Layout::parse(&FLAT.replace("near = 0\n", "near = 0\nsegment_bytes = 0\n"))
                .unwrap_err()
                .contains("segment_bytes")
        );
    }

    #[test]
    fn the_operations_a_machine_expands_are_its_descriptions_and_none_is_native() {
        assert!(!Layout::parse(FLAT).unwrap().expands("select"));
        let described = Layout::parse(&format!("expand = [\"select\", \"fptoui.i64\"]\n{FLAT}")).unwrap();
        assert!(described.expands("select") && described.expands("fptoui.i64") && !described.expands("fptosi.i64"));
        assert!(Layout::parse(&format!("expand = 3\n{FLAT}")).is_err());
    }

    #[test]
    fn a_malformed_layout_is_refused_with_what_is_wrong() {
        assert_eq!(Layout::parse("mode = 16\ndatalayout = \"e\"\n[pointers]\n").unwrap_err(), "[spaces] is missing");
        assert_eq!(
            Layout::parse(&FLAT.replace("near = 0", "near = \"x\"")).unwrap_err(),
            "spaces.near is not an address space number"
        );
    }
}
