//! What a target says of its data: the layout string MIR is built under, and
//! how a frontend's near, far, segment, huge and fixed addresses are its
//! address spaces. Read from the target's `datalayout.toml`.

use std::collections::BTreeMap;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::spaces::Spaces;
use llrm_mir::types::FloatKind;

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
    /// The `target triple` of the LLVM IR this target takes, where it takes
    /// any.
    pub llvm_triple: Option<String>,
    pub spaces: AddressSpaces,
    /// The operations (`select`, `fptoui.i64`) the machine has no instruction
    /// for, which the compiler expands before selection: LLVM's
    /// `setOperationAction(..., Expand)`. An operation not listed is native.
    pub expand: Vec<String>,
    /// The operations (`and.i16`) the machine has an instruction for but would
    /// rather not run at that width, which selection runs at the native width
    /// instead: LLVM's `isTypeDesirableForOp` and
    /// `setOperationAction(..., Promote)`. An operation not listed runs as it
    /// is.
    pub promote: Vec<String>,
    /// The cost model (`tti`: `"X86"`) and the CPU of it (`llvm_cpu`) the
    /// compiler prices this target with; none where the description names none.
    pub tti: Option<String>,
    pub llvm_cpu: Option<String>,
}

/// Whether the datalayout string `datalayout` states an x87 `f80`.
pub fn states_extended_float(datalayout: &str) -> bool {
    datalayout.split('-').any(|part| part.starts_with("f80:"))
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

    /// Whether the machine lacks an instruction for `operation` on values of
    /// the type `ty` (`select.i8`), or on any type (`select`).
    pub fn expands_for(
        &self,
        operation: &str,
        ty: &str,
    ) -> bool {
        self.expands(operation)
            || self
                .expand
                .iter()
                .any(|one| one.strip_prefix(operation).is_some_and(|rest| rest.strip_prefix('.') == Some(ty)))
    }

    /// Whether any value type of `operation` is expanded.
    pub fn expands_some(
        &self,
        operation: &str,
    ) -> bool {
        self.expand
            .iter()
            .any(|one| one == operation || one.strip_prefix(operation).is_some_and(|rest| rest.starts_with('.')))
    }

    /// Whether the machine would rather run `operation` at the native width
    /// (`promote` in the description).
    pub fn promotes(
        &self,
        operation: &str,
    ) -> bool {
        self.promote.iter().any(|one| one == operation)
    }

    /// The bytes a `long double` takes in memory, an array's stride: the x87
    /// type's 10 bytes rounded up to its alignment in the datalayout string
    /// (`f80`). The one place the size is stated; a frontend reads it here.
    pub fn long_double_bytes(&self) -> u64 {
        self.data().float_alloc_size(FloatKind::X86Fp80)
    }

    /// Whether the datalayout states an x87 `f80`: a target without one has no
    /// extended float, so no instruction for `pow` and its kin.
    pub fn extended_float(&self) -> bool {
        states_extended_float(&self.datalayout)
    }

    /// A `long double`'s alignment in bytes.
    pub fn long_double_align(&self) -> u64 {
        self.data().float_align(FloatKind::X86Fp80)
    }

    fn data(&self) -> DataLayout {
        DataLayout::parse(&self.datalayout).expect("a target's datalayout string parses")
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
        // A selector alone is no address, and a huge pointer's packed bits are
        // not one either (a displacement past 64K carries into its selector):
        // casts through their spaces do not compose (`ni:` marks them
        // non-integral for every pass that asks the layout).
        let non_integral: Vec<String> =
            [number("segment")?, number("huge")?].into_iter().flatten().map(|space| space.to_string()).collect();
        let datalayout = if non_integral.is_empty() || datalayout.split('-').any(|spec| spec.starts_with("ni:")) {
            datalayout
        } else {
            format!("{datalayout}-ni:{}", non_integral.join(":"))
        };
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
        let expand = match value.get("expand") {
            None => Vec::new(),
            Some(list) => list
                .as_array()
                .and_then(|list| list.iter().map(|one| one.as_str().map(str::to_owned)).collect())
                .ok_or("expand is not a list of operation names")?,
        };
        let promote = match value.get("promote") {
            None => Vec::new(),
            Some(list) => list
                .as_array()
                .and_then(|list| list.iter().map(|one| one.as_str().map(str::to_owned)).collect())
                .ok_or("promote is not a list of operation names")?,
        };
        let text = |name: &str| -> Result<Option<String>, String> {
            match value.get(name) {
                None => Ok(None),
                Some(one) => one.as_str().map(|one| Some(one.to_owned())).ok_or(format!("{name} is not a string")),
            }
        };
        let llvm_triple = text("llvm_triple")?;
        Ok(Self {
            mode,
            datalayout,
            llvm_triple,
            expand,
            promote,
            tti: text("tti")?,
            llvm_cpu: text("llvm_cpu")?,
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

    /// A target found by the triple a module states needs the triple read from
    /// its description.
    #[test]
    fn a_layout_reads_its_llvm_triple() {
        let text = format!("llvm_triple = \"x86_64-unknown-linux-gnu\"\n{FLAT}");
        assert_eq!(Layout::parse(&text).unwrap().llvm_triple.as_deref(), Some("x86_64-unknown-linux-gnu"));
        assert_eq!(Layout::parse(FLAT).unwrap().llvm_triple, None);
    }

    /// The cost model and the CPU a target is priced as were read nowhere: the
    /// targets spelled "i486" in code (m16 and m32 both).
    #[test]
    fn the_layout_names_the_cost_model_and_its_cpu() {
        let named = Layout::parse(&format!("tti = \"X86\"\nllvm_cpu = \"i386\"\n{FLAT}")).unwrap();
        assert_eq!((named.tti.as_deref(), named.llvm_cpu.as_deref()), (Some("X86"), Some("i386")));
        let bare = Layout::parse(FLAT).unwrap();
        assert_eq!((bare.tti, bare.llvm_cpu), (None, None));
    }

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

    /// A long double was 10 bytes everywhere in the frontends; the target's
    /// `f80` entry says the stride.
    #[test]
    fn a_long_double_is_ten_bytes_rounded_up_to_the_layouts_f80_alignment() {
        let with = |f80: &str| Layout::parse(&FLAT.replace("e-p:32:32", &format!("e-p:32:32-{f80}"))).unwrap();
        assert_eq!((with("f80:16").long_double_bytes(), with("f80:16").long_double_align()), (10, 2));
        assert_eq!((with("f80:32").long_double_bytes(), with("f80:32").long_double_align()), (12, 4));
        assert_eq!(Layout::parse(FLAT).unwrap().long_double_bytes(), 10);
    }

    /// `^` takes the library's `pow` where the datalayout has no `f80`; a
    /// layout that states one keeps the x87 sequence.
    #[test]
    fn a_layout_has_an_extended_float_when_it_states_f80() {
        let with = |f80: &str| Layout::parse(&FLAT.replace("e-p:32:32", &format!("e-p:32:32-{f80}"))).unwrap();
        assert!(with("f80:16").extended_float() && with("f80:32").extended_float());
        assert!(!Layout::parse(FLAT).unwrap().extended_float());
        assert!(!states_extended_float("e-m:e-p:32:32-i64:64-n32:64-S128"));
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
    fn the_operations_a_machine_promotes_are_its_descriptions_and_none_is_promoted_unless_listed() {
        assert!(!Layout::parse(FLAT).unwrap().promotes("and.i16"));
        let described = Layout::parse(&format!("promote = [\"and.i16\", \"xor.i16\"]\n{FLAT}")).unwrap();
        assert!(described.promotes("and.i16") && described.promotes("xor.i16") && !described.promotes("add.i16"));
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

#[cfg(test)]
mod segment_tests {
    use super::*;

    /// The selector-alone space is no address (casts through it do not
    /// compose): a target that has one marks it non-integral in the layout
    /// every pass reads; one that has none marks nothing.
    #[test]
    fn the_selector_space_is_non_integral_in_the_layout() {
        let text = |segment: &str| {
            format!(
                "mode = 16\ndatalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16\"\n[spaces]\nnear = 0\nfar = 1\ndata = 0\nstack = 0\n{segment}\n[pointers]\n4 = \"near\"\n"
            )
        };
        let with = Layout::parse(&text("segment = 2")).unwrap().data();
        let without = Layout::parse(&text("")).unwrap().data();
        assert!(with.is_non_integral_address_space(2));
        assert!(!without.is_non_integral_address_space(2));
    }
}
