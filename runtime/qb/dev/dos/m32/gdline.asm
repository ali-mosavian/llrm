;; name: gdline.asm
;; desc: the lines of the graphics screen, a loop for each kind of mode
;;
;; chng: oct/26 split from fill.asm [ali]
;; obs.: the patched bytes are in the code, written through the flat data
;; selector; the registers are C's; gdfill.inc has the structures

                .386
                .model  flat

                public  B$FLSEL
                public  B$FLIN
                public  B$FLIC
                public  B$FLIP


                include gdfill.inc

.code
;; B$FLSEL - set the line kernels' operation and colour, once per primitive.
;;
;; Args:
;;     operation (eax): 0 set, 1 and, 2 or, 3 xor.
;;     color     (dl): the colour byte.
;;
;; Saves:    ebx
;; Clobbers: eax, flags
;; Patches:  the `op mem, imm` sites of B$FLIN, B$FLIC and B$FLIP.
B$FLSEL         proc    uses ebx
                mov     ebx, eax
                and     ebx, 3
                mov     ax, word ptr byte_ops[ebx * 2]
                mov     word ptr [lx_op], ax
                mov     word ptr [ly_op], ax
                mov     word ptr [sx_op], ax
                mov     word ptr [sy_op], ax
                mov     word ptr [lv_op], ax
                mov     word ptr [sv_op], ax
                mov     byte ptr [lx_imm], dl
                mov     byte ptr [ly_imm], dl
                mov     byte ptr [sx_imm], dl
                mov     byte ptr [sy_imm], dl
                mov     byte ptr [lv_imm], dl
                mov     byte ptr [sv_imm], dl
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
;;     line (eax): ptr to a FillLine.
;;
;; Saves:    ebx ecx edx esi edi ebp
;; Clobbers: eax, flags
B$FLIN          proc    uses ebx ecx edx esi edi ebp
                mov     esi, eax
                push    [esi].FillLine.x_major
                mov     edi, [esi].FillLine.dst
                mov     ecx, [esi].FillLine.count
                mov     ebx, [esi].FillLine.decision
                mov     edx, [esi].FillLine.minor4
                mov     ebp, [esi].FillLine.step4
                mov     eax, [esi].FillLine.ystep
                movzx   esi, word ptr [esi].FillLine.style
                cmp     dword ptr [esp], 1
                je      @@x
                cmp     dword ptr [esp], 2
                je      @@v
                cmp     si, 0FFFFh
                jne     @@sy
@@ly:
ly_op           db      0C6h, 07h
ly_imm          db      0
                add     edi, eax
                test    ebx, ebx
                js      @@lyf
                add     ebx, ebp
                inc     edi
                dec     ecx
                jnz     @@ly
                jmp     @@done
@@lyf:
                add     ebx, edx
                dec     ecx
                jnz     @@ly
                jmp     @@done
@@sy:
                rol     si, 1
                jnc     @F
sy_op           db      0C6h, 07h
sy_imm          db      0
@@:
                add     edi, eax
                test    ebx, ebx
                js      @@syf
                add     ebx, ebp
                inc     edi
                dec     ecx
                jnz     @@sy
                jmp     @@done
@@syf:
                add     ebx, edx
                dec     ecx
                jnz     @@sy
                jmp     @@done
@@x:
                cmp     si, 0FFFFh
                jne     @@sx
@@lx:
lx_op           db      0C6h, 07h
lx_imm          db      0
                inc     edi
                test    ebx, ebx
                js      @@lxf
                add     ebx, ebp
                add     edi, eax
                dec     ecx
                jnz     @@lx
                jmp     @@done
@@lxf:
                add     ebx, edx
                dec     ecx
                jnz     @@lx
                jmp     @@done
@@sx:
                rol     si, 1
                jnc     @F
sx_op           db      0C6h, 07h
sx_imm          db      0
@@:
                inc     edi
                test    ebx, ebx
                js      @@sxf
                add     ebx, ebp
                add     edi, eax
                dec     ecx
                jnz     @@sx
                jmp     @@done
