# Rows of `foldICmpMulConstant` (#5670)

QCport growers of comparing a product with a constant as the pin does.

| row | scope | bytes | cause |
|---|---|--:|---|
| MC-1 | QCport 65, -Os | mdl +17 (ent -16, net +1) | not isolated |
| MC-2 | QCport 65, -O2 | mdl +8 (ent -106, net -98) | not isolated |

C corpus (284 objects, m16/m32, O2/Os): 0 changed.
