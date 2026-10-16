//! Open Watcom's register sets as the front end records them (`3c:0`: two
//! hex words, `cgx86reg.h`'s bits), as register names in the order a value
//! travels: the high half first, as `[__dx __ax]` writes it.

/// The registers Open Watcom's compiler has, as the target's description states
/// them (`[frontend.registers]` of its C runtime description): the bits each
/// has in a set, which half of a whole register it is, and the few facts the
/// translation reads about them by role.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Registers {
    entries: Vec<Entry>,
    /// The register a block's code reaches by default, and the one holding the
    /// flags.
    pub accumulator: String,
    pub flags: String,
    /// Four words of a 64-bit value, the top one first.
    pub quad: Vec<String>,
    /// Registers that hold a segment, not a number.
    pub segments: Vec<String>,
    /// What a function answers in by the width of the answer, in bytes.
    pub answers: std::collections::BTreeMap<i64, Vec<String>>,
    /// The whole registers an inline block may declare it changes.
    pub clobbered: Vec<String>,
    /// The bits no block may change (it restores them itself).
    fixed: Set,
    /// Every bit a set can have.
    universe: Set,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Entry {
    name: String,
    bits: u32,
    rank: u32,
    /// The whole register this is half of; itself for a whole one.
    whole: String,
    high: bool,
}

impl Registers {
    pub fn parse(table: &toml::Table) -> Result<Self, String> {
        let list = |key: &str| -> Result<Vec<String>, String> {
            table
                .get(key)
                .and_then(toml::Value::as_array)
                .ok_or_else(|| format!("frontend.registers.{key} is not a list"))?
                .iter()
                .map(|one| {
                    one.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| format!("frontend.registers.{key} holds a non-string"))
                })
                .collect()
        };
        let text = |key: &str| {
            table
                .get(key)
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| format!("frontend.registers.{key} is not a string"))
        };
        let mut entries = Vec::new();
        for one in table.get("set").and_then(toml::Value::as_array).ok_or("frontend.registers.set is not a list")? {
            let field = |key: &str| one.get(key).and_then(toml::Value::as_integer);
            let name = one.get("name").and_then(toml::Value::as_str).ok_or("a register has no name")?.to_owned();
            entries.push(Entry {
                bits: u32::try_from(field("bits").ok_or("a register has no bits")?).map_err(|e| e.to_string())?,
                rank: field("rank").map_or(0, |rank| rank as u32),
                whole: one.get("whole").and_then(toml::Value::as_str).map_or_else(|| name.clone(), str::to_owned),
                high: one.get("high").and_then(toml::Value::as_bool).unwrap_or(false),
                name,
            });
        }
        let mut answers = std::collections::BTreeMap::new();
        for (width, names) in
            table.get("answer").and_then(toml::Value::as_table).ok_or("frontend.registers.answer is not a table")?
        {
            let names = names
                .as_array()
                .ok_or("an answer is not a list")?
                .iter()
                .map(|one| one.as_str().map(str::to_owned).ok_or("an answer register is not a string"))
                .collect::<Result<_, _>>()?;
            answers.insert(width.parse::<i64>().map_err(|e| e.to_string())?, names);
        }
        let mut registers = Self {
            entries,
            accumulator: text("accumulator")?,
            flags: text("flags")?,
            quad: list("quad")?,
            segments: list("segments")?,
            answers,
            clobbered: list("clobbered")?,
            fixed: Set::default(),
            universe: Set::parse(&text("universe")?).ok_or("frontend.registers.universe is not a set")?,
        };
        for name in list("fixed")? {
            let bits = registers.entry(&name).ok_or_else(|| format!("frontend.registers.fixed names {name}"))?.bits;
            registers.fixed.low |= bits;
        }
        Ok(registers)
    }

    fn entry(
        &self,
        name: &str,
    ) -> Option<&Entry> {
        self.entries.iter().find(|one| one.name == name)
    }

    /// The bits of a register, if it has some.
    pub fn bits(
        &self,
        name: &str,
    ) -> Option<Set> {
        self.entry(name).map(|one| Set { low: one.bits, high: 0 })
    }

    /// The whole register `name` is, or part of: `al` is in `ax`.
    pub fn whole_of<'a>(
        &'a self,
        name: &'a str,
    ) -> &'a str {
        self.entry(name).map_or(name, |one| &one.whole)
    }

    /// Whether `name` is a half (`al`, `ah`), and if so whether the high one.
    pub fn half(
        &self,
        name: &str,
    ) -> Option<bool> {
        self.entry(name).filter(|one| one.whole != one.name).map(|one| one.high)
    }

    /// Whether `name` is a whole register that holds a number: not a half and
    /// not a segment.
    pub fn is_word(
        &self,
        name: &str,
    ) -> bool {
        self.entry(name).is_some_and(|one| one.whole == one.name) && !self.segments.iter().any(|one| one == name)
    }

    /// Whether `saved` holds every general register (the accumulator, the other
    /// three and the index registers): a function that keeps them all.
    pub fn keeps_general(
        &self,
        saved: Set,
    ) -> bool {
        self.entries
            .iter()
            .filter(|one| {
                one.whole == one.name && self.clobbered.iter().any(|name| *name == one.name) && one.bits < 0x400
            })
            .all(|one| saved.low & one.bits == one.bits)
    }

    /// Everything a set may have changed before a function says what it keeps.
    pub fn universe(&self) -> Set {
        self.universe
    }

    /// Whether `set` has a register no inline block may declare changed.
    pub fn has_fixed(
        &self,
        set: Set,
    ) -> bool {
        set.low & self.fixed.low != 0
    }

    /// The set widened to the whole registers it touches (`al` to `ax`).
    pub fn whole(
        &self,
        set: Set,
    ) -> Set {
        let mut low = set.low;
        for one in self.entries.iter().filter(|one| one.whole != one.name) {
            if set.low & one.bits != 0 {
                low |= self.bits(&one.whole).map_or(0, |whole| whole.low);
            }
        }
        Set { low, high: set.high }
    }

    /// The names in the set, whole registers where both halves are in, the
    /// high half first: a segment register above an offset, an `si` or `di`
    /// below the register it pairs with, else the later of `a`, `b`, `c`, `d`
    /// above the earlier (the ranks). None where a bit has no name here (FS,
    /// GS, the 32-bit halves).
    pub fn names(
        &self,
        set: Set,
    ) -> Option<Vec<String>> {
        let known = self.entries.iter().fold(0, |all, one| all | one.bits);
        if set.high != 0 || set.low & !known != 0 {
            return None;
        }
        let single = |one: &&Entry| one.bits.count_ones() == 1 && one.bits < 0x100;
        let mut found: Vec<&Entry> = Vec::new();
        let mut left = set.low;
        for one in self.entries.iter().filter(|one| !single(one)) {
            if left & one.bits == one.bits {
                found.push(one);
                left &= !one.bits;
            }
        }
        for one in self.entries.iter().filter(single) {
            if left & one.bits != 0 {
                found.push(one);
                left &= !one.bits;
            }
        }
        found.sort_by_key(|one| std::cmp::Reverse(one.rank));
        Some(found.into_iter().map(|one| one.name.clone()).collect())
    }
}

