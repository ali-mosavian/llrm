use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use llrm::driver;

fn main() -> ExitCode {
    match Invocation::parse(env::args().skip(1)) {
        Ok(invocation) => invocation.run(),
        Err(message) => {
            eprintln!("llrm-c: {message}");
            ExitCode::from(2)
        }
    }
}

struct Invocation {
    input: PathBuf,
    output: Option<PathBuf>,
    output_kind: OutputKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputKind {
    Ir,
    Machine,
    Object,
}

impl Invocation {
    fn parse(arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut arguments = arguments.peekable();
        let mut input = None;
        let mut output = None;
        let mut output_kind = None;
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "-o" => output = Some(PathBuf::from(required(&mut arguments, "-o")?)),
                "--emit" => {
                    let value = required(&mut arguments, "--emit")?;
                    let kind = match value.as_str() {
                        "qir" => OutputKind::Ir,
                        "qmir" => OutputKind::Machine,
                        "obj" => OutputKind::Object,
                        _ => return Err(format!("unknown output kind {value:?}")),
                    };
                    if output_kind.replace(kind).is_some() {
                        return Err("--emit may only be specified once".to_owned());
                    }
                }
                "-h" | "--help" => return Err(usage().to_owned()),
                _ if argument.starts_with('-') => {
                    return Err(format!("unknown option {argument:?}\n{}", usage()));
                }
                _ => {
                    if input.replace(PathBuf::from(argument)).is_some() {
                        return Err("expected exactly one input path".to_owned());
                    }
                }
            }
        }
        let input = input.ok_or_else(|| usage().to_owned())?;
        if !input
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("cgs"))
        {
            return Err("WCC capture input must have a .cgs extension".to_owned());
        }
        Ok(Self {
            input,
            output,
            output_kind: output_kind.unwrap_or(OutputKind::Ir),
        })
    }

    fn run(self) -> ExitCode {
        let source = match fs::read_to_string(&self.input) {
            Ok(source) => source,
            Err(error) => return failure(format!("{}: {error}", self.input.display())),
        };
        let module_name = self
            .input
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("module");
        let bytes = match compile_output(&source, module_name, self.output_kind) {
            Ok(bytes) => bytes,
            Err(error) => return failure(format!("{}: {error}", self.input.display())),
        };
        write_output(self.output.as_deref(), &bytes)
    }
}

fn compile_output(
    source: &str,
    module_name: &str,
    output_kind: OutputKind,
) -> Result<Vec<u8>, driver::Error> {
    let module = driver::compile_wcc_capture(source, module_name)?;
    match output_kind {
        OutputKind::Ir => Ok(llrm::ir::write_text(&module).into_bytes()),
        OutputKind::Machine => {
            let machine = driver::lower_ir_to_machine(&module)?;
            Ok(llrm::codegen::machine::write_text(&machine).into_bytes())
        }
        OutputKind::Object => {
            let machine = driver::lower_c_to_machine(&module)?;
            let mc = driver::lower_c_machine_to_mc(&machine)?;
            let encoded = driver::encode_x86_mc(&mc)?;
            driver::write_c_omf(module_name.as_bytes(), &encoded)
        }
    }
}

fn write_output(path: Option<&Path>, bytes: &[u8]) -> ExitCode {
    match path {
        Some(path) => match fs::write(path, bytes) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => failure(format!("{}: {error}", path.display())),
        },
        None => match io::stdout().write_all(bytes) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => failure(format!("stdout: {error}")),
        },
    }
}

fn required(arguments: &mut impl Iterator<Item = String>, option: &str) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn failure(message: String) -> ExitCode {
    eprintln!("llrm-c: {message}");
    ExitCode::FAILURE
}

fn usage() -> &'static str {
    "usage: llrm-c [--emit qir|qmir|obj] [-o FILE] INPUT.cgs"
}

#[cfg(test)]
mod tests {
    use super::{Invocation, OutputKind, compile_output};
    use llrm::driver;

    #[test]
    fn accepts_only_wcc_capture_input_and_supported_outputs() {
        assert!(Invocation::parse(["program.cgs".to_owned()].into_iter()).is_ok());
        assert!(Invocation::parse(["program.bas".to_owned()].into_iter()).is_err());
        assert_eq!(
            Invocation::parse(
                ["--emit", "qmir", "program.cgs"]
                    .into_iter()
                    .map(str::to_owned)
            )
            .unwrap()
            .output_kind,
            OutputKind::Machine
        );
        assert_eq!(
            Invocation::parse(
                ["--emit", "obj", "program.cgs"]
                    .into_iter()
                    .map(str::to_owned)
            )
            .unwrap()
            .output_kind,
            OutputKind::Object
        );
        assert!(
            Invocation::parse(
                ["--emit", "qhir", "program.cgs"]
                    .into_iter()
                    .map(str::to_owned)
            )
            .is_err()
        );
    }

    #[test]
    fn lowers_real_iparg_capture_to_portable_ir_text() {
        let module =
            driver::compile_wcc_capture(include_str!("../../fixtures/c/iparg.cgs"), "iparg")
                .unwrap();
        let text = llrm::ir::write_text(&module);

        assert!(text.starts_with("qir 6\nmodule \"iparg\"\n"));
        assert!(text.contains("function 0 \"_twice\""));
        assert!(text.contains("function 1 \"_answer_from_argument\""));
        assert!(text.contains("cc c"));
        assert!(text.contains("cc far_cdecl"));
        assert!(llrm::ir::parse_text(&text).is_ok());
    }

    #[test]
    fn selects_real_iparg_capture_to_machine_ir_text() {
        let module =
            driver::compile_wcc_capture(include_str!("../../fixtures/c/iparg.cgs"), "iparg")
                .unwrap();
        let machine = driver::lower_ir_to_machine(&module).unwrap();
        let text = llrm::codegen::machine::write_text(&machine);

        assert!(text.starts_with("qmir 6\n"));
        assert!(text.contains(" far_cdecl "));
        assert!(text.contains(" c "));
        assert!(llrm::codegen::machine::parse_text(&text).is_ok());
    }

    #[test]
    fn emits_real_iparg_capture_as_omf() {
        let bytes = compile_output(
            include_str!("../../fixtures/c/iparg.cgs"),
            "iparg",
            OutputKind::Object,
        )
        .unwrap();
        let object = driver::parse_omf(&bytes).unwrap();

        assert_eq!(object.to_bytes(), bytes);
    }
}
