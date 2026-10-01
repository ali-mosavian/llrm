"""
The loop corpus: every case through every frontend, checked for correctness
against the oracle and for quality against hand-derived bounds and the
reference compilers. See tools/readme.md.

    uv run --project tools python tools/loops/run.py --quick
    uv run --project tools python tools/loops/run.py [--family F] [--config 486-O2] [--dump DIR]

Fails for any wrong answer, any case that falls short and is not in
shortfalls.txt, and any shortfalls.txt entry that no longer falls short.
"""

from __future__ import annotations

import re
import sys
import json
import time
import shutil
import argparse
import subprocess
from pathlib import Path
from dataclasses import dataclass, field, replace
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, str(Path(__file__).resolve().parent))

import build  # noqa: E402
import dos  # noqa: E402
import mir  # noqa: E402
import expect  # noqa: E402
import oracle  # noqa: E402
import quality  # noqa: E402
import known  # noqa: E402
import emit_c  # noqa: E402
import emit_bas  # noqa: E402
import emit_nib  # noqa: E402
import innerloops  # noqa: E402
from spec import Case, walk, CallS  # noqa: E402
from cases import families  # noqa: E402

EMITTERS = {"c": emit_c, "bas": emit_bas, "nib": emit_nib}
BATCH = {"c": 24, "bas": 16, "nib": 24}
NEAR_BUDGET = 36000  # bytes of near arrays per program
WORKERS = 24


@dataclass
class Result:
    """Everything the run learned, keyed (case, lang, config)."""

    wrong: list[str] = field(default_factory=list)  # correctness failures: the bar
    unbuilt: list[str] = field(default_factory=list)  # compiles that failed
    skipped: dict = field(default_factory=dict)  # (case, lang) -> why not expressible
    facts: dict = field(default_factory=dict)  # (case, lang|ref, config) -> [Facts]
    mir_ivs: dict = field(default_factory=dict)  # (case, lang, config) -> [per inner loop], from llrm-mir --ivs
    short: set = field(default_factory=set)  # (case, lang, config, check)
    judged: set = field(default_factory=set)  # every check the run could evaluate, same keys
    notes: list[str] = field(default_factory=list)


def plans_for(cases: list[Case], lang: str) -> tuple[dict, dict]:
    """Valid input indices per case, and the expected report stream."""
    plans, streams = {}, {}
    for case in cases:
        results = oracle.evaluate(case, lang)
        plans[case.name] = [k for k, r in enumerate(results) if isinstance(r, list)]
        streams[case.name] = [x for r in results if isinstance(r, list) for x in r]
    return plans, streams


def batches(cases: list[Case], lang: str) -> list[list[Case]]:
    out, current, size = [], [], 0
    for case in cases:
        near = sum(a.bytes for a in case.arrays if a.ptr == "near" or lang != "c")
        if current and (len(current) >= BATCH[lang] or size + near > NEAR_BUDGET):
            out.append(current)
            current, size = [], 0
        current.append(case)
        size += near
    if current:
        out.append(current)
    return out


def source(lang: str, cases: list[Case], plans: dict) -> str:
    if lang == "c":
        return emit_c.driver(cases, plans, False)
    if lang == "bas":
        return emit_bas.driver(cases, {c.name: k + 1 for k, c in enumerate(cases)}, plans)
    return emit_nib.driver(cases, plans)


_programs = iter(range(10**6))


def unique(lang: str) -> str:
    """A DOS name no other program of the run has: 8 characters."""
    return f"{lang[0]}X{next(_programs):06d}"


def mir_name(lang: str, case: Case, number: int) -> str:
    if lang == "bas":
        return f'F{number}{"&" if case.ret.bits == 32 else "%"}'
    return case.symbol if lang == "nib" else f"_{case.symbol}"


def symbol(lang: str, case: Case, number: int) -> str:
    if lang == "bas":
        return f"F{number}"
    return f"_{case.symbol}"


