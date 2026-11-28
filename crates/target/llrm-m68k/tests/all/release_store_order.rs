//! A release store was made a volatile store before selection; the chain
//! combiner may move a plain store past a volatile one, so at O1 and up the
//! data store (folded into a move from the argument slot) landed after the
//! flag's: a reader that saw the flag read stale data. The pin selects the
//! atomic store node, whose ordered memory operand keeps it in place.

use std::process::Command;

use llrm_m68k::driver::{Level, assembly, cpu_of, object};

const LEVELS: [Level; 5] = [Level::O0, Level::O1, Level::O2, Level::O3, Level::Os];

/// Where `data` and `flag` are first named in the body of `publish`, which
/// starts at the line `label` ends.
fn order(
    text: &str,
    label: &str,
) -> (Option<usize>, Option<usize>) {
    let body: Vec<&str> =
        text.lines().skip_while(|line| !line.ends_with(label)).take_while(|line| !line.contains("rts")).collect();
    let at = |name: &str| body.iter().position(|line| line.contains(name));
    (at("data"), at("flag"))
}

#[test]
fn the_data_store_stays_before_a_release_store_of_the_flag() {
    let source = include_str!("../data/release_store_after_data.ll");
    for cpu in ["68000", "68020"] {
        for level in LEVELS {
            let text = assembly(source, cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{cpu}: {why}"));
            let (data, flag) = order(&text, "publish:");
            assert!(data.is_some() && data < flag, "{cpu} {level:?}: the flag is stored first:\n{text}");
            let bytes = object(source, cpu_of(cpu).unwrap(), level, "release.ll").expect("an object");
            let path = std::env::temp_dir().join(format!("release_store_order_{}_{cpu}.o", std::process::id()));
            std::fs::write(&path, bytes).unwrap();
            let listed = Command::new("m68k-linux-gnu-objdump").arg("-dr").arg(&path).output();
            std::fs::remove_file(&path).ok();
            let listed = listed.expect("m68k-linux-gnu-objdump runs");
            let text = String::from_utf8_lossy(&listed.stdout);
            let (data, flag) = order(&text, "<publish>:");
            assert!(data.is_some() && data < flag, "{cpu} {level:?}: the object stores the flag first:\n{text}");
        }
    }
}

/// The same through clang's generic `__atomic_store(4, &flag, &one,
/// __ATOMIC_RELEASE)`: it was inlined as a volatile store whatever its order.
#[test]
fn the_data_store_stays_before_a_generic_release_store_of_the_flag() {
    let source = include_str!("../data/generic_release_store_after_data.ll");
    for cpu in ["68000", "68020"] {
        for level in LEVELS {
            let text = assembly(source, cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{cpu}: {why}"));
            assert!(!text.contains("__atomic"), "{cpu} {level:?}: the store stayed a call:\n{text}");
            let (data, flag) = order(&text, "publish:");
            assert!(data.is_some() && data < flag, "{cpu} {level:?}: the flag is stored first:\n{text}");
        }
    }
}
