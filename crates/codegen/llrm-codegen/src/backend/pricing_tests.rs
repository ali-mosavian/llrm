//! The cost model's classes, priced by every CPU profile the backend schedules
//! for.

use std::collections::BTreeSet;

use llrm_lir::registers::RegId;
use llrm_x86_m16::cycles::*;

use crate::backend::{cpu, schedule};
use crate::model::{ir, lir};

#[test]
fn test_memory_pop_is_not_priced_as_a_register_pop() {
    assert_eq!(classify("pop", "dword [bp-4]", "668f46fc"), "pop_m");
    assert_eq!(cpu::profile("386").unwrap().cost("pop_m").unwrap(), 5);
    assert!(cpu::names().iter().all(|name| cpu::profile(*name).unwrap().prices("pop_m")));
}

#[test]
fn test_memory_compare_is_priced_as_a_read_not_a_read_modify_write() {
    assert_eq!(classify("cmp", "word [bp-4],0", "837efc00"), "alu_rm");
    assert_eq!(classify("test", "word [bp-4],1", "f746fc0100"), "alu_rm");
    assert_eq!(classify("add", "word [bp-4],1", "8346fc01"), "alu_mr");
}

#[test]
fn test_scalar_double_shifts_use_the_integer_shift_price() {
    for mnemonic in ["shld", "shrd"] {
        assert_eq!(classify(mnemonic, "edx,eax,16", ""), "shift_ri");
        let reg = |register| ir::Loc::Reg(ir::Reg { register, width: 4 });
        let what = ir::Semantics {
            name: Some(mnemonic.into()),
            dests: vec![reg(RegId::EDX)],
            sources: vec![
                reg(RegId::EDX),
                reg(RegId::EAX),
                ir::Loc::Imm(ir::Imm { value: 16, width: 1, address: None }),
            ],
            ..ir::Semantics::new(ir::Operation::Funnel)
        };
        let one = lir::Insn::new(0, Some((0, 0)), Some(what), vec![], vec![]);
        assert_eq!(schedule::_form(&one), "shift_ri");
        assert_eq!(schedule::_pair_class(16, &one), "np");
    }
}

/// The D1 shift by one was priced as the two-clock imm8 form; it takes
/// three on the 486, and `add r,r` one.
#[test]
fn test_a_shift_by_one_is_priced_as_the_three_clock_form() {
    assert_eq!(classify("shl", "ax,1", "d1e0"), "shift_r1");
    assert_eq!(classify("shl", "ax,2", "c1e002"), "shift_ri");
    assert_eq!(classify("shl", "word [bp-4],1", "d166fc"), "shift_ri");
    let reg = || ir::Loc::Reg(ir::Reg { register: RegId::AX, width: 2 });
    let what = ir::Semantics {
        name: Some("shl".into()),
        dests: vec![reg()],
        sources: vec![reg(), ir::Loc::Imm(ir::Imm { value: 1, width: 1, address: None })],
        ..ir::Semantics::new(ir::Operation::Binary)
    };
    let one = lir::Insn::new(0, Some((0, 0)), Some(what), vec![], vec![]);
    assert_eq!(schedule::_form(&one), "shift_r1");
    assert_eq!(cpu::profile("386").unwrap().cost("shift_r1").unwrap(), 3);
}

#[test]
fn test_complete_far_pointer_loads_share_one_priced_form() {
    for mnemonic in ["les", "lfs", "lgs"] {
        assert_eq!(classify(mnemonic, "bx,[bp+6]", ""), "les");
        assert!(cpu::names().iter().all(|name| cpu::profile(*name).unwrap().prices("les")));
    }
}

#[test]
fn test_x87_exchange_is_explicitly_priced_for_every_cpu() {
    let expected = [("386", 18), ("486", 4), ("P5", 1), ("P6", 0), ("K5", 1), ("K6", 2), ("K7", 0), ("Core", 0)];
    assert_eq!(classify("fxch", "st1", "d9c9"), "x87_exchange");
    let got: Vec<(&str, i64)> = cpu::names()
        .into_iter()
        .map(|name| (name, cpu::profile(name).unwrap().cost("x87_exchange").unwrap()))
        .collect();
    assert_eq!(got, expected);
}

