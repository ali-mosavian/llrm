//! nodes OBJ: each body's decoded nodes, one per line.
use llrm_bcmachine::model::ir::decode;
use llrm_bcmachine::model::ir::nodes::{Node, span};

fn main() {
    let path = std::env::args().nth(1).expect("an object");
    let found = llrm_omf::module::load(std::path::PathBuf::from(&path)).unwrap().unwrap();
    println!("calls {:?}", found.calls);
    for body in decode::decode_module(&found).unwrap() {
        println!("== {:?} {:?} seed {:#x} {:?}", body.body.kind, body.body.name, body.body.seed, body.body.ranges);
        for node in &body.nodes {
            let (lo, hi) = span(node);
            let what = match &**node {
                Node::Opaque(one) => format!("{}", one.insn.insn),
                Node::Long(one) => format!("{} (long)", one.insn.insn),
                Node::Call(one) => format!("{} call {}", one.insn.insn, one.name),
                Node::Restore(one) => format!("restore pair {}", one.pair),
                Node::Data(one) => format!("data {:?}", one.kind),
            };
            let s = node.semantics();
            println!("  {lo:#06x}..{hi:#06x} {what:40} {} {:?} <- {:?}", s.op, s.dests, s.sources);
        }
    }
}
