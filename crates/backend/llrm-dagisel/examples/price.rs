//! Prices the interpreter: selects the same few-node DAGs over and over.
//!
//!     cargo run --release --example price -p llrm-dagisel -- 2000
//!
//! Run it under `valgrind --tool=callgrind
//! --toggle-collect='*Selector*select*'` to read the instructions per selected
//! node; the DAGs are small so the test DAG's scans stay cheap.

use llrm_dagisel::dag::{LoadExt, SDValue};
use llrm_dagisel::generated::ids::{isd, mvt};
use llrm_dagisel::generated::x86_i486 as table;
use llrm_dagisel::interp::{Outcome, Scratch, Selector};
use llrm_dagisel::testing::{Node, TestDag, TestHooks};

fn main() {
    let rounds: usize = std::env::args().nth(1).and_then(|n| n.parse().ok()).unwrap_or(1000);
    let selector = Selector::new(table::TABLE);
    let mut selected = 0usize;
    let mut scratch = Scratch::default();
    for _ in 0..rounds {
        let mut dag = TestDag::new();
        let a = dag.input(1, mvt::i32);
        let b = dag.input(2, mvt::i32);
        let c = dag.make_constant(40, mvt::i32);
        let shl_amount = dag.make_constant(3, mvt::i8);
        let address = SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[a, b]), 0);
        let load = dag.add(Node {
            opcode: isd::LOAD,
            vts: vec![mvt::i32, mvt::Other],
            ops: vec![SDValue::new(0, 0), address, SDValue::new(0, 0)],
            load: Some(LoadExt::NonExt),
            memory_vt: Some(mvt::i32),
            ..Node::default()
        });
        let nodes = [
            dag.node(isd::ADD, &[mvt::i32], &[a, b]),
            dag.node(isd::SUB, &[mvt::i32], &[a, c]),
            dag.node(isd::AND, &[mvt::i32], &[a, b]),
            dag.node(isd::OR, &[mvt::i32], &[a, c]),
            dag.node(isd::XOR, &[mvt::i32], &[a, b]),
            dag.node(isd::MUL, &[mvt::i32], &[a, b]),
            dag.node(isd::SHL, &[mvt::i32], &[a, shl_amount]),
            load,
        ];
        let mut hooks = TestHooks { address: Some((a, b, 1, 0)) };
        for node in nodes {
            if selector.select_with(&mut scratch, &mut dag, &mut hooks, node) == Outcome::Selected {
                selected += 1;
            }
        }
    }
    let items = table::TABLE.items.len();
    let item = std::mem::size_of::<llrm_dagisel::matcher::Matcher>();
    println!(
        "table: {items} items x {item} bytes = {} bytes, {} operand bytes",
        std::mem::size_of_val(table::TABLE.items),
        std::mem::size_of_val(table::TABLE.operands)
    );
    println!("{selected} of {} nodes selected", rounds * 8);
}
