//! The rich-MIR corpus.

use crate::testing::corpus;

#[test]
fn every_corpus_module_parses_and_verifies() {
    let modules = corpus();
    for prefix in ["emitted/qb-", "emitted/demo-", "emitted/nib-", "optimized/qb-", "optimized/demo-", "optimized/nib-"] {
        assert!(modules.iter().any(|(name, _)| name.starts_with(prefix)), "the corpus holds no {prefix}");
    }
    for (name, module) in &modules {
        assert_eq!(llrm_mir::verify::verify(module), Vec::<String>::new(), "{name}");
    }
}
