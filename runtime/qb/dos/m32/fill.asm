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
                public  B$FGET
                public  B$FSCL
                public  B$FSCP
                public  B$FSCC
                public  B$FPSEL
                public  B$FPUC
                public  B$FPUP

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
                ystep           dword   ?               ;; bytes to the next row (CGA: 80, down; -80, up)
                style           dword   ?               ;; the 16 bits of the line style, the first pixel's the high bit
                x_major         dword   ?               ;; 1 along x, 0 along y, 2 straight down or up
                pmask           dword   ?               ;; the first pixel's mask in its byte (EGA: one bit; CGA: its pixel's bits)
                bpp             dword   ?               ;; CGA: bits a pixel
                color           dword   ?               ;; the byte of the colour: CGA all its pixels, EGA the nibble
FillLine        ends

FillXfer        struc
                screen          dword   ?
                array           dword   ?
                rows            dword   ?
                sbytes          dword   ?
                abytes          dword   ?
                stride          dword   ?
                shift           dword   ?
                first           dword   ?
                last            dword   ?
                step            dword   ?
                bank            dword   ?
FillXfer        ends

FillScan        struc
                at              dword   ?
                count           dword   ?
                c1              dword   ?
                c2              dword   ?
                flags           dword   ?
                first           dword   ?
                last            dword   ?
                middle          dword   ?
                found           dword   ?
                hits            dword   ?
FillScan        ends

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
;; B$FLIN (eax: ptr FillLine): the 256-colour mode, a pixel a byte; the colour and `mov` patched in by B$FSEL
B$FLIN          proc

                push    ebx
                push    ecx
                push    edx
                push    esi
                push    edi
                push    ebp
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
                pop     ebp
                pop     edi
                pop     esi
                pop     edx
                pop     ecx
                pop     ebx
                ret
B$FLIN          endp

;;::::::::::::::
;; B$FLIC (eax: ptr FillLine): the CGA modes, as QB's: the mask of the pixels of a line that fall in a byte is accumulated and
;; the byte blended once; a step down or up a row toggles the bank (bit 13) and adds 80 when it comes back to the first
B$FLIC          proc

                push    ebx
                push    ecx
                push    edx
                push    esi
                push    edi
                push    ebp
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
                mov     bl, 75h                 ;; down: a row that was odd comes back to even: add 80
                test    eax, eax
                jns     @F
                mov     bl, 74h                 ;; up: a row that was even comes out odd: subtract 80
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
                pop     ebp
                pop     edi
                pop     esi
                pop     edx
                pop     ecx
                pop     ebx
                ret
B$FLIC          endp

;;::::::::::::::
;; B$FLIP (eax: ptr FillLine): the EGA and VGA planar modes, as QB's: the mask of the pixels of a line in a byte is accumulated,
;; then `out` it and one `xchg` reads the latches and writes the colour; a vertical line sets the mask once.  The graphics controller is
;; left with the bit mask FFh.
B$FLIP          proc

                push    ebx
                push    ecx
                push    edx
                push    esi
                push    edi
                push    ebp
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
                inc     edx                     ;; the data port: the bit mask is addressed
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
ey_ys2          db      81h, 0D7h               ;; adc edi, ystep: a step along x to the next byte as well
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
                pop     ebp
                pop     edi
                pop     esi
                pop     edx
                pop     ecx
                pop     ebx
                ret
B$FLIP          endp


;;::::::::::::::
;; B$FGET (eax: ptr FillXfer): one plane of GET, as QB's NReadL: each array byte is the high byte of `rol ax, cl` over two screen bytes
B$FGET          proc

                push    ebx
                push    ecx
                push    edx
                push    esi
                push    edi
                push    ebp
                mov     ebx, eax
                mov     esi, [ebx].FillXfer.screen
                mov     edi, [ebx].FillXfer.array
                mov     ebp, [ebx].FillXfer.rows
                mov     cl, byte ptr [ebx].FillXfer.shift
@@row:
                push    esi
                push    edi
                mov     ch, byte ptr [ebx].FillXfer.abytes
                mov     ah, [esi]
                inc     esi
@@byte:
                lodsb
                mov     dl, al
                rol     ax, cl
                dec     ch
                jz      @@last
                mov     [edi], ah
                inc     edi
                mov     ah, dl
                jmp     @@byte