class Batch:
    def __init__(self, lang, cases, config, work, plans, streams, index):
        self.lang, self.cases, self.config, self.plans, self.streams = lang, cases, config, plans, streams
        self.work = work / config.tag / lang / f"b{index:02d}"
        self.stem = f"{lang[0]}{config.tag.replace('-', '')[:4]}{index:02d}"[:8]
        self.obj = self.work / "P.OBJ"

    @property
    def expected(self) -> list[int]:
        return [x for case in self.cases for x in self.streams[case.name]]

    def compile(self) -> None:
        if self.work.exists():
            shutil.rmtree(self.work)
        self.work.mkdir(parents=True)
        path = self.work / f"p{build.EXT[self.lang]}"
        text = source(self.lang, self.cases, self.plans)
        path.write_bytes(text.encode("latin-1"))
        build.llrm(self.lang, path, self.obj, self.config, self.work / "stages")

    def procedures(self) -> list[str] | None:
        listing = self.work / "stages" / "listing.asm"
        return innerloops.listed(listing) if self.lang == "nib" and listing.exists() else None


def check_mir(batch: Batch, result: Result) -> None:
    """Both ends of the pipeline, against the oracle; on a mismatch, each case alone."""
    want = mir.fold(batch.expected)
    first, last = mir.stages(batch.work / "stages")
    names = [mir_name(batch.lang, c, k + 1) for k, c in enumerate(batch.cases)]
    for name in [] if batch.config.inline else mir.called(last, names):
        result.wrong.append(f"{batch.config.tag} {batch.lang}: {name} was inlined into the driver, so went unchecked")
    for stage in (first, last):
        try:
            got = mir.run(stage, batch.lang, batch.work)
        except mir.Unrunnable as why:
            result.notes.append(f"{batch.config.tag} {batch.lang} {batch.stem}: MIR not runnable: {why}")
            return
        except RuntimeError as why:
            got = f"trapped: {str(why)[-300:]}"
        if got != want:
            culprits = isolate(batch, stage.name)
            result.wrong.append(
                f"{batch.config.tag} {batch.lang} MIR {stage.stem}: {', '.join(culprits) or 'the batch'} "
                f"disagrees with the oracle ({batch.work})")


def isolate(batch: Batch, stage_name: str) -> list[str]:
    """The cases that fail alone."""
    out = []
    for k, case in enumerate(batch.cases):
        single = Batch(batch.lang, [case], batch.config, batch.work / "alone", batch.plans, batch.streams, k)
        try:
            single.compile()
            stages = sorted((single.work / "stages").glob("[0-9][0-9]-*.ll"))
            stage = next((s for s in stages if s.name == stage_name), stages[-1])
            if mir.run(stage, batch.lang, single.work) != mir.fold(single.expected):
                out.append(case.name)
        except Exception:  # noqa: BLE001
            out.append(case.name)
    return out


def measured_from(batch: Batch, k: int) -> Batch:
    """The program case k's quality is read from: its own, so its code does
    not depend on which cases share the batch."""
    if len(batch.cases) == 1:
        return batch
    single = Batch(batch.lang, [batch.cases[k]], batch.config, batch.work / "one", batch.plans, batch.streams, k)
    single.compile()
    return single


