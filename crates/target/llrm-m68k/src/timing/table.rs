//! A published timing table, read from its transcription (CSV): cells as the
//! manual prints them, `12(2/0)+`, `6+2n(1/0)`, `12+4n(3+n/0)`.
//!
//! # How it works
//! Row 1 of a file is `#title,..,page,..,notes,..`, row 2 the header. A row's
//! first column (instruction, or the source mode of a move table) carries down
//! over the rows that leave it empty. [`Table::get`] finds a cell by row and
//! column label, both compared with spaces, `*` and the dash variants removed.

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
    columns: Vec<String>,
    /// `(first column carried down, second column, cells)`.
    rows: Vec<(String, String, Vec<String>)>,
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

/// A label without what typesetting varies: spaces, `*`, the dash kinds.
pub fn key(label: &str) -> String {
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
        let mut rows: Vec<(String, String, Vec<String>)> = Vec::new();
        let mut carried = String::new();
        for line in lines.filter(|line| !line.is_empty()) {
            let fields = split_csv(line);
            if !fields[0].is_empty() {
                carried = fields[0].clone();
            }
            let second = if two_labels { fields[1].clone() } else { String::new() };
            rows.push((carried.clone(), second, fields[skip..].to_vec()));
        }
        Table { columns: header[skip..].to_vec(), rows }
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
        // a column the manual marks with a stray `+` (a footnote mark on
        // `(d8,An,Xn)`) is the same column
        let column = key(column);
        let index = self.columns.iter().position(|name| key(name) == column || key(name) == format!("{column}+"))?;
        let row = key(row);
        let found = self
            .rows
            .iter()
            .filter(|(first, second, _)| {
                // `ASR, ASL` and `ADD/ADDA` list instructions; `(d16, An)` is
                // one mode
                let names: Vec<&str> =
                    if first.contains('(') { vec![first] } else { first.split(['/', ',']).collect() };
                names.iter().any(|one| key(one) == row) && size_ok(&key(second))
            })
            .nth(nth)?;
        found
            .2
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
