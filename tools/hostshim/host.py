"""tools/hostshim: host shim for the DOS-flavoured bench programs on arm64-macos, one for all of them (no per-program edit):
flat spellings of far/huge, the POSIX OS layer under bench_io.h, a host buffer for the text screen, and the
data files a "// data:" header names, written into the run directory under their DOS spelling."""
import hashlib, os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
FLAT = ["-include", HERE + "/host_shim.h", "-Dfar=", "-Dnear=", "-Dhuge=", "-D__huge=", "-D__386__=1", "-DPHYSICAL_TEXT_SCREEN=((unsigned long)host_text_screen)"]
SCREEN_C = "unsigned char host_text_screen[80 * 25 * 2] __attribute__((aligned(16)));\n"

def include_flags(repo, prog):
    """bench/<prog> too: the programs say "../bench_io.h", and the source is compiled from a copy elsewhere."""
    return ["-I", f"{repo}/runtime/shared/posix", "-I", f"{repo}/bench/{prog}"]

def objects(cc, sdk, work, repo):
    """os.c and the screen buffer, compiled once by cc; linked into every program."""
    os.makedirs(work, exist_ok=True)
    open(f"{work}/screen.c", "w").write(SCREEN_C)
    objs = []
    for src in (f"{repo}/runtime/shared/posix/os.c", f"{work}/screen.c"):
        o = f"{work}/{os.path.basename(src)}.o"
        r = subprocess.run([cc, "-isysroot", sdk, "-O2", "-w", "-c", src, "-o", o], capture_output=True, text=True)
        if r.returncode: raise SystemExit(f"host shim {src}: {r.stderr[:300]}")
        objs.append(o)
    return objs

def write_data(repo, prog, rundir):
    """The program's "// data:" files (a path under bench/<prog>, or @signal) as rundir/NAME in DOS capitals."""
    m = re.search(r"^//\s*data:\s*(\S+)", open(f"{repo}/bench/{prog}/{prog}.c").read(), re.M)
    if not m: return None
    what = m[1]
    if what == "@signal":
        sys.path.insert(0, f"{repo}/tools/dosbatch"); import signal_gen
        data, name = signal_gen.signal(), "SIGNAL"
    elif what.startswith("@"):
        return None  # another corpus (grep's @dickens): that program reads it through input_shim.c
    else:
        data, name = open(os.path.normpath(f"{repo}/bench/{prog}/{what}"), "rb").read(), os.path.basename(what).upper()
    open(f"{rundir}/{name}", "wb").write(data)
    return name, hashlib.md5(data).hexdigest()

FAR = re.compile(r"\b(far|huge|__huge|__far)\b")
def flat_reading(repo, prog):
    """True when the source uses DOS far/huge: every compiler gets the same flat source, so rows compare but are not real-mode access."""
    return bool(FAR.search(open(f"{repo}/bench/{prog}/{prog}.c").read()))

def timings(sh, exe, cwd, k, reps):
    """us/run, reps of them. A program that cannot run twice in one process (jpeg_*: "not a JPEG" on the second call, it is a
    one-shot DOS program) is timed one call per process instead."""
    try:
        t = [float(x) for x in sh([exe, str(k), str(reps)], cwd=cwd).stdout.split()]
        if len(t) == reps: return t
    except ValueError: pass
    return [float(sh([exe, "1", "1"], cwd=cwd).stdout.split()[0]) for _ in range(reps)]
