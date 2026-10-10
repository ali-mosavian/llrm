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
                ystep           word    ?               ;; bytes to the next row (CGA: 80, down; -80, up)
                style           word    ?               ;; the 16 bits of the line style, the first pixel's the high bit
                x_major         word    ?               ;; 1 along x, 0 along y, 2 straight down or up
                pmask           word    ?               ;; the first pixel's mask in its byte (EGA: one bit; CGA: its pixel's bits)
                bpp             word    ?               ;; CGA: bits a pixel
                color           word    ?               ;; the byte of the colour: CGA all its pixels, EGA the nibble
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
                mov     word ptr cs:[lx_op], ax
                mov     word ptr cs:[ly_op], ax
                mov     word ptr cs:[sx_op], ax
                mov     word ptr cs:[sy_op], ax
                mov     word ptr cs:[lv_op], ax
                mov     word ptr cs:[sv_op], ax
                mov     byte ptr cs:[lx_imm], dl
                mov     byte ptr cs:[ly_imm], dl
                mov     byte ptr cs:[sx_imm], dl
                mov     byte ptr cs:[sy_imm], dl
                mov     byte ptr cs:[lv_imm], dl
                mov     byte ptr cs:[sv_imm], dl
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
;; B$FLIN (ax: ptr FillLine): the 256-colour mode, a pixel a byte; the colour and `mov` patched in by B$FSEL
B$FLIN          proc    public

                push    bx
                push    cx
                push    dx
                push    si
                push    di
                push    bp
                mov     si, ax
                mov     ax, [si].FillLine.x_major
                mov     cs:[lin_mode], ax
                les     di, [si].FillLine.dst
                mov     cx, [si].FillLine.count
                mov     bx, [si].FillLine.decision
                mov     dx, [si].FillLine.minor4
                mov     bp, [si].FillLine.step4
                mov     ax, [si].FillLine.ystep
                mov     si, [si].FillLine.style
                cmp     word ptr cs:[lin_mode], 1
                je      @@x
                cmp     word ptr cs:[lin_mode], 2
                je      @@v
                cmp     si, 0FFFFh
                jne     @@sy
@@ly:
                db      26h
ly_op           db      0C6h, 05h
ly_imm          db      0
                add     di, ax
                test    bx, bx
                js      @@lyf
                add     bx, bp
                inc     di
                dec     cx
                jnz     @@ly
                jmp     @@done
@@lyf:
                add     bx, dx
                dec     cx
                jnz     @@ly
                jmp     @@done
@@sy:
                rol     si, 1
                jnc     @F
                db      26h
sy_op           db      0C6h, 05h
sy_imm          db      0
@@:
                add     di, ax
                test    bx, bx
                js      @@syf
                add     bx, bp
                inc     di
                dec     cx
                jnz     @@sy
                jmp     @@done
@@syf:
                add     bx, dx
                dec     cx
                jnz     @@sy
                jmp     @@done
@@x:
                cmp     si, 0FFFFh
                jne     @@sx
@@lx:
                db      26h
lx_op           db      0C6h, 05h
lx_imm          db      0
                inc     di
                test    bx, bx
                js      @@lxf
                add     bx, bp
                add     di, ax
                dec     cx
                jnz     @@lx
                jmp     @@done
@@lxf:
                add     bx, dx
                dec     cx
                jnz     @@lx
                jmp     @@done
@@sx:
                rol     si, 1
                jnc     @F
                db      26h
sx_op           db      0C6h, 05h
sx_imm          db      0
@@:
                inc     di
                test    bx, bx
                js      @@sxf
                add     bx, bp
                add     di, ax
                dec     cx
                jnz     @@sx
                jmp     @@done
@@sxf:
                add     bx, dx
                dec     cx
                jnz     @@sx
                jmp     @@done
@@v:
                cmp     si, 0FFFFh
                jne     @@sv
@@lv:
                db      26h
lv_op           db      0C6h, 05h
lv_imm          db      0
                add     di, ax
                dec     cx
                jnz     @@lv
                jmp     @@done
@@sv:
                rol     si, 1
                jnc     @F
                db      26h
