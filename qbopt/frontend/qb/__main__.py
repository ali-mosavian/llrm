import argparse
from pathlib import Path

from qbopt import hir
from qbopt import flow
from qbopt.frontend.qb import compile
from qbopt.frontend.qb.driver import parsed


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python -m qbopt.frontend.qb")
    parser.add_argument("source", type=Path)
    parser.add_argument("--dialect", default="vbdos")
    parser.add_argument("--runtime", default="vbdos")
    parser.add_argument(
        "--array-order",
        choices=tuple(sorted(one.value for one in hir.ArrayOrder)),
        default=hir.ArrayOrder.COLUMN_MAJOR.value,
        help="QB default is column-major; use row-major for BC /R sources",
    )
    parser.add_argument("--dump-hir", type=Path)
    parser.add_argument(
        "--huge-arrays",
        action="store_true",
        help="use the PDS /Ah huge dynamic-array addressing contract",
    )
    parser.add_argument(
        "--checked-arrays",
        action="store_true",
        help="use the BC /D checked array-access contract",
    )
    parser.add_argument(
        "--unchecked-bounds",
        action="store_true",
        help="LBOUND/UBOUND trust the descriptor: no allocation or dimension check",
    )
    parser.add_argument(
        "--alternate-math",
        action="store_true",
        help="use the PDS /FPa alternate floating-point runtime contract",
    )
    parser.add_argument("--include", action="append", default=[], type=Path)
    parser.add_argument("--mir", action="store_true")
    parser.add_argument("-o", "--output", type=Path)
    flow.level_option(parser)
    options = parser.parse_args(arguments)
    program = parsed(
        options.source,
        dialect=options.dialect,
        runtime=options.runtime,
        array_order=options.array_order,
        huge_arrays=options.huge_arrays,
        checked_arrays=options.checked_arrays,
        unchecked_bounds=options.unchecked_bounds,
        alternate_math=options.alternate_math,
        dump=options.dump_hir,
        include_dirs=tuple(options.include),
    )
    if options.output is not None:
        if options.mir:
            parser.error("--mir and --output cannot be used together")
        options.output.write_bytes(compile.object_bytes(program, options.source.name, options=options.options))
    elif options.mir:
        for body in hir.lower(program):
            print(hir.mir_text(body), end="")
    else:
        print(hir.encode(program), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