@@sxf:
                add     ebx, edx
                dec     ecx
                jnz     @@sx
                jmp     @@done
@@v:
                cmp     si, 0FFFFh
                jne     @@sv
@@lv:
lv_op           db      0C6h, 07h
lv_imm          db      0
                add     edi, eax
                dec     ecx
                jnz     @@lv
                jmp     @@done
@@sv:
                rol     si, 1
                jnc     @F
sv_op           db      0C6h, 07h
sv_imm          db      0
@@:
                add     edi, eax
                dec     ecx
                jnz     @@sv
@@done:
                add     esp, 4
                ret
B$FLIN          endp

;; B$FLIC - draw a line in the CGA modes, as QB's.
;;
;; The mask of the pixels of a line that fall in a byte is accumulated
;; and the byte blended once.  A step down or up a row toggles the
;; bank (bit 13) and adds 80 on the way back.
;;
;; Args:
;;     line (eax): ptr to a FillLine.
;;
;; Saves:    ebx ecx edx esi edi ebp
;; Clobbers: eax, flags
B$FLIC          proc    uses ebx ecx edx esi edi ebp
                mov     esi, eax
                mov     eax, [esi].FillLine.minor4
                mov     [cx_i1v], eax
                mov     [cy_i1v], eax
                mov     eax, [esi].FillLine.step4
                mov     [cx_i2v], eax
                mov     [cy_i2v], eax
                mov     eax, [esi].FillLine.ystep
                mov     [cx_sv], eax
                mov     [cy_sv], eax
                mov     [cv_sv], eax
                ;; down: a row that was odd comes back to even: add 80
                mov     bl, 75h
                test    eax, eax
                jns     @F
                ;; up: a row that was even comes out odd: subtract 80
                mov     bl, 74h
@@:
                mov     [cx_j], bl
                mov     [cy_j], bl
                mov     [cv_j], bl
                mov     edi, [esi].FillLine.dst
                mov     ebp, [esi].FillLine.decision
                mov     bl, byte ptr [esi].FillLine.pmask
                mov     cl, byte ptr [esi].FillLine.bpp
                mov     ah, byte ptr [esi].FillLine.color
                mov     bh, byte ptr [esi].FillLine.x_major
                movzx   edx, word ptr [esi].FillLine.style
                mov     esi, [esi].FillLine.count
                xor     al, al
                cmp     bh, 1
                je      @@xl
                cmp     bh, 2
                je      @@vl
@@yl:
                rol     dx, 1
                jnc     @@y2
                mov     bh, ah
                xor     bh, [edi]
                and     bh, bl
                xor     [edi], bh
@@y2:
                test    ebp, ebp
                jns     @@y3
cy_i1           db      81h, 0C5h               ;; add ebp, minor4
cy_i1v          dd      0
                jmp     @@ys
@@y3:
cy_i2           db      81h, 0C5h               ;; add ebp, step4
cy_i2v          dd      0
                ror     bl, cl
                adc     edi, 0
@@ys:
                xor     edi, 2000h
                test    edi, 2000h
cy_j            db      75h, 6
cy_s            db      81h, 0C7h               ;; add edi, +-80
cy_sv           dd      0
                dec     esi
                jnz     @@yl
                jmp     @@done
@@xl:
                rol     dx, 1
                jnc     @F
                or      al, bl
@@:
                test    ebp, ebp
                jns     @@xy
cx_i1           db      81h, 0C5h
cx_i1v          dd      0
                ror     bl, cl
                jc      @@xb
                dec     esi
                jnz     @@xl
                jmp     @@xe
@@xb:
                mov     bh, ah
                xor     bh, [edi]
                and     bh, al
                xor     [edi], bh
                xor     al, al
                inc     edi
                dec     esi
                jnz     @@xl
                jmp     @@xe
@@xy:
cx_i2           db      81h, 0C5h
cx_i2v          dd      0
                mov     bh, ah
                xor     bh, [edi]
                and     bh, al
                xor     [edi], bh
                xor     al, al
                ror     bl, cl
                adc     edi, 0
                xor     edi, 2000h
                test    edi, 2000h