def measure(batch: Batch, result: Result) -> None:
    procedures = batch.procedures()
    found = innerloops.loops(batch.obj.read_bytes(), calls=True, procedures=procedures)
    by_name: dict[str, list] = {}
    for loop in found:
        by_name.setdefault(loop.name.rsplit("#", 1)[0], []).append(quality.facts(loop))
    for k, case in enumerate(batch.cases):
        name = symbol(batch.lang, case, k + 1)
        result.facts[(case.name, batch.lang, batch.config.tag)] = by_name.get(name, [])
    for name, text in innerloops.procedures(batch.obj.read_bytes(), procedures).items():
        for problem in quality.bp_problems(text):
            result.wrong.append(f"{batch.config.tag} {batch.lang} {name}: {problem} ({batch.obj})")
    _, last = mir.stages(batch.work / "stages")
    done = subprocess.run([str(build.BIN / "llrm-mir"), "--ivs", str(last)], capture_output=True, text=True)
    counts: dict[str, list[int]] = {}
    for line in done.stdout.splitlines():
        function, _, count = line.split("\t")
        counts.setdefault(function.strip('"'), []).append(int(count))
    for k, case in enumerate(batch.cases):
        result.mir_ivs[(case.name, batch.lang, batch.config.tag)] = counts.get(mir_name(batch.lang, case, k + 1), [])


def references(cases: list[Case], config: build.Config, work: Path, result: Result) -> None:
    """Each case alone through each reference compiler: as for llrm, its
    loop must not depend on what else was in the file."""
    have = build.available()
    for ref, compile_ in build.REFERENCES.items():
        if not have[ref]:
            continue
        for case in cases:
            if emit_c.expressible(case) or (ref == "llvm" and _far(case)):
                continue
            place = work / config.tag / ref / case.name
            place.mkdir(parents=True, exist_ok=True)
            path = place / "p.c"
            path.write_text(emit_c.library([case], ref))
            obj = place / ("P.OBJ" if ref == "ow" else "p.o")
            try:
                compile_(path, obj, config)
            except build.CompileError as why:
                result.notes.append(f"{config.tag} {ref} {case.name}: {str(why)[:160]}")
                continue
            loops = innerloops.loops(obj.read_bytes(), calls=True)
            result.facts[(case.name, ref, config.tag)] = [
                quality.facts(one) for one in loops if one.name.rsplit("#", 1)[0].lstrip("_") == case.symbol]


def _far(case: Case) -> bool:
    return any(a.ptr != "near" for a in case.arrays)


# --- quality checks ------------------------------------------------------------


def total(facts: list) -> tuple[int, int]:
    return sum(f.size for f in facts), sum(f.memory for f in facts)


def reference_meets(result: Result, case: str, config: str, bound: int) -> bool:
    """Whether Open Watcom or gcc-ia16 compiled the case's C within `bound`."""
    for ref in ("ow", "gcc"):
        facts = result.facts.get((case, ref, config))
        if facts and max(f.ivs for f in facts) <= bound:
            return True
    return False


