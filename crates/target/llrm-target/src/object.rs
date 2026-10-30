//! What a target says of the object file and the listing it writes: the formats
//! it can write and the default among them, its record widths, and the
//! listing's opening lines. Read from the target's `object.toml`.

/// An object file format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    Omf,
    Elf,
    MachO,
    Coff,
}

impl Format {
    pub const ALL: [Format; 4] = [Format::Omf, Format::Elf, Format::MachO, Format::Coff];

    /// The name `-fobject-format=` and `object.toml` spell it.
    pub fn name(self) -> &'static str {
        match self {
            Format::Omf => "omf",
            Format::Elf => "elf",
            Format::MachO => "macho",
            Format::Coff => "coff",
        }
    }

    pub fn parse(name: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|one| one.name() == name)
            .ok_or_else(
                || format!("{name:?} is not one of {}", Self::ALL.map(|one| format!("{:?}", one.name())).join(", ")),
            )
    }
}

/// `LLRM_OBJECT_ALL_WRITERS=DIR`: the sweep's switch (tools/pairs/objsweep.py).
/// An object is also run through every writer of its architecture, each result
/// written to `DIR/<format>.bin`, so a writer the target does not
/// choose by default is compared too.
pub fn all_writers_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("LLRM_OBJECT_ALL_WRITERS").map(Into::into)
}

/// Writes `bytes` as `format`'s result in the sweep directory; a failure to
/// write is the sweep's to see, not the compile's.
pub fn record_writer(
    dir: &std::path::Path,
    format: Format,
    bytes: &[u8],
) {
    let _ = std::fs::write(dir.join(format!("{}.bin", format.name())), bytes);
}

/// Writes why a writer refused the object as `<format>.err`: a refusal the
/// sweep cannot see reads as two sides agreeing on nothing.
pub fn record_refusal(
    dir: &std::path::Path,
    format: Format,
    why: &str,
) {
    let _ = std::fs::write(dir.join(format!("{}.err", format.name())), why);
}

/// A target's object format.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectFormat {
    /// The formats it can write, and which one without a choice.
    pub formats: Vec<Format>,
    pub default: Format,
    /// 16 or 32: the mode instructions are encoded in, and a segment's USE.
    pub bitness: u32,
    /// The lines a listing opens with: its instruction set and memory model.
    pub header: Vec<String>,
    /// The DWARF version `-gdwarf` writes: the one the target's own consumers
    /// read.
    pub dwarf_version: u16,
    /// A symbol is reached from the instruction pointer (`[rip + symbol]`): no
    /// address mixes a symbol with a register, and a table of code
    /// addresses is not position independent.
    pub pc_relative: bool,
    /// A symbol defined elsewhere is reached through the global offset table:
    /// its address is loaded from an entry the linker fills, since a
    /// position independent program cannot assume where a shared library puts
    /// it.
    pub got_external: bool,
}

/// What `-gdwarf` writes where the target does not say.
pub const DEFAULT_DWARF_VERSION: u16 = 5;

/// How code is addressed: what the encoder, the relocations and the object's
/// machine depend on.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CodeModel {
    /// 16-bit segmented code, a near offset a word.
    Segmented16,
    /// 32-bit flat code, an offset a dword.
    Flat32,
    /// 64-bit flat code, reached from the instruction pointer.
    Flat64,
}

impl CodeModel {
    /// The model of code `bits` bits wide.
    pub fn of(bits: u32) -> Self {
        match bits {
            16 => Self::Segmented16,
            32 => Self::Flat32,
            64 => Self::Flat64,
            _ => panic!("no code model of {bits} bits"),
        }
    }

    /// The width of the mode instructions are encoded in.
    pub fn bits(self) -> u32 {
        match self {
            Self::Segmented16 => 16,
            Self::Flat32 => 32,
            Self::Flat64 => 64,
        }
    }

    pub fn segmented(self) -> bool {
        self == Self::Segmented16
    }

    pub fn flat(self) -> bool {
        !self.segmented()
    }

    /// The bytes of a near call's or jump's displacement.
    pub fn displacement_bytes(self) -> usize {
        if self.segmented() { 2 } else { 4 }
    }
}

impl ObjectFormat {
    /// The model of the code this format holds.
    pub fn code_model(&self) -> CodeModel {
        CodeModel::of(self.bitness)
    }

