//! What the machine promises, whatever the source language or runtime: how a
//! far address becomes a linear one, which segment registers the program
//! model reserves, which memory holds no program data, what each I/O port
//! may do to memory, and which CPU its code is priced for. One description per
//! target, read from TOML.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Addressing {
    /// selector * 16 + offset.
    Real,
    /// A selector names a descriptor: nothing follows from its value.
    Protected,
}

/// The segment registers the program model reserves, by name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Segments {
    /// The data group every access without a prefix reads.
    pub data: String,
    pub stack: String,
    pub code: String,
    /// The stack lives in the data group: `stack` reaches the data too.
    pub stack_is_data: bool,
}

/// What an `in` or `out` of a port may do to memory, narrowest first.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PortMemory {
    /// Nothing: the device's registers only.
    None,
    /// Start a transfer that reads or writes any memory.
    Dma,
    /// Read and write any memory: a port the description does not list.
    Any,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Port {
    pub low: i64,
    pub high: i64,
    pub memory: PortMemory,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Machine {
    pub addressing: Addressing,
    pub segments: Segments,
    /// Linear [low, high) ranges no program data occupies.
    pub foreign: Vec<(i64, i64)>,
    /// [low, high) port ranges and what their access does to memory.
    pub ports: Vec<Port>,
    /// The processor whose costs choose between equivalent code.
    pub cpu: String,
    /// A multi-byte access crossing a segment's last offset faults.
    pub segment_end_faults: bool,
    /// Under protected addressing, the selector stride a huge pointer takes
    /// per 64K, as a shift: the system's, so the description states it.
    pub protected_huge_shift: Option<u32>,
    /// The program's start-up zeroes the far uninitialised data (class FAR_BSS), as DOS leaves
    /// memory past the image as it found it. Where it does not, zero far data is stored.
    pub far_bss: bool,
}

impl Machine {
    /// `cpus` are the processor names the target prices; a description naming
    /// another is refused.
    pub fn parse(text: &str, cpus: &[&str]) -> Result<Self, String> {
        let table: toml::Table = text.parse().map_err(|error: toml::de::Error| error.to_string())?;
        let addressing = match table.get("addressing").and_then(toml::Value::as_str) {
            Some("real") => Addressing::Real,
            Some("protected") => Addressing::Protected,
            other => return Err(format!("addressing must be \"real\" or \"protected\", not {other:?}")),
        };
        let segments = table.get("segments").and_then(toml::Value::as_table).ok_or("segments is not a table")?;
        let name = |key: &str| {
            segments.get(key).and_then(toml::Value::as_str).map(str::to_owned).ok_or_else(|| format!("segments needs a {key}"))
        };
        let segments = Segments {
            data: name("data")?,
            stack: name("stack")?,
            code: name("code")?,
            stack_is_data: segments
                .get("stack_is_data")
                .and_then(toml::Value::as_bool)
                .ok_or("segments needs a boolean stack_is_data")?,
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
        let segment_end_faults = table.get("segment_end_faults").and_then(toml::Value::as_bool).ok_or("segment_end_faults is not a boolean")?;
        let cpu = table.get("cpu").and_then(toml::Value::as_str).ok_or("cpu is not a string")?;
        if !cpus.contains(&cpu) {
            return Err(format!("cpu {cpu:?} is not one of {cpus:?}"));
        }
        Ok(Self {
            addressing,
            segments,
            foreign: ranges("foreign")?,
            ports: ports(&table)?,
            cpu: cpu.to_owned(),
            segment_end_faults,
            protected_huge_shift: table.get("huge_shift").and_then(toml::Value::as_integer).map(|shift| shift as u32),
            far_bss: table.get("far_bss").and_then(toml::Value::as_bool).unwrap_or(false),
        })
    }

    /// The selector stride a huge pointer takes per 64K, as a shift of the
    /// carry: where a selector is a paragraph number, 64K is 1 << 12 of them.
    /// None where nothing states it.
    pub fn huge_shift(&self) -> Option<u32> {
        match self.addressing {
            Addressing::Real => Some(16 - 4),
            Addressing::Protected => self.protected_huge_shift,
        }
    }

    pub fn load(path: &std::path::Path, cpus: &[&str]) -> Result<Self, String> {
        Self::parse(&std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?, cpus)
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
        // Adjacent ranges cover as one.
        let mut ranges = self.foreign.clone();
        ranges.sort_unstable();
        let mut reached = start;
        for (from, to) in ranges {
            if from <= reached && reached < to {
                reached = to;
            }
        }
        (end <= reached).then_some((start, end))
    }

    /// Whether a `width`-byte access at an offset a multiple of `align` may
    /// trap. In real mode only one reaching past offset FFFFh may, where
    /// that faults; the last such offset is `0x10000 - align`.
    pub fn access_may_trap(&self, width: u64, align: u64) -> bool {
        self.addressing != Addressing::Real || self.segment_end_faults && width > align
    }

    /// What an access to any port in the inclusive range may do to memory.
    pub fn port_memory(&self, (low, high): (i64, i64)) -> PortMemory {
        let mut widest = PortMemory::None;
        let mut at = low;
        while at <= high {
            let Some(port) = self.ports.iter().find(|port| port.low <= at && at < port.high) else { return PortMemory::Any };
            widest = widest.max(port.memory);
            at = port.high;
        }
        widest
    }
}

fn ports(table: &toml::Table) -> Result<Vec<Port>, String> {
    let Some(rows) = table.get("ports") else { return Ok(Vec::new()) };
    let rows = rows.as_array().ok_or("ports is not an array of tables")?;
    rows.iter()
        .map(|row| {
            let bound = |name: &str| row.get(name).and_then(toml::Value::as_integer).ok_or_else(|| format!("ports needs an integer {name}"));
            let (low, high) = (bound("low")?, bound("high")?);
            if low >= high {
                return Err(format!("ports: {low:#x} is not below {high:#x}"));
            }
            let memory = match row.get("memory").and_then(toml::Value::as_str) {
                Some("none") => PortMemory::None,
                Some("dma") => PortMemory::Dma,
                other => return Err(format!("a port's memory must be \"none\" or \"dma\", not {other:?}")),
            };
            Ok(Port { low, high, memory })
        })
        .collect()
}