@@last:
                and     ah, byte ptr [ebx].FillXfer.last
                mov     [edi], ah
                pop     edi
                pop     esi
                add     edi, [ebx].FillXfer.stride
                cmp     [ebx].FillXfer.bank, 0
                je      @@flat
                xor     esi, 2000h
                test    esi, 2000h
                jnz     @@next
                add     esi, 80
                jmp     @@next
@@flat:
                add     esi, [ebx].FillXfer.step
@@next:
                dec     ebp
                jnz     @@row
                pop     ebp
                pop     edi
                pop     esi
                pop     edx
                pop     ecx
                pop     ebx
                ret
B$FGET          endp



;;::::::::::::::
;; B$FPSEL (eax: how): the way of the PUT that follows, once; 0 PSET, 1 AND, 2 OR, 3 XOR, 4 PRESET (PSET with the colours inverted)
B$FPSEL         proc

                push    ebx
                push    ecx
                mov     ebx, eax
                mov     ax, 9090h               ;; nothing before the write
                mov     cx, ax
                cmp     ebx, 4
                jne     @F
                mov     ax, 0D2F6h              ;; not dl, the middle bytes of a CGA PUT
                mov     cx, 0D5F6h              ;; not ch, the bytes of a planar one
@@:
                mov     word ptr [put_not], ax
                mov     word ptr [pn_single], cx
                mov     word ptr [pn_first], cx
                mov     word ptr [pn_mid], cx
                mov     word ptr [pn_last], cx
                mov     ax, word ptr put_mids[ebx * 2]
                mov     word ptr [put_mid], ax
                mov     eax, dword ptr put_edges[ebx * 4]
                mov     [put_edge_v], eax
                pop     ecx
                pop     ebx
                ret
B$FPSEL         endp

;; the edge bytes of a CGA PUT: ah = the source (kept), dl = the mask of the pixels it covers, [edi] the screen byte
put_pset:
                mov     dh, ah
                xor     dh, [edi]
                and     dh, dl
                xor     [edi], dh
                ret
put_preset:
                mov     dh, ah
                not     dh
                xor     dh, [edi]
                and     dh, dl
                xor     [edi], dh
                ret
put_and:
                mov     dh, dl
                not     dh
                or      dh, ah
                and     [edi], dh
                ret
put_or:
                mov     dh, ah
                and     dh, dl
                or      [edi], dh
                ret
put_xor:
                mov     dh, ah
                and     dh, dl
                xor     [edi], dh
                ret

;;::::::::::::::
;; B$FPUC (eax: ptr FillXfer): the CGA modes' PUT, as QB's NWriteL: each screen byte takes bits of two array bytes, `ror ax, cl`
;; over the pair; the middle bytes are `op [edi], dl` patched by B$FPSEL, the first and last go through a mask
B$FPUC          proc

                push    ebx
                push    ecx
                push    edx
                push    esi
                push    edi
                push    ebp
                mov     ebx, eax
                mov     esi, [ebx].FillXfer.array
                mov     edi, [ebx].FillXfer.screen
                mov     ebp, [ebx].FillXfer.rows
                mov     cl, byte ptr [ebx].FillXfer.shift
@@row:
                push    esi
                push    edi
                mov     ch, byte ptr [ebx].FillXfer.sbytes
                mov     ah, [esi]
                inc     esi
                ror     ax, cl
                dec     ch
                jnz     @@more
                mov     dl, byte ptr [ebx].FillXfer.first
                and     dl, byte ptr [ebx].FillXfer.last
                call    dword ptr [put_edge_v]
                jmp     @@rowdone
@@more:
                mov     dl, byte ptr [ebx].FillXfer.first
                call    dword ptr [put_edge_v]
                inc     edi
                dec     ch
                jz      @@lastbyte
@@mid:
                rol     ax, cl
                lodsb
                xchg    ah, al
                ror     ax, cl
                mov     dl, ah
put_not         db      90h, 90h                ;; not dl, for PRESET
put_mid         db      88h, 17h                ;; mov [edi], dl; and, or, xor by B$FPSEL
                inc     edi
                dec     ch
                jnz     @@mid
@@lastbyte:
                rol     ax, cl
                lodsb
                xchg    ah, al
                ror     ax, cl
                mov     dl, byte ptr [ebx].FillXfer.last
                call    dword ptr [put_edge_v]
