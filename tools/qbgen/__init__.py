"""Deterministic source-level QB/PDS/VBDOS test generation.

The recovered qbasic-port generators were deliberately about source semantics,
not an implementation's p-code.  This package keeps that useful part in-tree
for qbopt.  It emits DOS-safe BASIC input and a manifest of parser, semantic,
and HIR expectations; no p-code is generated, scanned, or asserted here.
"""

from tools.qbgen.families import cases
from tools.qbgen.model import GeneratedCase
from tools.qbgen.model import DialectOutcome

__all__ = ["DialectOutcome", "GeneratedCase", "cases"]