/// Homebrew's ndisasm 3.01 printed `D1EB` as `shr bx,0x0`, so the D1 shifts
/// priced as `shift_ri` on macOS: B$DVI4's loop scored 12 on the 386, not 16.
#[test]
fn test_shifts_by_one_decode_the_same_on_every_host() {
    let (_, _, _, det) = report("loop", DVI4_LOOP);
    let kinds: Vec<(&str, &str)> = det.iter().map(|d| (d.0.as_str(), d.1)).collect();
    assert_eq!(
        kinds,
        [
            ("shr bx,1", "shift_r1"),
            ("rcr cx,1", "shift_r1"),
            ("shr dx,1", "shift_r1"),
            ("rcr ax,1", "shift_r1"),
            ("or bx,bx", "alu_rr"),
            ("jne 0", "jcc"),
        ]
    );
}

/// Python's `report()` over every case: count, alone, bulk and notes.
const PYTHON: &str = "\
and: BC halves| 6 [8, 8, 10, 7, 7, 9, 11] [8.0, 8.0, 2.0, 1.5, 2.0, 2.0, 1.5] []
and: widened| 5 [12, 11, 10, 7, 7, 9, 11] [12.0, 11.0, 1.7, 1.2, 1.7, 1.7, 1.2] []
and: widened, no dx| 3 [7, 7, 10, 7, 7, 9, 11] [7.0, 7.0, 1.0, 0.8, 1.0, 1.0, 0.8] []
chain: BC halves| 10 [16, 16, 18, 13, 13, 15, 19] [16.0, 16.0, 3.3, 2.5, 3.3, 3.3, 2.5] []
chain: widened| 5 [13, 13, 18, 13, 13, 15, 19] [13.0, 13.0, 1.7, 1.2, 1.7, 1.7, 1.2] []
mul: stock fast| 14 [68, 34, 45, 14, 15, 17, 46] [68.0, 34.0, 45.3, 14.5, 15.3, 17.3, 46.5] []
mul: stock full| 22 [103, 62, 48, 16, 18, 20, 48] [103.0, 62.0, 48.0, 16.5, 18.0, 20.0, 48.5] []
mul: mgl call| 11 [82, 35, 50, 14, 14, 16, 49] [82.0, 35.0, 50.3, 13.8, 14.3, 16.3, 48.8] ['lcp']
mul: mgl pow2| 6 [11, 8, 4, 3, 3, 4, 5] [11.0, 8.0, 2.0, 1.5, 2.0, 2.0, 1.5] []
mul: qbopt absorbed, memory| 5 [40, 18, 14, 11, 10, 13, 14] [40.0, 18.0, 1.7, 1.2, 1.7, 1.7, 1.2] []
mul: qbopt absorbed, register| 6 [47, 19, 11, 9, 8, 10, 10] [47.0, 19.0, 2.0, 1.5, 2.0, 2.0, 1.5] []
cmp: stock| 20 [74, 34, 51, 18, 20, 22, 52] [74.0, 34.0, 50.7, 18.5, 19.7, 21.7, 51.5] []
cmp: mgl call| 11 [59, 26, 44, 14, 14, 16, 46] [59.0, 26.0, 44.3, 13.8, 14.3, 16.3, 45.8] []
cmp: mgl inlined| 6 [11, 9, 7, 5, 5, 6, 8] [11.0, 9.0, 2.0, 1.5, 2.0, 2.0, 1.5] []
cmp: qbopt absorbed, memory| 4 [12, 9, 11, 8, 8, 9, 12] [12.0, 9.0, 1.3, 1.0, 1.3, 1.3, 1.0] []
cmp: qbopt absorbed, stack| 10 [18, 15, 25, 13, 14, 18, 23] [18.0, 15.0, 3.3, 2.5, 3.3, 3.3, 2.5] ['partial-reg']
div256: stock| 33 [138, 91, 97, 67, 69, 71, 95] [138.0, 91.0, 97.0, 66.8, 69.0, 71.0, 94.8] []
div256: mgl call| 15 [111, 79, 90, 56, 56, 57, 76] [111.0, 79.0, 90.3, 56.5, 56.3, 57.3, 75.5] ['lcp']
div256: mgl shift| 14 [66, 32, 45, 14, 15, 17, 46] [66.0, 32.0, 45.3, 14.5, 15.3, 17.3, 46.5] []
div: qbopt absorbed, memory| 7 [62, 58, 48, 44, 43, 43, 33] [62.0, 58.0, 47.0, 43.5, 43.0, 42.0, 30.5] ['lcp']
div: qbopt absorbed, register| 7 [68, 58, 48, 44, 43, 42, 32] [68.0, 58.0, 47.0, 43.5, 43.0, 42.0, 30.5] ['lcp']
rmi4: stock| 31 [133, 89, 96, 66, 68, 70, 94] [133.0, 89.0, 96.3, 66.2, 68.3, 70.3, 94.2] []
rmi4: qbopt absorbed, memory| 8 [64, 60, 48, 44, 43, 43, 33] [64.0, 60.0, 47.3, 43.8, 43.3, 42.3, 30.8] ['lcp']
rmi4: qbopt absorbed, register| 8 [70, 60, 48, 44, 43, 42, 32] [70.0, 60.0, 47.3, 43.8, 43.3, 42.3, 30.8] ['lcp']
loop| 6 [16, 6, 2, 2, 2, 2, 2] [16.0, 6.0, 2.0, 1.5, 2.0, 2.0, 1.5] []
";