@@rowdone:
                pop     edi
                pop     esi
                add     esi, [ebx].FillXfer.stride
                cmp     [ebx].FillXfer.bank, 0
                je      @@flat
                xor     edi, 2000h
                test    edi, 2000h
                jnz     @@next
                add     edi, 80
                jmp     @@next
@@flat:
                add     edi, [ebx].FillXfer.step
@@next:
                dec     ebp
                jnz     @@row
                pop     ebp
                pop     edi
                pop     esi
                pop     edx
                pop     ecx
                pop     ebx
                ret
B$FPUC          endp

;;::::::::::::::
;; B$FPUP (eax: ptr FillXfer): one plane of a planar PUT: the bytes aligned as for the CGA, each written by one `xchg`, which loads the
;; latches and writes through the function the caller set; the bit mask is set round the first and last bytes of a row if they are partial
B$FPUP          proc

                push    ebx
                push    ecx
                push    edx
                push    esi
                push    edi
                push    ebp
                mov     ebx, eax
                mov     esi, [ebx].FillXfer.array
                mov     edi, [ebx].FillXfer.screen
                mov     cl, byte ptr [ebx].FillXfer.shift
                mov     edx, 3CEh
                mov     al, 8
                out     dx, al
                inc     edx                     ;; the data port: the bit mask is addressed
@@row:
                push    esi
                push    edi
                movzx   ebp, byte ptr [ebx].FillXfer.sbytes
                mov     ah, [esi]
                inc     esi
                ror     ax, cl
                dec     ebp
                jnz     @@more
                push    eax
                mov     al, byte ptr [ebx].FillXfer.first
                and     al, byte ptr [ebx].FillXfer.last
                out     dx, al
                pop     eax
                mov     ch, ah
pn_single       db      90h, 90h
                xchg    ch, [edi]
                jmp     @@maskff
@@more:
                cmp     byte ptr [ebx].FillXfer.first, 0FFh
                je      @F
                push    eax
                mov     al, byte ptr [ebx].FillXfer.first
                out     dx, al
                pop     eax
@@:
                mov     ch, ah
pn_first        db      90h, 90h
                xchg    ch, [edi]
                inc     edi
                cmp     byte ptr [ebx].FillXfer.first, 0FFh
                je      @F
                push    eax
                mov     al, 0FFh
                out     dx, al
                pop     eax
@@:
                dec     ebp
                jz      @@lastbyte
@@mid:
                rol     ax, cl
                lodsb
                xchg    ah, al
                ror     ax, cl
                mov     ch, ah
pn_mid          db      90h, 90h
                xchg    ch, [edi]
                inc     edi
                dec     ebp
                jnz     @@mid
@@lastbyte:
                rol     ax, cl
                lodsb
                xchg    ah, al
                ror     ax, cl
                cmp     byte ptr [ebx].FillXfer.last, 0FFh
                je      @F
                push    eax
                mov     al, byte ptr [ebx].FillXfer.last
                out     dx, al
                pop     eax
@@:
                mov     ch, ah
pn_last         db      90h, 90h
                xchg    ch, [edi]
@@maskff:
                mov     al, 0FFh
                out     dx, al
                pop     edi
                pop     esi
                add     esi, [ebx].FillXfer.stride
                add     edi, [ebx].FillXfer.step
                dec     dword ptr [ebx].FillXfer.rows
                jnz     @@row
                pop     ebp
                pop     edi
                pop     esi
                pop     edx
                pop     ecx
                pop     ebx
                ret
B$FPUP          endp

put_mids        dw      1788h, 1720h, 1708h, 1730h, 1788h        ;; mov, and, or, xor [edi], dl; PRESET is a mov of the inverted
put_edges       dd      put_pset, put_and, put_or, put_xor, put_preset
put_edge_v      dd      0

;;::::::::::::::
;; B$FSCL (eax: ptr FillScan): the 256-colour mode's scan, by the string instructions: `repne scasb` finds a colour, `repe scasb` skips one
B$FSCL          proc

                push    ebx
                push    ecx
                push    edx
                push    esi
                push    edi
                push    ebp
                push    eax
                mov     ebx, eax
                mov     edi, [ebx].FillScan.at
                mov     ebp, [ebx].FillScan.count
                mov     esi, [ebx].FillScan.flags
                mov     al, byte ptr [ebx].FillScan.c1
                mov     ah, byte ptr [ebx].FillScan.c2
                mov     edx, edi                ;; the start
                mov     ebx, 1                  ;; the way a pixel is stepped
                cld
                test    esi, 2
                jz      @F
                std
                mov     ebx, -1
