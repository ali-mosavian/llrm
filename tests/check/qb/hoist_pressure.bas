' RUN: llrm-qb %s --dialect qb45 --runtime qb45 -Os --cpu 486 -S -o /dev/stdout
' Two inner loops each read four arrays' bases. Hoisted out of the outer loop too, nine values lived
' across both and were spilled to the frame, 9 stores before the loops and a reload per read (#529).
' CHECK-LABEL: $QB$MAIN proc
' CHECK-NOT: $QB$FRAME
' CHECK: $QB$MAIN endp
DEFINT A-Z
DIM A(9), B(9), C(9), D(9), E(9), F(9), G(9), H(9), O(9)
FOR T = 1 TO 100
  FOR X = 0 TO 3
    O(X + A(1)) = A(X + A(2)) + B(X + B(1)) + C(X + C(1)) + D(X + D(1))
  NEXT
  FOR Y = 0 TO 3
    O(Y + E(1)) = E(Y + E(2)) + F(Y + F(1)) + G(Y + G(1)) + H(Y + H(1))
  NEXT
NEXT
PRINT O(0)
