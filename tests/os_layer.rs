//! The OS layer: every target implements the whole interface it declares, from the interface's own
//! declaration, and no implementation carries an OS fact of its own.

use llrm_target::os::Interface;
use llrm_target::Target;

fn targets() -> Vec<Box<dyn Target>> {
    vec![Box::new(llrm_x86_code16::Code16), Box::new(llrm_x86_code32::Code32)]
}

fn source(target: &dyn Target, field: &str) -> String {
    let layer = target.os_layer().expect("a target has an OS layer");
    let name = layer.string(field).unwrap();
    std::fs::read_to_string(format!("{}/{name}", layer.directory)).unwrap()
}

/// Every operation of every group a target declares is a public symbol of its implementation, and
/// the Nib binding rendered for it names that symbol: an operation added to interface.toml fails
/// here for each target that declares its group and has not written it.
#[test]
fn test_every_target_implements_every_operation_of_the_groups_it_declares() {
    let interface = Interface::shipped();
    for target in targets() {
        let layer = target.os_layer().unwrap();
        let groups = layer.groups().unwrap();
        assert!(groups.contains(&"core".to_owned()), "{} lacks the core group", target.name());
        let implementation = source(&*target, "implementation");
        let binding = layer.nib_module().unwrap();
        let publics: Vec<&str> = implementation.lines().filter_map(|line| line.trim().strip_prefix("public ")).collect();
        let mut count = 0;
        for op in interface.of_groups(&groups) {
            let symbol = interface.symbol(op);
            assert!(publics.contains(&symbol.as_str()), "{} does not implement {} ({symbol})", target.name(), op.name);
            assert!(binding.contains(&format!("name=\"{symbol}\"")), "{}'s Nib binding lacks {}", target.name(), op.name);
            count += 1;
        }
        assert!(count >= 7, "premise: the core's operations are found ({count})");
        let known: Vec<&str> = interface.ops.iter().map(|op| op.group.as_str()).collect();
        assert!(groups.iter().all(|group| known.contains(&group.as_str())), "{} declares a group the interface lacks", target.name());
    }
}

/// start.asm and os.asm each wrote DOS's function numbers (`mov ah, 3Dh`, `int 21h`) and the
/// DPMI call as literals, once per target. They are the OS's facts, said once (runtime/shared/dos/facts.toml).
#[test]
fn test_no_os_assembly_names_an_os_function_number() {
    let literal = regex::Regex::new(r"(?i)\b(mov\s+(ah|al|eax)\s*,\s*[0-9][0-9a-f]*h\b|int\s+[0-9][0-9a-f]*h\b)").unwrap();
    let check = |name: String, text: String| {
        let code: Vec<&str> = text.lines().map(|line| line.split(';').next().unwrap()).filter(|line| literal.is_match(line)).collect();
        assert!(code.is_empty(), "{name} names an OS fact: {code:?}");
    };
    for target in targets() {
        for field in ["start", "implementation"] {
            check(format!("{} {field}", target.name()), source(&*target, field));
        }
        // The languages' own assembly reaches the OS through the layer too: C's externals and both hooks.
        for language in ["nib", "c"] {
            let description = target.runtime(language).unwrap();
            for file in ["ext.asm", "init.asm"] {
                if let Ok(text) = std::fs::read_to_string(format!("{}/{file}", description.directory)) {
                    check(format!("{} {language} {file}", target.name()), text);
                }
            }
        }
    }
}

