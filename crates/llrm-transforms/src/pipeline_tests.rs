use llrm_analysis::testing::corpus;
use llrm_mir::interpret;

use crate::pipeline::{self, Applied};

// Enough for every corpus entry that finishes at all.
const FUEL: u64 = 2_000_000;

#[test]
fn the_pipeline_keeps_every_corpus_module_verifying_and_computing_the_same() {
    let applied = Applied { target: Some(std::rc::Rc::new(llrm_cycles::target::Dos)), ..Applied::default() };
    let mut ran = 0;
    for (name, mut module) in corpus() {
        let entry = module.named("main").filter(|&id| module.global(id).function().is_some_and(|one| !one.is_declaration() && one.parameters().is_empty()));
        let before = entry.map(|_| interpret::run(&module, "main", Vec::new(), FUEL));
        pipeline::applied(&mut module, &applied).unwrap_or_else(|error| panic!("{name}: {error}"));
        if let Some(Ok(before)) = before {
            assert_eq!(interpret::run(&module, "main", Vec::new(), FUEL), Ok(before), "{name}");
            ran += 1;
        }
    }
    assert!(ran > 0, "no corpus entry runs");
}
