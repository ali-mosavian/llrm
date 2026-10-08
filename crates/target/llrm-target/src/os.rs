//! The OS layer: one interface (`runtime/shared/interface.toml`) every target implements for every language's
//! runtime. A target ships a `Layer` -- its `os.toml`, the assembly that implements the
//! interface, and the facts of its operating system -- and the languages' bindings are rendered
//! from the interface and the target's pointer kind, so no binding is written by hand.

use std::collections::BTreeMap;

/// The interface's declaration.
pub const INTERFACE: &str = include_str!("../../../../runtime/shared/interface.toml");

/// One operation of the interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Op {
    pub group: String,
    pub name: String,
    pub doc: String,
    pub args: Vec<(String, String)>,
    pub returns: String,
}

/// The interface, parsed.
#[derive(Clone, Debug)]
pub struct Interface {
    pub prefix: String,
    /// The neutral error conditions and their codes.
    pub errors: BTreeMap<String, i64>,
    /// The handles a program starts with; the operating system's facts number them.
    pub handles: Vec<String>,
    pub ops: Vec<Op>,
}

impl Interface {
    pub fn parse(text: &str) -> Result<Self, String> {
        let table: toml::Table = text.parse().map_err(|error: toml::de::Error| error.to_string())?;
        let string = |one: &toml::Table, key: &str| one.get(key).and_then(|value| value.as_str()).map(str::to_owned).ok_or_else(|| format!("the interface lacks the string {key}"));
        let ops = table
            .get("op")
            .and_then(|one| one.as_array())
            .ok_or("the interface has no operations")?
            .iter()
            .map(|one| {
                let one = one.as_table().ok_or("an op is a table")?;
                let args = one
                    .get("args")
                    .and_then(|args| args.as_array())
                    .ok_or("an op has args")?
                    .iter()
                    .map(|pair| {
                        let pair = pair.as_array().filter(|pair| pair.len() == 2).ok_or("an argument is [name, type]")?;
                        Ok((pair[0].as_str().ok_or("an argument's name is a string")?.to_owned(), pair[1].as_str().ok_or("an argument's type is a string")?.to_owned()))
                    })
                    .collect::<Result<_, String>>()?;
                Ok(Op { group: string(one, "group")?, name: string(one, "name")?, doc: string(one, "doc").unwrap_or_default(), args, returns: string(one, "returns")? })
            })
            .collect::<Result<_, String>>()?;
        let errors = table.get("errors").and_then(|one| one.as_table()).ok_or("the interface has no errors")?.iter().map(|(name, code)| Ok((name.clone(), code.as_integer().ok_or("an error code is an integer")?))).collect::<Result<_, String>>()?;
        let handles = table.get("handles").and_then(|one| one.as_array()).ok_or("the interface has no handles")?.iter().map(|one| one.as_str().map(str::to_owned).ok_or_else(|| "a handle is a name".to_owned())).collect::<Result<_, String>>()?;
        Ok(Self { prefix: string(&table, "symbol_prefix")?, errors, handles, ops })
    }

    /// The interface this crate ships.
    pub fn shipped() -> Self {
        Self::parse(INTERFACE).expect("interface.toml parses")
    }

    /// The symbol that implements `op`.
    pub fn symbol(&self, op: &Op) -> String {
        format!("{}{}", self.prefix, op.name)
    }

    /// The operations of the groups a target implements.
    pub fn of_groups<'a>(&'a self, groups: &'a [String]) -> impl Iterator<Item = &'a Op> {
        self.ops.iter().filter(move |op| groups.contains(&op.group))
    }
}

/// A target's OS layer.
#[derive(Clone, Copy, Debug)]
pub struct Layer {
    /// The directory the files are in, for a build step that assembles them.
    pub directory: &'static str,
    /// The target's `os.toml`.
    pub text: &'static str,
    /// The operating system's facts (DOS's function numbers): one file, shared by the targets of
    /// that operating system.
    pub facts: &'static str,
}

impl Layer {
    pub fn table(&self) -> Result<toml::Table, String> {
        self.text.parse().map_err(|error: toml::de::Error| format!("os.toml: {error}"))
    }

    /// The integer field `key` of `os.toml`.
    pub fn integer(&self, key: &str) -> Result<i64, String> {
        self.table()?.get(key).and_then(toml::Value::as_integer).ok_or_else(|| format!("the OS layer has no integer {key}"))
    }

    pub fn string(&self, key: &str) -> Result<String, String> {
        self.table()?.get(key).and_then(|one| one.as_str()).map(str::to_owned).ok_or_else(|| format!("os.toml has no string {key}"))
    }