@@:
                mov     ecx, ebp
                test    esi, 1
                jnz     @@match
                mov     esi, ebx
                neg     esi                     ;; back to the pixel just tested
@@skip:                                         ;; the first pixel that is neither colour, the colour skipped and the other swapped
                repe    scasb
                je      @@none
                cmp     [edi + esi], ah
                jne     @@hit0
                xchg    al, ah
                jmp     @@skip
@@hit0:
                mov     eax, ebp
                sub     eax, ecx
                dec     eax
                jmp     @@done
@@match:                                        ;; the first pixel of c1 or of c2
                repne   scasb
                jne     @@no1
                mov     esi, ebp
                sub     esi, ecx
                dec     esi                     ;; where c1 is
                cmp     al, ah
                je      @@got1
                test    esi, esi
                jz      @@got1
                mov     edi, edx
                mov     ecx, esi
                mov     al, ah
                repne   scasb
                jne     @@got1
                mov     eax, esi
                sub     eax, ecx
                dec     eax
                jmp     @@done
@@got1:
                mov     eax, esi
                jmp     @@done
@@no1:
                cmp     al, ah
                je      @@none
                mov     edi, edx
                mov     ecx, ebp
                mov     al, ah
                repne   scasb
                jne     @@none
                mov     eax, ebp
                sub     eax, ecx
                dec     eax
                jmp     @@done
@@none:
                mov     eax, -1
@@done:
                cld
                pop     ebx
                mov     [ebx].FillScan.found, eax
                pop     ebp
                pop     edi
                pop     esi
                pop     edx
                pop     ecx
                pop     ebx
                ret
B$FSCL          endp

;;::::::::::::::
;; B$FSCP (eax: ptr FillScan): the planar modes' scan, a byte of eight pixels at a time by the controller's colour compare (read mode 1,
;; set up by the caller): one `out` and one read tell which of the eight are of a colour
B$FSCP          proc

                push    ebx
                push    ecx
                push    edx
                push    esi
                push    edi
                push    ebp
                mov     ebx, eax
                mov     edi, [ebx].FillScan.at
                mov     cl, byte ptr [ebx].FillScan.count
                mov     esi, [ebx].FillScan.c1
                shl     esi, 8
                or      esi, 2                  ;; the colour compare register, then the colour
                mov     ebp, [ebx].FillScan.c2
                shl     ebp, 8
                or      ebp, 2
                mov     edx, 3CEh
@@byte:
                mov     eax, esi
                out     dx, ax
                mov     ch, [edi]
                cmp     esi, ebp
                je      @F
                mov     eax, ebp
                out     dx, ax
                or      ch, [edi]
@@:
                test    byte ptr [ebx].FillScan.flags, 1
                jnz     @F
                not     ch
@@:
                and     ch, byte ptr [ebx].FillScan.first
                mov     byte ptr [ebx].FillScan.first, 0FFh
                cmp     cl, 1
                jne     @F
                and     ch, byte ptr [ebx].FillScan.last
@@:
                test    ch, ch
                jnz     @@hit
                test    byte ptr [ebx].FillScan.flags, 2
                jnz     @@back
                inc     edi
                jmp     @@next
@@back:
                dec     edi
@@next:
                dec     cl
                jnz     @@byte
                mov     [ebx].FillScan.found, -1
                mov     [ebx].FillScan.hits, 0
                jmp     @@done
@@hit:
                movzx   eax, cl
                neg     eax
                add     eax, [ebx].FillScan.count
                mov     [ebx].FillScan.found, eax
                movzx   eax, ch
                mov     [ebx].FillScan.hits, eax
@@done:
                pop     ebp
                pop     edi
                pop     esi
                pop     edx
                pop     ecx
                pop     ebx
                ret
B$FSCP          endp


;; the pixels of the byte at [edi] that are wanted, a bit for each slot, under the mask `maskop`; ZF set if none
BYTEHIT         macro   maskop
                local   one, both, keep
                mov     al, [edi]
                mov     ah, al
                xor     al, dl
                xor     ah, dh
                test    esi, 4
                jz      one
                mov     ch, al
                shr     ch, 1
                or      al, ch
                not     al
                mov     ch, ah
                shr     ch, 1
                or      ah, ch
                not     ah
                jmp     both
