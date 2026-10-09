fn f() {
    let kept = values.iter().copied().filter(|value| {
        // Zero is the sentinel.
        *value != 0
    });
    let trapped = HANDLED.replace("  call cc1000 addrspace(1) void @llrm.qb.onerror(i1 false)\n", "").replace(
        // The handler stores the code first.
        "  store i16 %code, ptr @\"CAUGHT%\"",
        "  store i16 %code, ptr @\"CAUGHT%\"\n  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 2)",
    );
}
