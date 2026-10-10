;; name: B$MOVE, B$FILL, B$COPY
;; desc: string moves by dwords
;;
;; args: eax:ptr BlockOp | dst, src: bytes forward; or by bytes if `value` is not 0 (an EGA latch copy holds one
;;                       | byte); B$FILL: dst, words of a 16-bit value
;; retn: none
;;
;; chng: oct/26 written [ali]
;; obs.: the registers are C's (the target's own convention: eax, edx, ebx, ecx), and all but eax are kept; es is
;;       ds under the extender.

                .386
                .model  flat

                public  B$MOVE
                public  B$FILL
                public  B$COPY

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

                push    ecx
                push    edx
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
                pop     edx
                pop     ecx
                ret
B$MOVE          endp

;;::::::::::::::
;; B$FILL (eax: op)
B$FILL          proc

                push    ecx
                push    edx
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
                pop     edx
                pop     ecx
                ret
B$FILL          endp
;;::::::::::::::
;; B$COPY (eax: to, edx: from, ebx: count)
;; the bytes forward, dwords and then what is left; ecx, edx, esi and edi are kept
B$COPY          proc

                push    ecx
                push    esi
                push    edi
                mov     edi, eax
                mov     esi, edx
                mov     ecx, ebx
                cld
                shr     ecx, 2
                rep     movsd
                mov     ecx, ebx
                and     ecx, 3
                rep     movsb
                pop     edi
                pop     esi
                pop     ecx
                ret
B$COPY          endp
                end
