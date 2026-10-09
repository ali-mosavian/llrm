fn f() {
    let found = blocks.iter().enumerate().find_map(|(index, block)| {
        let first = block.insns.first()?;
        (first.opcode == wanted).then_some(index)
    });
    let sum = values.iter().fold(0u64, |acc, value| {
        let wide = u64::from(*value);
        acc.wrapping_mul(31).wrapping_add(wide)
    });
}