#[test]
fn test_scores_match_python() {
    let mut cases: Vec<(&str, String)> = CASES.iter().map(|(n, h)| (*n, h.clone())).collect();
    cases.push(("loop", DVI4_LOOP.to_owned()));
    let mut got = String::new();
    for (name, hexs) in &cases {
        let (_, cnt, (lone, bulk), det) = report(name, hexs);
        let notes: BTreeSet<&str> = det.iter().flat_map(|d| d.3.iter().copied()).collect();
        let lone = lone.iter().map(i64::to_string).collect::<Vec<_>>().join(", ");
        let bulk = bulk.iter().map(|&x| crate::support::pyrepr::float(x)).collect::<Vec<_>>().join(", ");
        let notes = notes.iter().map(|n| format!("'{n}'")).collect::<Vec<_>>().join(", ");
        got += &format!("{name}| {cnt} [{lone}] [{bulk}] [{notes}]\n");
    }
    assert_eq!(got, PYTHON);
}

#[test]
fn test_g_matches_python_format() {
    assert_eq!(_g(12.0), "12");
    assert_eq!(_g(45.3), "45.3");
    assert_eq!(_g(0.8), "0.8");
    assert_eq!(_g(1234567.0), "1.23457e+06");
}

/// `lea esi, [ebx+ebx*2]` (a multiply by 3) has two registers and no cell: the
/// encoder named no form for it and priced it as nothing, so a function with
/// one could not be priced in bytes and its routes compared as unpriced
/// (nbody_fixed -Os).
#[test]
fn test_a_lea_of_a_register_and_a_scaled_copy_of_it_has_a_byte_price() {
    let reg = |register| ir::Loc::Reg(ir::Reg { register, width: 4 });
    let cell = ir::Mem {
        addr: None,
        through: RegId::EBX,
        index_through: RegId::EBX,
        index: Some(ir::Held { value: 1, width: 4 }),
        base: Some(ir::Held { value: 1, width: 4 }),
        scale: 2,
        width: 4,
        ..ir::Mem::new(None, 0)
    };
    let what = ir::Semantics {
        name: Some("lea".into()),
        dests: vec![reg(RegId::ESI)],
        sources: vec![ir::Loc::Mem(cell)],
        ..ir::Semantics::new(ir::Operation::Address)
    };
    let code = crate::backend::select::priced_in(32, &what, 0, None, false, false, None).expect("a price");
    assert_eq!(code.code, [0x8d, 0x34, 0x5b]);
}