    /// `text`, an `object.toml`: `formats`, `default`, `bitness`, `header` and,
    /// optionally, `dwarf_version`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: toml::Table = text.parse().map_err(|error: toml::de::Error| error.to_string())?;
        let formats = value
            .get("formats")
            .and_then(|one| one.as_array())
            .ok_or("formats is not a list")?
            .iter()
            .map(|one| {
                Format::parse(one.as_str().ok_or("a format is not a string")?)
                    .map_err(|error| format!("formats: {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let default =
            Format::parse(value.get("default").and_then(|one| one.as_str()).ok_or("default is not a string")?)
                .map_err(|error| format!("default: {error}"))?;
        if !formats.contains(&default) {
            return Err(format!("default {:?} is not one of formats", default.name()));
        }
        let bitness = value
            .get("bitness")
            .and_then(|one| one.as_integer())
            .and_then(|one| u32::try_from(one).ok())
            .filter(|one| matches!(one, 16 | 32 | 64))
            .ok_or("bitness is not 16, 32 or 64")?;
        let header = value
            .get("header")
            .and_then(|one| one.as_array())
            .ok_or("header is not a list")?
            .iter()
            .map(|line| line.as_str().map(str::to_owned).ok_or("a header line is not a string"))
            .collect::<Result<Vec<_>, _>>()?;
        let dwarf_version = match value.get("dwarf_version") {
            None => DEFAULT_DWARF_VERSION,
            Some(one) => one
                .as_integer()
                .and_then(|one| u16::try_from(one).ok())
                .filter(|one| (2..=5).contains(one))
                .ok_or("dwarf_version is not 2 to 5")?,
        };
        let pc_relative = match value.get("pc_relative") {
            None => false,
            Some(one) => one.as_bool().ok_or("pc_relative is not a boolean")?,
        };
        let got_external = match value.get("got_external") {
            None => false,
            Some(one) => one.as_bool().ok_or("got_external is not a boolean")?,
        };
        Ok(Self { formats, default, bitness, header, dwarf_version, pc_relative, got_external })
    }

    /// The format written when `asked` is the one `-fobject-format=` named, if
    /// any.
    pub fn choose(
        &self,
        target: &str,
        asked: Option<Format>,
    ) -> Result<Format, String> {
        match asked {
            None => Ok(self.default),
            Some(one) if self.formats.contains(&one) => Ok(one),
            Some(one) => Err(format!(
                "{target} cannot write {}; it writes {}",
                one.name(),
                self.formats.iter().map(|one| one.name()).collect::<Vec<_>>().join(", ")
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_object_format_names_its_formats_its_default_its_mode_and_its_header() {
        let flat = ObjectFormat::parse(
            "formats = [\"omf\", \"elf\"]\ndefault = \"omf\"\nbitness = 32\nheader = [\".386\", \".model flat\"]\n",
        )
        .unwrap();
        assert_eq!(
            (flat.formats.as_slice(), flat.default, flat.bitness, flat.header.len()),
            (&[Format::Omf, Format::Elf][..], Format::Omf, 32, 2)
        );
    }

    #[test]
    fn a_target_declares_the_dwarf_version_its_consumers_read() {
        let parse = |extra: &str| {
            ObjectFormat::parse(&format!("formats = [\"omf\"]\ndefault = \"omf\"\nbitness = 16\nheader = []\n{extra}"))
        };
        assert_eq!(parse("").unwrap().dwarf_version, DEFAULT_DWARF_VERSION);
        assert_eq!(parse("dwarf_version = 2\n").unwrap().dwarf_version, 2);
        assert_eq!(parse("dwarf_version = 6\n").unwrap_err(), "dwarf_version is not 2 to 5");
    }

    #[test]
    fn a_bad_object_format_is_refused_with_what_is_wrong() {
        let parse = |formats: &str, default: &str, bitness: u32| {
            ObjectFormat::parse(&format!(
                "formats = {formats}\ndefault = \"{default}\"\nbitness = {bitness}\nheader = []\n"
            ))
            .unwrap_err()
        };
        assert_eq!(parse("[\"pe\"]", "omf", 32), "formats: \"pe\" is not one of \"omf\", \"elf\", \"macho\", \"coff\"");
        assert_eq!(parse("[\"omf\"]", "elf", 32), "default \"elf\" is not one of formats");
        assert_eq!(parse("[\"omf\"]", "omf", 24), "bitness is not 16, 32 or 64");
    }

    /// A format the target does not list was written as the target's own, or
    /// refused with no word of what it can write.
    #[test]
    fn a_format_the_target_does_not_list_is_refused_naming_what_it_writes() {
        let real = ObjectFormat::parse("formats = [\"omf\"]\ndefault = \"omf\"\nbitness = 16\nheader = []\n").unwrap();
        assert_eq!(real.choose("x86-m16", None), Ok(Format::Omf));
        assert_eq!(real.choose("x86-m16", Some(Format::Elf)).unwrap_err(), "x86-m16 cannot write elf; it writes omf");
    }

    /// A writer that refused an object left no trace for the sweep, so a
    /// refusal on one side compared equal to a refusal on the other.
    #[test]
    fn a_refusing_writer_leaves_its_reason_in_the_sweep_directory() {
        let dir = std::env::temp_dir().join(format!("refusal-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        record_refusal(&dir, Format::MachO, "no name");
        assert_eq!(std::fs::read_to_string(dir.join("macho.err")).unwrap(), "no name");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
