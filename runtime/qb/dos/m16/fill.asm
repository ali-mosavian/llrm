;; name: B$FSEL, B$FBOX
;; desc: the box fill of the graphics screen: rows of a run of one operation with a pattern, edge bytes patched in and out
;;
;; args: B$FSEL (ax: operation, dx: the pattern byte) | 0 set, 1 and, 2 or, 3 xor; once for a primitive
;;       B$FBOX (ax: ptr FillBox)                     | the first row at dst, `rows` of them, `middle` bytes after an edge byte
;;                                                    | (a byte of and/xor constants) if `edges`, and before another; a row
;;                                                    | starts `steps[0]` after the one before it when that is even, `steps[1]`
;;                                                    | when odd (`phase`: which the first is, in bytes)
;; retn: none
;;
;; chng: oct/26 written [ali]
;; obs.: the run is `op mem, imm` with its opcode, mod/rm and immediate bytes patched by B$FSEL, as UGL patches its
;;       fillers; the code is written through cs.  The registers are C's (regparm3: ax, dx, cx); bx, si, di, bp, es kept.

                .model  medium, pascal
                .386
                option  proc:private

FillBox         struc
                dst             dword   ?
                rows            word    ?
                middle          word    ?
                steps           word    2 dup (?)
                phase           word    ?
                edges           word    ?
                left            word    ?               ;; the and in the low byte, the xor in the high
                right           word    ?
FillBox         ends

FillLine        struc
                dst             dword   ?
                count           word    ?               ;; pixels: the longer side and one
                decision        word    ?
                minor4          word    ?
                step4           word    ?
                steps           word    2 dup (?)       ;; bytes to the next row, after an even and after an odd one
                phase           word    ?               ;; 0 or 2: which of them follows the first
                x_major         word    ?
                bit             word    ?               ;; planar: the mask of the first pixel; CGA: its place in the byte
                pixels          word    ?               ;; CGA: pixels in a byte
                tab             word    ?               ;; CGA: the and of each place, then the xor
                color           word    ?               ;; planar
FillLine        ends

.code
;;::::::::::::::
;; B$FSEL (ax: operation, dx: pattern byte)
B$FSEL          proc    public

                push    bx
                mov     bx, ax
                and     bx, 3
                shl     bx, 1
                mov     ax, cs:dword_ops[bx]
                mov     word ptr cs:[dword_op + 2], ax
                mov     ax, cs:byte_ops[bx]
                mov     word ptr cs:[byte_op + 1], ax
                movzx   eax, dl
                imul    eax, 01010101h
                mov     dword ptr cs:[dword_imm], eax
                mov     dword ptr cs:[set_pattern], eax
                mov     al, 0
                cmp     bx, 0
                jne     @F
                mov     al, 1
@@:
                mov     byte ptr cs:[set_flag], al
                mov     byte ptr cs:[byte_imm], dl
                mov     ax, cs:byte_ops[bx]
                mov     word ptr cs:[lx_op + 1], ax
                mov     word ptr cs:[ly_op + 1], ax
                mov     byte ptr cs:[lx_imm], dl
                mov     byte ptr cs:[ly_imm], dl
                jmp     short @F                ;; the patched bytes are not in the prefetch queue
@@:
                pop     bx
                ret
B$FSEL          endp

;;::::::::::::::
;; B$FBOX (ax: ptr FillBox)
B$FBOX          proc    public

                push    si
                push    di
                push    bp
                push    es
                cld
                mov     bx, ax
                les     si, [bx].FillBox.dst
                mov     bp, [bx].FillBox.rows
@@row:
                mov     di, si
                cmp     [bx].FillBox.edges, 0
                je      @F
                mov     ax, [bx].FillBox.left
                mov     dl, es:[di]
                and     dl, al
                xor     dl, ah
                mov     es:[di], dl
                inc     di
