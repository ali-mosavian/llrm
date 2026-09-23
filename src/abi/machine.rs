//! What the machine promises, whatever the source language or runtime: how a
//! far address becomes a linear one, which memory holds no program data, and
//! which ports touch no memory. One description per target, read from TOML;
//! real-mode DOS is the default.

use std::sync::OnceLock;

pub const DOS: &str = include_str!("machines/dos.toml");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Addressing {
    /// selector * 16 + offset.
    Real,
    /// A selector names a descriptor: nothing follows from its value.
    Protected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Machine {
    pub addressing: Addressing,
    /// Linear [low, high) ranges no program data occupies.
    pub foreign: Vec<(i64, i64)>,
    /// [low, high) port ranges whose writes change no memory.
    pub silent_ports: Vec<(i64, i64)>,
}

impl Machine {
    pub fn parse(text: &str) -> Result<Self, String> {
        let table: toml::Table = text.parse().map_err(|error: toml::de::Error| error.to_string())?;
        let addressing = match table.get("addressing").and_then(toml::Value::as_str) {
            Some("real") => Addressing::Real,
            Some("protected") => Addressing::Protected,
            other => return Err(format!("addressing must be \"real\" or \"protected\", not {other:?}")),
        };
        let ranges = |key: &str| -> Result<Vec<(i64, i64)>, String> {
            let Some(rows) = table.get(key) else { return Ok(Vec::new()) };
            let rows = rows.as_array().ok_or_else(|| format!("{key} is not an array of tables"))?;
            rows.iter()
                .map(|row| {
                    let bound = |name: &str| {
                        row.get(name).and_then(toml::Value::as_integer).ok_or_else(|| format!("{key} needs an integer {name}"))
                    };
                    let (low, high) = (bound("low")?, bound("high")?);
                    if low >= high {
                        return Err(format!("{key}: {low:#x} is not below {high:#x}"));
                    }
                    Ok((low, high))
                })
                .collect()
        };
        Ok(Self { addressing, foreign: ranges("foreign")?, silent_ports: ranges("silent_ports")? })
    }

    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        Self::parse(&std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?)
    }

    /// The linear [start, end) that `width`-byte accesses at every selector
    /// and offset in the inclusive ranges given span, when all of it is memory
    /// no program data occupies.
    pub fn foreign_span(&self, selectors: (i64, i64), offsets: (i64, i64), width: i64) -> Option<(i64, i64)> {
        let word = |(low, high): (i64, i64)| 0 <= low && low <= high && high <= 0xFFFF;
        if self.addressing != Addressing::Real || !word(selectors) || !word(offsets) {
            return None;
        }
        let (start, end) = (selectors.0 * 16 + offsets.0, selectors.1 * 16 + offsets.1 + width);
        self.foreign.iter().any(|&(from, to)| from <= start && end <= to).then_some((start, end))
    }

    pub fn silent_port(&self, port: i64) -> bool {
        self.silent_ports.iter().any(|&(low, high)| low <= port && port < high)
    }
}

static CURRENT: OnceLock<Machine> = OnceLock::new();

/// Choose the target once, before compiling. Refused after the first use.
pub fn configure(machine: Machine) -> Result<(), String> {
    CURRENT.set(machine).map_err(|_| "the target machine is already fixed".to_owned())
}

/// The target: whatever `configure` chose, else real-mode DOS.
pub fn current() -> &'static Machine {
    CURRENT.get_or_init(|| Machine::parse(DOS).expect("the built-in DOS description parses"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vga_selectors_are_foreign_only_in_real_mode() {
        let dos = Machine::parse(DOS).unwrap();
        let every = (0, 0xFFFF);
        assert_eq!(dos.foreign_span((0xA000, 0xAF8C), every, 1), Some((0xA0000, 0xAF8C0 + 0x1_0000)));
        assert_eq!(dos.foreign_span((0xA000, 0xB001), every, 1), None);
        assert_eq!(dos.foreign_span((0x9FFF, 0xA000), every, 1), None);
        assert_eq!(dos.foreign_span((0x9FFF, 0x9FFF), (0x10, 0x11), 2), Some((0xA0000, 0xA0003)));
        let protected = Machine { addressing: Addressing::Protected, ..dos };
        assert_eq!(protected.foreign_span((0xA000, 0xA000), every, 1), None);
    }
}
