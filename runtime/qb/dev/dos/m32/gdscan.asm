;; name: gdscan.asm
;; desc: the scans for PAINT, a loop for each kind of mode
;;
;; chng: oct/26 split from fill.asm [ali]
;; obs.: the patched bytes are in the code, written through the flat data
;; selector; the registers are C's; gdfill.inc has the structures

                .386
                .model  flat

                public  B$FSCL
                public  B$FSCP
                public  B$FSCC
                public  B$FSCR


                include gdfill.inc

.code
;; B$FSCL - scan a 256-colour row for PAINT.
;;
;; `repne scasb` finds a colour, `repe scasb` skips one.
;;
;; Args:
;;     scan (eax): ptr to a FillScan.
;;
;; Returns:
;;     scan.found: the index of the pixel from `at`, -1 if none.
;;
;; Saves:    ebx ecx edx esi edi ebp
;; Clobbers: eax, flags
B$FSCL          proc uses ebx ecx edx esi edi ebp
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
;; the first pixel that is neither colour, the colour skipped and the other
;; swapped
@@skip:
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
;; the first pixel of c1 or of c2
@@match:
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
                ret
B$FSCL          endp

;; B$FSCP - scan a planar row for PAINT.
;;
;; A byte of eight pixels at a time by the controller's colour compare
;; (read mode 1, set up by the caller): one `out` and one read tell
;; which of the eight are of a colour.
;;
;; Args:
;;     scan (eax): ptr to a FillScan.
;;
;; Returns:
;;     scan.found: the index of the byte from `at`, -1 if none.
;;     scan.hits: the pixels of that byte that matched.
;;
;; Saves:    ebx ecx edx esi edi ebp
;; Clobbers: eax, flags
B$FSCP          proc    uses ebx ecx edx esi edi ebp
                mov     ebx, eax
                mov     edi, [ebx].FillScan.at
                mov     cl, byte ptr [ebx].FillScan.count
                mov     esi, [ebx].FillScan.c1
                shl     esi, 8
                ;; the colour compare register, then the colour
                or      esi, 2
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
                ret
B$FSCP          endp


;; the pixels of the byte at [edi] that are wanted, a bit for each slot, under
;; the mask `maskop`; ZF set if none
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
;;     scan (eax): ptr to a FillScan; `x` to `to` are the pixels to scan.
;;
;; Returns:
;;     scan.found: the pixel, -1 if none.
;;
;; Saves:    ebx ecx edx esi edi ebp
;; Clobbers: eax, flags
B$FSCC          proc    uses ebx ecx edx esi edi ebp
                mov     ebx, eax
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
;;     scan (eax): ptr to a FillScan.
;;
;; Returns:
;;     scan.found: 0, or -1 if the row had no such pixel.
;;     scan.left, scan.right: the run, when found is 0.
;;
;; Saves:    ebx ecx edx esi edi ebp
;; Clobbers: eax, flags
B$FSCR          proc    uses ebx ecx edx esi edi ebp
                mov     ebx, eax
                call    scan_one
                mov     eax, [ebx].FillScan.found
                or      eax, eax
                js      @@out
                mov     [ebx].FillScan.right, eax
                mov     [ebx].FillScan.x, eax
                mov     [ebx].FillScan.to, 0
                mov     eax, [ebx].FillScan.c1
                mov     [ebx].FillScan.c2, eax
                or      dword ptr [ebx].FillScan.flags, 1
                call    scan_one
                mov     eax, [ebx].FillScan.found
                inc     eax
                mov     [ebx].FillScan.left, eax
                mov     eax, [ebx].FillScan.right
                mov     [ebx].FillScan.x, eax
                mov     eax, [ebx].FillScan.limit
                mov     [ebx].FillScan.to, eax
                call    scan_one
                mov     eax, [ebx].FillScan.found
                or      eax, eax
                jns     @F
                mov     eax, [ebx].FillScan.limit
                inc     eax
@@:
                dec     eax
                mov     [ebx].FillScan.right, eax
                mov     [ebx].FillScan.found, 0
@@out:
                ret
B$FSCR          endp

