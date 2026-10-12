# Formats

How the backend's output reaches a file: the format-neutral object, one writer per format,
and the debug information each format carries. For contributors changing a writer or `-g`.

| Doc | Holds |
|---|---|
| [object-model](object-model.md) | The `Object` the backend builds and the writers that serialize it |
| [codeview](codeview.md) | CodeView debug info in OMF objects, C13 and CV4 |
| [turbo-debugger](turbo-debugger.md) | Turbo Debugger records, `-gtd` |
