//! A target's register file, from its `registers.regs`: one register a line,
//! `name bits root lane classes dwarf codeview`, classes comma separated and `-` for none; the last
//! two are the register's number in each debug format, `-` where it has none.

/// One register: a root, or a view of one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Register {
    pub name: String,
    pub bits: u32,
    /// The full register this is a view of (itself for a root).
    pub root: String,
    /// The bit offset inside the root.
    pub lane: u32,
    pub classes: Vec<String>,
    /// Its DWARF register number, where the psABI gives it one.
    pub dwarf: Option<u16>,
    /// Its CodeView register id (`CV_HREG_e`).
    pub codeview: Option<u16>,
}

impl Register {
    pub fn is(
        &self,
        class: &str,
    ) -> bool {
        self.classes.iter().any(|one| one == class)
    }
}

/// A number, or `-` for none.
fn optional(text: &str) -> Result<Option<u16>, String> {
    if text == "-" { Ok(None) } else { text.parse().map(Some).map_err(|_| text.to_owned()) }
}

pub fn parse(text: &str) -> Result<Vec<Register>, String> {
    let mut registers = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let columns: Vec<&str> = line.split_whitespace().collect();
        let [name, bits, root, lane, classes, dwarf, codeview] = columns[..] else {
            return Err(format!("registers.regs:{}: {} columns, not 7", index + 1, columns.len()));
        };
        let number =
            |text: &str, what: &str| text.parse().map_err(|_| format!("registers.regs:{}: {what} `{text}`", index + 1));
        registers.push(Register {
            name: name.to_owned(),
            bits: number(bits, "bits")?,
            root: root.to_owned(),
            lane: number(lane, "lane")?,
            classes: if classes == "-" { Vec::new() } else { classes.split(',').map(str::to_owned).collect() },
            dwarf: optional(dwarf).map_err(|text| format!("registers.regs:{}: dwarf `{text}`", index + 1))?,
            codeview: optional(codeview).map_err(|text| format!("registers.regs:{}: codeview `{text}`", index + 1))?,
        });
    }
    Ok(registers)
}

/// How many registers hold values: the roots of class `gpr`.
pub fn allocatable(registers: &[Register]) -> usize {
    registers.iter().filter(|one| one.is("gpr") && one.root == one.name).count()
}

/// The roots of class `class`, in file order.
pub fn of_class<'a>(
    registers: &'a [Register],
    class: &str,
) -> Vec<&'a str> {
    registers.iter().filter(|one| one.is(class) && one.root == one.name).map(|one| one.name.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_roots_of_class_gpr_hold_values() {
        let text = "# a file\neax 32 eax 0 gpr,byte,base 0 17\nebp 32 ebp 0 reserved,frame 5 22\nax 16 eax 0 - 0 9\nesi 32 esi 0 gpr,base 6 23\n";
        let registers = parse(text).unwrap();
        assert_eq!(registers.len(), 4);
        assert_eq!(allocatable(&registers), 2);
        assert!(registers[0].is("byte") && registers[2].classes.is_empty());
        assert_eq!(parse("eax 32 eax").unwrap_err(), "registers.regs:1: 3 columns, not 7");
        assert_eq!((registers[0].dwarf, registers[0].codeview), (Some(0), Some(17)));
    }

    /// A register with no number in a format says so with `-`, and a word that is no number is an
    /// error, not a register with none.
    #[test]
    fn a_dash_is_no_number_and_a_word_is_an_error() {
        let none = parse("ah 8 eax 8 - - 5").unwrap();
        assert_eq!((none[0].dwarf, none[0].codeview), (None, Some(5)));
        assert_eq!(parse("ah 8 eax 8 - x 5").unwrap_err(), "registers.regs:1: dwarf `x`");
    }
}
