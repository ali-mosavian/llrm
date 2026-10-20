The pin's DependenceAnalysis tests (llvm/test/Analysis/DependenceAnalysis) without their CHECK lines, and
their `.expected` answers from `opt -passes=print<da>` for each RUN line, in order (`== OPTIONS`, then per function
`@name` and each pair's answer). `tools/da_oracle.py --refresh` writes them; a file is kept only where every run
agrees with llrm.

Not kept (a Banerjee run at 64-bit pointers needs more than 128 bits, DEVIATE D1; a range
attribute llrm drops; numbered types): Banerjee.ll, ExactRDIV.ll, MIVCheckConst.ll, PR51512.ll, banerjee-single-iteration.ll, banerjee-symbolic.ll, gcd-miv-overflow.ll, run-specific-dependence-test.ll
