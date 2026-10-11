;; name: gdscan.asm
;; desc: the scans for PAINT, a loop for each kind of mode
;;
;; chng: oct/26 split from fill.asm [ali]
;; obs.: the patched bytes are in the code, written through cs; the registers
;; are C's; gdfill.inc has the structures

                .model  medium, pascal
                .386
                option  proc:private

                include gdfill.inc

.code
;; B$FSCL - scan a 256-colour row for PAINT.
;;
;; `repne scasb` finds a colour, `repe scasb` skips one.
;;
;; Args:
;;     scan (ax): ptr to a FillScan.
;;
;; Returns:
;;     scan.found: the index of the pixel from `at`, -1 if none.
;;
;; Saves:    bx cx dx si di bp es
;; Clobbers: ax, flags
B$FSCL          proc    public uses bx cx dx si di bp es
                push    ax
                mov     bx, ax
                les     di, [bx].FillScan.at
                mov     bp, [bx].FillScan.count
                mov     si, [bx].FillScan.flags
                mov     al, byte ptr [bx].FillScan.c1
                mov     ah, byte ptr [bx].FillScan.c2
                mov     dx, di                ;; the start
                mov     bx, -1                 ;; back to the pixel just tested
                cld
                test    si, 2
                jz      @F
                std
                mov     bx, 1
@@:
                mov     cx, bp
                test    si, 1
                jnz     @@match
;; the first pixel that is neither colour, the colour skipped and the other
;; swapped
@@skip:
                repe    scasb
                je      @@none
                cmp     es:[bx + di], ah
                jne     @@hit0
                xchg    al, ah
                jmp     @@skip
@@hit0:
                mov     ax, bp
                sub     ax, cx
                dec     ax
                jmp     @@done
;; the first pixel of c1 or of c2
@@match:
                repne   scasb
                jne     @@no1
                mov     si, bp
                sub     si, cx
                dec     si                     ;; where c1 is
                cmp     al, ah
                je      @@got1
                test    si, si
                jz      @@got1
                mov     di, dx
                mov     cx, si
                mov     al, ah
                repne   scasb
                jne     @@got1
                mov     ax, si
                sub     ax, cx
                dec     ax
                jmp     @@done
@@got1:
                mov     ax, si
                jmp     @@done
@@no1:
                cmp     al, ah
                je      @@none
                mov     di, dx
                mov     cx, bp
                mov     al, ah
                repne   scasb
                jne     @@none
                mov     ax, bp
                sub     ax, cx
                dec     ax
                jmp     @@done
@@none:
                mov     ax, -1
@@done:
                cld
                pop     bx
                mov     [bx].FillScan.found, ax
                ret
B$FSCL          endp

;; B$FSCP - scan a planar row for PAINT.
;;
;; A byte of eight pixels at a time by the controller's colour compare
;; (read mode 1, set up by the caller): one `out` and one read tell
;; which of the eight are of a colour.
;;
;; Args:
;;     scan (ax): ptr to a FillScan.
;;
;; Returns:
;;     scan.found: the index of the byte from `at`, -1 if none.
;;     scan.hits: the pixels of that byte that matched.
;;
;; Saves:    bx cx dx si di bp es
;; Clobbers: ax, flags
B$FSCP          proc    public uses bx cx dx si di bp es
                mov     bx, ax
                les     di, [bx].FillScan.at
                mov     cl, byte ptr [bx].FillScan.count
                mov     si, [bx].FillScan.c1
                shl     si, 8
                ;; the colour compare register, then the colour
                or      si, 2
                mov     bp, [bx].FillScan.c2
                shl     bp, 8
                or      bp, 2
                mov     dx, 3CEh
@@byte:
                mov     ax, si
                out     dx, ax
                mov     ch, es:[di]
                cmp     si, bp
                je      @F
                mov     ax, bp
                out     dx, ax
                or      ch, es:[di]
@@:
                test    byte ptr [bx].FillScan.flags, 1
                jnz     @F
                not     ch
@@:
                and     ch, byte ptr [bx].FillScan.first
                mov     byte ptr [bx].FillScan.first, 0FFh
                cmp     cl, 1
                jne     @F
                and     ch, byte ptr [bx].FillScan.last
@@:
                test    ch, ch
                jnz     @@hit
                test    byte ptr [bx].FillScan.flags, 2
                jnz     @@back
                inc     di
                jmp     @@next
@@back:
                dec     di
@@next:
                dec     cl
                jnz     @@byte
                mov     [bx].FillScan.found, -1
                mov     [bx].FillScan.hits, 0
                jmp     @@done
@@hit:
                movzx   ax, cl
                neg     ax
                add     ax, [bx].FillScan.count
                mov     [bx].FillScan.found, ax
                movzx   ax, ch
                mov     [bx].FillScan.hits, ax
@@done:
                ret
B$FSCP          endp

