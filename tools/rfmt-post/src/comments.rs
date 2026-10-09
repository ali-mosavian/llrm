//! Comments past COMMENT_WIDTH: found, classified, and fixed where whitespace
//! and line breaks can fix them.
//!
//! rustfmt wraps the comments it lays out, but leaves a trailing comment, a
//! comment between operands or in a macro, and any line holding a URL as
//! written. Here a trailing comment moves above its line, an own-line `//`
//! comment is broken at the last space that fits, and what cannot be broken (a
//! word past the width, a code block, a table) is left and exempt from the
//! check. Words and their order are the same; only the line breaks differ.

use std::iter::Peekable;

use super::{Doc, is_comment};

pub const COMMENT_WIDTH: usize = 80;

/// What a line past COMMENT_WIDTH is. The first three cannot be broken; the
/// others are for the formatter to fix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Unbreakable, // its first word already ends past the width: a URL, a path
    CodeBlock,   // in a doc comment's fenced or indented code
    Table,       // a markdown table row, a ruler
    Trailing,    // after code on its line
    Other,
}

impl Kind {
    pub fn exempt(self) -> bool {
        matches!(self, Kind::Unbreakable | Kind::CodeBlock | Kind::Table)
    }
}

pub struct Long {
    pub line: usize, // 1-based
    pub kind: Kind,
}

/// One line of a comment token.
struct CLine {
    n: usize,   // index in the file's lines
    tok: usize, // the comment token
    trailing: bool,
    verbatim: Option<Kind>, // a line to leave as it is
}

fn is_doc(text: &str) -> bool {
    text.starts_with("///") && !text.starts_with("////")
        || text.starts_with("//!")
        || text.starts_with("/**") && !text.starts_with("/***") && text != "/**/"
        || text.starts_with("/*!")
}

/// A line comment's marker (`//`, `///`, `//!`), the spaces after it, and the
/// rest.
fn split(text: &str) -> (&str, &str, &str) {
    let marker = match text.as_bytes().get(2) {
        Some(b'/') if text.as_bytes().get(3) != Some(&b'/') => 3,
        Some(b'!') => 3,
        _ => 2,
    };
    let rest = &text[marker..];
    let body = rest.trim_start_matches(' ');
    (&text[..marker], &rest[..rest.len() - body.len()], body)
}

/// A block comment's line without its decoration.
fn block_body(line: &str) -> &str {
    let t = line.trim_start();
    let t = t.strip_prefix("/*").or_else(|| t.strip_prefix('*')).unwrap_or(t);
    t.trim_start_matches(['!', '*']).trim_start()
}

fn width(line: &str) -> usize {
    line.chars().count()
}

fn comment_lines(
    doc: &Doc,
    lines: &[&str],
) -> Vec<CLine> {
    let mut out = vec![];
    let (mut fence, mut prev) = (false, usize::MAX);
    for (i, tok) in doc.toks.iter().enumerate() {
        if !is_comment(tok.kind) {
            continue;
        }
        let documents = is_doc(&tok.text);
        if !(documents && prev != usize::MAX && prev + 1 == i) {
            fence = false;
        }
        prev = if documents { i } else { usize::MAX };
        let line_comment = tok.text.starts_with("//");
        for (k, part) in tok.text.split('\n').enumerate() {
            let n = tok.line + k;
            let (gap, body) = match line_comment {
                true => (split(part).1.len(), split(part).2),
                false => (0, block_body(if k == 0 { part } else { lines[n] })),
            };
            let toggles = documents && (body.starts_with("```") || body.starts_with("~~~"));
            let code = documents && (fence || toggles || line_comment && gap >= 5);
            if toggles {
                fence = !fence;
            }
            let verbatim = if code {
                Some(Kind::CodeBlock)
            } else if body.starts_with('|') || body.starts_with('/') || line_comment && gap >= 5 {
                Some(Kind::Table)
            } else {
                None
            };
            out.push(CLine { n, tok: i, trailing: k == 0 && doc.first_on_line[tok.line] != i, verbatim });
        }
    }
    out
}

