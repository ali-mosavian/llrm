use llrm_z80::expand::{Line, price, price_in_loops};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let asm = std::fs::read_to_string(&args[1]).unwrap();
    let counts: std::collections::HashMap<String, u64> = std::fs::read_to_string(&args[2])
        .unwrap()
        .lines()
        .filter_map(|l| {
            let (a, b) = l.split_once(' ')?;
            Some((a.to_owned(), b.parse().ok()?))
        })
        .collect();
    let mut blocks: Vec<(String, Vec<Line>)> = vec![("<entry>".into(), vec![])];
    let mut all = Vec::new();
    for raw in asm.lines() {
        let text = raw.split(';').next().unwrap().trim_end();
        if text.is_empty() || text.trim_start().starts_with('.') || text.ends_with("::") {
            continue;
        }
        let line = if let Some(label) = text.strip_suffix(':') {
            if label.starts_with('L') && label.contains('_') {
                blocks.push((label.to_owned(), vec![]));
            }
            Line::Label(label.to_owned())
        } else {
            Line::Inst(text.trim().to_owned())
        };
        blocks.last_mut().unwrap().1.push(line.clone());
        all.push(line);
    }
    let (mut weighted, mut unweighted) = (0u64, 0u64);
    for (name, lines) in &blocks {
        let (_, clocks) = price(lines);
        let n = counts.get(name).copied().unwrap_or(if name == "<entry>" { 1 } else { 0 });
        weighted += n * u64::from(clocks);
        unweighted += u64::from(clocks);
    }
    let (bytes, _) = price(&all);
    let (_, loops) = price_in_loops(&all);
    println!("bytes={bytes} measured_count_clocks={weighted} static_once={unweighted} price_in_loops_clocks={loops}");
}