@@:
                mov     cx, [bx].FillBox.middle
                cmp     cs:set_flag, 0
                jne     @@set
                mov     dx, cx
                shr     cx, 2
                jz      @@tail
@@dwords:
dword_op        db      66h, 26h, 0C7h, 05h     ;; es: mov dword ptr [di], imm32; the opcode and mod/rm patched
dword_imm       dd      0
                add     di, 4
                dec     cx
                jnz     @@dwords
@@tail:
                and     dx, 3
                jz      @@right
@@bytes:
byte_op         db      26h, 0C6h, 05h          ;; es: mov byte ptr [di], imm8
byte_imm        db      0
                inc     di
                dec     dx
                jnz     @@bytes
                jmp     short @@right
@@set:                                          ;; a set is UGL's hlinef: dwords, then the bytes left
                mov     eax, cs:set_pattern
                mov     dx, cx
                shr     cx, 2
                rep     stosd
                mov     cx, dx
                and     cx, 3
                rep     stosb
@@right:
                cmp     [bx].FillBox.edges, 0
                je      @F
                mov     ax, [bx].FillBox.right
                mov     dl, es:[di]
                and     dl, al
                xor     dl, ah
                mov     es:[di], dl
@@:
                mov     ax, [bx].FillBox.phase
                mov     di, ax
                add     si, [bx+di].FillBox.steps
                xor     ax, 2
                mov     [bx].FillBox.phase, ax
                dec     bp
                jnz     @@row
                pop     es
                pop     bp
                pop     di
                pop     si
                ret
B$FBOX          endp

;; the opcode and the mod/rm of each operation, for a dword and for a byte

;;::::::::::::::
;; B$FLIN (ax: ptr FillLine): the 256-colour mode, a pixel a byte
B$FLIN          proc    public

                push    bx
                push    cx
                push    dx
                push    si
                push    di
                push    bp
                push    es
                mov     si, ax
                les     di, [si].FillLine.dst
                mov     cx, [si].FillLine.count
                mov     bx, [si].FillLine.decision
                mov     dx, [si].FillLine.minor4
                mov     bp, [si].FillLine.step4
                mov     ax, [si].FillLine.steps
                cmp     [si].FillLine.x_major, 0
                je      @@ymajor
@@xloop:
lx_op           db      26h, 0C6h, 05h          ;; es: mov byte ptr [di], imm8; patched as the box loop's is
lx_imm          db      0
                inc     di
                test    bx, bx
                js      @@xflat
                add     bx, bp
                add     di, ax
                dec     cx
                jnz     @@xloop
                jmp     @@done
@@xflat:
                add     bx, dx
                dec     cx
                jnz     @@xloop
                jmp     @@done
@@ymajor:
ly_op           db      26h, 0C6h, 05h
ly_imm          db      0
                add     di, ax
                test    bx, bx
                js      @@yflat
                add     bx, bp
                inc     di
                dec     cx
                jnz     @@ymajor
                jmp     @@done
@@yflat:
                add     bx, dx
                dec     cx
                jnz     @@ymajor
@@done:
                pop     es
                pop     bp
                pop     di
                pop     si
                pop     dx
                pop     cx
                pop     bx
                ret
B$FLIN          endp

;;::::::::::::::
;; B$FLIC (ax: ptr FillLine): the CGA modes, pixels packed in bytes, each under its own and and xor
B$FLIC          proc    public

                push    bx
                push    cx
                push    dx
                push    si
                push    di
                push    bp
                push    es
                mov     si, ax
                les     di, [si].FillLine.dst
                mov     cx, [si].FillLine.count
                mov     dx, [si].FillLine.decision
                mov     bp, [si].FillLine.bit
                cmp     [si].FillLine.x_major, 0
                je      @@ymajor
@@xloop:
                mov     bx, [si].FillLine.tab
                add     bx, bp
                mov     al, es:[di]
                and     al, [bx]
                xor     al, [bx + 8]
                mov     es:[di], al
                inc     bp
                cmp     bp, [si].FillLine.pixels
                jb      @F
                xor     bp, bp
                inc     di