/// Each language's runtime on each target names what the layer and its own files define: the stack check's
/// limit is a public of the layer, its handler of the language's externals, and a start-up hook is defined by
/// the file that names it and handed to the assembler as LANG_INIT. A language that named a word the
/// start-up never fills would compare with zero.
#[test]
fn test_each_languages_runtime_names_symbols_that_are_defined() {
    for target in targets() {
        let implementation = source(&*target, "implementation");
        for language in ["nib", "c"] {
            let description = target.runtime(language).unwrap();
            let table = description.table().unwrap();
            let stack: toml::Table = description.file(&description.string("stack").unwrap()).unwrap().parse().unwrap();
            let limit = stack["limit"].as_str().unwrap();
            assert!(implementation.contains(&format!("public {limit}")), "{} {language}: the layer does not define {limit}", target.name());
            if language == "c" {
                let externals = std::fs::read_to_string(format!("{}/ext.asm", description.directory)).unwrap();
                assert!(externals.contains(&format!("public {}", stack["handler"].as_str().unwrap())), "{} c: no handler", target.name());
            }
            if let Some(hook) = table.get("init").and_then(|one| one.as_str()) {
                let file = std::fs::read_to_string(format!("{}/{}", description.directory, description.string("init_file").unwrap())).unwrap();
                assert!(file.contains(&format!("public {hook}")), "{} {language}: init_file does not define {hook}", target.name());
                assert!(description.defines().unwrap().contains(&("LANG_INIT".to_owned(), hook.to_owned())), "{} {language}: LANG_INIT is not defined for the assembler", target.name());
            }
        }
    }
}

/// The layer returns DOS's own codes, so the interface's number for each condition must be DOS's.
#[test]
fn test_the_interfaces_error_codes_are_the_operating_systems_own() {
    let facts: toml::Table = llrm_x86::DOS_FACTS.parse().unwrap();
    let dos = facts["errors"].as_table().unwrap();
    for (name, code) in &Interface::shipped().errors {
        assert_eq!(dos[name].as_integer(), Some(*code), "{name}");
    }
}

/// The console's operations and the standard handles are the layer's: a program asks the key calls, and
/// reads and writes the console as `read` and `write_file` on the handles the facts number. The binding
/// carries those numbers, so no runtime spells a literal 1 for standard output.
#[test]
fn test_the_console_and_its_standard_handles_come_from_the_layer() {
    let facts: toml::Table = llrm_x86::DOS_FACTS.parse().unwrap();
    for target in targets() {
        let layer = target.os_layer().unwrap();
        assert!(layer.groups().unwrap().contains(&"console".to_owned()), "{} lacks the console group", target.name());
        for (name, number) in layer.handles().unwrap() {
            assert_eq!(facts[&name].as_integer(), Some(number));
        }
        let binding = layer.nib_module().unwrap();
        assert!(binding.contains("pub const STDOUT: i16 = 1") && binding.contains("pub fn console_read_key() -> u8") && binding.contains("pub fn console_key_ready() -> bool"), "{binding}");
        assert!(layer.c_header().unwrap().contains("#define LLRM_OS_STDIN 0"));
        // close's status was `i16`, a width written in the shared interface; it is the target's own word.
        assert!(binding.contains("pub fn close(handle: i16) -> isize"), "{binding}");
        for (language, file) in [("c", "ext.asm"), ("nib", "init.asm")] {
            if let Ok(text) = std::fs::read_to_string(format!("{}/{file}", target.runtime(language).unwrap().directory)) {
                let code: Vec<&str> = text.lines().map(|line| line.split(';').next().unwrap().trim()).collect();
                let literal = code.windows(2).any(|pair| pair[0] == "push 1" && pair[1].contains("_llrm_os_write_file"));
                assert!(!literal, "{} {language} writes to a literal handle 1", target.name());
            }
        }
    }
}

/// The host interpreter's console was a literal handle 1 and had no input: standard output is the OS facts'
/// stdout, and `read` on their stdin, `console_read_key` and `console_key_ready` take what the run is fed.
#[test]
fn test_the_interpreters_console_follows_the_standard_handles() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut run = Command::new(env!("CARGO_BIN_EXE_llrm-run")).arg(root.join("tests/run/nib/os_console_in.nib")).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    run.stdin.take().unwrap().write_all(&std::fs::read(root.join("tests/run/nib/conin.dat")).unwrap()).unwrap();
    let done = run.wait_with_output().unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let want = std::fs::read_to_string(root.join("tests/run/nib/os_console_in.out")).unwrap();
    assert_eq!(String::from_utf8_lossy(&done.stdout).replace("\r\n", "\n"), want);
}
