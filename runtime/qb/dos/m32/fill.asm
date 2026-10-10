;; name: B$FSEL, B$FBOX
;; desc: the box fill of the graphics screen: rows of a run of one operation with a pattern, edge bytes patched in and out
;;
;; args: B$FSEL (eax: operation, edx: the pattern byte) | 0 set, 1 and, 2 or, 3 xor; once for a primitive
;;       B$FBOX (eax: ptr FillBox)                      | the first row at dst, `rows` of them, `middle` bytes after an edge byte
;;                                                      | (a byte of and/xor constants) if `edges`, and before another; a row
;;                                                      | starts `steps[0]` after the one before it when that is even, `steps[1]`
;;                                                      | when odd (`phase`: which the first is, in bytes)
;; retn: none
;;
;; chng: oct/26 written [ali]
;; obs.: the run is `op mem, imm` with its opcode, mod/rm and immediate bytes patched by B$FSEL, as UGL patches its
;;       fillers; the code is written through the flat data selector.  The registers are C's (eax, edx, ebx, ecx); all but
;;       eax are kept.

                .386
                .model  flat

                public  B$FSEL
                public  B$FBOX

FillBox         struc
                dst             dword   ?
                rows            dword   ?
                middle          dword   ?
                steps           dword   2 dup (?)
                phase           dword   ?
                edges           dword   ?
                left            dword   ?               ;; the and in the low byte, the xor in the next
                right           dword   ?
FillBox         ends

.code
;;::::::::::::::
;; B$FSEL (eax: operation, edx: pattern byte)
B$FSEL          proc

                push    ebx
                mov     ebx, eax
                and     ebx, 3
                mov     ax, word ptr dword_ops[ebx * 2]
                mov     word ptr [dword_op], ax
                mov     ax, word ptr byte_ops[ebx * 2]
                mov     word ptr [byte_op], ax
                movzx   eax, dl
                imul    eax, 01010101h
                mov     dword ptr [dword_imm], eax
                mov     byte ptr [byte_imm], dl
                jmp     short @F                ;; the patched bytes are not in the prefetch queue
@@:
                pop     ebx
                ret
B$FSEL          endp

;;::::::::::::::
;; B$FBOX (eax: ptr FillBox)
B$FBOX          proc

                push    ebx
                push    ecx
                push    edx
                push    esi
                push    edi
                push    ebp
                mov     ebx, eax
                mov     esi, [ebx].FillBox.dst
                mov     ebp, [ebx].FillBox.rows
@@row:
                mov     edi, esi
                cmp     [ebx].FillBox.edges, 0
                je      @F
                mov     ecx, [ebx].FillBox.left
                mov     dl, [edi]
                and     dl, cl
                xor     dl, ch
                mov     [edi], dl
                inc     edi
@@:
                mov     ecx, [ebx].FillBox.middle
                mov     edx, ecx
                shr     ecx, 2
                jz      @@tail
@@dwords:
dword_op        db      0C7h, 07h               ;; mov dword ptr [edi], imm32; the opcode and mod/rm patched
dword_imm       dd      0
                add     edi, 4
                dec     ecx
                jnz     @@dwords
@@tail:
                and     edx, 3
                jz      @@right
@@bytes:
byte_op         db      0C6h, 07h               ;; mov byte ptr [edi], imm8
byte_imm        db      0
                inc     edi
                dec     edx
                jnz     @@bytes
@@right:
                cmp     [ebx].FillBox.edges, 0
                je      @F
                mov     ecx, [ebx].FillBox.right
                mov     dl, [edi]
                and     dl, cl
                xor     dl, ch
                mov     [edi], dl
@@:
                mov     eax, [ebx].FillBox.phase
                add     esi, [ebx + eax].FillBox.steps
                xor     eax, 4
                mov     [ebx].FillBox.phase, eax
                dec     ebp
                jnz     @@row
                pop     ebp
                pop     edi
                pop     esi
                pop     edx
                pop     ecx
                pop     ebx
                ret
B$FBOX          endp

;; the opcode and the mod/rm of each operation, for a dword and for a byte
dword_ops       dw      07C7h, 2781h, 0F81h, 3781h
byte_ops        dw      07C6h, 2780h, 0F80h, 3780h
                end