def judge(cases: list[Case], langs: list[str], configs: list, result: Result) -> None:
    configs = [one for one in configs if not one.inline]
    by_name = {c.name: c for c in cases}
    for case in cases:
        for lang in langs:
            if (case.name, lang) in result.skipped:
                continue
            wants = expect.want(case, lang)
            stated = {one[0]: one[1] for one in case.bound}
            if lang in stated and len(wants) == 1:
                wants = [expect.Want(stated[lang], 0, True, False, wants[0].classes, "stated in the case")]
            for config in configs:
                facts = result.facts.get((case.name, lang, config.tag))
                if facts is None:
                    continue
                key = (case.name, lang, config.tag)
                if not facts:
                    continue  # no loop left: a string instruction, or unrolled away
                fits = wants and all(w.fits for w in wants)
                if fits and all(w.ivs is not None for w in wants):
                    bound = max(w.ivs for w in wants)
                    # a bound no reference compiler met on the same loop is an ideal
                    kind = "ivs" if reference_meets(result, case.name, config.tag, bound) else "ivs-ideal"
                    result.judged |= {(*key, kind), (*key, "mir-" + kind)}
                    if max(f.ivs for f in facts) > bound:
                        result.short.add((*key, kind))
                    counted = result.mir_ivs.get(key)
                    if counted and max(counted) > bound:
                        result.short.add((*key, "mir-" + kind))
                if fits and all(w.invariant_loads == 0 for w in wants):
                    result.judged.add((*key, "invariant-loads"))
                    if sum(f.invariant_loads for f in facts) > 0:
                        result.short.add((*key, "invariant-loads"))
                if len(wants) == 1 and wants[0].shape and len(facts) == 1:
                    result.judged.add((*key, "shape"))
                    f = facts[0]
                    if not (f.ivs == 1 and f.compares == 0 and f.branch_after_step and f.overhead == 2):
                        result.short.add((*key, "shape"))
                if lang == "c":
                    refs = [result.facts.get((case.name, ref, config.tag)) for ref in ("ow", "gcc")]
                    # a reference loop that calls a helper hides its cost: not a bar
                    bars = [total(r) for r in refs if r and not any(f.calls for f in r)]
                    if bars and not any(f.calls for f in facts):
                        result.judged |= {(*key, "size-vs-ref"), (*key, "memory-vs-ref")}
                        size, memory = total(facts)
                        if size > min(b[0] for b in bars):
                            result.short.add((*key, "size-vs-ref"))
                        if memory > min(b[1] for b in bars):
                            result.short.add((*key, "memory-vs-ref"))
                if case.base and case.base in by_name:
                    base = result.facts.get((case.base, lang, config.tag))
                    if base and facts:
                        result.judged |= {(*key, "relation-ivs"), (*key, "relation-size")}
                        if max(f.ivs for f in facts) != max(f.ivs for f in base):
                            result.short.add((*key, "relation-ivs"))
                        if "same-size" in case.tags and total(facts)[0] != total(base)[0]:
                            result.short.add((*key, "relation-size"))


# --- the run -------------------------------------------------------------------


def configs_from(args) -> list[build.Config]:
    return [replace(one, inline=args.inline) for one in _configs_from(args)]


def _configs_from(args) -> list[build.Config]:
    if args.config:
        out = []
        for one in args.config:
            cpu, opt = one.split("-", 1)
            out.append(build.Config(cpu, "-" + opt))
        return out
    if args.quick:
        return [build.Config("486", "-O2")]
    return [build.Config(cpu, opt) for cpu in build.CPUS for opt in build.OPTS]


def validate(cases: list[Case], work: Path, result: Result, bc: bool = True) -> list[dos.Job]:
    """The oracle against real compilers: C by the host's clang now; the
    BASIC jobs returned run under BC in the DOS launch (`bc` False: clang
    only, which --quick does, as BC compiles in emulated DOS)."""
    place = work / "validate"
    place.mkdir(parents=True, exist_ok=True)
    chosen = [c for c in cases if not emit_c.expressible(c)]
    plans, streams = plans_for(chosen, "c")
    for k in range(0, len(chosen), 60):
        group = chosen[k : k + 60]
        path = place / f"h{k}.c"
        path.write_text(emit_c.driver(group, plans, True))
        exe = place / f"h{k}"
        done = subprocess.run(["clang", "-O1", "-w", "-o", str(exe), str(path)], capture_output=True, text=True)
        if done.returncode:
            result.wrong.append(f"host clang cannot build the oracle check: {done.stderr[:300]}")
            continue
        got = [int(x) for x in subprocess.run([str(exe)], capture_output=True, text=True).stdout.split()]
        _compare("oracle vs host clang", group, streams, got, result)
    jobs = []
    if not bc:
        return jobs
    chosen = [c for c in cases if not emit_bas.expressible(c)]
    plans, streams = plans_for(chosen, "bas")
    for k, group in enumerate(batches(chosen, "bas")):
        path = place / f"V{k:03d}.BAS"
        path.write_bytes(emit_bas.driver(group, {c.name: n + 1 for n, c in enumerate(group)}, plans).encode("latin-1"))
        job = dos.Job(f"V{k:03d}", "bas", path)
        job.expected = [x for c in group for x in streams[c.name]]
        job.cases = [(c.name, len(streams[c.name])) for c in group]
        job.what = "oracle vs BC"
        jobs.append(job)
    return jobs