cx_j            db      75h, 6
cx_s            db      81h, 0C7h
cx_sv           dd      0
                dec     esi
                jnz     @@xl
@@xe:
                mov     bh, ah
                xor     bh, [edi]
                and     bh, al
                xor     [edi], bh
                jmp     @@done
@@vl:
                rol     dx, 1
                jnc     @F
                mov     bh, ah
                xor     bh, [edi]
                and     bh, bl
                xor     [edi], bh
@@:
                xor     edi, 2000h
                test    edi, 2000h
cv_j            db      75h, 6
cv_s            db      81h, 0C7h
cv_sv           dd      0
                dec     esi
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
;;     line (eax): ptr to a FillLine.
;;
;; Saves:    ebx ecx edx esi edi ebp
;; Clobbers: eax, flags
B$FLIP          proc    uses ebx ecx edx esi edi ebp
                mov     esi, eax
                mov     eax, [esi].FillLine.minor4
                mov     [ex_i1v], eax
                mov     [ey_i1v], eax
                mov     eax, [esi].FillLine.step4
                mov     [ex_i2v], eax
                mov     [ey_i2v], eax
                mov     eax, [esi].FillLine.ystep
                mov     [ex_ysv], eax
                mov     [ey_ys1v], eax
                mov     [ey_ys2v], eax
                mov     [ev_ysv], eax
                mov     edi, [esi].FillLine.dst
                mov     ecx, [esi].FillLine.count
                mov     ebp, [esi].FillLine.decision
                mov     ah, byte ptr [esi].FillLine.color
                mov     bl, byte ptr [esi].FillLine.pmask
                mov     bh, byte ptr [esi].FillLine.x_major
                movzx   esi, word ptr [esi].FillLine.style
                mov     edx, 3CEh
                mov     al, 8
                out     dx, al
                ;; the data port: the bit mask is addressed
                inc     edx
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
                xchg    al, [edi]
@@y2:
                test    ebp, ebp
                jns     @@y3
ey_i1           db      81h, 0C5h
ey_i1v          dd      0
ey_ys1          db      81h, 0C7h               ;; add edi, ystep
ey_ys1v         dd      0
                loop    @@yl
                jmp     @@done
@@y3:
ey_i2           db      81h, 0C5h
ey_i2v          dd      0
                ror     bl, 1
                mov     al, bl
                out     dx, al
;; adc edi, ystep: a step along x to the next byte as well
ey_ys2          db      81h, 0D7h
ey_ys2v         dd      0
                loop    @@yl
                jmp     @@done
@@xl:
                rol     si, 1
                jnc     @F
                or      al, bl
@@:
                test    ebp, ebp
                jns     @@xy
ex_i1           db      81h, 0C5h
ex_i1v          dd      0
                ror     bl, 1
                jc      @@xb
                loop    @@xl
                jmp     @@xe
@@xb:
                out     dx, al
                mov     al, ah
                xchg    al, [edi]
                xor     al, al
                inc     edi
                loop    @@xl
                jmp     @@xe
@@xy:
ex_i2           db      81h, 0C5h
ex_i2v          dd      0
                out     dx, al
                mov     al, ah
                xchg    al, [edi]
                xor     al, al
                ror     bl, 1
ex_ys           db      81h, 0D7h
ex_ysv          dd      0
                loop    @@xl
@@xe:
                out     dx, al
                xchg    ah, [edi]
                jmp     @@done
@@v:
                mov     al, bl
                out     dx, al
@@vl:
                rol     si, 1
                jnc     @F
                mov     al, ah
                xchg    al, [edi]
@@:
ev_ys           db      81h, 0C7h
ev_ysv          dd      0
                loop    @@vl
@@done:
                mov     al, 0FFh
                out     dx, al
                ret
B$FLIP          endp


byte_ops        dw      07C6h, 2780h, 0F80h, 3780h
                end
