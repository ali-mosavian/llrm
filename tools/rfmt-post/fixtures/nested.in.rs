fn f() {
    let found = functions.iter().find_map(|function| {
        let seen = function.blocks.iter().any(|block| {
            let first = block.insns.first()?;
            first.reads_memory() && first.is_volatile_access()
        });
        function.users.iter().map(|u| u.weights.iter().map(|w| w.scaled_by(seen)).sum::<u64>()).max()
    });
}