;; the pixels of the byte at [di] that are wanted, a bit for each slot, under
;; the mask `maskop`; ZF set if none
BYTEHIT         macro   maskop
                local   one, both, keep
                mov     al, es:[di]
                mov     ah, al
                xor     al, dl
                xor     ah, dh
                test    si, 4
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
                test    si, 1
                jnz     keep
                not     al
keep:
                and     al, maskop
                endm

;; SHRMASK / SHLMASK (dl: bits a pixel, ch: pixels a byte less 1, dh: all the
;; pixels): al = the pixels of a byte from the one `slot` names
;; to its end: those that are the pixel and the ones after it (SHR), or it and
;; the ones before (SHL)
SHRMASK         macro   slot
                mov     al, slot
                and     al, ch
                mov     cl, dl
                dec     cl
                shl     al, cl
                mov     cl, al
                mov     al, 0FFh
                shr     al, cl
                and     al, dh
                endm
SHLMASK         macro   slot
                mov     al, slot
                not     al
                and     al, ch
                mov     cl, dl
                dec     cl
                shl     al, cl
                mov     cl, al
                mov     al, 0FFh
                shl     al, cl
                and     al, dh
                endm
;; PATTERN (al: a colour, dl: bits a pixel): al = the byte with it in every
;; pixel
PATTERN         macro
                local   one, done
                cmp     dl, 2
                jne     one
                and     al, 3
                mov     ah, 55h
                mul     ah
                jmp     done
one:
                and     al, 1
                neg     al
done:
                endm

;; B$FSCC - scan a CGA row for PAINT, a byte at a time.
;;
;; The pixels of a byte equal to a colour show as bits of the byte xor
;; the colour's pattern.  The first and last byte are taken under their
;; masks; the ones between, for one colour, by a short loop that only
;; says whether a pixel of the byte is wanted, and the byte it stops at
;; is taken as the others.
;;
;; Args:
;;     scan (ax): ptr to a FillScan; `x` to `to` are the pixels to scan.
;;
;; Returns:
;;     scan.found: the pixel, -1 if none.
;;
;; Saves:    bx cx dx si di bp es
;; Clobbers: ax, flags
B$FSCC          proc    public uses bx cx dx si di bp es
                mov     bx, ax
                call    scan_one
                ret
B$FSCC          endp

;; B$FSCR - scan a CGA PAINT row for the open pixel and the run round it.
;;
;; Searches `x` to `to` for a pixel that is neither colour `c1` nor `c2`.
;; The run round it of pixels that are not `c1` is `left` to `right`,
;; within 0 to `limit`.
;;
;; Args:
;;     scan (ax): ptr to a FillScan.
;;
;; Returns:
;;     scan.found: 0, or -1 if the row had no such pixel.
;;     scan.left, scan.right: the run, when found is 0.
;;
;; Saves:    bx cx dx si di bp es
;; Clobbers: ax, flags
B$FSCR          proc    public uses bx cx dx si di bp es
                mov     bx, ax
                call    scan_one
                mov     ax, [bx].FillScan.found
                or      ax, ax
                js      @@out
                mov     [bx].FillScan.right, ax
                mov     [bx].FillScan.x, ax
                mov     [bx].FillScan.to, 0
                mov     ax, [bx].FillScan.c1
                mov     [bx].FillScan.c2, ax
                or      word ptr [bx].FillScan.flags, 1
                call    scan_one
                mov     ax, [bx].FillScan.found
                inc     ax
                mov     [bx].FillScan.left, ax
                mov     ax, [bx].FillScan.right
                mov     [bx].FillScan.x, ax
                mov     ax, [bx].FillScan.limit
                mov     [bx].FillScan.to, ax
                call    scan_one
                mov     ax, [bx].FillScan.found
                or      ax, ax
                jns     @F
                mov     ax, [bx].FillScan.limit
                inc     ax
@@:
                dec     ax
                mov     [bx].FillScan.right, ax
                mov     [bx].FillScan.found, 0
@@out:
                ret
B$FSCR          endp

;; scan_one (bx: ptr FillScan): the search of the CGA pixels from `x` to `to`;
;; the registers are the callers' to save
scan_one        proc    near

                mov     si, [bx].FillScan.flags
                ;; pixels of 1 bit: 8 to a byte (log 3, last pixel of a byte 7)
                mov     dl, 1
                mov     dh, 3
                mov     ch, 7
                test    si, 4
                jz      @F
                mov     dl, 2                   ;; or 2 bits: 4 to a byte
                mov     dh, 2
                mov     ch, 3
@@:
                mov     bp, 1
                mov     ax, [bx].FillScan.x
                cmp     ax, [bx].FillScan.to
                jle     @F
                mov     bp, -1
                or      si, 2
