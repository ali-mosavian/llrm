use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use llrm::driver::{self, QbOptions};
use llrm::frontend::qb::{Dialect, source};
use llrm::hir::RuntimeProfile;

fn main() -> ExitCode {
    match Invocation::parse(env::args().skip(1)) {
        Ok(invocation) => invocation.run(),
        Err(message) => {
            eprintln!("llrm-qb: {message}");
            ExitCode::from(2)
        }
    }
}

struct Invocation {
    input: PathBuf,
    output: Option<PathBuf>,
    output_kind: OutputKind,
    include_dirs: Vec<PathBuf>,
    qb: QbOptions,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputKind {
    Hir,
    Ir,
    Machine,
}

impl Invocation {
    fn parse(arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut arguments = arguments.peekable();
        let mut input = None;
        let mut output = None;
        let mut output_kind = None;
        let mut include_dirs = Vec::new();
        let mut qb = QbOptions::default();

        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "-o" => output = Some(PathBuf::from(required(&mut arguments, "-o")?)),
                "--emit" => {
                    let kind = match required(&mut arguments, "--emit")?.as_str() {
                        "qhir" => OutputKind::Hir,
                        "qir" => OutputKind::Ir,
                        "qmir" => OutputKind::Machine,
                        value => return Err(format!("unknown output kind {value:?}")),
                    };
                    if output_kind.replace(kind).is_some() {
                        return Err("--emit may only be specified once".to_owned());
                    }
                }
                "--include" => {
                    include_dirs.push(PathBuf::from(required(&mut arguments, "--include")?));
                }
                "--dialect" => {
                    let value = required(&mut arguments, "--dialect")?;
                    qb.dialect = Dialect::parse(&value)
                        .ok_or_else(|| format!("unknown QB dialect {value:?}"))?;
                }
                "--runtime" => {
                    let value = required(&mut arguments, "--runtime")?;
                    qb.runtime = match value.as_str() {
                        "qb45" => RuntimeProfile::Qb45,
                        "pds71" => RuntimeProfile::Pds71,
                        "vbdos" => RuntimeProfile::Vbdos,
                        _ => return Err(format!("unknown QB runtime {value:?}")),
                    };
                }
                "--array-order" => {
                    let value = required(&mut arguments, "--array-order")?;
                    qb.row_major = match value.as_str() {
                        "column-major" => false,
                        "row-major" => true,
                        _ => return Err(format!("unknown array order {value:?}")),
                    };
                }
                "--huge-arrays" => qb.huge_arrays = true,
                "--checked-arrays" => qb.checked_arrays = true,
                "--mbf" => qb.mbf = true,
                "--alternate-math" => qb.alternate_math = true,
                "-h" | "--help" => return Err(usage().to_owned()),
                _ if argument.starts_with('-') => {
                    return Err(format!("unknown option {argument:?}\n{}", usage()));
                }
                _ => {
                    if input.is_some() {
                        return Err("expected exactly one input path".to_owned());
                    }
                    input = Some(PathBuf::from(argument));
                }
            }
        }

        let input = input.ok_or_else(|| usage().to_owned())?;
        if !input
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("bas"))
        {
            return Err("QB input must have a .bas extension".to_owned());
        }
        Ok(Self {
            input,
            output,
            output_kind: output_kind.unwrap_or(OutputKind::Hir),
            include_dirs,
            qb,
        })
    }

    fn run(self) -> ExitCode {
        let loaded = match source::load_with_map(&self.input, &self.include_dirs) {
            Ok(source) => source,
            Err(error) => return failure(format!("{}: {error}", self.input.display())),
        };
        let module_name = self
            .input
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("module");
        let program = match driver::compile_qb(&loaded.text, module_name, self.qb) {
            Ok(program) => program,
            Err(driver::Error::Parse(error)) => {
                let location = loaded.location(error.span.line);
                let path = location.map_or(self.input.as_path(), |one| one.path.as_path());
                let line = location.map_or(error.span.line, |one| one.line);
                return failure(format!(
                    "{}:{}:{}: {}",
                    path.display(),
                    line,
                    error.span.start + 1,
                    error.message
                ));
            }
            Err(error) => return failure(format!("{}: {error}", self.input.display())),
        };
        let text = match self.output_kind {
            OutputKind::Hir => llrm::hir::write_text(&program),
            OutputKind::Ir => match driver::lower_qb_to_ir(&program) {
                Ok(module) => llrm::ir::write_text(&module),
                Err(error) => return failure(format!("{}: {error}", self.input.display())),
            },
            OutputKind::Machine => match driver::lower_qb_to_machine(&program) {
                Ok(machine) => llrm::codegen::machine::write_text(&machine.module),
                Err(error) => return failure(format!("{}: {error}", self.input.display())),
            },
        };
        write_output(self.output.as_deref(), text.as_bytes())
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
    eprintln!("llrm-qb: {message}");
    ExitCode::FAILURE
}

fn usage() -> &'static str {
    "usage: llrm-qb [--emit qhir|qir|qmir] [-o FILE] [QB OPTIONS] INPUT.bas"
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{Invocation, OutputKind};
    use llrm::driver::{self, QbOptions};

    #[test]
    fn selects_portable_ir_output_for_qb_source() {
        let invocation = Invocation::parse(
            ["--emit", "qir", "program.bas"]
                .into_iter()
                .map(str::to_owned),
        )
        .unwrap();

        assert_eq!(invocation.output_kind, OutputKind::Ir);
    }

    #[test]
    fn lowers_minimal_qb_source_to_portable_ir_text() {
        let program = driver::compile_qb("", "program", QbOptions::default()).unwrap();
        let module = driver::lower_qb_to_ir(&program).unwrap();
        let text = llrm::ir::write_text(&module);

        assert!(text.starts_with("qir 4\nmodule \"program\"\n"));
        assert!(llrm::ir::parse_text(&text).is_ok());
    }

    #[test]
    fn lowers_minimal_qb_source_to_machine_ir_text() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the system clock must be after the Unix epoch")
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("llrm-qmir-cli-{}-{timestamp}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let source = directory.join("program.bas");
        let output = directory.join("program.qmir");
        fs::write(&source, "").unwrap();

        let invocation = Invocation::parse(
            [
                "--emit".to_owned(),
                "qmir".to_owned(),
                "-o".to_owned(),
                output.display().to_string(),
                source.display().to_string(),
            ]
            .into_iter(),
        )
        .unwrap();
        assert_eq!(invocation.output_kind, OutputKind::Machine);
        assert_eq!(invocation.run(), std::process::ExitCode::SUCCESS);

        let text = fs::read_to_string(&output).unwrap();
        let parsed = llrm::codegen::machine::parse_text(&text)
            .expect("the qmir writer must emit parseable text");

        assert!(text.starts_with("qmir 5\n"));
        parsed.verify().expect("parsed qmir must verify");
        fs::remove_dir_all(directory).unwrap();
    }
}