/// A register set: the front end's two words.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Set {
    pub low: u32,
    pub high: u32,
}

impl Set {
    /// `3c:0`; `-` and an empty text are the empty set.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() || text == "-" {
            return Some(Self::default());
        }
        let (low, high) = text.split_once(':')?;
        Some(Self { low: u32::from_str_radix(low, 16).ok()?, high: u32::from_str_radix(high, 16).ok()? })
    }

    pub fn is_empty(self) -> bool {
        self == Self::default()
    }

    pub fn has(
        self,
        other: Self,
    ) -> bool {
        self.low & other.low == other.low && self.high & other.high == other.high
    }

    pub fn without(
        self,
        other: Self,
    ) -> Self {
        Self { low: self.low & !other.low, high: self.high & !other.high }
    }

    pub fn with(
        self,
        other: Self,
    ) -> Self {
        Self { low: self.low | other.low, high: self.high | other.high }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registers() -> Registers {
        crate::compile::Profile::of(&llrm_x86_m16::M16).unwrap().hardware
    }

    fn names(text: &str) -> Vec<String> {
        registers().names(Set::parse(text).unwrap()).unwrap()
    }

    /// Open Watcom's `[__dx __ax]` is a long: the front end writes `c3:0` and
    /// the high half is dx. Read as a set of names it came out in bit order.
    #[test]
    fn test_a_pair_is_named_high_half_first() {
        assert_eq!(names("c3:0"), ["dx", "ax"]);
        assert_eq!(names("3c:0"), ["cx", "bx"]);
        assert_eq!(names("1100:0"), ["ds", "si"]);
        assert_eq!(names("2200:0"), ["es", "di"]);
        assert_eq!(names("c:0"), ["bx"]);
        assert_eq!(names("2:0"), ["al"]);
        assert_eq!(names("10103:0"), ["si", "ax", "st0"]);
    }

    /// `[__si __ax]` is a far pointer whose segment is SI (clib's `_fmemmove`
    /// moves SI into DS), whichever way it is written. A set read in bit order
    /// took AX for the segment, and `_fmemmove` copied from the wrong place.
    /// Each row is what Open Watcom's compiler does with that pair.
    #[test]
    fn test_the_segment_of_a_register_pair_is_the_one_the_front_end_picks() {
        for (set, high, low) in [
            ("103:0", "si", "ax"),
            ("10c:0", "si", "bx"),
            ("230:0", "cx", "di"),
            ("300:0", "di", "si"),
            ("f0:0", "cx", "dx"),
            ("20c:0", "di", "bx"),
            ("2200:0", "es", "di"),
        ] {
            assert_eq!(names(set), [high, low], "{set}");
        }
    }

    #[test]
    fn test_a_set_with_a_register_this_target_lacks_has_no_names() {
        assert!(registers().names(Set::parse("0:1").unwrap()).is_none());
        assert!(registers().names(Set::parse("1000000:0").unwrap()).is_none());
    }

    #[test]
    fn test_the_empty_set_and_set_arithmetic() {
        assert!(Set::parse("-").unwrap().is_empty());
        let all = Set::parse("ffffdfff:ffffffff").unwrap();
        let kept = Set::parse("2300:0").unwrap();
        assert_eq!(all.without(kept).low, 0xffff_dcff);
        assert!(registers().whole(Set::parse("c3:0").unwrap()).has(Set::parse("c0:0").unwrap()));
    }
}
