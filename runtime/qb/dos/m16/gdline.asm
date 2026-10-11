;; name: gdline.asm
;; desc: the lines of the graphics screen, a loop for each kind of mode
;;
;; chng: oct/26 split from fill.asm [ali]
;; obs.: the patched bytes are in the code, written through cs; the registers
;; are C's; gdfill.inc has the structures

                .model  medium, pascal
                .386
                option  proc:private

                include gdfill.inc

.code
;; B$FLSEL - set the line kernels' operation and colour, once per primitive.
;;
;; Args:
;;     operation (ax): 0 set, 1 and, 2 or, 3 xor.
;;     color     (dl): the colour byte.
;;
;; Saves:    bx
;; Clobbers: ax, flags
;; Patches:  the `op mem, imm` sites of B$FLIN, B$FLIC and B$FLIP.
B$FLSEL         proc    public uses bx
                mov     bx, ax
                and     bx, 3
                shl     bx, 1
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
                ;; the patched bytes are not in the prefetch queue
                jmp     short @F
@@:
                ret
B$FLSEL          endp

;; B$FLIN - draw a line in the 256-colour mode, a pixel a byte.
;;
;; The colour and the `mov` are patched in by B$FLSEL.
;;
;; Args:
;;     line (ax): ptr to a FillLine.
;;
;; Saves:    bx cx dx si di bp
;; Clobbers: ax, flags
B$FLIN          proc    public uses bx cx dx si di bp
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
                ret
B$FLIN          endp

;; B$FLIC - draw a line in the CGA modes, as QB's.
;;
;; The mask of the pixels of a line that fall in a byte is accumulated
;; and the byte blended once.  A step down or up a row toggles the
;; bank (bit 13) and adds 80 on the way back.
;;
;; Args:
;;     line (ax): ptr to a FillLine.
;;
;; Saves:    bx cx dx si di bp
;; Clobbers: ax, flags
B$FLIC          proc    public uses bx cx dx si di bp
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
                ;; down: a row that was odd comes back to even: add 80
                mov     bl, 75h
                test    ax, ax
                jns     @F
                ;; up: a row that was even comes out odd: subtract 80
                mov     bl, 74h
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
                ret
B$FLIC          endp

;; B$FLIP - draw a line in the EGA and VGA planar modes, as QB's.
;;
;; The mask of the pixels in a byte is accumulated, then `out` and one
;; `xchg` reads the latches and writes the colour.  A vertical line sets
;; the mask once.  The graphics controller's bit mask is left at FFh.
;;
;; Args:
;;     line (ax): ptr to a FillLine.
;;
;; Saves:    bx cx dx si di bp
;; Clobbers: ax, flags
B$FLIP          proc    public uses bx cx dx si di bp
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
                ;; the data port: the bit mask is addressed
                inc     dx
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
;; adc di, ystep: a step along x to the next byte as well
ey_ys2          db      81h, 0D7h
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
                ret
B$FLIP          endp

lin_mode        dw      0

byte_ops        dw      05C6h, 2580h, 0D80h, 3580h
                end