    /// The integer type a `handle` is on this target: as wide as the operating system's facts say.
    pub fn handle_type(&self) -> Result<&'static str, String> {
        let facts: toml::Table = self.facts.parse().map_err(|error: toml::de::Error| format!("the OS facts: {error}"))?;
        match facts.get("handle_bits").and_then(|one| one.as_integer()) {
            Some(16) => Ok("i16"),
            Some(32) => Ok("i32"),
            other => Err(format!("the OS facts' handle_bits is 16 or 32, not {other:?}")),
        }
    }

    /// The standard handles and their numbers: the interface's names, the operating system's facts.
    pub fn handles(&self) -> Result<Vec<(String, i64)>, String> {
        let facts: toml::Table = self.facts.parse().map_err(|error: toml::de::Error| format!("the OS facts: {error}"))?;
        Interface::shipped().handles.into_iter().map(|name| facts.get(&name).and_then(|one| one.as_integer()).map(|number| (name.clone(), number)).ok_or_else(|| format!("the OS facts lack the handle {name}"))).collect()
    }

    /// The groups of the interface the target implements.
    pub fn groups(&self) -> Result<Vec<String>, String> {
        let table = self.table()?;
        table.get("groups").and_then(|one| one.as_array()).ok_or("os.toml has no groups")?.iter().map(|one| one.as_str().map(str::to_owned).ok_or_else(|| "a group is a string".to_owned())).collect()
    }

    /// What the assembler is told: each integer of the operating system's facts as `<OS>_<KEY>`
    /// and each integer of `os.toml` as `<KEY>`, so the assembly names no constant of its own.
    pub fn defines(&self) -> Result<Vec<(String, String)>, String> {
        let os = self.string("os")?.to_uppercase();
        let facts: toml::Table = self.facts.parse().map_err(|error: toml::de::Error| format!("the OS facts: {error}"))?;
        let integers = |table: &toml::Table, prefix: &str| -> Vec<(String, String)> { table.iter().filter_map(|(key, value)| value.as_integer().map(|number| (format!("{prefix}{}", key.to_uppercase()), number.to_string()))).collect() };
        let mut defines = integers(&facts, &format!("{os}_"));
        defines.extend(integers(&self.table()?, ""));
        Ok(defines)
    }

    /// What a language is told of the layer, one line a field for a build step: its directory, the
    /// start-up and implementation files, the assembler's `SYMBOL=value` definitions (the layer's,
    /// then the language description's), and the binding rendered for `language`.
    pub fn report(&self, language: &crate::runtime::Description, field: &str) -> Result<String, String> {
        match field {
            "directory" => Ok(std::fs::canonicalize(self.directory).map_or_else(|_| self.directory.to_owned(), |path| path.to_string_lossy().into_owned())),
            "start" | "implementation" => self.string(field),
            "defines" => {
                let mut defines = self.defines()?;
                defines.extend(language.defines()?);
                Ok(defines.iter().map(|(symbol, value)| format!("{symbol}={value}")).collect::<Vec<_>>().join(" "))
            }
            "language_file" => Ok(language.string("init_file").ok().map_or_else(String::new, |name| format!("{}/{name}", std::fs::canonicalize(language.directory).map_or_else(|_| language.directory.to_owned(), |path| path.to_string_lossy().into_owned())))),
            "module" => self.nib_module(),
            "header" => self.c_header(),
            other => Err(format!("--os-layer takes directory, start, implementation, defines, language_file, module or header, not {other}")),
        }
    }

    /// The C header of the interface: each operation declared `__cdecl` as `llrm_os_<name>` (C's underscore
    /// makes the symbol), the data pointers far where the target has far data. The layer's routines are
    /// written in the target's C convention whatever its default is.
    pub fn c_header(&self) -> Result<String, String> {
        let interface = Interface::shipped();
        let pointer = self.string("pointer")?;
        let qualifier = if pointer == "far" { "__far " } else { "" };
        let groups = self.groups()?;
        let handle = self.handle_type()?;
        let kind = |name: &str| -> Option<String> {
            Some(match name {
                "handle" => if handle == "i16" { "short" } else { "long" }.to_owned(),
                "bool" => "unsigned char".to_owned(),
                "path" => format!("const char {qualifier}*"),
                "bytes" => format!("const unsigned char {qualifier}*"),
                "bytes_mut" => format!("unsigned char {qualifier}*"),
                "heap" => "unsigned char *".to_owned(),
                "i16" => "short".to_owned(),
                "isize" => "int".to_owned(),
                "i32" => "long".to_owned(),
                "u8" => "unsigned char".to_owned(),
                "usize" => "unsigned".to_owned(),
                "void" => "void".to_owned(),
                _ => return None,
            })
        };
        let mut text = format!("/* The OS layer's interface for this target (runtime/shared/interface.toml), rendered: {pointer} data pointers. */
#ifndef LLRM_OS_H
#define LLRM_OS_H

");
        for op in interface.of_groups(&groups) {
            let types: Option<Vec<String>> = op.args.iter().map(|(_, one)| kind(one)).collect();
            let (Some(types), Some(returns)) = (types, kind(&op.returns)) else { continue };
            let args = if types.is_empty() { "void".to_owned() } else { types.join(", ") };
            text += &format!("{returns} __cdecl llrm_os_{}({args});
", op.name);
        }
        for (name, code) in &interface.errors {
            text += &format!("#define LLRM_OS_{} {code}\n", name.to_uppercase()).replace("\n", "
");
        }
        for (name, number) in self.handles()? {
            text += &format!("#define LLRM_OS_{} {number}\n", name.to_uppercase());
        }
        text += &match pointer.as_str() {
            "far" => "
/* The colour text screen, 80x25 cells of a character and an attribute. */
#define LLRM_OS_TEXT_SCREEN ((unsigned char __far *)((((unsigned long)PHYSICAL_TEXT_SCREEN >> 4) << 16) | ((unsigned long)PHYSICAL_TEXT_SCREEN & 15)))
".to_owned(),
            _ => "
/* The colour text screen, 80x25 cells of a character and an attribute. */
#define LLRM_OS_TEXT_SCREEN ((unsigned char *)PHYSICAL_TEXT_SCREEN)
".to_owned(),
        };
        text += "
#endif
";
        Ok(text)
    }

    /// Nib's `os` module: the interface's operations bound to this target's pointers and
    /// convention, then what a program writes through them.
    pub fn nib_module(&self) -> Result<String, String> {
        let interface = Interface::shipped();
        let convention = self.string("convention")?;
        let pointer = self.string("pointer")?;
        let groups = self.groups()?;
        let handle = self.handle_type()?;
        let kind = |name: &str| -> Result<String, String> {
            Ok(match name {
                "handle" => handle.to_owned(),
                "path" => format!("*{pointer} char"),
                "bytes" => format!("*{pointer} u8"),
                "bytes_mut" => format!("*{pointer} mut u8"),
                "heap" => "*near mut u8".to_owned(),
                "handler" => "extern \"interrupt16\" fn() -> void".to_owned(),
                "i16" | "i32" | "u8" | "usize" | "isize" | "bool" | "void" => name.to_owned(),
                other => return Err(format!("the interface has a type {other} the Nib binding does not know")),
            })
        };
        let mut text = format!(
            "# The operating system under this target: what only assembly reaches. Rendered from the OS layer's interface\n# (runtime/shared/interface.toml) for {pointer} data pointers, {convention}. A call returns its result,\n# or its error code negated.\n\n"
        );
        for op in interface.of_groups(&groups) {
            for line in op.doc.lines() {
                text += &format!("# {line}\n");
            }
            let args = op.args.iter().map(|(name, one)| Ok(format!("{name}: {}", kind(one)?))).collect::<Result<Vec<_>, String>>()?.join(", ");
            text += &format!("@extern(\"{convention}\", name=\"{}\")\npub fn {}({args}) -> {}\n", interface.symbol(op), op.name, kind(&op.returns)?);
        }
        text += "\n# The error codes the interface names.\n";
        for (name, code) in &interface.errors {
            text += &format!("pub const {}: i16 = {code}\n", name.to_uppercase());
        }
        text += "\n# The handles a program starts with.\n";
        for (name, number) in self.handles()? {
            text += &format!("pub const {}: {handle} = {number}\n", name.to_uppercase());
        }
        text += &format!("\n# `length` bytes of `text` to standard output.\npub fn write(text: *{pointer} u8, length: usize) -> void:\n    unsafe:\n        write_file(STDOUT, text, length)\n\n");
        text += "# The colour text screen: 80x25 cells of a character and an attribute, at the machine's physical address\n# (the target's description names it).\n";
        match pointer.as_str() {
            "far" => {
                text += "# Real mode reaches memory outside its data segment by a far pointer: segment = address >> 4, offset = address & 15.\n";
                text += "const SCREEN_FAR = ((PHYSICAL_TEXT_SCREEN >> 4) << 16) | (PHYSICAL_TEXT_SCREEN & 15)\n\npub fn text_screen() -> *far mut u8:\n    unsafe:\n        let screen: *far mut u8 = SCREEN_FAR\n        return screen\n";
            }
            "near" => {
                text += "# Its physical address is a linear address in the flat, zero-based space, so it is the pointer as it is.\nconst SCREEN = PHYSICAL_TEXT_SCREEN\n\npub fn text_screen() -> *mut u8:\n    unsafe:\n        let screen: *mut u8 = SCREEN\n        return screen\n";
            }
            other => return Err(format!("os.toml: pointer is far or near, not {other}")),
        }
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAYER: Layer = Layer { directory: "/x", text: "os = \"dos\"\nconvention = \"cdecl32\"\npointer = \"near\"\ngroups = [\"core\"]\nheap_bytes = 4096\n", facts: "open = 0x3D\nint = 0x21\nhandle_bits = 16\nstdin = 0\nstdout = 1\nstderr = 2\n" };

    #[test]
    fn the_shipped_interface_parses_and_every_op_is_in_a_group_with_a_known_type() {
        let interface = Interface::shipped();
        assert!(interface.ops.iter().any(|op| op.name == "open" && op.group == "core"));
        for groups in [&["core".to_owned()][..], &["core".to_owned(), "vectors".to_owned()][..]] {
            let layer = Layer { text: &*Box::leak(format!("os = \"x\"\nconvention = \"cdecl16\"\npointer = \"far\"\ngroups = {groups:?}\n").into_boxed_str()), ..LAYER };
            layer.nib_module().expect("every type of the interface renders");
        }
    }

    #[test]
    fn the_assembler_is_told_the_os_facts_prefixed_and_the_layers_own_fields() {
        let told: Vec<(String, String)> = [("DOS_OPEN", "61"), ("DOS_INT", "33"), ("DOS_HANDLE_BITS", "16"), ("DOS_STDIN", "0"), ("DOS_STDOUT", "1"), ("DOS_STDERR", "2"), ("HEAP_BYTES", "4096")].into_iter().map(|(name, value)| (name.to_owned(), value.to_owned())).collect();
        assert_eq!(LAYER.defines().unwrap(), told);
    }

    #[test]
    fn the_c_header_declares_each_operation_with_the_targets_pointers() {
        let flat = LAYER.c_header().unwrap();
        assert!(flat.contains("short __cdecl llrm_os_open(const char *, unsigned char);") && flat.contains("long __cdecl llrm_os_read(short, unsigned char *, unsigned);"), "{flat}");
        let far = Layer { text: "os = \"dos\"\nconvention = \"cdecl16\"\npointer = \"far\"\ngroups = [\"core\", \"vectors\"]\n", ..LAYER };
        let text = far.c_header().unwrap();
        assert!(text.contains("short __cdecl llrm_os_open(const char __far *, unsigned char);") && !text.contains("llrm_os_vector"), "{text}");
    }

    #[test]
    fn a_flat_binding_names_near_pointers_and_a_far_one_far() {
        assert!(LAYER.nib_module().unwrap().contains("pub fn open(name: *near char, mode: u8) -> i16"));
        let far = Layer { text: "os = \"dos\"\nconvention = \"cdecl16\"\npointer = \"far\"\ngroups = [\"core\"]\n", ..LAYER };
        assert!(far.nib_module().unwrap().contains("@extern(\"cdecl16\", name=\"_llrm_os_open\")\npub fn open(name: *far char, mode: u8) -> i16"));
    }

    /// A handle was spelled `i16` in the interface, a target's width in a shared file: it takes the
    /// width the operating system's facts give, and the standard handles their numbers.
    #[test]
    fn a_handle_is_as_wide_as_the_facts_say_and_the_standard_handles_are_theirs() {
        let wide = Layer { facts: "handle_bits = 32\nstdin = 0\nstdout = 1\nstderr = 2\n", ..LAYER };
        let module = wide.nib_module().unwrap();
        assert!(module.contains("pub fn open(name: *near char, mode: u8) -> i32") && module.contains("pub fn read(handle: i32,"), "{module}");
        assert!(module.contains("pub const STDOUT: i32 = 1") && module.contains("write_file(STDOUT, text, length)"), "{module}");
        assert!(wide.c_header().unwrap().contains("long __cdecl llrm_os_open(const char *, unsigned char);"));
        assert!(wide.c_header().unwrap().contains("#define LLRM_OS_STDERR 2"));
        let narrow = Layer { facts: "handle_bits = 16\nstdin = 0\nstdout = 1\nstderr = 2\n", ..LAYER };
        assert!(narrow.nib_module().unwrap().contains("pub fn read(handle: i16,"));
        let lacking = Layer { facts: "handle_bits = 16\n", ..LAYER };
        assert!(lacking.nib_module().is_err(), "a standard handle the facts do not number is an error");
    }
}
