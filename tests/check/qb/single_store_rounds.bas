' RUN: llrm-qb %s --isel=dag -Os -m16 -S -o /dev/stdout
' A SINGLE assigned to an element was rounded through a stack temp (`fstp`,
' `fld`) and then stored, which rounds again: 12 bytes more per assignment
' under dag than old (bench/fpbench m16 -Os +87 B).
' CHECK-LABEL: SETIT proc
' CHECK: fadd dword ptr [si]
' CHECK-NEXT: fstp dword ptr es:
DEFINT A-Z
DECLARE SUB Setit (a() AS SINGLE, b AS SINGLE)
DIM a(3) AS SINGLE
DIM b AS SINGLE
b = 1.5
Setit a(), b
PRINT a(1)
END

SUB Setit (a() AS SINGLE, b AS SINGLE)
    a(1) = a(1) + b
END SUB