/// Every line past COMMENT_WIDTH, with why.
pub(crate) fn scan(
    doc: &Doc,
    text: &str,
) -> Vec<Long> {
    let lines: Vec<&str> = text.split('\n').collect();
    comment_lines(doc, &lines)
        .into_iter()
        .filter(|c| width(lines[c.n]) > COMMENT_WIDTH)
        .map(|c| {
            let line = lines[c.n];
            let kind = match c.verbatim {
                _ if c.trailing => Kind::Trailing,
                Some(kind) => kind,
                None if first_word_end(line) > COMMENT_WIDTH => Kind::Unbreakable,
                None => Kind::Other,
            };
            Long { line: c.n + 1, kind }
        })
        .collect()
}

/// Where the first word after a comment marker ends.
fn first_word_end(line: &str) -> usize {
    let t = line.trim_start();
    let t = t.strip_prefix("/*").or_else(|| t.strip_prefix('*')).unwrap_or(t);
    let t = t.strip_prefix("//").unwrap_or(t);
    let t = t.strip_prefix(['/', '!', '*']).unwrap_or(t);
    let body = t.trim_start();
    width(line) - width(body) + body.split_whitespace().next().map_or(0, width)
}

/// A list item's marker at the start of `body`, with the space after it.
fn bullet(body: &str) -> usize {
    let digits = body.chars().take_while(char::is_ascii_digit).count();
    let (head, tail) = body.split_at(digits);
    match tail.chars().next() {
        Some('-' | '*' | '+') if digits == 0 && tail[1..].starts_with(' ') => 2,
        Some('.' | ')') if digits > 0 && tail[1..].starts_with(' ') => head.len() + 2,
        _ => 0,
    }
}

/// A word that begins a markdown block, which a line must not start with when
/// it only continues one.
fn starts_block(word: &str) -> bool {
    word.starts_with(['#', '>', '|', '-', '*', '+', '`', '~', '=']) || bullet(&format!("{word} ")) > 0
}

/// `line` (a line comment, its indent included) broken into lines of at most
/// COMMENT_WIDTH where a space allows.
fn wrap(line: &str) -> Vec<String> {
    let indent = line.len() - line.trim_start().len();
    let (marker, gap, body) = split(&line[indent..]);
    let prefix = format!("{}{marker}{gap}{}", &line[..indent], " ".repeat(bullet(body)));
    let mut out = vec![];
    let mut cur = line.trim_end().to_string();
    // the first word of `cur` begins here
    let mut start = indent + marker.len() + gap.len();
    while width(&cur) > COMMENT_WIDTH {
        // Spaces after a word that ends within the width; else the first one
        // past it.
        let spaces: Vec<usize> = cur
            .char_indices()
            .filter(|&(p, c)| c == ' ' && p > start && !cur[..p].ends_with(' '))
            .map(|(p, _)| p)
            .collect();
        let rest_of = |p: usize| cur[p..].trim_start_matches(' ');
        let fits =
            spaces.iter().rev().find(|&&p| width(&cur[..p]) <= COMMENT_WIDTH && !starts_block(first_word(rest_of(p))));
        let Some(&at) = fits.or_else(|| spaces.first()) else { break };
        let rest = rest_of(at);
        if rest.is_empty() {
            break;
        }
        out.push(cur[..at].to_string());
        cur = format!("{prefix}{rest}");
        start = prefix.len();
    }
    out.push(cur);
    out
}

fn first_word(text: &str) -> &str {
    text.split_whitespace().next().unwrap_or("")
}

