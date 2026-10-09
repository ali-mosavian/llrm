//! What the whole-module step spends is bounded by what it changed, not by what
//! it asked.
use std::sync::atomic::{AtomicUsize, Ordering};

use llrm_mir::passes::Observer;
use llrm_mir::program::{Program, ProgramAnalyses};
use llrm_transforms::inline::Threshold;
use llrm_transforms::interprocedural::{managers, optimized, roots};

static SCANS: AtomicUsize = AtomicUsize::new(0);
static NAME_TABLES: AtomicUsize = AtomicUsize::new(0);

fn counting(
    what: &'static str,
    _hit: bool,
) {
    match what {
        "callees" => SCANS.fetch_add(1, Ordering::Relaxed),
        "declared names" => NAME_TABLES.fetch_add(1, Ordering::Relaxed),
        _ => 0,
    };
}

/// `callers` functions, each calling two small callees, all called from `main`.
fn source(callers: usize) -> String {
    let mut text = String::from(
        "define internal i16 @c0(i16 %x) {\nb:\n  %y = add i16 %x, 3\n  ret i16 %y\n}\n\ndefine internal i16 @c1(i16 %x) {\nb:\n  %y = mul i16 %x, 5\n  ret i16 %y\n}\n\n",
    );
    for j in 0..callers {
        text += &format!(
            "define internal i16 @g{j}(i16 %x) {{\nb:\n  %a = call i16 @c0(i16 %x)\n  %b = call i16 @c1(i16 %a)\n  %c = add i16 %b, {j}\n  ret i16 %c\n}}\n\n"
        );
    }
    text += "define i16 @main(i16 %x) {\nb:\n";
    let mut last = "%x".to_owned();
    for j in 0..callers {
        text += &format!("  %r{j} = call i16 @g{j}(i16 {last})\n");
        last = format!("%r{j}");
    }
    text + &format!("  ret i16 {last}\n}}\n")
}

/// `memory::callees` scans every function of the module. The step used to scan
/// for every procedure it looked at in every round, so a module of n callers
/// paid n scans of n functions though nothing changed: n^2 at -O2 and worse at
/// -Os. It now scans after a body changed (one pipeline run each), not once
/// more for each question.
#[test]
fn test_the_whole_module_step_scans_the_callees_after_a_change_not_for_every_question() {
    llrm_mir::passes::observe(Observer { span: |_, _, run| run(), function: |_, run| run(), count: counting });
    let mut prior = None;
    for callers in [8, 16, 32] {
        let mut m = llrm_mir::parse::module(&source(callers)).unwrap_or_else(|error| panic!("{error}"));
        let edits = std::cell::Cell::new(0usize);
        SCANS.store(0, Ordering::Relaxed);
        NAME_TABLES.store(0, Ordering::Relaxed);
        Program::lend(&mut m, std::rc::Rc::new(llrm_mir::target::Neutral), |program| {
            let mut modules = managers(program, &mut ProgramAnalyses::default());
            let roots = roots(program);
            let costs = llrm_transforms::profit::OperationCosts::default();
            optimized::<String>(
                program,
                &mut modules,
                &roots,
                &costs,
                Some(&costs),
                1,
                costs.call,
                Threshold::default(),
                &mut |_, _, _, _| {
                    edits.set(edits.get() + 1);
                    Ok(())
                },
                &mut |_, _, function| Ok(function),
                &mut |_, _, _| Ok(()),
            )
            .unwrap();
        })
        .unwrap();
        let scans = SCANS.load(Ordering::Relaxed);
        assert!(scans <= edits.get() + 8, "{callers} callers: {scans} scans for {} changed bodies", edits.get());
        if let Some((before, small)) = prior {
            assert!(
                scans <= 2 * before + 8,
                "{scans} scans at {callers} callers, {before} at {small}: grows faster than the module"
            );
        }
        // A name table of every global, built and copied for each body looked
        // at: no pass here declares a function.
        let tables = NAME_TABLES.load(Ordering::Relaxed);
        assert!(tables <= 8, "{callers} callers: {tables} name tables of the module's globals built");
        prior = Some((scans, callers));
    }
}
