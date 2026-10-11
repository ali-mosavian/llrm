;; name: gdxfer.asm
;; desc: GET and PUT of the CGA and planar modes, a plane or a row at a time
;;
;; chng: oct/26 split from fill.asm [ali]
;; obs.: the patched bytes are in the code, written through cs; the registers
;; are C's; gdfill.inc has the structures

                .model  medium, pascal
                .386
                option  proc:private

                include gdfill.inc

.code
;; B$FGET - read one plane of a box into a GET array, as QB's NReadL.
;;
;; Each array byte is the high byte of `rol ax, cl` over two screen bytes.
;;
;; Args:
;;     xfer (ax): ptr to a FillXfer.
;;
;; Saves:    bx cx dx si di bp es fs
;; Clobbers: ax, flags
B$FGET          proc    public uses bx cx dx si di bp es fs
                mov     bx, ax
                lfs     si, [bx].FillXfer.screen
                les     di, [bx].FillXfer.array
                mov     bp, [bx].FillXfer.rows
                mov     cl, byte ptr [bx].FillXfer.shift
@@row:
                push    si
                push    di
                mov     ch, byte ptr [bx].FillXfer.abytes
                mov     ah, fs:[si]
                inc     si
@@byte:
                lods    byte ptr fs:[si]
                mov     dl, al
                rol     ax, cl
                dec     ch
                jz      @@last
                mov     es:[di], ah
                inc     di
                mov     ah, dl
                jmp     @@byte
@@last:
                and     ah, byte ptr [bx].FillXfer.last
                mov     es:[di], ah
                pop     di
                pop     si
                add     di, [bx].FillXfer.stride
                cmp     [bx].FillXfer.bank, 0
                je      @@flat
                xor     si, 2000h
                test    si, 2000h
                jnz     @@next
                add     si, 80
                jmp     @@next
@@flat:
                add     si, [bx].FillXfer.step
@@next:
                dec     bp
                jnz     @@row
                ret
B$FGET          endp

;; B$FPSEL - set the way of the PUT that follows, once.
;;
;; Args:
;;     how (ax): 0 PSET, 1 AND, 2 OR, 3 XOR, 4 PRESET.
;;
;; Saves:    bx cx
;; Clobbers: ax, flags
;; Patches:  the write of B$FPUC and B$FPUP.
B$FPSEL         proc    public uses bx cx
                mov     bx, ax
                mov     ax, 9090h               ;; nothing before the write
                mov     cx, ax
                cmp     bx, 4
                jne     @F
                ;; not dl, the middle bytes of a CGA PUT
                mov     ax, 0D2F6h
                ;; not ch, the bytes of a planar one
                mov     cx, 0D5F6h
@@:
                mov     word ptr cs:[put_not], ax
                mov     word ptr cs:[pn_single], cx
                mov     word ptr cs:[pn_first], cx
                mov     word ptr cs:[pn_mid], cx
                mov     word ptr cs:[pn_last], cx
                shl     bx, 1
                mov     ax, word ptr cs:put_mids[bx]
                mov     word ptr cs:[put_mid], ax
                mov     ax, word ptr cs:put_edges[bx]
                mov     cs:[put_edge_v], ax
                ret
B$FPSEL         endp

;; the edge bytes of a CGA PUT: ah = the source (kept), dl = the mask of the
;; pixels it covers, es:[di] the screen byte
put_pset:
                mov     dh, ah
                xor     dh, es:[di]
                and     dh, dl
                xor     es:[di], dh
                ret
put_preset:
                mov     dh, ah
                not     dh
                xor     dh, es:[di]
                and     dh, dl
                xor     es:[di], dh
                ret
put_and:
                mov     dh, dl
                not     dh
                or      dh, ah
                and     es:[di], dh
                ret
put_or:
                mov     dh, ah
                and     dh, dl
                or      es:[di], dh
                ret
put_xor:
                mov     dh, ah
                and     dh, dl
                xor     es:[di], dh
                ret

;; B$FPUC - PUT a box in the CGA modes, as QB's NWriteL.
;;
;; Each screen byte takes bits of two array bytes by `ror ax, cl` over
;; the pair.  The middle bytes are the `op [di], dl` that B$FPSEL
;; patched; the first and last go through a mask.
;;
;; Args:
;;     xfer (ax): ptr to a FillXfer.
;;
;; Saves:    bx cx dx si di bp es fs
;; Clobbers: ax, flags
B$FPUC          proc    public uses bx cx dx si di bp es fs
                mov     bx, ax
                lfs     si, [bx].FillXfer.array
                les     di, [bx].FillXfer.screen
                mov     bp, [bx].FillXfer.rows
                mov     cl, byte ptr [bx].FillXfer.shift
