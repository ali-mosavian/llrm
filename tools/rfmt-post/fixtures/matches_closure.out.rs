fn f() {
    let frame = one
        .what
        .as_ref()
        .is_some_and(
            |what| what.dests
                .iter()
                .chain(&what.sources)
                .any(
                    |place| matches!(
                        place,
                        Loc::Mem(mem) if mem.addr.is_some_and(|addr| addr.space == Space::Frame)
                    ),
                ),
        );
}
