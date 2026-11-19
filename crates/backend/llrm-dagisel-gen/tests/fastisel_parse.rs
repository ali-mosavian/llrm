//! `fastisel::parse` reads the pin's `-gen-fast-isel` output into rows.

use llrm_dagisel_gen::fastisel::{self, Shape};

const EXCERPT: &str = include_str!("fixtures/fastisel_excerpt.inc");

/// The first parser kept only the first arm of `ADD i32 rr`: the table asked for
/// `ADD32rr_ND` on a CPU without NDD. Arms keep the pin's order and conditions.
#[test]
fn add_i32_rr_keeps_both_arms_in_the_pins_order() {
    let table = fastisel::parse(EXCERPT).unwrap();
    let rows = table.rows("ISD", "ADD", "i32", Shape::Rr);
    assert_eq!(rows.ret, Some("i32".to_owned()));
    let ops: Vec<_> = rows.arms.iter().map(|a| (a.opcode.as_str(), a.class.as_str())).collect();
    assert_eq!(ops, [("ADD32rr_ND", "GR32"), ("ADD32rr", "GR32")]);
    assert_eq!(rows.arms[0].cond.as_deref(), Some("Subtarget->hasNDD()"));
    assert_eq!(rows.arms[1].cond.as_deref(), Some("!Subtarget->hasNDD()"));
}

/// A `_Predicate_` variant is a different function from the plain one: the
/// immediate must satisfy the named test.
#[test]
fn an_immediate_predicate_is_part_of_the_key() {
    let table = fastisel::parse(EXCERPT).unwrap();
    let plain = table.rows("ISD", "ADD", "i32", Shape::Ri);
    assert!(plain.imm_predicate.is_none());
    let wide = table.rows("ISD", "ADD", "i64", Shape::RiPredicated);
    assert_eq!(wide.imm_predicate.as_deref(), Some("i64immSExt32"));
}

/// Dispatchers (`fastEmit_ISD_ADD_rr`) are derived from the rows, not stored;
/// a function the parser cannot read is counted, never dropped.
#[test]
fn dispatchers_are_skipped_and_the_count_adds_up() {
    let table = fastisel::parse(EXCERPT).unwrap();
    // ADD i32 rr/ri, ADD i64 ri predicated, ABS v16i8, SIGN_EXTEND i8 to i32 and to i64.
    assert_eq!(table.parsed(), 6);
    // ADD rr, and SIGN_EXTEND i8 whose body switches on `RetVT`.
    assert_eq!(table.dispatchers(), 2);
    // MUL i8 rr copies into AL first: a form kept for step 2, counted here.
    assert_eq!(table.unread(), 1);
}

/// `SIGN_EXTEND i8 -> i32` and `-> i64` are two functions named with both types;
/// the first reader found neither (487 such functions unread on the pin file),
/// so a sign-extend had no row.
#[test]
fn a_function_named_with_a_source_and_a_result_type_has_its_own_row() {
    let table = fastisel::parse(EXCERPT).unwrap();
    let to32 = table.rows_to("ISD", "SIGN_EXTEND", "i8", "i32", Shape::R);
    let to64 = table.rows_to("ISD", "SIGN_EXTEND", "i8", "i64", Shape::R);
    assert_eq!(to32.arms.len(), 1);
    assert_eq!((to32.arms[0].opcode.as_str(), to32.arms[0].class.as_str()), ("MOVSX32rr8", "GR32"));
    assert_eq!((to64.arms[0].opcode.as_str(), to64.arms[0].class.as_str()), ("MOVSX64rr8", "GR64"));
}

/// Over the whole pin file nothing is lost: every `fastEmit_` head is a row, a
/// dispatcher or counted unread, and the integer rows the m32 core needs exist.
/// Runs when `LLRM_FASTISEL_INC` names the pin's `X86GenFastISel.inc`.
#[test]
fn the_whole_pin_file_is_accounted_for() {
    let Ok(path) = std::env::var("LLRM_FASTISEL_INC") else { return };
    let text = std::fs::read_to_string(&path).unwrap();
    let heads = text.lines().filter(|l| l.starts_with("Register fastEmit_")).count();
    let table = fastisel::parse(&text).unwrap();
    eprintln!("heads {heads}: parsed {}, dispatchers {}, unread {}", table.parsed(), table.dispatchers(), table.unread());
    assert_eq!(table.parsed() + table.dispatchers() + table.unread(), heads, "a function was dropped");
    // The 12 left are the two forms steps 2-3 write by hand: six i8 multiplies, shifts and
    // rotates that copy into AL/CL first, and six TRUNCATEs that extract a subregister.
    assert_eq!(table.unread(), 12, "the unread set changed");
    assert_eq!((table.parsed(), table.dispatchers()), (1659, 474));
    for (op, shape) in [("ADD", Shape::Rr), ("SUB", Shape::Rr), ("AND", Shape::Rr), ("OR", Shape::Rr), ("XOR", Shape::Rr), ("MUL", Shape::Rr), ("ADD", Shape::Ri)] {
        assert!(!table.rows("ISD", op, "i32", shape).arms.is_empty(), "no i32 {op} {shape:?} row");
    }
}

