//! A published timing table, read from its transcription (CSV): cells as the
//! manual prints them, `12(2/0)+`, `6+2n(1/0)`, `12+4n(3+n/0)`.
//!
//! # How it works
//! Row 1 of a file is `#title,..,page,..,notes,..`, row 2 the header. A row's
//! first column (instruction, or the source mode of a move table) carries down
//! over the rows that leave it empty. [`Table::get`] finds a cell by row and
//! column label, both compared with spaces, `*` and the dash variants removed.

use std::collections::HashMap;

/// A cell's clocks: `base`, `per` more for each of `n` (a shift count, a
/// register count), and whether the effective address time is to be added.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cell {
    pub base: u32,
    pub per: u32,
    pub plus_ea: bool,
    /// `*`: the manual marks the number a maximum.
    pub maximum: bool,
}

#[derive(Debug)]
pub struct Table {
    /// The position of each column by [`key`] of its label; a label with a
    /// stray `+` (a footnote mark on `(d8,An,Xn)`) is also the one without.
    columns: HashMap<String, usize>,
    /// `(second column as a key, cells)` of each row.
    rows: Vec<(String, Vec<String>)>,
    /// The rows that list an instruction, in table order, by its [`key`]:
    /// `ASR, ASL` and `ADD/ADDA` list instructions, `(d16, An)` is one mode.
    by_name: HashMap<String, Vec<usize>>,
}

pub(super) fn split_csv(line: &str) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                fields.last_mut().unwrap().push('"');
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(String::new()),
            c => fields.last_mut().unwrap().push(c),
        }
    }
    fields
}

thread_local! {
    static KEYS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many labels [`key`] normalized on this thread: a lookup costs a few,
/// not one per row of the table.
pub fn keys_made() -> usize {
    KEYS.with(std::cell::Cell::get)
}

/// A label without what typesetting varies: spaces, `*`, the dash kinds.
pub fn key(label: &str) -> String {
    KEYS.with(|count| count.set(count.get() + 1));
    label
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(*c, '*' | '\u{2020}'))
        .map(|c| if matches!(c, '\u{2013}' | '\u{2212}' | '\u{2014}') { '-' } else { c.to_ascii_lowercase() })
        .collect()
}

impl Table {
    /// `two_labels`: the first two columns are labels (instruction and size),
    /// else one (the source mode).
    pub fn parse(
        text: &str,
        two_labels: bool,
    ) -> Table {
        let mut lines = text.lines();
        lines.next().expect("a title row");
        let header = split_csv(lines.next().expect("a header row"));
        let skip = if two_labels { 2 } else { 1 };
        let mut rows = Vec::new();
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        let mut carried = String::new();
        for line in lines.filter(|line| !line.is_empty()) {
            let fields = split_csv(line);
            if !fields[0].is_empty() {
                carried = fields[0].clone();
            }
            let second = if two_labels { key(&fields[1]) } else { String::new() };
            let names: Vec<&str> =
                if carried.contains('(') { vec![&carried] } else { carried.split(['/', ',']).collect() };
            for name in names {
                let listed = by_name.entry(key(name)).or_default();
                if listed.last() != Some(&rows.len()) {
                    listed.push(rows.len());
                }
            }
            rows.push((second, fields[skip..].to_vec()));
        }
        let mut columns = HashMap::new();
        for (at, name) in header[skip..].iter().enumerate() {
            let name = key(name);
            columns.entry(name.strip_suffix('+').unwrap_or(&name).to_owned()).or_insert(at);
            columns.entry(name).or_insert(at);
        }
        Table { columns, rows, by_name }
    }

    /// The text of the cell at `column` of the row whose label is `row`, and
    /// whose second label (the size) `size_ok` accepts.
    pub fn text(
        &self,
        row: &str,
        size_ok: impl Fn(&str) -> bool,
        column: &str,
    ) -> Option<&str> {
        self.text_nth(row, 0, size_ok, column)
    }

    /// [`Table::text`] of the `nth` row that matches (MULS/MULU prints two).
    pub fn text_nth(
        &self,
        row: &str,
        nth: usize,
        size_ok: impl Fn(&str) -> bool,
        column: &str,
    ) -> Option<&str> {
        let index = *self.columns.get(&key(column))?;
        let found = self
            .by_name
            .get(&key(row))?
            .iter()
            .map(|&at| &self.rows[at])
            .filter(|(second, _)| size_ok(second))
            .nth(nth)?;
        found
            .1
            .get(index)
            .map(String::as_str)
            .filter(|text| !text.is_empty() && key(text) != "-" && *text != "\u{2014}")
    }

    pub fn get(
        &self,
        row: &str,
        size_ok: impl Fn(&str) -> bool,
        column: &str,
    ) -> Option<Cell> {
        parse_cell(self.text(row, size_ok, column)?)
    }
}

/// `12(2/0)+`, `6+2n(1/0)`, `12+4n(3+n/0)`, `0(0/0)`: the clocks before the bus
/// cycle counts.
pub fn parse_cell(text: &str) -> Option<Cell> {
    let head = text.split('(').next()?.trim();
    let (head, maximum) = (head.trim_end_matches('*'), text.contains(")*") || text.ends_with('*'));
    let plus_ea = text.trim_end_matches('*').trim_end().ends_with('+') && text.contains(')');
    let mut parts = head.trim_end_matches('+').split('+');
    let base: u32 = parts.next()?.trim().parse().ok()?;
    let per = match parts.next() {
        None => 0,
        Some(term) => {
            let term = term.trim().strip_suffix('n')?;
            if term.is_empty() { 1 } else { term.parse().ok()? }
        }
    };
    Some(Cell { base, per, plus_ea, maximum })
}
