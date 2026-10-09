//! What CodeView's $$TYPES records (CodeView 4 and C13 alike) are made of.

/// The filler byte `left` bytes before a record's end: `LF_PAD0 | left`.
pub fn pad(left: usize) -> u8 {
    0xF0 | left as u8
}

/// A number the way a leaf holds one: itself under 0x8000, else a tagged
/// integer of the smallest width that holds it.
pub fn numeric(
    out: &mut Vec<u8>,
    value: i64,
) {
    let put16 = |out: &mut Vec<u8>, value: u16| out.extend(value.to_le_bytes());
    if (0..0x8000).contains(&value) {
        put16(out, value as u16);
    } else if value < 0 {
        match (i8::try_from(value), i16::try_from(value), i32::try_from(value)) {
            (Ok(one), ..) => {
                put16(out, 0x8000);
                out.push(one as u8);
            }
            (_, Ok(one), _) => {
                put16(out, 0x8001);
                out.extend(one.to_le_bytes());
            }
            (.., Ok(one)) => {
                put16(out, 0x8003);
                out.extend(one.to_le_bytes());
            }
            _ => {
                put16(out, 0x8009);
                out.extend(value.to_le_bytes());
            }
        }
    } else if let Ok(one) = u16::try_from(value) {
        put16(out, 0x8002);
        put16(out, one);
    } else if let Ok(one) = u32::try_from(value) {
        put16(out, 0x8004);
        out.extend(one.to_le_bytes());
    } else {
        put16(out, 0x800A);
        out.extend(value.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 0x8000 and over, and every negative number, are tagged: reading `0x8000`
    /// as a number below it put a struct's size 32768 bytes short.
    #[test]
    fn a_number_is_tagged_from_0x8000_and_by_the_smallest_width() {
        let of = |value| {
            let mut out = Vec::new();
            numeric(&mut out, value);
            out
        };
        assert_eq!(of(0x7FFF), [0xFF, 0x7F]);
        assert_eq!(of(0x8000), [0x02, 0x80, 0x00, 0x80]);
        assert_eq!(of(0x1_0000), [0x04, 0x80, 0, 0, 1, 0]);
        assert_eq!(of(-1), [0x00, 0x80, 0xFF]);
        assert_eq!(of(-200), [0x01, 0x80, 0x38, 0xFF]);
        assert_eq!(of(1 << 40).len(), 10);
        assert_eq!(pad(3), 0xF3);
    }
}
