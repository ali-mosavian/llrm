//! What a target says of the object file and the listing it writes: the formats it can write and
//! the default among them, its record widths, and the listing's opening lines. Read from the
//! target's `object.toml`.

/// An object file format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    Omf,
    Elf,
    MachO,
}

impl Format {
    pub const ALL: [Format; 3] = [Format::Omf, Format::Elf, Format::MachO];

    /// The name `-fobject-format=` and `object.toml` spell it.
    pub fn name(self) -> &'static str {
        match self {
            Format::Omf => "omf",
            Format::Elf => "elf",
            Format::MachO => "macho",
        }
    }

    pub fn parse(name: &str) -> Result<Self, String> {
        Self::ALL.into_iter().find(|one| one.name() == name).ok_or_else(|| format!("{name:?} is not one of {}", Self::ALL.map(|one| format!("{:?}", one.name())).join(", ")))
    }
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
}

impl ObjectFormat {
    /// `text`, an `object.toml`: `formats`, `default`, `bitness` and `header`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: toml::Table = text.parse().map_err(|error: toml::de::Error| error.to_string())?;
        let formats = value
            .get("formats")
            .and_then(|one| one.as_array())
            .ok_or("formats is not a list")?
            .iter()
            .map(|one| Format::parse(one.as_str().ok_or("a format is not a string")?).map_err(|error| format!("formats: {error}")))
            .collect::<Result<Vec<_>, _>>()?;
        let default = Format::parse(value.get("default").and_then(|one| one.as_str()).ok_or("default is not a string")?).map_err(|error| format!("default: {error}"))?;
        if !formats.contains(&default) {
            return Err(format!("default {:?} is not one of formats", default.name()));
        }
        let bitness = value.get("bitness").and_then(|one| one.as_integer()).and_then(|one| u32::try_from(one).ok()).filter(|one| matches!(one, 16 | 32)).ok_or("bitness is not 16 or 32")?;
        let header = value
            .get("header")
            .and_then(|one| one.as_array())
            .ok_or("header is not a list")?
            .iter()
            .map(|line| line.as_str().map(str::to_owned).ok_or("a header line is not a string"))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { formats, default, bitness, header })
    }

    /// The format written when `asked` is the one `-fobject-format=` named, if any.
    pub fn choose(&self, target: &str, asked: Option<Format>) -> Result<Format, String> {
        match asked {
            None => Ok(self.default),
            Some(one) if self.formats.contains(&one) => Ok(one),
            Some(one) => Err(format!("{target} cannot write {}; it writes {}", one.name(), self.formats.iter().map(|one| one.name()).collect::<Vec<_>>().join(", "))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_object_format_names_its_formats_its_default_its_mode_and_its_header() {
        let flat = ObjectFormat::parse("formats = [\"omf\", \"elf\"]\ndefault = \"omf\"\nbitness = 32\nheader = [\".386\", \".model flat\"]\n").unwrap();
        assert_eq!((flat.formats.as_slice(), flat.default, flat.bitness, flat.header.len()), (&[Format::Omf, Format::Elf][..], Format::Omf, 32, 2));
    }

    #[test]
    fn a_bad_object_format_is_refused_with_what_is_wrong() {
        let parse = |formats: &str, default: &str, bitness: u32| ObjectFormat::parse(&format!("formats = {formats}\ndefault = \"{default}\"\nbitness = {bitness}\nheader = []\n")).unwrap_err();
        assert_eq!(parse("[\"coff\"]", "omf", 32), "formats: \"coff\" is not one of \"omf\", \"elf\", \"macho\"");
        assert_eq!(parse("[\"omf\"]", "elf", 32), "default \"elf\" is not one of formats");
        assert_eq!(parse("[\"omf\"]", "omf", 24), "bitness is not 16 or 32");
    }

    /// A format the target does not list was written as the target's own, or refused with no
    /// word of what it can write.
    #[test]
    fn a_format_the_target_does_not_list_is_refused_naming_what_it_writes() {
        let real = ObjectFormat::parse("formats = [\"omf\"]\ndefault = \"omf\"\nbitness = 16\nheader = []\n").unwrap();
        assert_eq!(real.choose("x86-code16", None), Ok(Format::Omf));
        assert_eq!(real.choose("x86-code16", Some(Format::Elf)).unwrap_err(), "x86-code16 cannot write elf; it writes omf");
    }
}
