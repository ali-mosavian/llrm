use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::kernels::{library_table, source_in, table};

/// `schedcmp OUTDIR`: every bench function's assembly at speed, size and pinned.
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let mut sets: Vec<(&str, &str)> = table().into_iter().map(|(n, ..)| ("kernels", n)).collect();
    sets.extend(library_table().into_iter().map(|(n, ..)| ("library", n)));
    let cpu = if std::env::var("CPU").is_ok_and(|c| c == "m68020") { Cpu::M68020 } else { Cpu::M68000 };
    for (set, name) in sets {
        let mut text = source_in(set, name);
        if set == "library" {
            text.push_str(source_in(set, "rt").split_once('\n').map_or("", |(_, r)| r));
            text.push_str(source_in(set, "h").split_once('\n').map_or("", |(_, r)| r));
        }
        let text = if text.contains("target datalayout") {
            text
        } else {
            format!("target datalayout = \"{}\"\n{text}", llrm_m68k::kernels::DATALAYOUT)
        };
        let Ok(module) = llrm_mir::parse::module(&text) else { eprintln!("{name}: does not parse"); continue };
        let funcs: Vec<String> = module
            .functions()
            .filter(|(_, _, f)| !f.is_declaration())
            .filter_map(|(_, g, _)| g.name.as_deref().map(str::to_owned))
            .collect();
        for f in funcs {
            for (label, options) in [
                ("speed", Options::default()),
                ("size", Options { size: true, ..Options::default() }),
                ("pinned", Options::PINNED),
            ] {
                let out = match text_with(&module, &f, cpu, options) {
                    Ok(t) => t,
                    Err(e) => format!("ERROR {e}\n"),
                };
                std::fs::write(dir.join(format!("{set}.{name}.{f}.{label}.s")), out).unwrap();
            }
        }
    }
}