const FEATURES: &str = include_str!("../features.toml");

/// Choosing an arm took the first listed, so an i486 asked for the NDD form
/// (`ADD32rr_ND`, an APX instruction). The first arm whose condition the CPU
/// states true wins, as in the pin's `fastEmit_*` bodies.
#[test]
fn an_i486_gets_the_plain_add_not_the_ndd_form() {
    let cpu = llrm_dagisel_gen::features::Cpu::load(FEATURES, "i486").unwrap();
    let table = fastisel::parse(EXCERPT).unwrap();
    let chosen = table.resolve(&cpu).unwrap();
    let add = chosen.pick("ISD", "ADD", "i32", Shape::Rr).unwrap();
    assert_eq!((add.opcode.as_str(), add.class.as_str()), ("ADD32rr", "GR32"));
    let imm = chosen.pick("ISD", "ADD", "i32", Shape::Ri).unwrap();
    assert_eq!(imm.opcode, "ADD32ri");
}

/// An arm no CPU feature allows is no row: `PABSB` needs SSSE3, which an i486
/// lacks, so `ABS v16i8` is absent rather than the first arm.
#[test]
fn a_row_with_no_arm_the_cpu_allows_is_absent() {
    let cpu = llrm_dagisel_gen::features::Cpu::load(FEATURES, "i486").unwrap();
    let chosen = fastisel::parse(EXCERPT).unwrap().resolve(&cpu).unwrap();
    assert!(chosen.pick("ISD", "ABS", "v16i8", Shape::R).is_none());
    // Of the six rows in the excerpt only ABS has no arm an i486 allows.
    assert_eq!(chosen.len(), 5);
}

/// The chosen rows are written as a sorted Rust table the compiler looks up by
/// binary search; an unsorted table would miss rows without any error.
#[test]
fn the_rust_table_is_sorted_and_names_the_i486_add() {
    let cpu = llrm_dagisel_gen::features::Cpu::load(FEATURES, "i486").unwrap();
    let chosen = fastisel::parse(EXCERPT).unwrap().resolve(&cpu).unwrap();
    let text = chosen.render("i486");
    assert!(text.contains(r#"("ISD", "ADD", "i32", "", 1, "ADD32rr", "GR32"),"#), "{text}");
    assert!(text.contains(r#"("ISD", "SIGN_EXTEND", "i8", "i32", 0, "MOVSX32rr8", "GR32"),"#));
    let rows: Vec<&str> = text.lines().filter(|l| l.trim_start().starts_with("(\"")).collect();
    assert_eq!(rows.len(), chosen.len());
    let mut sorted = rows.clone();
    sorted.sort();
    assert_eq!(rows, sorted, "rows are not in key order");
}

/// An arm whose condition the CPU file says nothing of must not be taken, and
/// must not stop the whole table: the row is undecided and the compiler falls
/// back to the DAG for it. (The first `resolve` returned the error and the
/// regen of every CPU died on the first unknown atom.)
#[test]
fn a_condition_nothing_states_leaves_its_row_undecided() {
    let text = "Register fastEmit_ISD_ADD_MVT_i32_rr(MVT RetVT, Register Op0, Register Op1) {\n  if (RetVT.SimpleTy != MVT::i32)\n    return Register();\n  if ((Subtarget->madeUpThing())) {\n    return fastEmitInst_rr(X86::ADD32rr, &X86::GR32RegClass, Op0, Op1);\n  }\n  return Register();\n}\n";
    let cpu = llrm_dagisel_gen::features::Cpu::load(FEATURES, "i486").unwrap();
    let chosen = fastisel::parse(text).unwrap().resolve(&cpu).unwrap();
    assert!(chosen.pick("ISD", "ADD", "i32", Shape::Rr).is_none());
    assert_eq!(chosen.undecided(), 1);
}
