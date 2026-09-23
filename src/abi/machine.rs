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

    /// Whether every far address with a selector in [low, high] lands in
    /// memory no program data occupies, whatever its offset.
    pub fn foreign_selectors(&self, low: i64, high: i64) -> bool {
        if self.addressing != Addressing::Real || low < 0 || high > 0xFFFF || low > high {
            return false;
        }
        let (start, end) = (low * 16, high * 16 + 0x1_0000);
        self.foreign.iter().any(|&(from, to)| from <= start && end <= to)
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
        assert!(dos.foreign_selectors(0xA000, 0xAF8C));
        assert!(!dos.foreign_selectors(0xA000, 0xB001));
        assert!(!dos.foreign_selectors(0x9FFF, 0xA000));
        let protected = Machine { addressing: Addressing::Protected, ..dos };
        assert!(!protected.foreign_selectors(0xA000, 0xA000));
    }
}
