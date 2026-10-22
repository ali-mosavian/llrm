# Formats

How the backend's output reaches a file: the format-neutral object, one writer per format, and the debug
information each format carries. For contributors changing a writer or `-g`.

| Doc | Holds |
|---|---|
| [object-model](object-model.md) | The `Object` the backend builds, the writers that serialize it, and how a format is chosen |
| [omf](omf.md) | OMF records, segments and groups, fixups, symbols; BC objects recompiled |
| [coff-elf-macho](coff-elf-macho.md) | The three flat-address writers: sections, symbols, relocation types, REL and RELA |
| [debug-info](debug-info.md) | `debug::Info`, the `-g` flags, what each format emits, known gaps, verification |
| [codeview](codeview.md) | CodeView 4 in OMF objects: C7 form and BC's pre-link form |
| [turbo-debugger](turbo-debugger.md) | Turbo Debugger records, `-gtd`, and loading them in Turbo Debugger |
| [qlb](qlb.md) | QuickBASIC, PDS and VBDOS Quick libraries: the MZ file, its zero runs, symbol tables and relocations |
