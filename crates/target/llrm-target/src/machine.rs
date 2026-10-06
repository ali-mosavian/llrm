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
    /// One linear address space: no selectors, no segments.
    Flat,
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
    /// The segment registers the program model reserves; none on a flat machine.
    pub segments: Option<Segments>,
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
    /// The data layout and address spaces of the target this machine is the platform of: a recompiler of
    /// objects (BC's) reads them from the machine it is handed, not from a target it names.
    pub layout: Option<crate::layout::Layout>,
}

impl Machine {
    /// This machine with the stack in the data group (SS is DS): the runtime of a language that keeps its stack in
    /// DGROUP says so; the flag `-mstack-is-data` says it of a program.
    pub fn with_stack_in_data(self) -> Self {
        Self { segments: self.segments.map(|segments| Segments { stack_is_data: true, ..segments }), ..self }
    }

    /// This machine as the platform of the target whose layout is `layout`.
    pub fn with_layout(self, layout: crate::layout::Layout) -> Self {
        Self { layout: Some(layout), ..self }
    }

    /// The layout the machine was given.
    pub fn layout(&self) -> &crate::layout::Layout {
        self.layout.as_ref().expect("a machine of a target states its layout")
    }

    /// The machine `text` describes, priced for `cpu`: the target's `default_cpu`,
    /// stated once in its `timings.times`, not by the machine.
    pub fn parse(text: &str, cpu: &str) -> Result<Self, String> {
        let table: toml::Table = text.parse().map_err(|error: toml::de::Error| error.to_string())?;
        let addressing = match table.get("addressing").and_then(toml::Value::as_str) {
            Some("real") => Addressing::Real,
            Some("protected") => Addressing::Protected,
            Some("flat") => Addressing::Flat,
            other => return Err(format!("addressing must be \"real\", \"protected\" or \"flat\", not {other:?}")),
        };
        let segments = if addressing == Addressing::Flat {
            if table.contains_key("segments") {
                return Err("a flat machine has no segments".to_owned());
            }
            None
        } else {
            Some(Self::segments(&table)?)
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
        if table.contains_key("cpu") {
            return Err("a machine states no cpu: the target's timings.times names the default".to_owned());
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
            layout: None,
        })
    }

    fn segments(table: &toml::Table) -> Result<Segments, String> {
        let segments = table.get("segments").and_then(toml::Value::as_table).ok_or("segments is not a table")?;
        let name = |key: &str| {
            segments.get(key).and_then(toml::Value::as_str).map(str::to_owned).ok_or_else(|| format!("segments needs a {key}"))
        };
        Ok(Segments {
            data: name("data")?,
            stack: name("stack")?,
            code: name("code")?,
            stack_is_data: segments
                .get("stack_is_data")
                .and_then(toml::Value::as_bool)
                .ok_or("segments needs a boolean stack_is_data")?,
        })
    }

    /// The selector stride a huge pointer takes per 64K, as a shift of the
    /// carry: where a selector is a paragraph number, 64K is 1 << 12 of them.
    /// None where nothing states it.
    pub fn huge_shift(&self) -> Option<u32> {
        match self.addressing {
            Addressing::Real => Some(16 - 4),
            Addressing::Protected => self.protected_huge_shift,
            Addressing::Flat => None,
        }
    }

    pub fn load(path: &std::path::Path, cpu: &str) -> Result<Self, String> {
        Self::parse(&std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?, cpu)
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


#[cfg(test)]
mod tests {
    use super::*;

    const FLAT: &str = "addressing = \"flat\"\nsegment_end_faults = false\n";

    /// A flat machine had no way to be described: `segments` was required and
    /// `addressing` was real or protected, so m32 could not state its platform.
    #[test]
    fn test_a_flat_machine_parses_without_segments() {
        let flat = Machine::parse(FLAT, "486").expect("a flat machine parses");
        assert_eq!(flat.addressing, Addressing::Flat);
        assert!(flat.segments.is_none());
        assert_eq!(flat.huge_shift(), None);
        assert_eq!(flat.foreign_span((0, 0), (0x100, 0x100), 1), None);
        assert!(flat.access_may_trap(1, 1));
    }

    #[test]
    fn test_a_flat_machine_with_segments_is_refused() {
        let text = format!("{FLAT}[segments]\ndata = \"ds\"\nstack = \"ss\"\ncode = \"cs\"\nstack_is_data = true\n");
        assert_eq!(Machine::parse(&text, "486"), Err("a flat machine has no segments".to_owned()));
    }

    /// The PC's ports are one file every PC platform appends to its own text.
    #[test]
    fn test_a_platform_takes_the_shared_pc_ports() {
        let flat = Machine::parse(&format!("{FLAT}{}", crate::PC_PORTS), "486").unwrap();
        assert_eq!(flat.port_memory((0x3C4, 0x3C5)), PortMemory::None);
        assert_eq!(flat.port_memory((0x0B, 0x0B)), PortMemory::Dma);
        assert_eq!(flat.port_memory((0x300, 0x300)), PortMemory::Any);
    }

    /// BASIC's machine was a second static beside the built-in one (`BASIC`), made by hand from it: the runtime that
    /// keeps its stack in the data group asks for it of any machine, and a flat one has no segments to change.
    #[test]
    fn test_a_machine_keeps_its_stack_in_the_data_group_when_asked() {
        let real = "addressing = \"real\"\nsegment_end_faults = true\n[segments]\ndata = \"ds\"\nstack = \"ss\"\ncode = \"cs\"\nstack_is_data = false\n";
        let machine = Machine::parse(real, "486").unwrap();
        assert!(!machine.segments.as_ref().unwrap().stack_is_data);
        assert!(machine.with_stack_in_data().segments.unwrap().stack_is_data);
        assert!(Machine::parse(FLAT, "486").unwrap().with_stack_in_data().segments.is_none());
    }

    #[test]
    fn test_a_segmented_machine_still_needs_its_segments() {
        let text = "addressing = \"real\"\nsegment_end_faults = true\n";
        assert_eq!(Machine::parse(text, "486"), Err("segments is not a table".to_owned()));
    }

    /// dos.toml said `cpu = "486"` while timings.times said `default_cpu 386`: two
    /// defaults, so Nib and BASIC compiled for one CPU and C for another. A machine
    /// that states a CPU is refused; the target's timings.times is the one place.
    #[test]
    fn test_a_machine_states_no_cpu() {
        let text = format!("{FLAT}cpu = \"486\"\n");
        assert!(Machine::parse(&text, "486").unwrap_err().contains("timings.times"));
        assert_eq!(Machine::parse(FLAT, "P5").unwrap().cpu, "P5");
    }
}
