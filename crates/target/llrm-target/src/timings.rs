//! A target's CPU timings, from its `timings.times`: a `cpus` line names the
//! columns, scalar rows give each CPU's issue width and prefix costs, and the
//! `[clocks]` and `[latency]` tables give each instruction form's price, `-`
//! where the CPU has none.

/// One CPU's column.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CpuTable {
    pub issue: i64,
    pub in_order: bool,
    pub prefix: i64,
    pub partial_stall: i64,
    pub lcp_stall: i64,
    pub pairing: bool,
    /// gcc's by-pieces limits for a constant memset of this CPU: stores while
    /// their number is under the ratio, else the stringop algorithm (`rep
    /// stos`); `None` where gcc's algorithm for the CPU is not one this target
    /// emits. `set` is a nonzero byte, `clear` zero; `_size` is the
    /// optimise-for-size path.
    pub set_ratio: Option<i64>,
    pub clear_ratio: Option<i64>,
    pub set_ratio_size: Option<i64>,
    pub clear_ratio_size: Option<i64>,
    /// The fewest case values for which a switch is a jump table.
    pub jump_table: Option<i64>,
    /// Bytes of a line of the first-level data cache.
    pub cache_line: Option<i64>,
    /// Form and clocks, in file order.
    pub clocks: Vec<(String, i64)>,
    pub latency: Vec<(String, i64)>,
}

/// The price a tune file gives a row of `section`: a scalar at the top, a form
/// under `clocks` or `latency`. `docs` holds each CPU's file once parsed.
fn tune_value(
    tunes: &[(&str, &str)],
    docs: &mut [Option<toml::Table>],
    marches: &[String],
    at_cpu: usize,
    section: &str,
    name: &str,
) -> Result<String, String> {
    let march = marches.get(at_cpu).ok_or("`=` before the `march` line")?;
    if docs[at_cpu].is_none() {
        let text = tunes.iter().find(|(one, _)| one == march).map(|(_, text)| *text);
        let text = text.ok_or(format!("no tune file for {march}"))?;
        docs[at_cpu] = Some(text.parse().map_err(|e| format!("tune {march}: {e}"))?);
    }
    let doc = docs[at_cpu].as_ref().expect("parsed above");
    let found = match section {
        "scalars" => doc.get(name),
        other => doc.get(other).and_then(|table| table.get(name)),
    };
    match found.and_then(|one| one.as_integer()) {
        Some(number) => Ok(number.to_string()),
        None => Err(format!("tune {march} has no `{name}` in {section}")),
    }
}

#[derive(Clone, Debug)]
pub struct Timings {
    cpus: Vec<String>,
    tables: Vec<CpuTable>,
    default: Option<String>,
    /// What gcc's `-march` calls each CPU, in column order.
    marches: Vec<String>,
}

impl Timings {
    pub fn parse(text: &str) -> Result<Self, String> {
        Self::parse_tuned(text, &[])
    }

