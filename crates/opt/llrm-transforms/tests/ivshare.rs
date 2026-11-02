//! The loop shapes of the LSR design (`tools/ivshare-shapes.py`), run through
//! `LoopStrengthReduce` under each x86 width's target. A row either has the
//! shape's expected form or is listed in `ivshare/xfail.txt` with the reason
//! the form is not built yet; a row that passes while listed fails the test,
//! so the list only shrinks. `IVSHARE_BLESS=1` rewrites the list from the run.
//!
//! Forms:
//! - every positive row, on every width: exactly one loop-carried induction
//!   register besides the accumulators (no second counter, no pointer bump per
//!   array);
//! - `end`, where the width's costs say a zero test pays (p16, p32): the exit
//!   also compares that register with 0 (the step's flags end the loop), no
//!   limit register. At p64 the compare macro-fuses, so a zero-based index and
//!   a `cmp` is as good (the pin's choice);
//! - `keep` (negative rows): the end form must not appear.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use llrm_mir::target::Machine as _;
use llrm_transforms::lsr_pass::LoopStrengthReduce;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ivshare")
}

fn expected(id: &str) -> &'static str {
    if id.starts_with('N') || id == "F32" { "keep" } else { "end" }
}

/// The text of the `loop:` block of the printed module.
fn loop_block(text: &str) -> Vec<String> {
    let mut lines = text.lines().skip_while(|l| !l.starts_with("loop"));
    lines.next();
    lines.take_while(|l| !l.is_empty() && !l.ends_with(':')).map(|l| l.trim().to_owned()).collect()
}

fn induction_registers(block: &[String]) -> usize {
    block.iter().filter(|l| l.contains("= phi ") && !l.starts_with("%acc")).count()
}

fn has_end_form(block: &[String]) -> bool {
    let ivs = induction_registers(block);
    let compares: Vec<&String> = block.iter().filter(|l| l.contains("= icmp ")).collect();
    ivs == 1 && compares.len() == 1 && compares[0].trim_end().ends_with(", 0")
}

/// A flat x86 target: the neutral machine with the CPU's LSR model.
struct Flat(&'static str);

impl llrm_mir::target::Machine for Flat {
    fn foreign_span(
        &self,
        selectors: (i64, i64),
        offsets: (i64, i64),
        width: i64,
    ) -> Option<(i64, i64)> {
        llrm_mir::target::Neutral.foreign_span(selectors, offsets, width)
    }

    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        llrm_mir::target::Neutral.spaces()
    }

    fn costs(&self) -> llrm_mir::target::OperationCosts {
        llrm_mir::target::Neutral.costs()
    }

    fn registers(&self) -> i64 {
        llrm_mir::target::Neutral.registers()
    }

    fn call_registers(&self) -> i64 {
        llrm_mir::target::Neutral.call_registers()
    }

    fn address_forms(&self) -> Vec<llrm_mir::target::AddressForm> {
        llrm_mir::target::Neutral.address_forms()
    }

    fn tti(&self) -> Option<Rc<dyn llrm_mir::tti::Tti>> {
        Some(Rc::new(llrm_x86::tti::X86::of_cpu(self.0).unwrap()))
    }
}

fn reduced(
    width: &str,
    text: &str,
) -> String {
    let mut module = llrm_mir::parse::module(text).unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(LoopStrengthReduce { size: false });
    let target: Rc<dyn llrm_mir::target::Machine> = match width {
        "p16" => Rc::new(llrm_x86_m16::Dos::pin_tuned()),
        "p32" => Rc::new(Flat("i486")),
        _ => Rc::new(Flat("x86-64")),
    };
    manager.run_module(&mut module, target).expect("runs");
    llrm_mir::print::module(&module)
}

