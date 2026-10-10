;; name: B$OUTB, B$OUTW, B$INB, B$CLI, B$STI
;; desc: I/O ports and the interrupt flag
;;
;; args: eax:port, edx:value | a byte or a word to the port: for a word the index is the low byte and the datum
;;                           | the high one, as the VGA's index/data pairs take it
;; retn: B$INB eax -> the byte
;;
;; chng: oct/26 written [ali]
;; obs.: the registers are C's (the target's own convention: eax, edx, ebx, ecx), so no thunk is needed; all
;;       but eax are kept.

                .386
                .model  flat

                public  B$OUTB
                public  B$OUTW
                public  B$INB
                public  B$CLI
                public  B$STI

.code
;;::::::::::::::
;; B$OUTB (eax: port, edx: value)
B$OUTB          proc

                xchg    eax, edx                ;; dx the port, al the byte
                out     dx, al
                xchg    eax, edx                ;; edx as it came
                ret
B$OUTB          endp

;;::::::::::::::
;; B$OUTW (eax: port, edx: value)
B$OUTW          proc

                xchg    eax, edx                ;; dx the port, ax the word
                out     dx, ax
                xchg    eax, edx
                ret
B$OUTW          endp

;;::::::::::::::
;; B$INB (eax: port)
B$INB           proc

                push    edx
                mov     edx, eax
                xor     eax, eax
                in      al, dx
                pop     edx
                ret
B$INB           endp

;;::::::::::::::
;; B$CLI (), B$STI ()
B$CLI           proc

                cli
                ret
B$CLI           endp

B$STI           proc

                sti
                ret
B$STI           endp
                end