    /// `parse`, where a `=` in a CPU's cell reads the row from that CPU's tune
    /// file: `tunes` pairs a `march` name with the file's text.
    pub fn parse_tuned(
        text: &str,
        tunes: &[(&str, &str)],
    ) -> Result<Self, String> {
        let mut cpus: Vec<String> = Vec::new();
        let mut tables: Vec<CpuTable> = Vec::new();
        let mut default = None;
        let mut marches: Vec<String> = Vec::new();
        let mut section = "scalars";
        let mut docs: Vec<Option<toml::Table>> = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let at = format!("timings.times:{}", index + 1);
            if let Some(name) = line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
                section = match name {
                    "clocks" => "clocks",
                    "latency" => "latency",
                    other => return Err(format!("{at}: no section [{other}]")),
                };
                continue;
            }
            let columns: Vec<&str> = line.split_whitespace().collect();
            let (name, values) = (columns[0], &columns[1..]);
            if name == "default_cpu" {
                default = Some(values.first().ok_or_else(|| format!("{at}: default_cpu names no CPU"))?.to_string());
                continue;
            }
            if name == "cpus" {
                cpus = values.iter().map(|one| (*one).to_owned()).collect();
                tables = vec![CpuTable::default(); cpus.len()];
                docs = vec![None; cpus.len()];
                continue;
            }
            if values.len() != cpus.len() {
                return Err(format!("{at}: {} values for {} CPUs", values.len(), cpus.len()));
            }
            if name == "march" {
                marches = values.iter().map(|one| (*one).to_owned()).collect();
                continue;
            }
            for (at_cpu, (table, value)) in tables.iter_mut().zip(values).enumerate() {
                let tuned;
                let value = if *value == "=" {
                    tuned = tune_value(tunes, &mut docs, &marches, at_cpu, section, name)
                        .map_err(|e| format!("{at}: {e}"))?;
                    &tuned.as_str()
                } else {
                    value
                };
                let number = || value.parse::<i64>().map_err(|_| format!("{at}: `{value}` is no number"));
                if *value == "-" {
                    continue;
                }
                match (section, name) {
                    ("scalars", "issue") => table.issue = number()?,
                    ("scalars", "in_order") => table.in_order = number()? != 0,
                    ("scalars", "prefix") => table.prefix = number()?,
                    ("scalars", "partial_stall") => table.partial_stall = number()?,
                    ("scalars", "lcp_stall") => table.lcp_stall = number()?,
                    ("scalars", "pairing") => table.pairing = number()? != 0,
                    ("scalars", "set_ratio") => table.set_ratio = Some(number()?),
                    ("scalars", "clear_ratio") => table.clear_ratio = Some(number()?),
                    ("scalars", "set_ratio_size") => table.set_ratio_size = Some(number()?),
                    ("scalars", "clear_ratio_size") => table.clear_ratio_size = Some(number()?),
                    ("scalars", "jump_table") => table.jump_table = Some(number()?),
                    ("scalars", "cache_line") => table.cache_line = Some(number()?),
                    ("scalars", other) => return Err(format!("{at}: no scalar `{other}`")),
                    ("clocks", form) => table.clocks.push((form.to_owned(), number()?)),
                    (_, form) => table.latency.push((form.to_owned(), number()?)),
                }
            }
        }
        if cpus.is_empty() {
            return Err("timings.times: no `cpus` line".to_owned());
        }
        if let Some(name) = default.as_deref().filter(|name| !cpus.iter().any(|one| one == name)) {
            return Err(format!("timings.times: default_cpu {name} is no column"));
        }
        if !marches.is_empty() && marches.iter().enumerate().any(|(at, name)| marches[..at].contains(name)) {
            return Err("timings.times: two CPUs share a march name".to_owned());
        }
        Ok(Self { cpus, tables, default, marches })
    }

    /// The CPUs' names, in column order.
    pub fn cpus(&self) -> Vec<&str> {
        self.cpus.iter().map(String::as_str).collect()
    }

    /// The CPU a compile is priced for when none is asked: the target's
    /// `default_cpu` line.
    pub fn default_cpu(&self) -> Option<&str> {
        self.default.as_deref()
    }

    /// The names gcc's `-march` and `-mtune` take for this target's CPUs, in
    /// column order.
    pub fn marches(&self) -> Vec<&str> {
        self.marches.iter().map(String::as_str).collect()
    }

    /// The CPU `-march=name` names.
    pub fn march(
        &self,
        name: &str,
    ) -> Option<&str> {
        self.marches.iter().position(|one| one == name).map(|at| self.cpus[at].as_str())
    }

    pub fn cpu(
        &self,
        name: &str,
    ) -> Option<&CpuTable> {
        self.cpus.iter().position(|one| one == name).map(|at| &self.tables[at])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// gcc's `-march` names were a table in the flag parser, three of eight
    /// CPUs: the target states them beside its `cpus`, one a column.
    #[test]
    fn a_cpu_has_the_name_gccs_march_gives_it() {
        let timings = Timings::parse("cpus a b\nmarch x86a pent\nissue 1 2\n").unwrap();
        assert_eq!(
            (timings.march("pent"), timings.march("a"), timings.marches()),
            (Some("b"), None, vec!["x86a", "pent"])
        );
        assert!(Timings::parse("cpus a b\nmarch one\n").unwrap_err().contains("1 values for 2 CPUs"));
        assert!(Timings::parse("cpus a b\nmarch same same\n").unwrap_err().contains("share a march name"));
    }

    #[test]
    fn a_column_is_a_cpu_and_a_dash_is_no_price() {
        let text = "cpus a b\nissue 1 2\nin_order 1 0\nprefix 0 3\npartial_stall 0 0\nlcp_stall 0 5\npairing 0 1\n[clocks]\nalu 2 -\nmul 9 8\n[latency]\nalu 2 1\n";
        let timings = Timings::parse(text).unwrap();
        assert_eq!(timings.cpus(), ["a", "b"]);
        let b = timings.cpu("b").unwrap();
        assert_eq!((b.issue, b.in_order, b.prefix, b.lcp_stall, b.pairing), (2, false, 3, 5, true));
        assert_eq!(b.clocks, [("mul".to_owned(), 8)]);
        assert_eq!(timings.cpu("a").unwrap().clocks.len(), 2);
        assert!(timings.cpu("c").is_none());
        assert_eq!(timings.default_cpu(), None);
        assert_eq!(Timings::parse("cpus a b\ndefault_cpu b\n").unwrap().default_cpu(), Some("b"));
        assert_eq!(Timings::parse("cpus a\ndefault_cpu z\n").unwrap_err(), "timings.times: default_cpu z is no column");
        assert_eq!(Timings::parse("cpus a\nalu 1 2\n").unwrap_err(), "timings.times:2: 2 values for 1 CPUs");
    }

    #[test]
    fn a_cpu_states_gccs_by_pieces_ratios_or_none() {
        let timings =
            Timings::parse("cpus a b\nset_ratio 3 -\nclear_ratio 3 6\nset_ratio_size 3 3\nclear_ratio_size 2 2\n")
                .unwrap();
        let (a, b) = (timings.cpu("a").unwrap(), timings.cpu("b").unwrap());
        assert_eq!(
            (a.set_ratio, a.clear_ratio, a.set_ratio_size, a.clear_ratio_size),
            (Some(3), Some(3), Some(3), Some(2))
        );
        assert_eq!((b.set_ratio, b.clear_ratio), (None, Some(6)));
    }

    #[test]
    fn an_equals_cell_reads_the_cpus_tune_file() {
        let text = "cpus a b\nmarch ma mb\nissue = 2\n[clocks]\nalu = 4\nmul 9 =\n";
        let tunes = [("ma", "issue = 1\n[clocks]\nalu = 3\n"), ("mb", "issue = 5\n[clocks]\nmul = 7\n")];
        let timings = Timings::parse_tuned(text, &tunes).unwrap();
        let (a, b) = (timings.cpu("a").unwrap(), timings.cpu("b").unwrap());
        assert_eq!((a.issue, b.issue), (1, 2));
        assert_eq!(
            (a.clocks.clone(), b.clocks.clone()),
            (vec![("alu".into(), 3), ("mul".into(), 9)], vec![("alu".into(), 4), ("mul".into(), 7)])
        );
    }

    #[test]
    fn an_equals_cell_with_no_tune_row_is_an_error_not_a_zero() {
        let text = "cpus a\nmarch ma\n[clocks]\nalu =\n";
        assert!(Timings::parse(text).unwrap_err().contains("no tune file for ma"));
        assert!(Timings::parse_tuned(text, &[("ma", "")]).unwrap_err().contains("no `alu` in clocks"));
        assert!(Timings::parse_tuned("cpus a\n[clocks]\nalu =\n", &[]).unwrap_err().contains("before the `march`"));
    }
}
