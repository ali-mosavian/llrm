;; name: B$MOVE, B$FILL
;; desc: string moves by dwords
;;
;; args: eax:ptr BlockOp | dst, src: bytes forward; or by bytes if `value` is not 0 (an EGA latch copy holds one
;;                       | byte); B$FILL: dst, words of a 16-bit value
;; retn: none
;;
;; chng: oct/26 written [ali]
;; obs.: the registers are C's (the target's own convention: eax, edx, ebx, ecx); es is ds under the extender.

                .386
                .model  flat

                public  B$MOVE
                public  B$FILL

BlockOp         struc
                dst             dword   ?
                src             dword   ?
                count           dword   ?               ;; bytes, or words for B$FILL
                value           dword   ?               ;; for B$FILL
BlockOp         ends

.code
;;::::::::::::::
;; B$MOVE (eax: op)
B$MOVE          proc

                push    esi
                push    edi
                mov     edi, [eax].BlockOp.dst
                mov     esi, [eax].BlockOp.src
                mov     ecx, [eax].BlockOp.count
                mov     edx, [eax].BlockOp.value
                cld
                test    edx, edx
                jnz     @F
                mov     eax, ecx
                shr     ecx, 2
                rep     movsd
                mov     ecx, eax
                and     ecx, 3
@@:
                rep     movsb
                pop     edi
                pop     esi
                ret
B$MOVE          endp

;;::::::::::::::
;; B$FILL (eax: op)
B$FILL          proc

                push    edi
                mov     edi, [eax].BlockOp.dst
                movzx   edx, word ptr [eax].BlockOp.value
                mov     ecx, edx
                shl     ecx, 16
                or      edx, ecx                ;; the word twice
                mov     ecx, [eax].BlockOp.count
                mov     eax, edx
                cld
                shr     ecx, 1
                rep     stosd
                jnc     @F
                stosw
@@:
                pop     edi
                ret
B$FILL          endp
                end
