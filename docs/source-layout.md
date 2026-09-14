# Source layout

The package root contains pipeline orchestration: `flow`, `rewrite`, and
`wholeseg`. Implementation modules are grouped by responsibility:

| Package | Responsibility |
| --- | --- |
| `objectfile` | OMF records, module metadata, relocation, object writing |
| `frontend` | Decode, partition, recognize BC idioms, raise SSA values |
| `cfront` | C through Open Watcom's front end (`owshim/`): its cg stream as HIR, raised to MIR |
| `model` | MIR, LIR, decoded IR, floating semantics, phase interfaces |
| `analysis` | SSA, liveness, ranges, loops, induction, memory/value facts |
| `optimize` | MIR transformations |
| `backend` | Lowering, instruction selection, allocation, frame/layout, peepholes |
| `abi` | Runtime contracts and the adjacent `runtime.toml` data file |
| `legacy` | Older lifting, call absorption, and MIR-era register allocation |
| `cycles` | Existing vendored instruction-cost model |

Import modules from their owning packages, for example:

```python
from qbopt.model import mir
from qbopt.backend import lower
from qbopt.frontend import raising_floats
```

There are no flat-module compatibility wrappers. Tests and tools use the
same package paths as production. Public pipeline entry points such as
`qbopt.wholeseg` stay at the root.

This organization makes ownership visible; it does not claim the architectural
migration is finished. Existing dependency cycles and machine-aware MIR
transformations remain debt documented in `split.md`. In particular, moving
a module does not make its current imports a permitted long-term dependency.
Recognition belongs in the frontend, machine-independent optimization above
lowering, and physical placement in the backend. `model.ir` is the older
decoded machine representation, not the machine-independent MIR contract.
