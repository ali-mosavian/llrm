# Rows of SimplifyCFG's `requestResimplify` (#5662)

QCport growers of repeating a block's visit after `simplifyCondBranch` folds it, as the pin's `SimplifyCFGOpt::run` does.

| row | scope | bytes | cause |
|---|---|--:|---|
| RS-1 | QCport 65, -Os | +65 (8 modules; mdl +7, combat +16, pl_trace +16, triggers +18, weapons +9, savegame +4, sb_build +3, menu -8) | not isolated |
| RS-2 | QCport 65, -O2 | +100 (10 modules; combat -86, weapons +77, snd +30, mdl +30) | not isolated |

C corpus (284 objects, m16/m32, O2/Os): 0 changed. The change is what the pin does: a compare chain folds into its first block one block at a time (QCport `_savegame_selftest`: 12 `br i1` become 6, the pin's count).
