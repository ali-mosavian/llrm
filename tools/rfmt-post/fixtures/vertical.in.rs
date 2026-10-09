fn f() {
    let total = self
        .blocks
        .iter()
        .map(|block| {
            let size = block.insns.len();
            size + block.phis.len() + block.params.len()
        })
        .sum::<usize>();
}
