//! A reload of the slot a store just filled, into the register that filled it,
//! is gone (`peephole.peep`, `forwarded_reloads`).

mod common;

/// PLASMA's loop: eight values live across the inner loop at m16.
const SOURCE: &str = "short unf[321], unfunf[321], sine[513];
void p(short n)
{
    short f, x;
    for (f = 1; f <= n; f++)
        for (x = 0; x <= 320; x++) {
            unf[x] = sine[(x * 11 + f * 7) & 511] + sine[(3 * x + 7 * f + 3) & 511];
            unfunf[x] = sine[(x * 4 + f * 5) & 511] + sine[(9 * x + 2 * f + 371) & 511];
        }
}
";

fn listing() -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let done = common::cc::llrm_cc()
        .current_dir(scratch.path())
        .args(["-m16", "-march=i386", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// `mov [bp-14], di` straight followed by `mov di, [bp-14]`: a load of what the
/// register still held (PLASMA, one in each inner loop).
fn reloads_after_a_store(asm: &str) -> Vec<String> {
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    lines
        .windows(2)
        .filter_map(|pair| {
            let (cell, register) = pair[0].strip_prefix("mov word ptr ")?.split_once(", ")?;
            (pair[1] == format!("mov {register}, word ptr {cell}")).then(|| pair.join(" | "))
        })
        .collect()
}

#[test]
fn a_store_is_not_followed_by_a_reload_into_the_same_register() {
    let asm = listing();
    assert!(asm.contains("[bp-"), "premise: the loop spills at m16: {asm}");
    assert_eq!(reloads_after_a_store(&asm), Vec::<String>::new(), "{asm}");
}

#[test]
fn the_check_finds_the_pair_it_names() {
    let found = reloads_after_a_store("    mov word ptr [bp-14], di\n    mov di, word ptr [bp-14]\n");
    assert_eq!(found.len(), 1);
}
