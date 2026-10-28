' RUN: llrm-qb %s -fqb-runtime=llrm --isel={dag | old} -Os -fno-inline-functions -fno-inline-functions-called-once -march=i486 -m32 -S -o /dev/stdout
' A call of a BASIC function disturbs nothing the caller must save: the DAG selector listed the whole caller-saved set, so
' every caller pushed and popped ebx, ecx, edx, esi and edi it never wrote (qb fib -Os bq32: 591064 instructions against 350253).
' CHECK-LABEL: CALLER proc
' CHECK-NOT: push e{{(bx|cx|dx|si|di)}}
' CHECK: call LEAF
' CHECK: call LEAF
' CHECK-NOT: pop e{{(bx|cx|dx|si|di)}}
' CHECK: ret
DEFINT A-Z
DECLARE FUNCTION Leaf% (n AS INTEGER)
DECLARE FUNCTION Caller& (n AS INTEGER)
PRINT LTRIM$(STR$(Caller&(3)))

FUNCTION Leaf% (n AS INTEGER)
    IF n < 0 THEN Leaf% = -n * 7 + 3 ELSE Leaf% = n * 5 - 9
END FUNCTION

FUNCTION Caller& (n AS INTEGER)
    Caller& = Leaf%(n) * 1000& + Leaf%(n + 1)
END FUNCTION