#[test]
fn every_shape_has_its_form_or_a_reason() {
    let xfail_path = root().join("xfail.txt");
    let listed: BTreeMap<String, String> = fs::read_to_string(&xfail_path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let (row, reason) = l.split_once(' ').unwrap_or((l, ""));
            (row.to_owned(), reason.to_owned())
        })
        .collect();
    let mut files: Vec<PathBuf> = fs::read_dir(root().join("shapes")).unwrap().map(|e| e.unwrap().path()).collect();
    files.sort();
    let (mut failing, mut problems) = (Vec::new(), Vec::new());
    for file in files {
        let name = file.file_stem().unwrap().to_string_lossy().into_owned();
        let (id, width) = name.split_once('.').unwrap();
        let after = reduced(width, &fs::read_to_string(&file).unwrap());
        let block = loop_block(&after);
        let ok = if expected(id) == "end" {
            induction_registers(&block) == 1 && (width == "p64" || has_end_form(&block))
        } else {
            !has_end_form(&block)
        };
        let row = format!("{id}.{width}");
        match (ok, listed.contains_key(&row)) {
            (false, false) => failing.push(row.clone()),
            (true, true) => problems.push(format!("{row} passes but is listed")),
            (false, true) | (true, false) => {}
        }
    }
    if std::env::var_os("IVSHARE_BLESS").is_some() {
        let mut out = String::from("# rows that do not have their form yet: row reason\n");
        for row in listed.keys().chain(failing.iter()).collect::<std::collections::BTreeSet<_>>() {
            let reason = listed.get(row).map_or("shared counter or end form not built (S3)", String::as_str);
            out += &format!("{row} {reason}\n");
        }
        fs::write(&xfail_path, out).unwrap();
        return;
    }
    assert!(failing.is_empty() && problems.is_empty(), "failing: {failing:?}\n{problems:?}");
}

/// The instrument: a loop written in the end form reads as one, and the
/// shapes' own input (limit register, zero-based index) does not.
#[test]
fn the_end_form_predicate_reads_a_hand_written_end_form() {
    let end = "entry:\n  br label %loop\nloop:\n  %lsr.iv = phi i32 [ -400, %entry ], [ %lsr.iv.next, %loop ]\n  %acc = phi i32 [ 0, %entry ], [ %acc2, %loop ]\n  %acc2 = add i32 %acc, 1\n  %lsr.iv.next = add nsw i32 %lsr.iv, 4\n  %c = icmp eq i32 %lsr.iv.next, 0\n  br i1 %c, label %exit, label %loop\nexit:\n  ret void\n";
    assert!(has_end_form(&loop_block(end)));
    let limit = end.replace("%lsr.iv.next, 0", "%lsr.iv.next, %n");
    assert!(!has_end_form(&loop_block(&limit)));
}

/// The instrument under each configuration: a loop already in the end form
/// stays one through `reduced`, and the constant-trip shape (B09) is rewritten
/// into it at 16 and 32 bits. At 64 bits LSR turns a given end form back into
/// the zero-based index and a compare against the constant (the pin's choice:
/// the compare macro-fuses, so the zero test saves nothing), so no shape
/// reaches the end form there until the zero test is priced (S3, H10).
#[test]
fn each_configuration_reads_the_end_form_it_makes_or_is_given() {
    for (width, ty) in [("p16", "i16"), ("p32", "i32"), ("p64", "i64")] {
        let layout = fs::read_to_string(root().join("shapes").join(format!("B09.{width}.ll")))
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .to_owned();
        let end = format!(
            "{layout}\ndefine void @f(ptr %p) {{\nentry:\n  br label %loop\nloop:\n  %i = phi {ty} [ -400, %entry ], [ %next, %loop ]\n  %a = getelementptr i8, ptr %p, {ty} %i\n  %b = getelementptr i8, ptr %a, {ty} 400\n  store i32 0, ptr %b\n  %next = add nsw {ty} %i, 4\n  %c = icmp eq {ty} %next, 0\n  br i1 %c, label %exit, label %loop\nexit:\n  ret void\n}}\n"
        );
        let kept = has_end_form(&loop_block(&reduced(width, &end)));
        assert_eq!(kept, width != "p64", "{width}: the given end form is kept: {kept}");
    }
    for width in ["p16", "p32"] {
        let text = fs::read_to_string(root().join("shapes").join(format!("B09.{width}.ll"))).unwrap();
        assert!(has_end_form(&loop_block(&reduced(width, &text))), "{width}: B09 does not reach the end form");
    }
}
