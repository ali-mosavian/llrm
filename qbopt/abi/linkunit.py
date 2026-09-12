"""The OMF objects and libraries one LINK invocation sees, in order.

An external call is not an isolated module fact. Its definition may be in a
sibling OBJ or an archive member, and treating an absent contract as a reason
to copy the caller unchanged hides precisely the unsupported code a corpus is
meant to expose. This module resolves those names before any object is changed
and supplies a deliberately conservative interface for a resolved definition
whose detailed runtime contract is not yet known.

The conservative interface is not a purity claim: it retains every allocatable
GP input and keeps the worst memory, clobber, control and error effects. That is
enough for allocation to reproduce the incoming machine state while every MIR
pass still treats the call as a barrier.
"""

import hashlib
from pathlib import Path
from dataclasses import replace
from dataclasses import dataclass

from qbopt.abi import runtime
from qbopt.abi import inputscan
from qbopt.objectfile import omf
from qbopt.objectfile import module


class LinkUnitError(ValueError):
    """An input cannot participate in one unambiguous link unit."""


@dataclass(frozen=True, slots=True)
class ObjectInput:
    path: Path
    data: bytes
    records: tuple[omf.Record, ...]


@dataclass(frozen=True, slots=True)
class Definition:
    path: Path
    member: str | None
    segment: int
    offset: int

    @property
    def label(self) -> str:
        within = f"({self.member})" if self.member is not None else ""
        return f"{self.path}{within}:{self.segment}:{self.offset:#x}"


def _symbol(name: str) -> str:
    """Microsoft LINK's symbol identity, independent of OMF spelling case."""
    return name.casefold()


@dataclass(frozen=True, slots=True)
class LinkUnit:
    inputs: tuple[Path, ...]
    objects: tuple[ObjectInput, ...]
    definitions: dict[str, tuple[Definition, ...]]
    fingerprint: str
    scanner: inputscan.Scanner

    @classmethod
    def read(cls, paths: list[Path] | tuple[Path, ...]) -> "LinkUnit":
        if not paths:
            raise LinkUnitError("a link unit needs at least one OMF input")
        objects: list[ObjectInput] = []
        definitions: dict[str, list[Definition]] = {}
        scanned: list[tuple[str, tuple[omf.Record, ...]]] = []
        digest = hashlib.sha256()

        for order, requested in enumerate(paths):
            path = requested.resolve()
            data = path.read_bytes()
            digest.update(order.to_bytes(4, "little"))
            encoded = str(path).encode("utf-8")
            digest.update(len(encoded).to_bytes(4, "little"))
            digest.update(encoded)
            digest.update(len(data).to_bytes(8, "little"))
            digest.update(data)

            archived = omf.library_modules(data)
            members = archived or ((None, omf.parse(data)),)
            for member_name, records in members:
                label = f"{path}({member_name})" if member_name is not None else str(path)
                scanned.append((label, tuple(records)))
                unknown = sorted({record.type for record in records if record.type not in omf.NAMES})
                if unknown:
                    kinds = ", ".join(f"{kind:#04x}" for kind in unknown)
                    where = f"{path}({member_name})" if member_name is not None else str(path)
                    raise LinkUnitError(f"{where}: unrecognized OMF record type(s) {kinds}")
                for name, (segment, offset) in omf.public_definitions(records).items():
                    definitions.setdefault(_symbol(name), []).append(Definition(path, member_name, segment, offset))
            if not archived:
                records = members[0][1]
                if module.of(records) is None:
                    raise LinkUnitError(f"{path}: standalone input has no recognized code module")
                objects.append(ObjectInput(path, data, tuple(records)))

        if not objects:
            raise LinkUnitError("the link unit contains libraries but no standalone OBJ to optimize")
        return cls(
            tuple(path.resolve() for path in paths),
            tuple(objects),
            {name: tuple(found) for name, found in definitions.items()},
            digest.hexdigest(),
            inputscan.Scanner(scanned),
        )

    def definition(self, name: str) -> Definition:
        found = self.definitions.get(_symbol(name), ())
        if not found:
            raise LinkUnitError(f"unresolved external {name!r}")
        # Explicit objects all participate in the link. Two of them exporting
        # the same name is a multiply-defined public and must fail. Archive
        # members are demand-loaded, however: a definition already supplied
        # by an object prevents a library member from being selected, and in
        # an all-library search the first definition in command/member order
        # satisfies the unresolved symbol. Treating every unselected archive
        # member as a simultaneous definition rejected valid LINK lines whose
        # libraries intentionally offer overlapping implementations.
        objects = tuple(one for one in found if one.member is None)
        if len(objects) > 1:
            locations = ", ".join(one.label for one in objects)
            raise LinkUnitError(f"multiply-defined public {name!r}: {locations}")
        return objects[0] if objects else found[0]

    def contracts_for(self, source: ObjectInput) -> dict[str, runtime.Contract]:
        """Every external call's interface, after resolving the whole unit."""
        records = list(source.records)
        externals = omf.externals(records)
        referenced = {index for fixup in omf.fixups(records) for index in omf.names_externals(fixup)}
        for index in sorted(referenced):
            if not 0 < index < len(externals):
                raise LinkUnitError(f"{source.path}: fixup names missing EXTDEF index {index}")
            # Calls need an ABI contract below; data and frame references do
            # not, but LINK must still be able to resolve them. Validating all
            # live fixups here makes this a link-unit view rather than a call-
            # site-only approximation and fails before optimizing any object.
            self.definition(externals[index])
        found = module.of(records)
        if found is None:
            raise LinkUnitError(f"{source.path}: code module disappeared while resolving contracts")
        # Start with the module's complete per-site view, including event
        # entries, dynamic cleanup and language-ABI inference. Rebuilding
        # from names alone loses an adjacent caller-cleanup ADD SP on C
        # calls, after which a name-keyed link contract would overwrite the
        # more precise site fact with cleanup=None.
        selected = runtime.for_module(found)
        contracts: dict[str, runtime.Contract] = {}
        for at, name in sorted(found.calls.items()):
            definition = self.definition(name)
            routine = selected[at]
            if runtime.established_inputs(routine):
                continue
            label = f"{definition.path}({definition.member})" if definition.member is not None else str(definition.path)
            chosen = (label, definition.segment, definition.offset)
            try:
                discovered = self.scanner.inputs(name, chosen)
                kept = self.scanner.kept(name, chosen)
            except inputscan.Unrecognized as error:
                raise LinkUnitError(f"{definition.label}: input-contract discovery failed: {error}") from error
            contracts[name] = replace(
                routine,
                inputs=discovered.registers,
                clobbers=routine.clobbers - kept.registers,
                # The scan keeps nothing past an unresolved edge, and the
                # error funnel never returns to the caller.
                clobbers_reached=True,
                evidence=(
                    f"Link-unit definition {definition.label}. {discovered.evidence}. {kept.evidence}. "
                    "The callee remains an opaque barrier: no memory, control, cleanup or error effect is "
                    "relaxed. "
                    "Incoming arithmetic flags are excluded by the compiler calling convention; a "
                    "hand-written flag-taking entry needs an explicit audited contract."
                ),
            )
        return contracts