sv_op           db      0C6h, 05h
sv_imm          db      0
@@:
                add     di, ax
                dec     cx
                jnz     @@sv
@@done:
                pop     bp
                pop     di
                pop     si
                pop     dx
                pop     cx
                pop     bx
                ret
B$FLIN          endp

;;::::::::::::::
;; B$FLIC (ax: ptr FillLine): the CGA modes, as QB's: the mask of the pixels of a line that fall in a byte is accumulated and
;; the byte blended once; a step down or up a row toggles the bank (bit 13) and adds 80 when it comes back to the first
B$FLIC          proc    public

                push    bx
                push    cx
                push    dx
                push    si
                push    di
                push    bp
                mov     si, ax
                mov     ax, [si].FillLine.minor4
                mov     cs:[cx_i1v], ax
                mov     cs:[cy_i1v], ax
                mov     ax, [si].FillLine.step4
                mov     cs:[cx_i2v], ax
                mov     cs:[cy_i2v], ax
                mov     ax, [si].FillLine.ystep
                mov     cs:[cx_sv], ax
                mov     cs:[cy_sv], ax
                mov     cs:[cv_sv], ax
                mov     bl, 75h                 ;; down: a row that was odd comes back to even: add 80
                test    ax, ax
                jns     @F
                mov     bl, 74h                 ;; up: a row that was even comes out odd: subtract 80
@@:
                mov     cs:[cx_j], bl
                mov     cs:[cy_j], bl
                mov     cs:[cv_j], bl
                les     di, [si].FillLine.dst
                mov     bp, [si].FillLine.decision
                mov     bl, byte ptr [si].FillLine.pmask
                mov     cl, byte ptr [si].FillLine.bpp
                mov     ah, byte ptr [si].FillLine.color
                mov     bh, byte ptr [si].FillLine.x_major
                mov     dx, [si].FillLine.style
                mov     si, [si].FillLine.count
                xor     al, al
                cmp     bh, 1
                je      @@xl
                cmp     bh, 2
                je      @@vl
@@yl:
                rol     dx, 1
                jnc     @@y2
                mov     bh, ah
                xor     bh, es:[di]
                and     bh, bl
                xor     es:[di], bh
@@y2:
                test    bp, bp
                jns     @@y3
cy_i1           db      81h, 0C5h               ;; add bp, minor4
cy_i1v          dw      0
                jmp     @@ys
@@y3:
cy_i2           db      81h, 0C5h               ;; add bp, step4
cy_i2v          dw      0
                ror     bl, cl
                adc     di, 0
@@ys:
                xor     di, 2000h
                test    di, 2000h
cy_j            db      75h, 4
cy_s            db      81h, 0C7h               ;; add di, +-80
cy_sv           dw      0
                dec     si
                jnz     @@yl
                jmp     @@done
@@xl:
                rol     dx, 1
                jnc     @F
                or      al, bl
@@:
                test    bp, bp
                jns     @@xy
cx_i1           db      81h, 0C5h
cx_i1v          dw      0
                ror     bl, cl
                jc      @@xb
                dec     si
                jnz     @@xl
                jmp     @@xe
@@xb:
                mov     bh, ah
                xor     bh, es:[di]
                and     bh, al
                xor     es:[di], bh
                xor     al, al
                inc     di
                dec     si
                jnz     @@xl
                jmp     @@xe
@@xy:
cx_i2           db      81h, 0C5h
cx_i2v          dw      0
                mov     bh, ah
                xor     bh, es:[di]
                and     bh, al
                xor     es:[di], bh
                xor     al, al
                ror     bl, cl
                adc     di, 0
                xor     di, 2000h
                test    di, 2000h
cx_j            db      75h, 4
cx_s            db      81h, 0C7h
cx_sv           dw      0
                dec     si
                jnz     @@xl
@@xe:
                mov     bh, ah
                xor     bh, es:[di]
                and     bh, al
                xor     es:[di], bh
                jmp     @@done
@@vl:
                rol     dx, 1
                jnc     @F
                mov     bh, ah
                xor     bh, es:[di]
                and     bh, bl
                xor     es:[di], bh
@@:
                xor     di, 2000h
                test    di, 2000h
