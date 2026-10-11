;; name: gdxfer.asm
;; desc: GET and PUT of the CGA and planar modes, a plane or a row at a time
;;
;; chng: oct/26 split from fill.asm [ali]
;; obs.: the patched bytes are in the code, written through the flat data
;; selector; the registers are C's; gdfill.inc has the structures

                .386
                .model  flat

                public  B$FGET
                public  B$FPSEL
                public  B$FPUC
                public  B$FPUP


                include gdfill.inc

.code
;; B$FGET - read one plane of a box into a GET array, as QB's NReadL.
;;
;; Each array byte is the high byte of `rol ax, cl` over two screen bytes.
;;
;; Args:
;;     xfer (eax): ptr to a FillXfer.
;;
;; Saves:    ebx ecx edx esi edi ebp
;; Clobbers: eax, flags
B$FGET          proc    uses ebx ecx edx esi edi ebp
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
                ret
B$FGET          endp



;; B$FPSEL - set the way of the PUT that follows, once.
;;
;; Args:
;;     how (eax): 0 PSET, 1 AND, 2 OR, 3 XOR, 4 PRESET.
;;
;; Saves:    ebx ecx
;; Clobbers: eax, flags
;; Patches:  the write of B$FPUC and B$FPUP.
B$FPSEL         proc    uses ebx ecx
                mov     ebx, eax
                mov     ax, 9090h               ;; nothing before the write
                mov     cx, ax
                cmp     ebx, 4
                jne     @F
                ;; not dl, the middle bytes of a CGA PUT
                mov     ax, 0D2F6h
                ;; not ch, the bytes of a planar one
                mov     cx, 0D5F6h
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
                ret
B$FPSEL         endp

;; the edge bytes of a CGA PUT: ah = the source (kept), dl = the mask of the
;; pixels it covers, [edi] the screen byte
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

;; B$FPUC - PUT a box in the CGA modes, as QB's NWriteL.
;;
;; Each screen byte takes bits of two array bytes by `ror ax, cl` over
;; the pair.  The middle bytes are the `op [di], dl` that B$FPSEL
;; patched; the first and last go through a mask.
;;
;; Args:
;;     xfer (eax): ptr to a FillXfer.
;;
;; Saves:    ebx ecx edx esi edi ebp
;; Clobbers: eax, flags
B$FPUC          proc    uses ebx ecx edx esi edi ebp
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
;; mov [edi], dl; and, or, xor by B$FPSEL
put_mid         db      88h, 17h
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
;;     xfer (eax): ptr to a FillXfer.
;;
;; Saves:    ebx ecx edx esi edi ebp
;; Clobbers: eax, flags
B$FPUP          proc    uses ebx ecx edx esi edi ebp
                mov     ebx, eax
                mov     esi, [ebx].FillXfer.array
                mov     edi, [ebx].FillXfer.screen
                mov     cl, byte ptr [ebx].FillXfer.shift
                mov     edx, 3CEh
                mov     al, 8
                out     dx, al
                ;; the data port: the bit mask is addressed
                inc     edx
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
                ret
B$FPUP          endp

;; mov, and, or, xor [edi], dl; PRESET is a mov of the inverted
put_mids        dw      1788h, 1720h, 1708h, 1730h, 1788h
put_edges       dd      put_pset, put_and, put_or, put_xor, put_preset
put_edge_v      dd      0
                end
