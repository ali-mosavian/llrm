use llrm_m68k::compile::{Item, items};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::{amiga, atari, mac, object};

/// `images IR DIR CALLEE ARGS..`: the program calling `CALLEE(ARGS)` as an
/// Atari PRG, an Amiga hunk file, a Mac resource fork and a Linux executable in
/// `DIR`.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let module = llrm_mir::parse::module(&std::fs::read_to_string(&args[1]).unwrap()).unwrap();
    let numbers: Vec<i64> = args[4..].iter().map(|one| one.parse().unwrap()).collect();
    let mut functions: Vec<Item> = Vec::new();
    for (_, global, function) in module.functions() {
        if !function.is_declaration() {
            functions.extend(items(&module, global.name.as_deref().unwrap(), Cpu::M68000).unwrap());
        }
    }
    let with = |start: Vec<Item>| {
        start
            .into_iter()
            .chain(functions.iter().map(|one| match one {
                Item::Label(name) => Item::Label(name.clone()),
                Item::Inst(inst) => Item::Inst(inst.clone()),
                Item::Bytes(bytes) => Item::Bytes(bytes.clone()),
            }))
            .collect::<Vec<_>>()
    };
    let dir = std::path::Path::new(&args[2]);
    std::fs::write(dir.join("a.prg"), atari::program(&with(atari::start(&args[3], &numbers))).unwrap()).unwrap();
    std::fs::write(dir.join("a.hunk"), amiga::load_file(&with(amiga::start(&args[3], &numbers))).unwrap()).unwrap();
    std::fs::write(dir.join("a.rsrc"), mac::resource_file(&with(mac::start(&args[3], &numbers))).unwrap()).unwrap();
    let exe = object::executable(&with(object::start(&args[3], &numbers))).unwrap();
    object::write_executable(&dir.join("a.elf"), &exe).unwrap();
}