/// Every float function the selector can emit has a byte price, in both
/// modes: `fatan`, `flog2` and `fexp2` are sequences of x87 instructions that
/// no instruction encodes, and had none, so at -Os with `LLRM_VERIFY=1` a
/// function of deedlines' TSC.BAS panicked ("no byte price for ... flog2
/// ST0", llrm#1307) and, unverified, was priced as nothing.
#[test]
fn test_every_float_function_the_selector_emits_has_a_byte_price() {
    let st0 = vec![ir::Loc::st(0)];
    for bits in [16, 32] {
        for (_, name) in crate::backend::isel::FLOAT_FUNCTIONS {
            let what = ir::Semantics {
                name: Some(name.into()),
                dests: st0.clone(),
                sources: st0.clone(),
                ..ir::Semantics::new(ir::Operation::FloatUnary)
            };
            let priced = crate::backend::select::priced_in(bits, &what, 0, None, false, false, None);
            assert!(priced.is_some_and(|code| !code.code.is_empty()), "{name} at {bits} bits has no byte price");
        }
    }
}

/// The names written after `Operation::FloatUnary,` in `text`.
fn float_unary_names(text: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut rest = text;
    while let Some(at) = rest.find("Operation::FloatUnary,") {
        rest = &rest[at + "Operation::FloatUnary,".len()..];
        let Some(open) = rest.trim_start().strip_prefix('"') else { continue };
        if let Some(close) = open.find('"') {
            names.insert(open[..close].to_owned());
        }
    }
    names
}

#[test]
fn test_the_scan_for_float_unary_names_reads_a_literal_after_the_operation() {
    let text = "semantics(Operation::FloatUnary, \"fnew\", d, s); sem(Operation::FloatUnary,\n  \"fchs\", a, b);
                semantics(Operation::FloatUnary, name, d, s)";
    assert_eq!(float_unary_names(text), BTreeSet::from(["fnew".to_owned(), "fchs".to_owned()]));
}

/// llrm#1307 was one function missing a price (`flog2`), found at -Os under
/// `LLRM_VERIFY=1` on a demo. Every x87 unary the backend writes by name, in
/// any of its files, has a byte price in both modes, so the next one fails
/// here.
#[test]
fn test_every_float_unary_the_backend_names_has_a_byte_price() {
    let mut names: BTreeSet<String> =
        crate::backend::isel::FLOAT_FUNCTIONS.iter().map(|(_, name)| (*name).to_owned()).collect();
    let mut dirs = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Sources only: not a crate's integration tests.
                if !path.file_name().is_some_and(|name| name == "tests") {
                    dirs.push(path);
                }
            } else if path.extension().is_some_and(|ext| ext == "rs")
                && !path.file_name().is_some_and(|name| name.to_string_lossy().ends_with("_tests.rs"))
            {
                names.extend(float_unary_names(&std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    assert!(["fabs", "fchs", "fsqrt", "flog2"].iter().all(|one| names.contains(*one)), "{names:?}");
    let st0 = vec![ir::Loc::st(0)];
    for bits in [16, 32] {
        for name in &names {
            let what = ir::Semantics {
                name: Some(name.as_str().into()),
                dests: st0.clone(),
                sources: st0.clone(),
                ..ir::Semantics::new(ir::Operation::FloatUnary)
            };
            let priced = crate::backend::select::priced_in(bits, &what, 0, None, false, false, None);
            assert!(priced.is_some_and(|code| !code.code.is_empty()), "{name} at {bits} bits has no byte price");
        }
    }
}

/// The sequences the encoder prices are the bytes QB's finalizer places.
#[test]
fn test_the_sequences_priced_are_the_bytes_the_finalizer_places() {
    for (name, bytes) in llrm_x86::select::X87_SEQUENCES {
        assert_eq!(crate::driver::basic::sequence_bytes(name).as_deref(), Some(bytes), "{name}");
    }
}