def _compare(what: str, cases: list[Case], streams: dict, got: list[int], result: Result) -> None:
    at = 0
    for case in cases:
        want = streams[case.name]
        mine = got[at : at + len(want)]
        if mine != want:
            first = next((k for k in range(len(want)) if k >= len(mine) or mine[k] != want[k]), len(want))
            result.wrong.append(f"{what}: {case.name} report {first} is {mine[first:first + 3]}, "
                                f"wants {want[first:first + 3]}")
            return
        at += len(want)
    if at != len(got):
        result.wrong.append(f"{what}: {len(got) - at} more values than expected")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--quick", action="store_true", help="the anchors, one configuration: under a minute")
    parser.add_argument("--family", action="append", help="only these families")
    parser.add_argument("--case", action="append", help="only cases whose name starts so")
    parser.add_argument("--seed", type=int, default=families.SEED, help="the fuzz family's seed")
    parser.add_argument("--lang", action="append", choices=list(EMITTERS))
    parser.add_argument("--config", action="append", help="CPU-OPT, e.g. 486-O2")
    parser.add_argument("--dump", type=Path, default=build.ROOT / "build" / "loops", help="where everything goes")
    parser.add_argument("--inline", action="store_true", help="let the compiler inline the functions under test: correctness only, no shortfalls")
    parser.add_argument("--no-dos", action="store_true", help="MIR only")
    parser.add_argument("--no-refs", action="store_true")
    parser.add_argument("--no-validate", action="store_true", help="skip checking the oracle against clang and BC")
    parser.add_argument("--write-known", action="store_true", help="rewrite shortfalls.txt to what falls short now")
    args = parser.parse_args()

    started = time.monotonic()
    stamp = build.binaries_stamp()
    cases = families.load(args.family, quick=args.quick, seed=args.seed)
    if args.case:
        cases = [c for c in cases if any(c.name.startswith(p) for p in args.case)]
    langs = args.lang or list(EMITTERS)
    configs = configs_from(args)
    work = args.dump
    work.mkdir(parents=True, exist_ok=True)
    result = Result()

    broken = set()
    for case in cases:
        try:
            for lang in langs:
                if not EMITTERS[lang].expressible(case):
                    oracle.evaluate(case, lang)
        except oracle.Broken as why:
            result.wrong.append(f"the generator made a broken case: {why}")
            broken.add(case.name)
    cases = [c for c in cases if c.name not in broken]
    per_lang = {}
    for lang in langs:
        chosen = []
        for case in cases:
            why = EMITTERS[lang].expressible(case)
            if why:
                result.skipped[(case.name, lang)] = why
            else:
                chosen.append(case)
        per_lang[lang] = (chosen, *plans_for(chosen, lang))

    jobs: list[dos.Job] = [] if args.no_validate or args.no_dos else validate(cases, work, result, bc=not args.quick)
    all_batches = []
    for config in configs:
        for lang in langs:
            chosen, plans, streams = per_lang[lang]
            for index, group in enumerate(batches(chosen, lang)):
                all_batches.append(Batch(lang, group, config, work, plans, streams, index))

    def one(batch: Batch):
        try:
            batch.compile()
        except build.CompileError:
            # case by case: one that fails, or a batch too slow together,
            # hides no other
            jobs = []
            for k, case in enumerate(batch.cases):
                single = Batch(batch.lang, [case], batch.config, batch.work / "alone", batch.plans, batch.streams, k)
                single.stem = unique(batch.lang)
                try:
                    single.compile()
                except build.CompileError as why:
                    result.unbuilt.append(f"{batch.config.tag} {batch.lang} {case.name}: {str(why)[:300]}")
                    continue
                jobs += finish(single)
            return jobs
        return finish(batch)

    def finish(batch: Batch) -> list:
        check_mir(batch, result)
        # quality from one program per case: a case's code must not depend
        # on which others share its program
        for k, case in enumerate(batch.cases):
            try:
                measure(measured_from(batch, k), result)
            except build.CompileError as why:
                result.notes.append(f"{batch.config.tag} {batch.lang} {case.name} alone: {str(why)[:200]}")
        if args.no_dos:
            return []
        try:
            return [job_for(batch)]
        except build.CompileError as why:
            if len(batch.cases) == 1:
                result.unbuilt.append(f"{batch.config.tag} {batch.lang} {batch.cases[0].name}: {str(why)[:300]}")
                return []
            # each case alone, so one that cannot link hides no other
            result.notes.append(f"{batch.config.tag} {batch.lang} {batch.stem}: linked case by case: {str(why)[:200]}")
        jobs = []
        for k, case in enumerate(batch.cases):
            single = Batch(batch.lang, [case], batch.config, batch.work / "alone", batch.plans, batch.streams, k)
            single.stem = unique(batch.lang)
            try:
                single.compile()
                jobs.append(job_for(single))
            except build.CompileError as why:
                result.unbuilt.append(f"{batch.config.tag} {batch.lang} {case.name}: {str(why)[:300]}")
        return jobs

    def job_for(batch: Batch) -> dos.Job:
        exe = batch.work / "P.EXE"
        if batch.lang == "c":
            dos.link_c(batch.obj, exe, batch.work)
        elif batch.lang == "nib":
            dos.link_nib(batch.obj, exe, batch.work, batch.config.opt)
        job = dos.Job(batch.stem, "obj" if batch.lang == "bas" else "exe", batch.obj if batch.lang == "bas" else exe)
        job.expected = batch.expected
        job.cases = [(c.name, len(batch.streams[c.name])) for c in batch.cases]
        job.what = f"{batch.config.tag} {batch.lang} on DOS"
        return job

    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        jobs += [j for got in pool.map(one, all_batches) if got for j in got]
        if not (args.no_refs or args.inline):
            chosen = per_lang.get("c", ([],))[0]
            chunks = [chosen[k : k + 25] for k in range(0, len(chosen), 25)]
            list(pool.map(lambda job: references(job[1], job[0], work / "refs", result),
                          [(cfg, chunk) for cfg in configs for chunk in chunks]))

    if jobs:
        # one launch per configuration (and one for the oracle's BC checks), side by side
        groups: dict[str, list] = {}
        for job in jobs:
            groups.setdefault(job.what.split(" ")[0] if job.what != "oracle vs BC" else "validate", []).append(job)
        with ThreadPoolExecutor(max_workers=len(groups)) as pool:
            parts = pool.map(lambda kv: dos.run(kv[1], work / "dos" / kv[0]), groups.items())
        runs = {k: v for part in parts for k, v in part.items()}
        for job in jobs:
            got = runs.get(job.stem)
            if isinstance(got, str):
                result.wrong.append(f"{job.what} ({job.stem}): {got}")
                continue
            if isinstance(got, dos.Stopped):
                at, where = 0, "after the last case"
                for name, count in job.cases:
                    if got.partial[at : at + count] != job.expected[at : at + count]:
                        where = f"in {name}"
                        break
                    at += count
                result.wrong.append(f"{job.what} ({job.stem}) stopped {where}: {got.why}")
                continue
            at = 0
            for name, count in job.cases:
                mine, want = got[at : at + count], job.expected[at : at + count]
                if mine != want:
                    first = next((k for k in range(count) if k >= len(mine) or mine[k] != want[k]), count)
                    result.wrong.append(f"{job.what}: {name} report {first} is {mine[first:first + 3]}, "
                                        f"wants {want[first:first + 3]}")
                    break
                at += count

    judge(cases, langs, configs, result)
    ratchet = known.compare(result.short, result.judged)
    if args.write_known:
        known.write(result.short, ratchet.kept_issues, result.judged)
    report(cases, langs, configs, result, ratchet, work, time.monotonic() - started)
    if build.binaries_stamp() != stamp:
        print("the llrm binaries changed during the run: rerun")
        return 2
    bad = result.wrong or result.unbuilt or (not args.write_known and (ratchet.new or ratchet.fixed))
    return 1 if bad else 0