/// `first` broken, its overflow joined to the lines `rest` offers that go on
/// with the comment (same marker and indent, not a block of their own) and
/// broken in turn, so a wrapped line leaves no short stub before the next one.
/// Also how many of `rest` were taken.
fn reflow(
    first: &str,
    rest: &mut Peekable<impl Iterator<Item = String>>,
) -> (Vec<String>, usize) {
    let head = |line: &str| {
        let (marker, _, body) = split(line.trim_start());
        (line.len() - line.trim_start().len(), marker.to_string(), body.to_string())
    };
    let (indent, marker, _) = head(first);
    let (mut out, mut taken, mut cur) = (vec![], 0, first.to_string());
    loop {
        let mut pieces = wrap(&cur);
        let tail = pieces.pop().expect("wrap returns a line");
        let broke = !pieces.is_empty();
        out.extend(pieces);
        let continues = |line: &String| {
            let (i, m, body) = head(line);
            i == indent && m == marker && !body.is_empty() && !starts_block(first_word(&body))
        };
        match rest.next_if(|line| broke && continues(line)) {
            Some(line) => {
                cur = format!("{tail} {}", head(&line).2);
                taken += 1;
            }
            None => {
                out.push(tail);
                return (out, taken);
            }
        }
    }
}

/// Which comments a pass moves.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Trailing ones, above their lines: before rustfmt's last run, which
    /// aligns the ones that stay.
    Hoist,
    /// Own-line `//` ones, broken: last, after the edits that indent lines.
    Wrap,
}

/// The text with the comments `step` names that pass COMMENT_WIDTH fixed, and
/// how many.
pub fn fix(
    text: &str,
    step: Step,
) -> Result<(String, usize), String> {
    let doc = Doc::parse(text)?;
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<Option<Vec<String>>> = vec![None; lines.len()];
    let mut count = 0;
    let clines = comment_lines(&doc, &lines);
    let mut by_line: Vec<Option<&CLine>> = vec![None; lines.len()];
    clines.iter().for_each(|c| by_line[c.n] = Some(c));
    // The own-line `//` comment on line `n` that no edit has taken yet.
    let own_line = |n: usize, out: &[Option<Vec<String>>]| {
        by_line
            .get(n)
            .copied()
            .flatten()
            .filter(
                |d| !d.trailing && d.verbatim.is_none() && doc.toks[d.tok].text.starts_with("//") && out[n].is_none(),
            )
    };
    for c in &clines {
        let tok = &doc.toks[c.tok];
        let line = lines[c.n];
        if out[c.n].is_some() || width(line) <= COMMENT_WIDTH || c.verbatim.is_some() || !tok.text.starts_with("//") {
            continue;
        }
        if c.trailing != (step == Step::Hoist) {
            continue;
        }
        if !c.trailing {
            if doc.indent(c.tok).is_none() {
                continue;
            }
            let below = (c.n + 1..).map_while(|n| own_line(n, &out).map(|_| lines[n].to_string()));
            let (broken, taken) = reflow(line, &mut below.peekable());
            (c.n + 1..=c.n + taken).for_each(|n| out[n] = Some(vec![]));
            out[c.n] = Some(broken);
            count += 1;
            continue;
        }
        // After code: above the line it ends.
        let Some(indent) = doc.indent(doc.first_on_line[tok.line]) else { continue };
        let col = tok.start - doc.line_starts[tok.line];
        let mut moved = wrap(&format!("{}{}", " ".repeat(indent), &line[col..]));
        moved.push(line[..col].trim_end().to_string());
        out[c.n] = Some(moved);
        count += 1;
    }
    let mut text = String::with_capacity(text.len());
    for (k, line) in lines.iter().enumerate() {
        match &out[k] {
            Some(replaced) => replaced.iter().for_each(|l| text += &format!("{l}\n")),
            None => text += &format!("{line}\n"),
        }
    }
    text.pop(); // `split('\n')` made the file's last line empty
    Ok((text, count))
}

/// Each comment word with the kind of comment it is in, in order; where lines
/// break and where the comment stands among the code do not matter.
pub(crate) fn words(doc: &Doc) -> Vec<(&str, &str)> {
    let mut out = vec![];
    for tok in doc.toks.iter().filter(|t| is_comment(t.kind)) {
        if tok.text.starts_with("//") {
            let (marker, _, body) = split(&tok.text);
            out.extend(body.split_whitespace().map(|word| (marker, word)));
        } else {
            out.extend(tok.text.split_whitespace().map(|word| ("/*", word)));
        }
    }
    out
}