cv_j            db      75h, 4
cv_s            db      81h, 0C7h
cv_sv           dw      0
                dec     si
                jnz     @@vl
@@done:
                pop     bp
                pop     di
                pop     si
                pop     dx
                pop     cx
                pop     bx
                ret
B$FLIC          endp

;;::::::::::::::
;; B$FLIP (ax: ptr FillLine): the EGA and VGA planar modes, as QB's: the mask of the pixels of a line in a byte is accumulated,
;; then `out` it and one `xchg` reads the latches and writes the colour; a vertical line sets the mask once.  The graphics controller is
;; left with the bit mask FFh.
B$FLIP          proc    public

                push    bx
                push    cx
                push    dx
                push    si
                push    di
                push    bp
                mov     si, ax
                mov     ax, [si].FillLine.minor4
                mov     cs:[ex_i1v], ax
                mov     cs:[ey_i1v], ax
                mov     ax, [si].FillLine.step4
                mov     cs:[ex_i2v], ax
                mov     cs:[ey_i2v], ax
                mov     ax, [si].FillLine.ystep
                mov     cs:[ex_ysv], ax
                mov     cs:[ey_ys1v], ax
                mov     cs:[ey_ys2v], ax
                mov     cs:[ev_ysv], ax
                les     di, [si].FillLine.dst
                mov     cx, [si].FillLine.count
                mov     bp, [si].FillLine.decision
                mov     ah, byte ptr [si].FillLine.color
                mov     bl, byte ptr [si].FillLine.pmask
                mov     bh, byte ptr [si].FillLine.x_major
                mov     si, [si].FillLine.style
                mov     dx, 3CEh
                mov     al, 8
                out     dx, al
                inc     dx                     ;; the data port: the bit mask is addressed
                xor     al, al
                cmp     bh, 1
                je      @@xl
                cmp     bh, 2
                je      @@v
                mov     al, bl
                out     dx, al
@@yl:
                rol     si, 1
                jnc     @@y2
                mov     al, ah
                xchg    al, es:[di]
@@y2:
                test    bp, bp
                jns     @@y3
ey_i1           db      81h, 0C5h
ey_i1v          dw      0
ey_ys1          db      81h, 0C7h               ;; add di, ystep
ey_ys1v         dw      0
                loop    @@yl
                jmp     @@done
@@y3:
ey_i2           db      81h, 0C5h
ey_i2v          dw      0
                ror     bl, 1
                mov     al, bl
                out     dx, al
ey_ys2          db      81h, 0D7h               ;; adc di, ystep: a step along x to the next byte as well
ey_ys2v         dw      0
                loop    @@yl
                jmp     @@done
@@xl:
                rol     si, 1
                jnc     @F
                or      al, bl
@@:
                test    bp, bp
                jns     @@xy
ex_i1           db      81h, 0C5h
ex_i1v          dw      0
                ror     bl, 1
                jc      @@xb
                loop    @@xl
                jmp     @@xe
@@xb:
                out     dx, al
                mov     al, ah
                xchg    al, es:[di]
                xor     al, al
                inc     di
                loop    @@xl
                jmp     @@xe
@@xy:
ex_i2           db      81h, 0C5h
ex_i2v          dw      0
                out     dx, al
                mov     al, ah
                xchg    al, es:[di]
                xor     al, al
                ror     bl, 1
ex_ys           db      81h, 0D7h
ex_ysv          dw      0
                loop    @@xl
@@xe:
                out     dx, al
                xchg    ah, es:[di]
                jmp     @@done
@@v:
                mov     al, bl
                out     dx, al
@@vl:
                rol     si, 1
                jnc     @F
                mov     al, ah
                xchg    al, es:[di]
@@:
ev_ys           db      81h, 0C7h
ev_ysv          dw      0
                loop    @@vl
@@done:
                mov     al, 0FFh
                out     dx, al
                pop     bp
                pop     di
                pop     si
                pop     dx
                pop     cx
                pop     bx
                ret
B$FLIP          endp

lin_mode        dw      0
set_pattern     dd      0
set_flag        db      0
dword_ops       dw      05C7h, 2581h, 0D81h, 3581h
byte_ops        dw      05C6h, 2580h, 0D80h, 3580h
                end
