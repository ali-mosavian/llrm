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
/// DPMI call as literals, once per target. They are the OS's facts, said once (llrm-x86/os/dos.toml).
#[test]
fn test_no_os_assembly_names_an_os_function_number() {
    let literal = regex::Regex::new(r"(?i)\b(mov\s+(ah|al|eax)\s*,\s*[0-9][0-9a-f]*h\b|int\s+[0-9][0-9a-f]*h\b)").unwrap();
    for target in targets() {
        for field in ["start", "implementation"] {
            let text = source(&*target, field);
            let code: Vec<&str> = text.lines().map(|line| line.split(';').next().unwrap()).filter(|line| literal.is_match(line)).collect();
            assert!(code.is_empty(), "{} {field} names an OS fact: {code:?}", target.name());
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
