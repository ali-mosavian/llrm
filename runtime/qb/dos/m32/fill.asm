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
                public  B$FLIN
                public  B$FLIC
                public  B$FLIP

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

FillLine        struc
                dst             dword   ?
                count           dword   ?               ;; pixels: the longer side and one
                decision        dword   ?
                minor4          dword   ?
                step4           dword   ?
                steps           dword   2 dup (?)       ;; bytes to the next row, after an even and after an odd one
                phase           dword   ?               ;; 0 or 4: which of them follows the first
                x_major         dword   ?
                bit             dword   ?               ;; planar: the mask of the first pixel; CGA: its place in the byte
                pixels          dword   ?               ;; CGA: pixels in a byte
                tab             dword   ?               ;; CGA: the and of each place, then the xor
                color           dword   ?               ;; planar
FillLine        ends

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
                mov     dword ptr [set_pattern], eax
                mov     byte ptr [set_flag], 0
                test    ebx, ebx
                jnz     @F
                mov     byte ptr [set_flag], 1
@@:
                mov     byte ptr [byte_imm], dl
                mov     ax, word ptr byte_ops[ebx * 2]
                mov     word ptr [lx_op], ax
                mov     word ptr [ly_op], ax
                mov     byte ptr [lx_imm], dl
                mov     byte ptr [ly_imm], dl
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
                cld
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
                cmp     [set_flag], 0
                jne     @@set
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
                jmp     short @@right
@@set:                                          ;; a set is UGL's hlinef: dwords, then the bytes left
                mov     eax, [set_pattern]
                mov     edx, ecx
                shr     ecx, 2
                rep     stosd
                mov     ecx, edx
                and     ecx, 3
                rep     stosb
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

;;::::::::::::::
;; B$FLIN (eax: ptr FillLine): the 256-colour mode, a pixel a byte
B$FLIN          proc

                push    ebx
                push    ecx
                push    edx
                push    esi
                push    edi
                push    ebp
                mov     esi, eax
                mov     edi, [esi].FillLine.dst
                mov     ecx, [esi].FillLine.count
                mov     ebx, [esi].FillLine.decision
                mov     edx, [esi].FillLine.minor4
                mov     ebp, [esi].FillLine.step4
                mov     eax, [esi].FillLine.steps
                cmp     [esi].FillLine.x_major, 0
                je      @@ymajor
@@xloop:
lx_op           db      0C6h, 07h               ;; mov byte ptr [edi], imm8; patched as the box loop's is
lx_imm          db      0
                inc     edi
                test    ebx, ebx
                js      @@xflat
                add     ebx, ebp
                add     edi, eax
                dec     ecx
                jnz     @@xloop
                jmp     @@done
@@xflat:
                add     ebx, edx
                dec     ecx
                jnz     @@xloop
                jmp     @@done
@@ymajor:
ly_op           db      0C6h, 07h
ly_imm          db      0
                add     edi, eax
                test    ebx, ebx
                js      @@yflat
                add     ebx, ebp
                inc     edi
                dec     ecx
                jnz     @@ymajor
                jmp     @@done
@@yflat:
                add     ebx, edx
                dec     ecx
                jnz     @@ymajor
@@done:
                pop     ebp
                pop     edi
                pop     esi
                pop     edx
                pop     ecx
                pop     ebx
                ret
B$FLIN          endp

;;::::::::::::::
;; B$FLIC (eax: ptr FillLine): the CGA modes, pixels packed in bytes, each under its own and and xor
B$FLIC          proc

                push    ebx
                push    ecx
                push    edx
                push    esi
                push    edi
                push    ebp
                mov     esi, eax
                mov     edi, [esi].FillLine.dst
                mov     ecx, [esi].FillLine.count
                mov     ebx, [esi].FillLine.decision
                mov     ebp, [esi].FillLine.bit
                cmp     [esi].FillLine.x_major, 0
                je      @@ymajor
@@xloop:
                mov     edx, [esi].FillLine.tab
                mov     al, [edi]
                and     al, [edx + ebp]
                xor     al, [edx + ebp + 8]
                mov     [edi], al
                inc     ebp
                cmp     ebp, [esi].FillLine.pixels
                jb      @F
                xor     ebp, ebp
                inc     edi
