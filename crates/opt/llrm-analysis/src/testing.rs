//! What the tests share: a module read from its text, and the corpus.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Function, Module, ValueId};

/// The layout HIR's modules state, as a module's first line.
pub const DOS: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32\"\n\n";

pub fn parsed(text: &str) -> Module {
    llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

/// The function `name` of `module`.
pub fn function<'m>(
    module: &'m Module,
    name: &str,
) -> &'m Function {
    let global = module.named(name).unwrap_or_else(|| panic!("no @{name}"));
    module.global(global).function().expect("a function")
}

/// The layout `module` states.
pub fn layout(module: &Module) -> DataLayout {
    module.datalayout.as_deref().map_or_else(DataLayout::default, |text| DataLayout::parse(text).expect("a datalayout"))
}

/// The value named `name`: a parameter or an instruction's result.
pub fn value(
    function: &Function,
    name: &str,
) -> ValueId {
    let results = function.walk().filter_map(|(_, inst)| function.instruction(inst).result);
    function
        .parameters()
        .iter()
        .copied()
        .chain(results)
        .find(|&one| function.value(one).name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no %{name}"))
}

/// The block named `name`.
pub fn block(
    function: &Function,
    name: &str,
) -> BlockId {
    function
        .layout()
        .iter()
        .copied()
        .find(|&one| function.block(one).name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no %{name}"))
}

/// Every module of `corpus/` (`tools/mir-corpus.sh`), named by its
/// directory and program, as `emitted/qb-addrm`.
pub fn corpus() -> Vec<(String, Module)> {
    corpus_files().into_iter().map(|(name, path)| (name.clone(), parse_corpus_entry(&name, &path))).collect()
}

/// The corpus entries' names and files, in order, unparsed: a test that spreads the entries over threads parses each on
/// its own.
pub fn corpus_files() -> Vec<(String, std::path::PathBuf)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus");
    let mut paths: Vec<_> = ["emitted", "optimized"]
        .iter()
        .flat_map(|stage| std::fs::read_dir(root.join(stage)).unwrap_or_else(|error| panic!("{stage}: {error}")))
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let stage = path.parent().unwrap().file_name().unwrap().to_string_lossy();
            (format!("{stage}/{}", path.file_stem().unwrap().to_string_lossy()), path)
        })
        .collect()
}

pub fn parse_corpus_entry(
    name: &str,
    path: &std::path::Path,
) -> Module {
    let text = std::fs::read_to_string(path).unwrap();
    llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{name}: {error}"))
}

/// `unit` carrying what is known of its body without memory and its shape, derived once for the body a test built,
/// which no manager has seen. (Kept for the life of the test run.)
pub fn with_registers(unit: crate::memory::Unit<'_>) -> crate::memory::Unit<'_> {
    let known =
        Box::leak(Box::new(crate::consts::known(&crate::memory::Unit { registers: None, ..unit }, None, None, None)));
    let shape = Box::leak(Box::new(crate::cfg::Shape::of(unit.function)));
    unit.with_registers(known).with_shape(shape)
}
