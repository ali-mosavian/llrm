//! A target's register file, from its `registers.regs`: one register a line,
//! `name bits root lane classes`, classes comma separated and `-` for none.

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
}

impl Register {
    pub fn is(&self, class: &str) -> bool {
        self.classes.iter().any(|one| one == class)
    }
}

pub fn parse(text: &str) -> Result<Vec<Register>, String> {
    let mut registers = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let columns: Vec<&str> = line.split_whitespace().collect();
        let [name, bits, root, lane, classes] = columns[..] else {
            return Err(format!("registers.regs:{}: {} columns, not 5", index + 1, columns.len()));
        };
        let number = |text: &str, what: &str| text.parse().map_err(|_| format!("registers.regs:{}: {what} `{text}`", index + 1));
        registers.push(Register {
            name: name.to_owned(),
            bits: number(bits, "bits")?,
            root: root.to_owned(),
            lane: number(lane, "lane")?,
            classes: if classes == "-" { Vec::new() } else { classes.split(',').map(str::to_owned).collect() },
        });
    }
    Ok(registers)
}

/// How many registers hold values: the roots of class `gpr`.
pub fn allocatable(registers: &[Register]) -> usize {
    registers.iter().filter(|one| one.is("gpr") && one.root == one.name).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_roots_of_class_gpr_hold_values() {
        let text = "# a file\neax 32 eax 0 gpr,byte,base\nebp 32 ebp 0 reserved,frame\nax 16 eax 0 -\nesi 32 esi 0 gpr,base\n";
        let registers = parse(text).unwrap();
        assert_eq!(registers.len(), 4);
        assert_eq!(allocatable(&registers), 2);
        assert!(registers[0].is("byte") && registers[2].classes.is_empty());
        assert_eq!(parse("eax 32 eax").unwrap_err(), "registers.regs:1: 3 columns, not 5");
    }
}
