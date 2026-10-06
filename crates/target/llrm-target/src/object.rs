//! What a target says of the object file and the listing it writes: the one
//! OMF writer reads its record widths and the listing's opening lines here, not
//! from a branch on the target. Read from the target's `object.toml`.

/// A target's object format.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectFormat {
    /// The writer: `omf`.
    pub writer: String,
    /// 16 or 32: the mode instructions are encoded in, and a segment's USE.
    pub bitness: u32,
    /// The lines a listing opens with: its instruction set and memory model.
    pub header: Vec<String>,
}

impl ObjectFormat {
    /// `text`, an `object.toml`: `writer`, `bitness` and `header`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: toml::Table = text.parse().map_err(|error: toml::de::Error| error.to_string())?;
        let writer = value.get("writer").and_then(|one| one.as_str()).ok_or("writer is not a string")?.to_owned();
        if writer != "omf" {
            return Err(format!("writer {writer:?} is not one of \"omf\""));
        }
        let bitness = value.get("bitness").and_then(|one| one.as_integer()).and_then(|one| u32::try_from(one).ok()).filter(|one| matches!(one, 16 | 32)).ok_or("bitness is not 16 or 32")?;
        let header = value
            .get("header")
            .and_then(|one| one.as_array())
            .ok_or("header is not a list")?
            .iter()
            .map(|line| line.as_str().map(str::to_owned).ok_or("a header line is not a string"))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { writer, bitness, header })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_object_format_names_its_writer_its_mode_and_its_header() {
        let flat = ObjectFormat::parse("writer = \"omf\"\nbitness = 32\nheader = [\".386\", \".model flat\"]\n").unwrap();
        assert_eq!((flat.writer.as_str(), flat.bitness, flat.header.len()), ("omf", 32, 2));
    }

    #[test]
    fn a_bad_object_format_is_refused_with_what_is_wrong() {
        assert_eq!(ObjectFormat::parse("writer = \"coff\"\nbitness = 32\nheader = []\n").unwrap_err(), "writer \"coff\" is not one of \"omf\"");
        assert_eq!(ObjectFormat::parse("writer = \"omf\"\nbitness = 24\nheader = []\n").unwrap_err(), "bitness is not 16 or 32");
    }
}
