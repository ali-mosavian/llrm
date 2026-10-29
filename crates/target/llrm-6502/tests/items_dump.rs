use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::select::function_items;

#[test]
fn dump() {
    let (name, path) = (std::env::var("K").unwrap(), std::env::var("LL").unwrap());
    let text = std::fs::read_to_string(path).unwrap();
    let text: String = text.lines().filter(|l| !l.starts_with("source_filename")).collect::<Vec<_>>().join("\n");
    let text = text
        .lines()
        .filter(|l| {
            !l.starts_with(';') && !l.starts_with('!') && !l.starts_with("attributes") && !l.starts_with("declare")
        })
        .map(|l| {
            let mut l = l.split(" ; ").next().unwrap().to_owned();
            for w in [
                "dso_local ",
                "zeroext ",
                "noundef ",
                "local_unnamed_addr ",
                " #0",
                " nuw",
                " nsw",
                ", !llvm.loop !2",
                "nonnull ",
                "readonly ",
                "nocapture ",
            ] {
                l = l.replace(w, "");
            }
            l
        })
        .collect::<Vec<_>>()
        .join("\n");
    let module = llrm_mir::parse::module(&text).unwrap();
    let sel = select_function(&module, &name, Cpu::Mos6502).unwrap();
    let (items, _) = function_items(&module, &name, &sel).unwrap();
    for i in items {
        println!("ITEM {i:?}");
    }
}