@@:
                test    ebx, ebx
                js      @@xflat
                add     ebx, [esi].FillLine.step4
                mov     edx, [esi].FillLine.phase
                add     edi, [esi + edx].FillLine.steps
                xor     edx, 4
                mov     [esi].FillLine.phase, edx
                dec     ecx
                jnz     @@xloop
                jmp     @@done
@@xflat:
                add     ebx, [esi].FillLine.minor4
                dec     ecx
                jnz     @@xloop
                jmp     @@done
@@ymajor:
                mov     edx, [esi].FillLine.tab
                mov     al, [edi]
                and     al, [edx + ebp]
                xor     al, [edx + ebp + 8]
                mov     [edi], al
                mov     edx, [esi].FillLine.phase
                add     edi, [esi + edx].FillLine.steps
                xor     edx, 4
                mov     [esi].FillLine.phase, edx
                test    ebx, ebx
                js      @@yflat
                add     ebx, [esi].FillLine.step4
                inc     ebp
                cmp     ebp, [esi].FillLine.pixels
                jb      @F
                xor     ebp, ebp
                inc     edi
@@:
                dec     ecx
                jnz     @@ymajor
                jmp     @@done
@@yflat:
                add     ebx, [esi].FillLine.minor4
                dec     ecx
                jnz     @@ymajor
@@done:
                pop     ebp
                pop     edi
                pop     esi
                pop     edx
                pop     ecx
                pop     ebx
                ret
B$FLIC          endp

;;::::::::::::::
;; B$FLIP (eax: ptr FillLine): the planar modes, the bit mask written for each pixel, the colour through write mode 2; the
;; mask is FFh again at the end
B$FLIP          proc

                push    ebx
                push    ecx
                push    edx
                push    esi
                push    edi
                push    ebp
                mov     esi, eax
                mov     edi, [esi].FillLine.dst
                mov     ecx, [esi].FillLine.count
                mov     ebx, [esi].FillLine.decision
                mov     ebp, [esi].FillLine.bit
                mov     edx, 3CEh
                cmp     [esi].FillLine.x_major, 0
                je      @@ymajor
@@xloop:
                mov     eax, ebp
                shl     eax, 8
                mov     al, 8
                out     dx, ax                  ;; the bit mask
                mov     al, [edi]               ;; loads the latches
                mov     al, byte ptr [esi].FillLine.color
                mov     [edi], al
                shr     ebp, 1
                jnz     @F
                mov     ebp, 80h
                inc     edi
@@:
                test    ebx, ebx
                js      @@xflat
                add     ebx, [esi].FillLine.step4
                add     edi, [esi].FillLine.steps
                dec     ecx
                jnz     @@xloop
                jmp     @@done
@@xflat:
                add     ebx, [esi].FillLine.minor4
                dec     ecx
                jnz     @@xloop
                jmp     @@done
@@ymajor:
                mov     eax, ebp
                shl     eax, 8
                mov     al, 8
                out     dx, ax
                mov     al, [edi]
                mov     al, byte ptr [esi].FillLine.color
                mov     [edi], al
                add     edi, [esi].FillLine.steps
                test    ebx, ebx
                js      @@yflat
                add     ebx, [esi].FillLine.step4
                shr     ebp, 1
                jnz     @F
                mov     ebp, 80h
                inc     edi
@@:
                dec     ecx
                jnz     @@ymajor
                jmp     @@done
@@yflat:
                add     ebx, [esi].FillLine.minor4
                dec     ecx
                jnz     @@ymajor
@@done:
                mov     eax, 0FF08h
                out     dx, ax
                pop     ebp
                pop     edi
                pop     esi
                pop     edx
                pop     ecx
                pop     ebx
                ret
B$FLIP          endp

set_pattern     dd      0
set_flag        db      0
dword_ops       dw      07C7h, 2781h, 0F81h, 3781h
byte_ops        dw      07C6h, 2780h, 0F80h, 3780h
                end
