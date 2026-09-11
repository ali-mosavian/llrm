# Audited external-call profiles

Profiles turn reviewed ABI facts into reproducible compiler inputs. They are
not automatic output from `tools/contracts.py`: its unknown paths still need
review. Never infer purity or preservation from an input-only audit.

```sh
uv run python -m qbopt.rewrite /path/to/objects/main.obj -o main-opt.obj \
  --native-fpu --contracts docs/contracts/qrender-main.json \
  --contract-root /path/to/objects

uv run python tools/stages.py /path/to/objects/main.obj --dump build/main-stages \
  --native-fpu --contracts docs/contracts/qrender-main.json \
  --contract-root /path/to/objects
```

`qrender-main.json` records the 65 project-call interfaces audited for MAIN.
It requires the pinned original objects listed in its artifact map, not their
optimized replacements. It reproduces MAIN's accepted native emission; it
does not yet replace the other modules' audit scripts.

Version 1 has three top-level fields:

- `version`: integer `1`.
- `artifacts`: relative filename → lowercase SHA-256. Include every dependency
  used by the audit; all are checked, even when not directly called.
- `contracts`: symbol → `defined_in`, `inputs`, `evidence`, optional `cleanup`.

`defined_in` names a verified OMF object exporting the symbol. `inputs` uses
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
