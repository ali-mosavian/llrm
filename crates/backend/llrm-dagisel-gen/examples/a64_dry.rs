//! Dry run: decode and prune the pin's AArch64GenDAGISel.inc. Usage: a64_dry INC IDS FEATURES CPU
use llrm_dagisel_gen::{decode, features, flatten, inc, prune};

fn main() -> Result<(), String> {
    let a: Vec<String> = std::env::args().collect();
    let read = |p: &str| std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"));
    let ids = inc::Ids::parse(&read(&a[2])?)?;
    let parsed = inc::parse(&read(&a[1])?, &ids)?;
    eprintln!(
        "table {} bytes; {} pattern predicates, {} node predicates, {} complex patterns, {} xforms",
        parsed.matcher_table.len(),
        parsed.pattern_predicates.len(),
        parsed.node_predicates.len(),
        parsed.complex_patterns.len(),
        parsed.xforms.len()
    );
    let i64 = llrm_dagisel::matcher::Vt(ids.get("MVT::i64")? as u16);
    let decoded = decode::decode(&parsed.matcher_table, &parsed.operand_lists, &ids, &decode::HwModes(vec![i64, i64]))?;
    eprintln!("decoded");
    let cpu = features::Cpu::load(&read(&a[3])?, &a[4])?;
    let mut pruner = prune::Pruner::new(&cpu, &parsed.pattern_predicates, &ids)?;
    let kept = pruner.prune(&decoded.root);
    let flat = flatten::flatten(&kept, &decoded.lists);
    eprintln!("{}: {} items, {} operand entries", a[4], flat.items.len(), flat.operands.len());
    if let Some(path) = a.get(5) {
        let mut out = String::new();
        for item in &flat.items {
            match item {
                llrm_dagisel::matcher::Matcher::EmitNode(e) | llrm_dagisel::matcher::Matcher::MorphNodeTo(e) => {
                    out.push_str(&format!("{}\n", e.opcode))
                }
                _ => {}
            }
        }
        std::fs::write(path, out).map_err(|e| e.to_string())?;
    }
    Ok(())
}
