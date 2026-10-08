' RUN: llrm-qb %s -O2 -march=i486 -S -o /dev/stdout
' An element's address computed in both arms of an IF and read after the join
' was a phi of two equal addresses: each arm built it in BX (`mov bx, offset
' A%+2`), where the access names it as its displacement (#386).
' CHECK-LABEL: $QB$MAIN proc
' CHECK-NOT: mov bx, offset VALUES%
' CHECK: word ptr VALUES%+2
DEFINT A-Z
DATA 5, 0, 5, 1
READ inputValue, branchChoice
' $DYNAMIC
DIM values(1 TO 4)
IF branchChoice THEN
    values(2) = inputValue + 1
ELSE
    values(2) = inputValue + 2
END IF
answer = values(2) + 3
PRINT answer
