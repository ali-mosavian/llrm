//! A table of label differences (a jump table): each entry is `(label - base)
//! >> shift`, `width` bytes, where the labels and the base are in one section,
//! so the assembler resolves the entries and the object holds plain bytes.

use crate::Unsupported;

/// How one entry is stored.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Entry {
    /// Bytes per entry: 1, 2, 4 or 8.
    pub width: u8,
    /// The difference is divided by `1 << shift`, which must be exact.
    pub shift: u8,
    /// The difference may be negative; otherwise it must not be.
    pub signed: bool,
}

/// The bytes of the table whose entries are the labels `entries` less `base`,
/// the offsets of the labels being `offsets`.
pub fn differences(
    entry: Entry,
    base: &str,
    entries: &[String],
    offsets: &impl Fn(&str) -> Option<i64>,
) -> Result<Vec<u8>, Unsupported> {
    let at =
        |name: &str| offsets(name).ok_or_else(|| Unsupported(format!("a jump table names the unplaced label {name}")));
    let base = at(base)?;
    let mut out = Vec::with_capacity(entries.len() * usize::from(entry.width));
    for name in entries {
        let distance = at(name)? - base;
        if distance & ((1 << entry.shift) - 1) != 0 {
            return Err(Unsupported(format!("{name} is not a multiple of {} from the table base", 1 << entry.shift)));
        }
        let value = distance >> entry.shift;
        let bits = u32::from(entry.width) * 8;
        let fits = if entry.signed {
            bits >= 64 || (-(1i64 << (bits - 1))..(1i64 << (bits - 1))).contains(&value)
        } else {
            value >= 0 && (bits >= 64 || value < (1i64 << bits))
        };
        if !fits {
            return Err(Unsupported(format!("{name} is out of range of a {}-byte table entry", entry.width)));
        }
        out.extend_from_slice(&value.to_le_bytes()[..usize::from(entry.width)]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(name: &str) -> Option<i64> {
        match name {
            "base" => Some(8),
            "near" => Some(12),
            "far" => Some(8 + 4 * 300),
            "before" => Some(0),
            _ => None,
        }
    }

    #[test]
    fn entries_are_scaled_differences() {
        let entry = Entry { width: 1, shift: 2, signed: false };
        let bytes = differences(entry, "base", &["base".into(), "near".into()], &labels).unwrap();
        assert_eq!(bytes, [0, 1]);
    }

    #[test]
    fn an_entry_that_does_not_fit_is_refused() {
        let entry = Entry { width: 1, shift: 2, signed: false };
        assert!(differences(entry, "base", &["far".into()], &labels).is_err());
        let wide = Entry { width: 2, shift: 2, signed: false };
        assert_eq!(differences(wide, "base", &["far".into()], &labels).unwrap(), 300u16.to_le_bytes());
    }

    #[test]
    fn a_signed_entry_may_precede_the_base() {
        let entry = Entry { width: 4, shift: 0, signed: true };
        assert_eq!(differences(entry, "base", &["before".into()], &labels).unwrap(), (-8i32).to_le_bytes());
        let unsigned = Entry { signed: false, ..entry };
        assert!(differences(unsigned, "base", &["before".into()], &labels).is_err());
    }
}
