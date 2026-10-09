//! A target's OS layer as a language's runtime needs it: the start-up and
//! operating-system routines in assembly, the language's own module over them,
//! what its stack check compares. Each target crate ships one description per
//! language, `runtime/<language>/<language>.toml`, with the files it names
//! embedded; a language reads what it needs and never asks which target.

/// One language's runtime on one target.
#[derive(Clone, Copy, Debug)]
pub struct Description {
    /// The directory the description and its files are in, for a build step
    /// that assembles them.
    pub directory: &'static str,
    /// The description, TOML.
    pub text: &'static str,
    /// Each file it names, embedded: its name and its text.
    pub files: &'static [(&'static str, &'static str)],
}

impl Description {
    /// The text of file `name`, which the description's fields name.
    pub fn file(
        &self,
        name: &str,
    ) -> Option<&'static str> {
        self.files.iter().find(|(one, _)| *one == name).map(|(_, text)| *text)
    }

    /// The description parsed.
    pub fn table(&self) -> Result<toml::Table, String> {
        self.text.parse().map_err(|error: toml::de::Error| error.to_string())
    }

    /// What the assembler is told of the description: `assembler_defines =
    /// ["field:SYMBOL"]`, each field an integer or a string of the
    /// description, as `SYMBOL` and its value.
    pub fn defines(&self) -> Result<Vec<(String, String)>, String> {
        let table = self.table()?;
        table
            .get("assembler_defines")
            .and_then(|one| one.as_array())
            .into_iter()
            .flatten()
            .map(|entry| {
                let (field, symbol) = entry
                    .as_str()
                    .and_then(|one| one.split_once(':'))
                    .ok_or("assembler_defines are \"field:SYMBOL\"")?;
                let value = match table.get(field) {
                    Some(toml::Value::Integer(number)) => number.to_string(),
                    Some(toml::Value::String(text)) => text.clone(),
                    _ => return Err(format!("{field} is not an integer or a string")),
                };
                Ok((symbol.to_owned(), value))
            })
            .collect()
    }

    /// The string field `key` of the description.
    pub fn string(
        &self,
        key: &str,
    ) -> Result<String, String> {
        self.table()?
            .get(key)
            .and_then(|one| one.as_str())
            .map(str::to_owned)
            .ok_or_else(|| format!("the runtime description has no string {key}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: Description = Description {
        directory: "/x",
        text: "os = \"os.nib\"\nfar_bss = false\n",
        files: &[("os.nib", "pub fn f() -> void: pass")],
    };

    #[test]
    fn a_description_names_its_files_and_they_are_embedded() {
        assert_eq!(ONE.string("os").unwrap(), "os.nib");
        assert_eq!(ONE.file("os.nib"), Some("pub fn f() -> void: pass"));
        assert!(ONE.string("start").is_err() && ONE.file("start.asm").is_none());
    }
}
