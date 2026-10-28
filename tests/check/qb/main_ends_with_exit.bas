' RUN: llrm-qb %s -fqb-runtime=llrm --isel={dag | old} -O2 -march=i486 {-m16 | -m32} -S -o /dev/stdout
' A program whose last statement calls the runtime must still end in the
' runtime's exit. The DAG's sibling call made that statement a `jmp` Return and
' `ends_program` lost the `call B$CENP`, so $QB$MAIN fell out of its end into
' garbage (fib at -m32: "6765", then DOS/32A exception 06h at EIP 12345678).
' CHECK-LABEL: $QB$MAIN proc
' CHECK: call {{(far ptr )?}}B$PEI2
' CHECK-NEXT: call {{(far ptr )?}}B$CENP
' CHECK-NEXT: $QB$MAIN endp
PRINT 1
