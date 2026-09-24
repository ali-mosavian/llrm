# HARR: a BC object before and after

`suite/harr.bas` stores and immediately rereads a two-dimensional INTEGER
array element. The helper is `B$HARY`; `harr` is the benchmark name.

Before, BC performs the address calculation twice per inner iteration:

```asm
push column
push row
push 2
push descriptor
call B$HARY                 ; returns ES:BX
mov  [es:bx],value

push column
push row
push 2
push descriptor
call B$HARY                 ; recomputes the same address
mov  ax,[es:bx]
add  [total],ax
```

Its effective offset is:

```text
((column - lowerColumn) * rowCount + row - lowerRow) * elementSize + base
```

After CSE, LICM, forwarding and induction lowering, descriptor setup is
outside both loops:

```asm
mov  bx,[descriptor]
mov  es,[bx+2]              ; selector loaded once
mov  dx,44
add  dx,[bx+10]
mov  bx,dx                  ; outer pointer
```

The hot inner loop has no helper, multiply, descriptor load, selector reload,
or array reread:

```asm
inner:
mov  [es:di],si             ; matrix(row,column) = row + column
add  cx,si                  ; reuse the just-stored value
add  si,1                   ; column/value induction
add  di,42                  ; 21 INTEGERs * 2 bytes
cmp  si,dx
jne  inner
```

The outer latch uses `add bx,2`. `di` and `bx` are the inner and outer address
recurrences; `si` is the `row + column` recurrence.
