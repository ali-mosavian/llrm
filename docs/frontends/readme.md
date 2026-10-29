# Frontends

Audience: contributors and users choosing a frontend. Each frontend takes its
input to MIR, which the shared optimizer and backend take from there.

| Frontend | Input | Doc |
|---|---|---|
| `llrm-qb` | QuickBASIC, QBasic, PDS and VBDOS source, QuickrBASIC | [qb](qb/readme.md) |
| `llrm-nib` | Nib source | [nib](nib/readme.md) |
| `llrm-cc` | C source, through clang | [c](c/readme.md) |
| `llrm-omf` | Microsoft BASIC compiler objects | [bc](bc.md) |

Each frontend's calling conventions are in [abi](../abi/readme.md).
