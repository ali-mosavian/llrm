fn f() {
    let n = items.iter().filter(|x| x.is_ok()).count();
    self.items.retain(|item| {
        let keep = item.live();
        keep && item.used() && item.uses.iter().all(|u| u.block.is_reachable())
    });
    let only = list
        .first()
        .map(|first| first.value.unwrap_or_default().saturating_add(offset).saturating_mul(scale_factor_here));
}
