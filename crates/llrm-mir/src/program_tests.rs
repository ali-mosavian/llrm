use std::cell::RefCell;
use std::rc::Rc;

use crate::context::GlobalId;
use crate::module::{GlobalKind, Linkage, Module};
use crate::opcode::Attribute;
use crate::parse;
use crate::passes::{ModuleAnalyses, ModuleAnalysis, ModulePass, PassManager};
use crate::program::{Program, ProgramAnalyses, ProgramAnalysis, ProgramPass};
use crate::target::Neutral;

fn module(text: &str) -> Module {
    parse::module(text).unwrap_or_else(|error| panic!("{error}"))
}

fn program() -> Program {
    let caller = module("declare i16 @g()\n\ndefine i16 @f() {\nentry:\n  %x = call i16 @g()\n  ret i16 %x\n}\n");
    let callee = module("define i16 @g() {\nentry:\n  ret i16 1\n}\n\ndefine internal i16 @h() {\nentry:\n  ret i16 2\n}\n");
    Program::new(vec![caller, callee], Rc::new(Neutral)).unwrap()
}

/// A name resolves to the module defining it, not the first declaring it.
#[test]
fn a_name_resolves_to_its_definition_across_modules() {
    let program = program();
    assert_eq!(program.resolve("g"), Some((1, GlobalId(0))));
    assert_eq!(program.resolve("f"), Some((0, GlobalId(1))));
    assert_eq!(program.resolve("nothing"), None);
}

/// Modules stating different datalayouts are no one program.
#[test]
fn modules_of_one_program_share_a_datalayout() {
    let near = module("target datalayout = \"e-p:16:16\"\n");
    let far = module("target datalayout = \"e-p:32:32\"\n");
    assert!(Program::new(vec![near, far], Rc::new(Neutral)).is_err());
}

/// How many functions the program defines.
struct Defined;

impl ProgramAnalysis for Defined {
    type Result = usize;
    const NAME: &'static str = "defined";
    fn run(program: &Program, _: &mut ProgramAnalyses) -> usize {
        program.modules.iter().flat_map(|one| one.functions()).filter(|(_, _, function)| !function.is_declaration()).count()
    }
}

thread_local! {
    static SEEN: RefCell<Vec<Option<usize>>> = const { RefCell::new(Vec::new()) };
}

/// Records what the program proxy holds of `Defined`, then marks every
/// function it names `readonly`: an exported one's changes the module's
/// interface, an internal one's does not.
struct Mark(&'static str);

impl ModulePass for Mark {
    fn name(&self) -> &'static str {
        "mark"
    }

    fn run(&mut self, module: &mut Module, analyses: &mut ModuleAnalyses) -> Vec<GlobalId> {
        SEEN.with_borrow_mut(|seen| seen.push(analyses.program().cached::<Defined>().map(|one| *one)));
        let Some(id) = module.named(self.0) else { return Vec::new() };
        let GlobalKind::Function(function) = &mut module.globals[id.0 as usize].kind else { panic!("a function") };
        function.attrs.push(Attribute::Flag("readonly".to_owned()));
        vec![id]
    }
}

fn seen(marked: &'static str) -> Vec<Option<usize>> {
    let mut program = program();
    let mut analyses = ProgramAnalyses::default();
    analyses.get::<Defined>(&program);
    SEEN.with_borrow_mut(Vec::clear);
    let mut passes = PassManager::default();
    passes.add_module(Mark(marked));
    ProgramPass::run(&mut passes, &mut program, &mut analyses).unwrap();
    SEEN.take()
}

/// LLVM's outer proxy: a program result is read by every module's run
/// until one changes what it exports.
#[test]
fn a_change_to_a_modules_interface_drops_the_program_results() {
    assert_eq!(seen("f"), [Some(3), None]);
    assert_eq!(seen("h"), [Some(3), Some(3)]);
    assert_eq!(program().modules[1].global(GlobalId(1)).linkage, Linkage::Internal);
}

thread_local! {
    static COUNTED: RefCell<usize> = const { RefCell::new(0) };
}

/// How many globals the module holds, counting its computations.
struct Globals;

impl ModuleAnalysis for Globals {
    type Result = usize;
    const NAME: &'static str = "globals";
    fn run(module: &Module, _: &mut ModuleAnalyses) -> usize {
        COUNTED.with_borrow_mut(|one| *one += 1);
        module.globals.len()
    }
}

/// Twice `Globals`, asked of the manager.
struct Twice;

impl ModuleAnalysis for Twice {
    type Result = usize;
    const NAME: &'static str = "twice";
    fn run(module: &Module, analyses: &mut ModuleAnalyses) -> usize {
        2 * *analyses.get::<Globals>(module)
    }
}

/// A module analysis asks another, computed once for both.
#[test]
fn a_module_analysis_asks_another_once() {
    let module = module("define i16 @f() {\nentry:\n  ret i16 0\n}\n");
    let mut analyses = ModuleAnalyses::of(&module, Rc::new(Neutral));
    COUNTED.set(0);
    assert_eq!((*analyses.get::<Twice>(&module), *analyses.get::<Globals>(&module)), (2, 1));
    assert_eq!(COUNTED.take(), 1);
}
