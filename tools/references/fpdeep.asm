.386

; JWasm flat binary: code followed by a relocation manifest, not executable data.
REFCODE segment byte public use16
assume ds:REFCODE
zero label byte
count = 0
relocation macro displacement, source, delta
    local field
field label byte
REFCODE ends
REFMETA segment byte public use16
    dd offset field + displacement + 30h, source, delta
REFMETA ends
REFCODE segment byte public use16
    count = count + 1
endm
store4 macro source, value, delta := <0>
    relocation 3, source, delta
    mov dword ptr [zero], value
endm
counter macro value
    relocation 2, 142h, 0
    mov word ptr [zero], value
endm
descriptor macro source
    relocation 1, source, 0
    db 68h
    dw 0
endm
runtime macro source
    relocation 1, source, 0
    db 9ah
    dd 0
endm
row macro index, singleBits, answer, labelField
    wait
    store4 7ah, singleBits
    descriptor labelField
    runtime 82h                 ; B$PSSD
    db 68h
    dw index
    runtime 8bh                 ; B$PSI2
    descriptor 90h              ; equals descriptor
    runtime 82h
    wait                        ; output calls can leave pending exceptions
    db 66h, 68h
    dd answer
    runtime 0a4h                ; B$PEI4
endm

store4 32h, 41400000h            ; p(1) = 12
store4 3eh, 41e00000h            ; p(2) = 28
store4 4ah, 42700000h            ; p(3) = 60
store4 56h, 40800000h            ; k = 4
counter 1
row 1, 43100000h, 144, 7fh
row 1, 40c00000h, 6, 0c7h
row 1, 3f000000h, 512, 10fh
counter 2
row 2, 44440000h, 784, 7fh
row 2, 41600000h, 14, 0c7h
row 2, 3f400000h, 768, 10fh
counter 3
row 3, 45610000h, 3600, 7fh
row 3, 41f00000h, 30, 0c7h
row 3, 3f600000h, 896, 10fh
counter 4
store4 14dh, 0
store4 14dh, 40280000h, 4        ; d = 12, before the first DOUBLE load
wait
store4 172h, 0
store4 172h, 40180000h, 4        ; e = 6
descriptor 177h
runtime 82h
wait
db 66h, 68h
dd 144
runtime 0a4h
descriptor 195h
runtime 82h
wait
db 66h, 68h
dd 6
runtime 0a4h
descriptor 1aeh
runtime 1b1h                    ; B$PESD
runtime 1b6h                    ; B$CEND, explicit END
REFCODE ends
REFMETA segment byte public use16
dd count
db "QREF"
REFMETA ends
end
