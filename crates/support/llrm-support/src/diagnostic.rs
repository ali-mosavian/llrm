//! Diagnostic values shared by llrm subsystems.
//!
//! Diagnostics carry information only. Rendering and output policy belong to
//! the driver, so parsers and compiler passes can report failures without
//! acquiring I/O side effects.
