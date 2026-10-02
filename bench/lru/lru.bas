' No far pointers or struct fields: far pointers become module-level arrays indexed in place; Cache is separate globals.
DEFINT A-Z
DECLARE FUNCTION BenchLru& (block AS INTEGER)
DECLARE SUB LruUse (block AS INTEGER)
DIM SHARED bord(2), bprev(2), bnext(2)
DIM SHARED bstamp(2) AS LONG
DIM SHARED clock AS LONG
DIM SHARED lhead(6), ltail(6)

PRINT LTRIM$(STR$(BenchLru&(1)))

SUB LruUse (block AS INTEGER)
    IF block < 0 THEN EXIT SUB
    clock = clock + 1
    bstamp(block) = clock
    chn = bord(block)
    IF bprev(block) >= 0 OR bnext(block) >= 0 OR lhead(chn) = block THEN EXIT SUB
    tail = ltail(chn)
    bprev(block) = tail
    bnext(block) = -1
    IF tail >= 0 THEN bnext(tail) = block ELSE lhead(chn) = block
    ltail(chn) = block
END SUB

FUNCTION BenchLru& (block AS INTEGER)
    clock = 7
    FOR index = 0 TO 2
        bord(index) = 0
        bprev(index) = -1
        bnext(index) = -1
        bstamp(index) = 0
    NEXT index
    FOR index = 0 TO 6
        lhead(index) = -1
        ltail(index) = -1
    NEXT index
    bord(1) = 2
    lhead(2) = 0
    ltail(2) = 0
    LruUse block
    BenchLru& = clock + bstamp(block) * 3& + bprev(block) * 5& + (bnext(block) + 1) * 7& + bnext(0) * 11& + lhead(2) * 13& + ltail(2) * 17&
END FUNCTION