def report(cases, langs, configs, result: Result, ratchet, work: Path, seconds: float) -> None:
    lines = [f"loop corpus: {len(cases)} cases, {len(langs)} languages, {len(configs)} configurations, {seconds:.0f}s"]
    for lang in langs:
        skipped = sum(1 for (c, l) in result.skipped if l == lang)
        lines.append(f"  {lang}: {len(cases) - skipped} expressed, {skipped} not expressible")
    lines.append(f"correctness: {len(result.wrong)} wrong, {len(result.unbuilt)} not built")
    known_bugs = known.bugs()
    tagged = lambda line: next((f" (known: {issue})" for match, issue in known_bugs if re.search(match, line)), "")  # noqa: E731
    lines += [f"  WRONG {one}{tagged(one)}" for one in result.wrong]
    lines += [f"  UNBUILT {one}{tagged(one)}" for one in result.unbuilt]
    lines.append(f"quality: {len(result.short)} shortfalls; {len(ratchet.new)} new, {len(ratchet.fixed)} fixed")
    lines += [f"  NEW {' '.join(one)}" for one in sorted(ratchet.new)]
    lines += [f"  FIXED {' '.join(one)} (run --write-known)" for one in sorted(ratchet.fixed)]
    lines += [f"  note: {one}" for one in result.notes[:20]]
    text = "\n".join(lines)
    print(text)
    (work / "report.txt").write_text(text + "\n\n" + details(cases, langs, configs, result))
    rows = {f"{c}|{l}|{g}": [f.row() for f in fs] for (c, l, g), fs in result.facts.items()}
    (work / "facts.json").write_text(json.dumps(rows, indent=1))
    print(f"details: {work / 'report.txt'}")


