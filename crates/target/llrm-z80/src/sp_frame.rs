//! A frame addressed from SP, not from IX.
//!
//! # How it works
//! The IX frame costs `push ix; ld ix,#0; add ix,sp` (8 B, 44 T) and `ld sp,ix; pop ix` on every
//! return, whatever the function touches in it. Without it the frame objects and stack arguments
//! lie where SP puts them: `ld iy,#k; add iy,sp` (6 B, 29 T) sets a base for a run of accesses
//! with no call (IY is call-clobbered) and no label in between, and `pop iy` (or one `add iy,sp`)
//! undoes the frame. The pushes since entry are tracked by name, so a label reached with two
//! different depths keeps the IX frame. Carry is saved around `add iy,sp` where a later
//! instruction may read it.
//!
//! The form stays only where the function is no larger and no slower, loops weighted, than with
//! IX (`price_emitted`). A label reached through a jump table must lie at the depth of the dispatch.
//! The caller pops stack arguments (`cleanup = "caller"`), so a call leaves the depth as it found it; the epilogue
//! is followed by `ret` or a tail `jp`, neither of which reads flags, so `add iy,sp` there needs no carry guard.
//!
//! DEVIATE: a pass over the printed lines, not a frame lowering that knows its own form. Pin
//! counterpart: `X86FrameLowering::hasFP` and `eliminateFrameIndex` choose between a frame
//! pointer and SP-relative slots; the z80 has no SP-relative addressing, so the base is rebuilt.
//! Exit 2026-11-24 (owner z80): the choice made in `Frame`, before the lines are printed.

use crate::expand::{Line, price_emitted};

const PROLOGUE: [&str; 3] = ["push ix", "ld ix, #0", "add ix, sp"];

/// The `d` of every `d(ix)` in `text`, with the byte range of the whole operand.
fn displacements(text: &str) -> Vec<(usize, usize, i64)> {
    let mut found = Vec::new();
    for (close, _) in text.match_indices("(ix)") {
        let bytes = text.as_bytes();
        let mut start = close;
        while start > 0 && bytes[start - 1].is_ascii_digit() {
            start -= 1;
        }
        if start > 0 && bytes[start - 1] == b'-' {
            start -= 1;
        }
        if let Ok(d) = text[start..close].parse::<i64>() {
            found.push((start, close + "(ix)".len(), d));
        }
    }
    found
}

fn mnemonic(text: &str) -> &str {
    text.split([' ', ',']).next().unwrap_or("")
}

/// The condition of a jump, call or return, if it has one.
fn condition(text: &str) -> Option<&str> {
    let rest = text.split_once(' ')?.1;
    match mnemonic(text) {
        "jp" | "jr" | "call" => rest.split_once(", ").map(|(cc, _)| cc),
        "ret" => Some(rest),
        _ => None,
    }
}

/// Whether carry, which `add iy,sp` changes, may be read after this point.
fn carry_live(rest: &[Line]) -> bool {
    for line in rest {
        let text = match line {
            Line::Label(_) => return true,
            Line::Loc(_) => continue,
            Line::Inst(text) if text.starts_with('.') => continue,
            Line::Inst(text) => text.as_str(),
        };
        let op = mnemonic(text);
        let carry_condition = matches!(condition(text), Some("c" | "nc"));
        if carry_condition
            || matches!(op, "adc" | "sbc" | "rla" | "rra" | "rl" | "rr" | "daa" | "ccf")
            || text == "push af"
        {
            return true;
        }
        let sets_carry = matches!(
            op,
            "cp" | "sub" | "and" | "or" | "xor" | "adc" | "sbc" | "neg" | "scf" | "rl" | "rr" | "rlc" | "rrc" | "sla" | "sra"
                | "srl" | "rla" | "rra" | "rlca" | "rrca"
        ) || text.starts_with("add a")
            || text.starts_with("add hl");
        if sets_carry {
            return false;
        }
        if matches!(op, "jp" | "jr" | "ret" | "call") && !text.contains(',') && condition(text).is_none() {
            return false;
        }
    }
    false
}

