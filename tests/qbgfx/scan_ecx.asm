;; name: scan_ecx.asm
;; desc: B$FSCC called with the upper half of ecx dirty, as C leaves it
                .386
                .model  flat

                public  B$DIRTY
                extrn   B$FSCC:near

.code
B$DIRTY         proc
                mov     ecx, 0A0000h
                jmp     B$FSCC
B$DIRTY         endp
                end