def details(cases, langs, configs, result: Result) -> str:
    out = []
    for config in configs:
        out.append(f"== {config.tag}: instructions/memory/ivs/invariant loads per case")
        out.append(f"{'case':<28}" + "".join(f"{who:>18}" for who in [*langs, "ow", "gcc", "llvm"]) + "   want")
        for case in cases:
            cells = []
            for who in [*langs, "ow", "gcc", "llvm"]:
                facts = result.facts.get((case.name, who, config.tag))
                if not facts:
                    cells.append(f"{'-':>18}")
                    continue
                size, memory = total(facts)
                cells.append(f"{size:>6}/{memory}/{max(f.ivs for f in facts)}/{sum(f.invariant_loads for f in facts)}"
                             f"{'c' if any(f.calls for f in facts) else ' '}".rjust(18))
            wants = expect.want(case, "c")
            mirs = [result.mir_ivs.get((case.name, lang, config.tag)) for lang in langs]
            out.append(f"{case.name:<28}" + "".join(cells) + f"   {[w.ivs for w in wants]} mir {mirs}")
        out += matrix(cases, langs, config, result)
    out += causes(cases, langs, configs, result)
    out.append("\n== coverage")
    out += coverage(cases, result, langs)
    return "\n".join(out) + "\n"


def matrix(cases, langs, config, result: Result) -> list[str]:
    """concurrent's sharing grid: arrays against sizes and bases, each cell
    the induction variables wanted / measured per language and reference."""
    rows: dict[int, dict[str, str]] = {}
    columns: list[str] = []
    for case in cases:
        if case.family != "concurrent" or case.base or not {"form:index", "use:sum", "trip:n", "step:1",
                                                             "call:False"} <= case.tags:
            continue
        if any(t.startswith("index:") and t != "index:i" for t in case.tags):
            continue
        n = int(next(t for t in case.tags if t.startswith("arrays:")).split(":")[1])
        sizes = next(t for t in case.tags if t.startswith("sizes:")).split(":")[1]
        bases = "+".join(sorted({t.split(":")[1] for t in case.tags if t.startswith("base:")}))
        column = f"{sizes} {bases}"
        if column not in columns:
            columns.append(column)
        cells = []
        for who in [*langs, "ow", "gcc"]:
            facts = result.facts.get((case.name, who, config.tag))
            cells.append(str(max(f.ivs for f in facts)) if facts else "-")
        wants = expect.want(case, "c")
        want = wants[0].ivs if wants and wants[0].ivs is not None else "*"
        rows.setdefault(n, {})[column] = f"{want}:{'/'.join(cells)}"
    if not rows:
        return []
    out = [f"\n== {config.tag}: concurrent sharing, want:{'/'.join([*langs, 'ow', 'gcc'])} induction variables "
           "(* past the registers)"]
    out.append("n   " + "".join(f"{c[:22]:>24}" for c in columns))
    for n in sorted(rows):
        out.append(f"{n:<4}" + "".join(f"{rows[n].get(c, ''):>24}" for c in columns))
    return out