@@:
                mov     cl, dh
                mov     ax, [bx].FillScan.x
                shr     ax, cl
                les     di, [bx].FillScan.at
                add     di, ax
                mov     [bx].FillScan.count, ax
                mov     ax, [bx].FillScan.to
                shr     ax, cl
                sub     ax, [bx].FillScan.count
                jge     @F
                neg     ax
@@:
                inc     ax
                mov     [bx].FillScan.count, ax
                ;; the pixels of a byte: a bit each, the low bit of each if 2
                ;; bits
                mov     dh, 0FFh
                cmp     dl, 2
                jne     @F
                mov     dh, 55h
@@:
                test    si, 2
                jnz     @@maskback
                SHRMASK byte ptr [bx].FillScan.x
                mov     byte ptr [bx].FillScan.first, al
                SHLMASK byte ptr [bx].FillScan.to
                mov     byte ptr [bx].FillScan.last, al
                jmp     @@maskdone
@@maskback:
                SHLMASK byte ptr [bx].FillScan.x
                mov     byte ptr [bx].FillScan.first, al
                SHRMASK byte ptr [bx].FillScan.to
                mov     byte ptr [bx].FillScan.last, al
@@maskdone:
                mov     byte ptr [bx].FillScan.middle, dh
                mov     al, byte ptr [bx].FillScan.c1
                PATTERN
                push    ax
                mov     al, byte ptr [bx].FillScan.c2
                PATTERN
                mov     dh, al
                pop     ax
                mov     dl, al
                mov     cl, byte ptr [bx].FillScan.count
@@:
                cmp     cl, 1
                jne     @@several
                BYTEHIT byte ptr [bx].FillScan.first
                and     al, byte ptr [bx].FillScan.last
                jnz     @@hit
                jmp     @@none
@@several:
                BYTEHIT byte ptr [bx].FillScan.first
                jnz     @@hit
                add     di, bp
                dec     cl
                cmp     cl, 1
                je      @@lastbyte
                dec     cl                      ;; the bytes between
                cmp     dl, dh
                jne     @@twocolours
                test    si, 4
                jz      @@onebit
                test    si, 1
                jz      @@skipequal2
;; 2-bit pixels, a pixel of the colour: some pixel of the byte xor the pattern
;; is 00
@@findequal2:
                mov     al, es:[di]
                xor     al, dl
                mov     ch, al
                shr     ch, 1
                or      al, ch
                and     al, 55h
                cmp     al, 55h
                jne     @@mid
                add     di, bp
                dec     cl
                jnz     @@findequal2
                jmp     @@lastbyte
;; 2-bit pixels, a pixel that is not: some byte that is not the pattern
@@skipequal2:
                mov     al, dl
                jmp     @@scanbytes
@@onebit:
                mov     al, dl
                test    si, 1
                jz      @@scanbytes
                ;; a pixel of the colour: some byte that is not its complement
                not     al
                jmp     @@scanbytes
;; the bytes between, none of them al: repe scasb, backwards when bp is -1
@@scanbytes:
                xor     ch, ch
                cld
                cmp     bp, 0
                jg      @F
                std
@@:
                repe    scasb
                cld
                je      @@lastbyte
                sub     di, bp
                jmp     @@mid
@@twocolours:
                BYTEHIT byte ptr [bx].FillScan.middle
                jnz     @@hit
                add     di, bp
                dec     cl
                jnz     @@twocolours
                jmp     @@lastbyte
@@mid:
                BYTEHIT byte ptr [bx].FillScan.middle
                jnz     @@hit
                ;; (the loops say a wanted pixel is here)
                add     di, bp
                dec     cl
                jmp     @@lastbyte
@@lastbyte:
                BYTEHIT byte ptr [bx].FillScan.last
                jnz     @@hit
@@none:
                mov     [bx].FillScan.found, -1
                mov     [bx].FillScan.hits, 0
                jmp     @@done
@@hit:
                mov     ah, 0
                ;; the pixels of the byte that were found, a bit each
                mov     dx, ax
                mov     ax, di
                sub     ax, word ptr [bx].FillScan.at
                mov     cl, 3
                mov     dh, 80h
                mov     bp, 1
                test    si, 4
                jz      @F
                mov     cl, 2
                mov     dh, 40h
                mov     bp, 2
@@:
                shl     ax, cl               ;; the byte's first pixel
                mov     cx, bp
                test    si, 2
                jnz     @@hitback
@@hitforward:
                test    dl, dh
                jnz     @@gotpixel
                inc     ax
                shl     dl, cl
                jmp     @@hitforward
@@hitback:
                add     ax, 7                ;; the byte's last pixel
                test    si, 4
                jz      @@hitbackloop
                sub     ax, 4
@@hitbackloop:
                test    dl, 1
                jnz     @@gotpixel
                dec     ax
                shr     dl, cl
                jmp     @@hitbackloop
@@gotpixel:
                mov     [bx].FillScan.found, ax
@@done:
                ret
scan_one        endp
                end
