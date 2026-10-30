//! Prints the dominator and post-dominator tree of every function of the given
//! modules, in the format of `tools/llvmport/domdump.cpp`: blocks are named by
//! their position in the function. usage: domdump FILE.ll...

use llrm_mir::domtree::DomTreeBase;
use llrm_mir::module::{BlockId, Function, GlobalKind};
use llrm_mir::parse;

fn dump<const POST: bool>(
    tag: &str,
    tree: &DomTreeBase<POST>,
    function: &Function,
) {
    let index = |block: BlockId| function.layout().iter().position(|one| *one == block).expect("a placed block");
    let list = |mut blocks: Vec<usize>| {
        blocks.sort_unstable();
        blocks.iter().map(|one| format!("{one},")).collect::<String>()
    };
    let line = |name: String, block: Option<BlockId>| {
        let Some(level) = block.map_or(Some(0), |one| tree.level(one)) else {
            println!("{tag} {name} x");
            return;
        };
        let idom = match block.and_then(|one| tree.immediate_dominator(one)) {
            Some(above) => index(above).to_string(),
            None if POST && block.is_some() => "V".to_string(),
            None => "-".to_string(),
        };
        let kids = list(tree.children(block).into_iter().map(index).collect());
        println!("{tag} {name} {idom} {level} {kids}");
    };
    if POST {
        line("V".to_string(), None);
    }
    for (at, &block) in function.layout().iter().enumerate() {
        line(at.to_string(), Some(block));
    }
    println!("{tag} roots {}", list(tree.roots().iter().map(|one| index(*one)).collect()));
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("llrm-mir domdump");
        return;
    }
    for path in std::env::args().skip(1) {
        let text = std::fs::read_to_string(&path).expect("a readable file");
        let module = parse::module(&text).unwrap_or_else(|why| panic!("{path}: {why}"));
        for global in &module.globals {
            let GlobalKind::Function(function) = &global.kind else { continue };
            if function.is_declaration() {
                continue;
            }
            println!("@{}", global.name.as_deref().unwrap_or(""));
            let (mut dt, mut pdt) = (llrm_mir::domtree::DomTree::new(), llrm_mir::domtree::PostDomTree::new());
            dt.recalculate(&**function);
            pdt.recalculate(&**function);
            dump("dt", &dt, function);
            dump("pdt", &pdt, function);
        }
    }
}
