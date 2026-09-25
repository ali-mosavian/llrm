# Audited external-call profiles

Profiles turn reviewed ABI facts into reproducible compiler inputs. They are
not automatic output from `tools/contracts.py`: its unknown paths still need
review. Never infer purity or preservation from an input-only audit.

Pass `--contracts` more than once to compose independent audits. The loader
checks every profile against the shared `--contract-root`, refuses a symbol
declared by more than one profile rather than choosing by argument order, and
records one order-independent fingerprint for the combination. A single
profile keeps its own fingerprint unchanged.

`qrender-uglv.json` contains conservative cleanup-only contracts audited from
the pinned UGLV archive. `qrender-r-sweep.json` does the same for the assembly
span-row helper. These are separate because the evidence and artifacts have
independent provenance; callers compose only the sets they actually need.

```sh
uv run python -m qbopt.rewrite /path/to/objects/main.obj -o main-opt.obj \
  --native-fpu --contracts docs/qrender/contracts/qrender-main.json \
  --contract-root /path/to/objects

uv run python -m qbopt.rewrite /path/to/objects/r_span.obj -o r_span-opt.obj \
  --native-fpu \
  --contracts docs/qrender/contracts/qrender-uglv.json \
  --contracts /path/to/objects/basic-interfaces.json \
  --contract-root /path/to/objects

uv run python tools/stages.py /path/to/objects/main.obj --dump build/main-stages \
  --native-fpu --contracts docs/qrender/contracts/qrender-main.json \
  --contract-root /path/to/objects
```

The qrender profiles retain the per-module interfaces from the native build
audits in `docs/qrender/readme.md`. They require the pinned original
objects in their artifact maps, not optimized replacements. The eight early
modules share their original contract set; later modules keep separate sets
so that stronger cleanup claims do not silently spread to other callers.

Rebuild all 21 BASIC objects from the qbopt directory:

```sh
objects=/path/to/original/objects
output=/path/to/native/objects
mkdir -p "$output"
for module in main common h_bench h_frame qglarr qglchk qgldiff qglface sys vid \
              d_mdl d_surf d_turb mod_tex model r_bsp screen ent in_main pl_move view; do
  case "$module" in
    common|qglarr|qglchk|vid|d_turb|ent|in_main|view) profile=early-eight ;;
    *) profile=$(printf '%s' "$module" | tr '_' '-') ;;
  esac
  uv run python -m qbopt.rewrite "$objects/$module.obj" -o "$output/$module.obj" \
    --native-fpu --contracts "docs/qrender/contracts/qrender-$profile.json" \
    --contract-root "$objects" || break
done
```

Keep the original C/ASM objects for linking. A successful CLI exit alone is
not acceptance: require each output's completion marker and inspect any
refusal. The migration comparison and runtime evidence are recorded in the
integration document; a profile does not broaden that runtime coverage.

Version 1 has three top-level fields:

- `version`: integer `1`.
- `artifacts`: relative filename → lowercase SHA-256. Include every dependency
  used by the audit; all are checked, even when not directly called.
- `contracts`: symbol → `defined_in`, `inputs`, `evidence`, optional `cleanup`
  and optional archive `member`.

`defined_in` names a verified OMF object or library exporting the symbol. A
library is read in member order through its F0 page layout, stopping before
the F1 dictionary. `member` names the defining THEADR module; it is required
when more than one member exports the symbol and otherwise checked when
supplied. `inputs` uses
the ABI's word-register names (`ax`, `bx`, `cx`, `dx`, `si`, `di`, `bp`, `sp`,
`ds`, `es`, `flags`), not partial-register analyzer lanes. `cleanup` is an
even byte count or null; omitted means unknown. Evidence must identify the
reviewed mechanism. Memory effects, clobbers, callbacks and control flow stay
conservative. Unknown fields and duplicate keys are rejected.

Artifact paths resolve under `--contract-root`, defaulting to the profile's
directory. A stale/missing artifact or wrong symbol fails before output is
written. Link against the audited artifact versions. Validation establishes
identity, not the truth of a manually supplied contract.

The canonical profile hash is stored in the output marker and manifest.
Reprocessing an optimized object with a different profile is refused; start
from BC's original object instead. A completion marker, not the legacy
region count in `--report`, distinguishes emitted output from unchanged input.