;; scan_one (bx: ptr FillScan): the search of the CGA pixels from `x` to `to`;
;; the registers are the callers' to save
scan_one        proc    near

                mov     esi, [ebx].FillScan.flags
                ;; pixels of 1 bit: 8 to a byte (log 3, last pixel of a byte 7)
                mov     dl, 1
                mov     dh, 3
                mov     ch, 7
                test    esi, 4
                jz      @F
                mov     dl, 2                   ;; or 2 bits: 4 to a byte
                mov     dh, 2
                mov     ch, 3
@@:
                mov     ebp, 1
                mov     eax, [ebx].FillScan.x
                cmp     eax, [ebx].FillScan.to
                jle     @F
                mov     ebp, -1
                or      esi, 2
@@:
                mov     cl, dh
                mov     eax, [ebx].FillScan.x
                shr     eax, cl
                mov     edi, [ebx].FillScan.at
                add     edi, eax
                mov     [ebx].FillScan.count, eax
                mov     eax, [ebx].FillScan.to
                shr     eax, cl
                sub     eax, [ebx].FillScan.count
                jge     @F
                neg     eax
@@:
                inc     eax
                mov     [ebx].FillScan.count, eax
                ;; the pixels of a byte: a bit each, the low bit of each if 2
                ;; bits
                mov     dh, 0FFh
                cmp     dl, 2
                jne     @F
                mov     dh, 55h
@@:
                test    esi, 2
                jnz     @@maskback
                SHRMASK byte ptr [ebx].FillScan.x
                mov     byte ptr [ebx].FillScan.first, al
                SHLMASK byte ptr [ebx].FillScan.to
                mov     byte ptr [ebx].FillScan.last, al
                jmp     @@maskdone
@@maskback:
                SHLMASK byte ptr [ebx].FillScan.x
                mov     byte ptr [ebx].FillScan.first, al
                SHRMASK byte ptr [ebx].FillScan.to
                mov     byte ptr [ebx].FillScan.last, al
@@maskdone:
                mov     byte ptr [ebx].FillScan.middle, dh
                mov     al, byte ptr [ebx].FillScan.c1
                PATTERN
                push    eax
                mov     al, byte ptr [ebx].FillScan.c2
                PATTERN
                mov     dh, al
                pop     eax
                mov     dl, al
                mov     cl, byte ptr [ebx].FillScan.count
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
;; 2-bit pixels, a pixel of the colour: some pixel of the byte xor the pattern
;; is 00
@@findequal2:
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
;; 2-bit pixels, a pixel that is not: some byte that is not the pattern
@@skipequal2:
                mov     al, dl
                jmp     @@scanbytes
@@onebit:
                mov     al, dl
                test    esi, 1
                jz      @@scanbytes
                ;; a pixel of the colour: some byte that is not its complement
                not     al
                jmp     @@scanbytes
;; the bytes between, none of them al: repe scasb, backwards when bp is -1
@@scanbytes:
                movzx   ecx, cl                 ;; all of ecx counts: the caller's upper half is not 0
                cld
                cmp     ebp, 0
                jg      @F
                std
@@:
                repe    scasb
                cld
                je      @@lastbyte
                sub     edi, ebp
                jmp     @@mid
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
                ;; (the loops say a wanted pixel is here)
                add     edi, ebp
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
                mov     ah, 0
                ;; the pixels of the byte that were found, a bit each
                mov     edx, eax
                mov     eax, edi
                sub     eax, [ebx].FillScan.at
                mov     cl, 3
                mov     dh, 80h
                mov     ebp, 1
                test    esi, 4
                jz      @F
                mov     cl, 2
                mov     dh, 40h
                mov     ebp, 2
@@:
                shl     eax, cl               ;; the byte's first pixel
                mov     ecx, ebp
                test    esi, 2
                jnz     @@hitback
@@hitforward:
                test    dl, dh
                jnz     @@gotpixel
                inc     eax
                shl     dl, cl
                jmp     @@hitforward
@@hitback:
                add     eax, 7                ;; the byte's last pixel
                test    esi, 4
                jz      @@hitbackloop
                sub     eax, 4
@@hitbackloop:
                test    dl, 1
                jnz     @@gotpixel
                dec     eax
                shr     dl, cl
                jmp     @@hitbackloop
@@gotpixel:
                mov     [ebx].FillScan.found, eax
@@done:
                ret
scan_one        endp
                end
