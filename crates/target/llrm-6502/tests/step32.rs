//! `x + 1` and `x - 1` on a long step the low byte and carry up only when it
//! wraps (llvm-mos `G_INC`/`G_DEC` of `MOSLegalizerInfo::legalizeAddSub`). The
//! carry chain (`clc; adc #1; adc #0` over four bytes) was 31 clocks
//! for a +1 that needs 5 on 255 of every 256 calls; licm_graph_callee_footprint
//! ran 408,681 clocks against llvm-mos' 297,188.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn function(body: &str) -> String {
    let ir = format!("target datalayout = \"{}\"\ndefine i32 @f(i32 %a) {{\n{body}\n}}\n", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    let start = text.find("\n__r_f:").unwrap_or_else(|| panic!("f is not emitted:\n{text}"));
    let end = text[start..].find("\n.segment").map_or(text.len(), |at| start + at);
    text[start..end].to_owned()
}

fn carries(text: &str) -> usize {
    text.lines().filter(|line| line.trim().starts_with("adc ") || line.trim().starts_with("sbc ")).count()
}

#[test]
fn a_long_plus_one_steps_without_a_carry_chain() {
    let text = function("%r = add i32 %a, 1\nret i32 %r");
    assert_eq!(carries(&text), 0, "{text}");
    assert!(text.contains("bne"), "{text}");
}

#[test]
fn a_long_minus_one_steps_without_a_carry_chain() {
    for body in ["%r = add i32 %a, -1\nret i32 %r", "%r = sub i32 %a, 1\nret i32 %r"] {
        let text = function(body);
        assert_eq!(carries(&text), 0, "{text}");
        assert!(text.contains("bne"), "{text}");
    }
}

/// `report(g_i + 1)` for twelve volatile longs.
fn many_steps(level: Level) -> String {
    let mut ir = format!("target datalayout = \"{}\"\ndeclare void @report(i32)\n", datalayout());
    for i in 0..12 {
        ir += &format!("@g{i} = global i32 {}\n", i * 0x10001);
    }
    ir += "define void @f() {\n";
    for i in 0..12 {
        ir += &format!(
            "  %v{i} = load volatile i32, ptr @g{i}\n  %w{i} = add i32 %v{i}, 1\n  call void @report(i32 %w{i})\n"
        );
    }
    ir += "  ret void\n}\n";
    assembly(&ir, &Request::new(level)).unwrap()
}

/// div_constants_long -O1 grew from 13,520 B to 14,464 B when each of its
/// hundreds of `x + 1` became a stepping chain: the outliner shares the carry
/// chain (a `jsr` a site) and cannot share a chain with branches. A module
/// where the stepping form is more bytes keeps the carry chain.
#[test]
fn many_long_steps_stay_a_shared_carry_chain() {
    let text = many_steps(Level::O2);
    // the stepping chain's branch labels (`_pa`) are the mark of the stepped
    // form
    assert!(!text.contains("_pa"), "{text}");
}
