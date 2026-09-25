.386

REFCODE segment byte public use16
assume ds:REFCODE
zero label byte
count = 0
relocation macro displacement, source
    local field
field label byte
REFCODE ends
REFMETA segment byte public use16
    dd offset field + displacement + 30h, source, 0
REFMETA ends
REFCODE segment byte public use16
    count = count + 1
endm
runtime macro source
    relocation 1, source
    db 9ah
    dd 0
endm
descriptor macro source
    relocation 1, source
    db 68h
    dw 0
endm
input macro source
    push ds
    pop es
    push es
    descriptor source
    runtime 39h
endm
scalar macro mnemonic, source
    wait
    relocation 2, source
    mnemonic dword ptr [zero]
endm

input 32h                       ; a, b, c: runtime READ order
input 3eh
input 4ah
relocation 3, 57h
mov dword ptr [zero], 0          ; s = 0
mov ax, 1
loopBody:
relocation 1, 0ach
mov word ptr [zero], ax          ; observable i before arithmetic
scalar fld, 6bh
scalar fadd, 70h
scalar fmul, 75h
scalar fstp, 7ah                 ; p rounds to SINGLE
wait
scalar fld, 81h
scalar fadd, 86h
scalar fdiv, 8bh
scalar fstp, 90h                 ; q rounds to SINGLE
wait
scalar fld, 97h
scalar fadd, 9ch
scalar fadd, 0a1h
scalar fstp, 0a6h                ; (s + p) + q, then SINGLE
wait
inc ax
cmp ax, 11
jne loopBody
relocation 1, 0ach
mov word ptr [zero], ax          ; final i = 11
descriptor 0b4h
runtime 0b7h
relocation 2, 0bdh
push word ptr [zero]
relocation 2, 0c1h
push word ptr [zero]
runtime 0c4h
descriptor 0c9h
runtime 0cch
runtime 0d1h
REFCODE ends
REFMETA segment byte public use16
dd count
db "QREF"
REFMETA ends
end
