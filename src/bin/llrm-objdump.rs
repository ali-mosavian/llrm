use std::env;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use llrm::object::omf::archive::Module;
use llrm::object::omf::file::File;
use llrm::object::omf::module::{DecodedModule, ModuleError};
use llrm::object::omf::record::Record;

fn main() -> ExitCode {
    let path = match parse_arguments(env::args().skip(1)) {
        Ok(path) => path,
        Err(message) => {
            eprintln!("llrm-objdump: {message}");
            return ExitCode::from(2);
        }
    };
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("llrm-objdump: {}: {error}", path.display());
            return ExitCode::FAILURE;
        }
    };
    let file = match File::parse(&bytes) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("llrm-objdump: {}: {error}", path.display());
            return ExitCode::FAILURE;
        }
    };
    match dump(&file, &mut io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("llrm-objdump: stdout: {error}");
            ExitCode::FAILURE
        }
    }
}

fn parse_arguments(mut arguments: impl Iterator<Item = String>) -> Result<PathBuf, &'static str> {
    let path = arguments.next().ok_or("usage: llrm-objdump INPUT")?;
    if arguments.next().is_some() {
        return Err("usage: llrm-objdump INPUT");
    }
    Ok(PathBuf::from(path))
}

fn dump(file: &File, writer: &mut impl Write) -> Result<(), DumpError> {
    match file {
        File::Object(records) => {
            dump_records(records, writer)?;
            dump_decoded(records, writer)
        }
        File::Library(archive) => {
            writeln!(
                writer,
                "library page_size={} modules={}",
                archive.page_size(),
                archive.modules().len()
            )?;
            for module in archive.modules() {
                dump_module(module, writer)?;
            }
            if let Some(offset) = archive.dictionary_offset() {
                writeln!(writer, "{offset:08x} dictionary")?;
            }
            Ok(())
        }
    }
}

fn dump_module(module: &Module, writer: &mut impl Write) -> Result<(), DumpError> {
    write!(writer, "{:08x} module ", module.offset())?;
    write_bytes(module.name(), writer)?;
    writeln!(writer)?;
    dump_records(module.records(), writer)?;
    dump_decoded(module.records(), writer)
}

fn dump_records(records: &[Record], writer: &mut impl Write) -> io::Result<()> {
    for record in records {
        let offset = record.offset().unwrap_or(0);
        let checksum = if record.is_checksum_valid() {
            "valid"
        } else {
            "invalid"
        };
        writeln!(
            writer,
            "{offset:08x} type={:02x} body={} checksum={checksum}",
            record.record_type(),
            record.body().len()
        )?;
    }
    Ok(())
}

fn write_bytes(bytes: &[u8], writer: &mut impl Write) -> io::Result<()> {
    for byte in bytes {
        match *byte {
            b' '..=b'~' if !matches!(*byte, b'\\' | b'\"') => writer.write_all(&[*byte])?,
            _ => write!(writer, "\\x{byte:02x}")?,
        }
    }
    Ok(())
}

fn dump_decoded(records: &[Record], writer: &mut impl Write) -> Result<(), DumpError> {
    let module = DecodedModule::parse(records)?;
    for (index, external) in module.symbols.externals.iter().enumerate().skip(1) {
        let Some(external) = external else {
            continue;
        };
        write!(writer, "external {index} ")?;
        write_bytes(&external.name, writer)?;
        writeln!(writer, " type={}", external.type_index)?;
    }
    for public in &module.declarations.publics {
        write!(writer, "public {:?} ", public.scope)?;
        write_bytes(&public.name, writer)?;
        writeln!(
            writer,
            " base={:?} offset={:#x}",
            public.base, public.offset
        )?;
    }
    for fixup in &module.fixups {
        writeln!(
            writer,
            "relocation segment={} offset={:#x} location={:?} mode={:?} target={:?}:{} displacement={:#x}",
            fixup.segment_index,
            fixup.patch_offset,
            fixup.location,
            fixup.mode,
            fixup.target.method,
            fixup.target.datum,
            fixup.displacement
        )?;
    }
    Ok(())
}

#[derive(Debug)]
enum DumpError {
    Io(io::Error),
    Module(ModuleError),
}

impl fmt::Display for DumpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Module(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DumpError {}

impl From<io::Error> for DumpError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ModuleError> for DumpError {
    fn from(error: ModuleError) -> Self {
        Self::Module(error)
    }
}

#[cfg(test)]
mod tests {
    use super::dump;
    use llrm::object::omf::file::File;
    use llrm::object::omf::record::Record;

    #[test]
    fn reports_invalid_checksums_without_refusing_the_object() {
        let mut bytes = Record::new(0x80, vec![1, b'm']).unwrap().to_bytes();
        *bytes.last_mut().unwrap() = 0x5a;
        let file = File::parse(&bytes).unwrap();
        let mut output = Vec::new();

        dump(&file, &mut output).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "00000000 type=80 body=2 checksum=invalid\n"
        );
    }

    #[test]
    fn reports_typed_relocations() {
        let records = vec![
            Record::new(0xa0, vec![1, 0, 0, 0, 0]).unwrap(),
            Record::new(0x9c, vec![0x84, 0, 0x44, 1]).unwrap(),
        ];
        let mut output = Vec::new();

        dump(&File::Object(records), &mut output).unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains(
            "relocation segment=1 offset=0x0 location=Offset16 mode=SelfRelative target=Segment:1 displacement=0x0"
        ));
    }
}