@@:
                test    dx, dx
                js      @@xflat
                add     dx, [si].FillLine.step4
                mov     bx, [si].FillLine.phase
                add     di, [bx + si].FillLine.steps
                xor     bx, 2
                mov     [si].FillLine.phase, bx
                dec     cx
                jnz     @@xloop
                jmp     @@done
@@xflat:
                add     dx, [si].FillLine.minor4
                dec     cx
                jnz     @@xloop
                jmp     @@done
@@ymajor:
                mov     bx, [si].FillLine.tab
                add     bx, bp
                mov     al, es:[di]
                and     al, [bx]
                xor     al, [bx + 8]
                mov     es:[di], al
                mov     bx, [si].FillLine.phase
                add     di, [bx + si].FillLine.steps
                xor     bx, 2
                mov     [si].FillLine.phase, bx
                test    dx, dx
                js      @@yflat
                add     dx, [si].FillLine.step4
                inc     bp
                cmp     bp, [si].FillLine.pixels
                jb      @F
                xor     bp, bp
                inc     di
@@:
                dec     cx
                jnz     @@ymajor
                jmp     @@done
@@yflat:
                add     dx, [si].FillLine.minor4
                dec     cx
                jnz     @@ymajor
@@done:
                pop     es
                pop     bp
                pop     di
                pop     si
                pop     dx
                pop     cx
                pop     bx
                ret
B$FLIC          endp

;;::::::::::::::
;; B$FLIP (ax: ptr FillLine): the planar modes, the bit mask written for each pixel, the colour through write mode 2; the
;; mask is FFh again at the end
B$FLIP          proc    public

                push    bx
                push    cx
                push    dx
                push    si
                push    di
                push    bp
                push    es
                mov     si, ax
                les     di, [si].FillLine.dst
                mov     cx, [si].FillLine.count
                mov     bx, [si].FillLine.decision
                mov     bp, [si].FillLine.bit
                mov     dx, 3CEh
                cmp     [si].FillLine.x_major, 0
                je      @@ymajor
@@xloop:
                mov     ax, bp
                shl     ax, 8
                mov     al, 8
                out     dx, ax                  ;; the bit mask
                mov     al, es:[di]             ;; loads the latches
                mov     al, byte ptr [si].FillLine.color
                mov     es:[di], al
                shr     bp, 1
                jnz     @F
                mov     bp, 80h
                inc     di
@@:
                test    bx, bx
                js      @@xflat
                add     bx, [si].FillLine.step4
                add     di, [si].FillLine.steps
                dec     cx
                jnz     @@xloop
                jmp     @@done
@@xflat:
                add     bx, [si].FillLine.minor4
                dec     cx
                jnz     @@xloop
                jmp     @@done
@@ymajor:
                mov     ax, bp
                shl     ax, 8
                mov     al, 8
                out     dx, ax
                mov     al, es:[di]
                mov     al, byte ptr [si].FillLine.color
                mov     es:[di], al
                add     di, [si].FillLine.steps
                test    bx, bx
                js      @@yflat
                add     bx, [si].FillLine.step4
                shr     bp, 1
                jnz     @F
                mov     bp, 80h
                inc     di
@@:
                dec     cx
                jnz     @@ymajor
                jmp     @@done
@@yflat:
                add     bx, [si].FillLine.minor4
                dec     cx
                jnz     @@ymajor
@@done:
                mov     ax, 0FF08h
                out     dx, ax
                pop     es
                pop     bp
                pop     di
                pop     si
                pop     dx
                pop     cx
                pop     bx
                ret
B$FLIP          endp

set_pattern     dd      0
set_flag        db      0
dword_ops       dw      05C7h, 2581h, 0D81h, 3581h
byte_ops        dw      05C6h, 2580h, 0D80h, 3580h
                end