one:
                not     al
                not     ah
both:
                or      al, ah
                test    esi, 1
                jnz     keep
                not     al
keep:
                and     al, maskop
                endm

;;::::::::::::::
;; B$FSCC (eax: ptr FillScan): the CGA modes' scan, a byte at a time: the pixels of a byte equal to a colour show as bits of the byte xor the
;; colour's pattern.  The first and last byte are taken under their masks; the ones between, for one colour, by a short loop that
;; only says whether some pixel of the byte is wanted, and the byte it stops at is taken as the others are
B$FSCC          proc

                push    ebx
                push    ecx
                push    edx
                push    esi
                push    edi
                push    ebp
                mov     ebx, eax
                mov     edi, [ebx].FillScan.at
                mov     cl, byte ptr [ebx].FillScan.count
                mov     dl, byte ptr [ebx].FillScan.c1
                mov     dh, byte ptr [ebx].FillScan.c2
                mov     esi, [ebx].FillScan.flags
                mov     ebp, 1
                test    esi, 2
                jz      @F
                mov     ebp, -1
@@:
                cmp     cl, 1
                jne     @@several
                BYTEHIT byte ptr [ebx].FillScan.first
                and     al, byte ptr [ebx].FillScan.last
                jnz     @@hit
                jmp     @@none
@@several:
                BYTEHIT byte ptr [ebx].FillScan.first
                jnz     @@hit
                add     edi, ebp
                dec     cl
                cmp     cl, 1
                je      @@lastbyte
                dec     cl                      ;; the bytes between
                cmp     dl, dh
                jne     @@twocolours
                test    esi, 4
                jz      @@onebit
                test    esi, 1
                jz      @@skipequal2
@@findequal2:                                   ;; 2-bit pixels, a pixel of the colour: some pixel of the byte xor the pattern is 00
                mov     al, [edi]
                xor     al, dl
                mov     ch, al
                shr     ch, 1
                or      al, ch
                and     al, 55h
                cmp     al, 55h
                jne     @@mid
                add     edi, ebp
                dec     cl
                jnz     @@findequal2
                jmp     @@lastbyte
@@skipequal2:                                   ;; 2-bit pixels, a pixel that is not: some pixel of the byte xor the pattern is not 00
                mov     al, [edi]
                xor     al, dl
                mov     ch, al
                shr     ch, 1
                or      al, ch
                test    al, 55h
                jnz     @@mid
                add     edi, ebp
                dec     cl
                jnz     @@skipequal2
                jmp     @@lastbyte
@@onebit:
                test    esi, 1
                jz      @@skipequal1
@@findequal1:
                mov     al, [edi]
                xor     al, dl
                cmp     al, 0FFh
                jne     @@mid
                add     edi, ebp
                dec     cl
                jnz     @@findequal1
                jmp     @@lastbyte
@@skipequal1:
                mov     al, [edi]
                xor     al, dl
                jnz     @@mid
                add     edi, ebp
                dec     cl
                jnz     @@skipequal1
                jmp     @@lastbyte
@@twocolours:
                BYTEHIT byte ptr [ebx].FillScan.middle
                jnz     @@hit
                add     edi, ebp
                dec     cl
                jnz     @@twocolours
                jmp     @@lastbyte
@@mid:
                BYTEHIT byte ptr [ebx].FillScan.middle
                jnz     @@hit
                add     edi, ebp                ;; (the loops say a wanted pixel is here)
                dec     cl
                jmp     @@lastbyte
@@lastbyte:
                BYTEHIT byte ptr [ebx].FillScan.last
                jnz     @@hit
@@none:
                mov     [ebx].FillScan.found, -1
                mov     [ebx].FillScan.hits, 0
                jmp     @@done
@@hit:
                movzx   eax, al
                mov     [ebx].FillScan.hits, eax
                mov     eax, edi
                sub     eax, [ebx].FillScan.at
                imul    eax, ebp
                mov     [ebx].FillScan.found, eax
@@done:
                pop     ebp
                pop     edi
                pop     esi
                pop     edx
                pop     ecx
                pop     ebx
                ret
B$FSCC          endp

set_pattern     dd      0
set_flag        db      0
dword_ops       dw      07C7h, 2781h, 0F81h, 3781h
byte_ops        dw      07C6h, 2780h, 0F80h, 3780h
                end
