//! `-g -S` on m68k: the `.loc` rows the pin writes for a function
//! (`g-for-all-targets-card.md`, step 2A). The instructions had no source
//! place, so a function compiled with `-g` had no rows at all.

use std::process::Command;

fn assembly(debug: bool) -> String {
    assembly_of("lines.ll", debug)
}

fn assembly_of(
    fixture: &str,
    debug: bool,
) -> String {
    let directory = tempfile::tempdir().unwrap();
    let out = directory.path().join("lines.s");
    let mut command = Command::new(env!("CARGO_BIN_EXE_llrm-m68k-llc"));
    command.args(["-S", "-O2"]);
    if debug {
        command.arg("-g");
    }
    let said =
        command.arg("-o").arg(&out).arg(format!("{}/tests/g/{fixture}", env!("CARGO_MANIFEST_DIR"))).output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    std::fs::read_to_string(out).unwrap()
}

/// `(line, column)` of each `.loc` row, in order.
fn rows(text: &str) -> Vec<(u32, u32)> {
    text.lines()
        .filter_map(|line| {
            let words: Vec<&str> = line.trim().strip_prefix(".loc")?.split_whitespace().collect();
            Some((words.get(1)?.parse().ok()?, words.get(2)?.parse().ok()?))
        })
        .collect()
}

/// The pin writes rows for lines 3 (column 15) and 4 (columns 14 and 5) of
/// the fixture; every line with code has one, and none is line 0.
#[test]
fn a_function_compiled_with_g_has_a_row_for_each_source_line_with_code() {
    let found = rows(&assembly(true));
    assert!(found.contains(&(3, 15)), "{found:?}");
    assert!(found.contains(&(4, 14)), "{found:?}");
    assert!(found.iter().all(|(line, _)| *line != 0), "{found:?}");
}

/// Without `-g` the listing is what it was: no directive, and the same
/// instructions with `-g` on once the directives are taken out.
#[test]
fn g_adds_directives_and_leaves_the_instructions_alone() {
    let off = assembly(false);
    assert!(rows(&off).is_empty(), "{off}");
    let on = assembly(true);
    let kept: Vec<&str> = on.lines().filter(|line| !line.trim_start().starts_with(".loc")).collect();
    assert_eq!(kept, off.lines().collect::<Vec<_>>());
}

/// The directive of each `.loc` row as `(line, column, flags)`.
fn flagged(text: &str) -> Vec<(u32, u32, String)> {
    text.lines()
        .filter_map(|line| {
            let words: Vec<&str> = line.trim().strip_prefix(".loc")?.split(';').next()?.split_whitespace().collect();
            Some((words.get(1)?.parse().ok()?, words.get(2)?.parse().ok()?, words.get(3..)?.join(" ")))
        })
        .collect()
}

/// The pin's rows for the same function (`gcard-m68k/m_pin.s`): the line the
/// function is declared at ahead of the first instruction, `prologue_end` on
/// the first row after the frame is made, and the epilogue's row with the
/// return's place, `epilogue_begin`, and `is_stmt 0` because it stays on line
/// 4. Ours had only the three rows between, with no flag.
#[test]
fn the_rows_have_the_pins_scope_row_and_flags() {
    assert_eq!(
        flagged(&assembly(true)),
        [
            (2, 0, String::new()),
            (3, 15, "prologue_end".to_owned()),
            (4, 14, String::new()),
            (4, 5, "epilogue_begin is_stmt 0".to_owned()),
        ]
    );
}

/// A block's first instruction stands under a row of its own, as the pin's does
/// (a located one, or line 0 for one with no place): the argument move at the
/// top of a switch's block was listed under the row of the block before it,
/// so a debugger stopped there showed the wrong line.
#[test]
fn the_first_instruction_of_a_block_has_a_row_of_its_own() {
    let text = assembly_of("switch.ll", true);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let mut checked = 0;
    for (at, line) in lines.iter().enumerate() {
        if !line.starts_with(".LBB") {
            continue;
        }
        let next = lines[at + 1..].iter().find(|one| !one.starts_with(".LBB"));
        assert!(next.is_some_and(|one| one.starts_with(".loc")), "{line} is followed by {next:?} in\n{text}");
        checked += 1;
    }
    assert!(checked >= 2, "the fixture has blocks to check: {text}");
}