/// The bytes of stack the function has pushed since its prologue at each point, rewritten into
/// SP-relative accesses; `None` where the IX frame has to stay.
fn rewritten(lines: &[Line]) -> Option<Vec<Line>> {
    let texts: Vec<Option<&str>> = lines
        .iter()
        .map(|line| match line {
            Line::Inst(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    // IY may be used only by the big-frame prologue `ld iy,#-n; add iy,sp; ld sp,iy`
    for text in texts.iter().flatten() {
        if text.contains("iy") && !(text.starts_with("ld iy, #-") || *text == "add iy, sp" || *text == "ld sp, iy") {
            return None;
        }
    }
    let prologue = texts.iter().position(|text| *text == Some(PROLOGUE[0]))?;
    if (0..3).any(|k| texts.get(prologue + k).copied().flatten() != Some(PROLOGUE[k])) {
        return None;
    }
    // IX appears only in `d(ix)` operands and the prologue and epilogue
    for text in texts.iter().flatten() {
        let mut stripped = text.to_string();
        for (start, end, _) in displacements(text).into_iter().rev() {
            stripped.replace_range(start..end, "");
        }
        if stripped.contains("ix") && !PROLOGUE.contains(text) && *text != "ld sp, ix" && *text != "pop ix" {
            return None;
        }
    }
    let mut out: Vec<Line> = Vec::with_capacity(lines.len() + 8);
    // bytes pushed since entry; a local is at SP + pushed + d, an argument at SP + pushed + d - 2
    let mut pushed: i64 = 0;
    // the displacement the live IY stands for, if any
    let mut base: Option<i64> = None;
    let mut label_depth: std::collections::HashMap<&str, i64> = std::collections::HashMap::new();
    // the depth at each `jp (rr)` and the labels of the `.dw` entries it may reach
    let mut dispatches: Vec<i64> = Vec::new();
    let mut entries: Vec<&str> = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        let line = &lines[at];
        let Some(text) = texts[at] else {
            if let Line::Label(name) = line {
                if label_depth.get(name.as_str()).is_some_and(|depth| *depth != pushed) {
                    return None;
                }
                label_depth.insert(name.as_str(), pushed);
                base = None;
            }
            out.push(line.clone());
            at += 1;
            continue;
        };
        if (prologue..prologue + 3).contains(&at) {
            at += 1;
            continue;
        }
        // a jump records the depth its target must have
        if matches!(mnemonic(text), "jp" | "jr" | "djnz") {
            let target = text.rsplit([' ', ',']).next().unwrap_or("");
            if target.starts_with('(') {
                // a jump table's dispatch: its entries are checked at the end
                dispatches.push(pushed);
            } else {
                if label_depth.get(target).is_some_and(|depth| *depth != pushed) {
                    return None;
                }
                label_depth.entry(target).or_insert(pushed);
            }
        }
        if let Some(target) = text.strip_prefix(".dw ") {
            entries.push(target);
        }
        if text == "ld sp, ix" && texts.get(at + 1).copied().flatten() == Some("pop ix") {
            if pushed < 0 || pushed % 2 != 0 {
                return None;
            }
            if pushed <= 6 {
                out.extend(std::iter::repeat_n(Line::Inst("pop iy".to_owned()), (pushed / 2) as usize));
            } else {
                out.push(Line::Inst(format!("ld iy, #{pushed}")));
                out.push(Line::Inst("add iy, sp".to_owned()));
                out.push(Line::Inst("ld sp, iy".to_owned()));
            }
            at += 2;
            continue;
        }
        if text == "ld sp, ix" || text == "pop ix" {
            return None;
        }
        match text {
            "push af" | "push bc" | "push de" | "push hl" => pushed += 2,
            "pop af" | "pop bc" | "pop de" | "pop hl" => pushed -= 2,
            "ld sp, iy" if at >= 2 && texts[at - 2].is_some_and(|prior| prior.starts_with("ld iy, #-")) => {
                pushed += -texts[at - 2].and_then(|prior| prior.split('#').nth(1)).and_then(|n| n.parse::<i64>().ok())?;
            }
            _ if text.starts_with("ex (sp)") => {}
            _ if text.starts_with("ld sp,")
                || text.starts_with("inc sp")
                || text.starts_with("dec sp")
                || text.starts_with("pop ix")
                || text.starts_with("pop iy")
                || text.starts_with("push ix")
                || text.starts_with("push iy") =>
            {
                if text != "ld sp, iy" {
                    return None;
                }
            }
            _ => {}
        }
        if text.starts_with("call") {
            base = None;
        }
        let accesses = displacements(text);
        let mut text = text.to_owned();
        if !accesses.is_empty() {
            // the saved IX and the return address are not here
            if accesses.iter().any(|(_, _, d)| (0..4).contains(d)) {
                return None;
            }
            let relative = |d: i64| if d < 0 { d } else { d - 2 };
            let first = relative(accesses[0].2);
            if base.is_none_or(|b| !(-128..=127).contains(&(first - b))) {
                base = Some(first);
                let keep = carry_live(&lines[at + 1..]) || matches!(mnemonic(&text), "adc" | "sbc");
                if keep {
                    out.push(Line::Inst("push af".to_owned()));
                    out.push(Line::Inst(format!("ld iy, #{}", pushed + 2 + first)));
                    out.push(Line::Inst("add iy, sp".to_owned()));
                    out.push(Line::Inst("pop af".to_owned()));
                } else {
                    out.push(Line::Inst(format!("ld iy, #{}", pushed + first)));
                    out.push(Line::Inst("add iy, sp".to_owned()));
                }
            }
            let live = base?;
            for (start, end, d) in accesses.into_iter().rev() {
                let offset = relative(d) - live;
                if !(-128..=127).contains(&offset) {
                    return None;
                }
                text.replace_range(start..end, &format!("{offset}(iy)"));
            }
        }
        // an address built from SP: SP is 2 higher without the saved IX
        if let Some(pair) = text.strip_prefix("add ").and_then(|rest| rest.strip_suffix(", sp")) {
            if matches!(pair, "hl" | "de" | "bc" | "iy") && texts.get(at + 1).copied().flatten() != Some("ld sp, iy") {
                let Some(Line::Inst(previous)) = out.last_mut() else { return None };
                let k: i64 = previous.strip_prefix(&format!("ld {pair}, #"))?.parse().ok()?;
                let r = k - pushed;
                if (0..4).contains(&r) {
                    return None;
                }
                if r >= 4 {
                    *previous = format!("ld {pair}, #{}", k - 2);
                }
            }
        }
        out.push(Line::Inst(text));
        at += 1;
    }
    // a label reached through a table is entered at the depth of the dispatch
    for entry in entries {
        if !label_depth.get(entry).is_some_and(|depth| dispatches.iter().all(|dispatch| dispatch == depth)) {
            return None;
        }
    }
    Some(out)
}

/// The function with its frame on SP where that is no larger and no faster to run than IX's.
pub fn without_ix(lines: Vec<Line>, speed: bool) -> Vec<Line> {
    let Some(candidate) = rewritten(&lines) else { return lines };
    let (bytes, clocks) = price_emitted(&lines, speed);
    let (new_bytes, new_clocks) = price_emitted(&candidate, speed);
    if new_bytes <= bytes && new_clocks <= clocks { candidate } else { lines }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inst(text: &str) -> Line {
        Line::Inst(text.to_owned())
    }

    fn texts(lines: &[&str]) -> Vec<Line> {
        lines.iter().map(|text| inst(text)).collect()
    }

    #[test]
    fn a_frame_touched_once_is_addressed_from_sp() {
        let lines = texts(&[
            "push ix", "ld ix, #0", "add ix, sp", "push af", "push af", "ld -2(ix), l", "ld -1(ix), h", "ld l, -2(ix)",
            "ld h, -1(ix)", "ld sp, ix", "pop ix", "ret",
        ]);
        let out = rewritten(&lines).expect("rewritten");
        assert_eq!(
            out,
            texts(&["push af", "push af", "ld iy, #2", "add iy, sp", "ld 0(iy), l", "ld 1(iy), h", "ld l, 0(iy)", "ld h, 1(iy)", "pop iy", "pop iy", "ret"])
        );
    }

    #[test]
    fn the_saved_ix_and_the_return_address_keep_the_ix_frame() {
        let lines = texts(&["push ix", "ld ix, #0", "add ix, sp", "ld c, 2(ix)", "ld sp, ix", "pop ix", "ret"]);
        assert!(rewritten(&lines).is_none());
    }

    /// A case block that pushes a call's arguments and returns through `ld sp, ix` leaves the depth high; the next
    /// label, reached only through the table, is entered at the dispatch's depth.
    #[test]
    fn a_label_reached_through_a_table_at_another_depth_keeps_the_ix_frame() {
        let lines = vec![
            inst("push ix"),
            inst("ld ix, #0"),
            inst("add ix, sp"),
            inst("jp (hl)"),
            Line::Label("A".to_owned()),
            inst("push hl"),
            inst("call g"),
            inst("ld sp, ix"),
            inst("pop ix"),
            inst("ret"),
            Line::Label("B".to_owned()),
            inst("ld sp, ix"),
            inst("pop ix"),
            inst("ret"),
            Line::Label("JT".to_owned()),
            inst(".dw A"),
            inst(".dw B"),
        ];
        assert!(rewritten(&lines).is_none());
    }

    #[test]
    fn two_depths_at_one_label_keep_the_ix_frame() {
        let lines = vec![
            inst("push ix"),
            inst("ld ix, #0"),
            inst("add ix, sp"),
            inst("jr L1"),
            inst("push bc"),
            Line::Label("L1".to_owned()),
            inst("ld sp, ix"),
            inst("pop ix"),
            inst("ret"),
        ];
        assert!(rewritten(&lines).is_none());
    }
}