@@row:
                push    si
                push    di
                mov     ch, byte ptr [bx].FillXfer.sbytes
                mov     ah, fs:[si]
                inc     si
                ror     ax, cl
                dec     ch
                jnz     @@more
                mov     dl, byte ptr [bx].FillXfer.first
                and     dl, byte ptr [bx].FillXfer.last
                call    word ptr cs:[put_edge_v]
                jmp     @@rowdone
@@more:
                mov     dl, byte ptr [bx].FillXfer.first
                call    word ptr cs:[put_edge_v]
                inc     di
                dec     ch
                jz      @@lastbyte
@@mid:
                rol     ax, cl
                lods    byte ptr fs:[si]
                xchg    ah, al
                ror     ax, cl
                mov     dl, ah
put_not         db      90h, 90h                ;; not dl, for PRESET
                db      26h
;; mov es:[di], dl; and, or, xor by B$FPSEL
put_mid         db      88h, 15h
                inc     di
                dec     ch
                jnz     @@mid
@@lastbyte:
                rol     ax, cl
                lods    byte ptr fs:[si]
                xchg    ah, al
                ror     ax, cl
                mov     dl, byte ptr [bx].FillXfer.last
                call    word ptr cs:[put_edge_v]
@@rowdone:
                pop     di
                pop     si
                add     si, [bx].FillXfer.stride
                cmp     [bx].FillXfer.bank, 0
                je      @@flat
                xor     di, 2000h
                test    di, 2000h
                jnz     @@next
                add     di, 80
                jmp     @@next
@@flat:
                add     di, [bx].FillXfer.step
@@next:
                dec     bp
                jnz     @@row
                ret
B$FPUC          endp

;; B$FPUP - PUT one plane of a box in the planar modes.
;;
;; The bytes are aligned as for the CGA and each is written by one
;; `xchg`, which loads the latches and writes through the function the
;; caller set.  The bit mask is set round the first and last byte of a
;; row if they are partial.
;;
;; Args:
;;     xfer (ax): ptr to a FillXfer.
;;
;; Saves:    bx cx dx si di bp es fs
;; Clobbers: ax, flags
B$FPUP          proc    public uses bx cx dx si di bp es fs
                mov     bx, ax
                lfs     si, [bx].FillXfer.array
                les     di, [bx].FillXfer.screen
                mov     cl, byte ptr [bx].FillXfer.shift
                mov     dx, 3CEh
                mov     al, 8
                out     dx, al
                ;; the data port: the bit mask is addressed
                inc     dx
@@row:
                push    si
                push    di
                movzx   bp, byte ptr [bx].FillXfer.sbytes
                mov     ah, fs:[si]
                inc     si
                ror     ax, cl
                dec     bp
                jnz     @@more
                push    ax
                mov     al, byte ptr [bx].FillXfer.first
                and     al, byte ptr [bx].FillXfer.last
                out     dx, al
                pop     ax
                mov     ch, ah
pn_single       db      90h, 90h
                xchg    ch, es:[di]
                jmp     @@maskff
@@more:
                cmp     byte ptr [bx].FillXfer.first, 0FFh
                je      @F
                push    ax
                mov     al, byte ptr [bx].FillXfer.first
                out     dx, al
                pop     ax
@@:
                mov     ch, ah
pn_first        db      90h, 90h
                xchg    ch, es:[di]
                inc     di
                cmp     byte ptr [bx].FillXfer.first, 0FFh
                je      @F
                push    ax
                mov     al, 0FFh
                out     dx, al
                pop     ax
@@:
                dec     bp
                jz      @@lastbyte
@@mid:
                rol     ax, cl
                lods    byte ptr fs:[si]
                xchg    ah, al
                ror     ax, cl
                mov     ch, ah
pn_mid          db      90h, 90h
                xchg    ch, es:[di]
                inc     di
                dec     bp
                jnz     @@mid
@@lastbyte:
                rol     ax, cl
                lods    byte ptr fs:[si]
                xchg    ah, al
                ror     ax, cl
                cmp     byte ptr [bx].FillXfer.last, 0FFh
                je      @F
                push    ax
                mov     al, byte ptr [bx].FillXfer.last
                out     dx, al
                pop     ax
@@:
                mov     ch, ah
pn_last         db      90h, 90h
                xchg    ch, es:[di]
@@maskff:
                mov     al, 0FFh
                out     dx, al
                pop     di
                pop     si
                add     si, [bx].FillXfer.stride
                add     di, [bx].FillXfer.step
                dec     word ptr [bx].FillXfer.rows
                jnz     @@row
                ret
B$FPUP          endp

;; mov, and, or, xor es:[di], dl; PRESET is a mov of the inverted
put_mids        dw      1588h, 1520h, 1508h, 1530h, 1588h
put_edges       dw      put_pset, put_and, put_or, put_xor, put_preset
put_edge_v      dw      0
                end