def causes(cases, langs, configs, result: Result) -> list[str]:
    """What falls short, commonest first: per check, then per dimension the
    share of measured cases that fall short, and the smallest example."""
    by_name = {c.name: c for c in cases}
    measured: dict[str, int] = {}
    for (name, who, config), facts in result.facts.items():
        if who in langs and facts:
            for tag in by_name[name].tags | {f"lang:{who}", f"config:{config}"}:
                measured[tag] = measured.get(tag, 0) + 1
    checks: dict[str, list] = {}
    for one in result.short:
        checks.setdefault(one[3], []).append(one)
    out = ["\n== what falls short, commonest first"]
    for check, found in sorted(checks.items(), key=lambda kv: -len(kv[1])):
        out.append(f"\n{check}: {len(found)}")
        counts: dict[str, int] = {}
        for name, lang, config, _ in found:
            for tag in by_name[name].tags | {f"lang:{lang}", f"config:{config}"}:
                counts[tag] = counts.get(tag, 0) + 1
        dims: dict[str, list] = {}
        for tag, n in counts.items():
            dim = tag.split(":")[0]
            if dim in ("same-size",):
                continue
            dims.setdefault(dim, []).append((n, tag))
        for dim, values in sorted(dims.items(), key=lambda kv: -max(n for n, _ in kv[1])):
            cells = ", ".join(f"{t.split(':', 1)[1]} {n}/{measured.get(t, n)}" for n, t in sorted(values, reverse=True)[:12])
            out.append(f"  {dim}: {cells}")
        name, lang, config, _ = min(found, key=lambda one: (len(by_name[one[0]].arrays), len(by_name[one[0]].body),
                                                             one[0], one[1], one[2]))
        facts = result.facts.get((name, lang, config), [])
        out.append(f"  smallest: {name} {lang} {config}: {by_name[name].note or ''}")
        for f in facts:
            out += [f"      {line}" for line in f.lines]
        out.append(f"      listing and MIR: {config}/{lang}/b*/stages (listing.asm, the last NN-*.ll)")
    return out


def coverage(cases, result: Result, langs) -> list[str]:
    seen: dict[str, dict[str, int]] = {}
    for case in cases:
        for tag in case.tags:
            dim, _, value = tag.partition(":")
            if not value:
                continue
            for lang in langs:
                if (case.name, lang) not in result.skipped:
                    seen.setdefault(dim, {}).setdefault(f"{value}/{lang}", 0)
                    seen[dim][f"{value}/{lang}"] += 1
    return [f"{dim}: " + ", ".join(f"{k}={n}" for k, n in sorted(values.items())) for dim, values in sorted(seen.items())]


if __name__ == "__main__":
    sys.exit(main())
