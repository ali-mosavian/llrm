;; name: B$MOVE, B$FILL, B$COPY, B$FILLB, B$CMPB
;; desc: string moves by dwords
;;
;; chng: oct/26 written [ali]
;; obs.: the registers are C's (the target's own convention: eax, edx, ebx,
;;       ecx); es is ds under the extender.

                .386
                .model  flat

                public  B$MOVE
                public  B$FILL
                public  B$FILLB
                public  B$CMPB
                public  B$COPY

BlockOp         struc
                dst             dword   ?
                src             dword   ?
                ;; bytes, or words for B$FILL
                count           dword   ?
                value           dword   ?               ;; for B$FILL
BlockOp         ends

.code
;; B$MOVE - copy `count` bytes from `src` to `dst`, forward.
;;
;; By dwords and then the bytes left; by bytes alone if `value` is not 0 (an EGA
;; latch copy holds one byte).
;;
;; Args:
;;     op (eax): ptr to a BlockOp.
;;
;; Saves:    ecx edx esi edi
;; Clobbers: eax, flags
B$MOVE          proc uses ecx edx esi edi

                mov     edi, [eax].BlockOp.dst
                mov     esi, [eax].BlockOp.src
                mov     ecx, [eax].BlockOp.count
                mov     edx, [eax].BlockOp.value
                cld
                test    edx, edx
                jnz     @F
                mov     eax, ecx
                shr     ecx, 2
                rep     movsd
                mov     ecx, eax
                and     ecx, 3
@@:
                rep     movsb
                ret
B$MOVE          endp

;; B$FILL - fill `count` words at `dst` with the 16-bit `value`.
;;
;; Two words at a time, and the last one if `count` is odd.
;;
;; Args:
;;     op (eax): ptr to a BlockOp.
;;
;; Saves:    ecx edx edi
;; Clobbers: eax, flags
B$FILL          proc uses ecx edx edi

                mov     edi, [eax].BlockOp.dst
                movzx   edx, word ptr [eax].BlockOp.value
                mov     ecx, edx
                shl     ecx, 16
                or      edx, ecx                ;; the word twice
                mov     ecx, [eax].BlockOp.count
                mov     eax, edx
                cld
                shr     ecx, 1
                rep     stosd
                jnc     @F
                stosw
@@:
                ret
B$FILL          endp

;; B$COPY - copy `count` bytes from `from` to `to`, forward.
;;
;; By dwords and then the bytes left; byte by byte when `to` is 0 to 3 above
;; `from`, as a loop would, or when there are fewer than 4.
;;
;; Args:
;;     to    (eax): ptr.
;;     from  (edx): ptr.
;;     count (ebx): bytes.
;;
;; Saves:    ecx esi edi
;; Clobbers: eax, flags
B$COPY          proc uses ecx esi edi

                mov     edi, eax
                mov     esi, edx
                mov     ecx, ebx
                cld
                ;; under a dword: bytes alone, no split
                cmp     ecx, 4
                jb      @F
                mov     eax, edi
                sub     eax, esi
                cmp     eax, 4
                ;; `to` 0 to 3 above `from`: byte by byte, as a loop would
                jb      @F
                mov     eax, ecx
                shr     ecx, 2
                rep     movsd
                mov     ecx, eax
                and     ecx, 3
@@:
                rep     movsb
                ret
B$COPY          endp

;; B$FILLB - set `count` bytes at `to` to the byte `value`.
;;
;; By dwords and then the bytes left.
;;
;; Args:
;;     to    (eax): ptr.
;;     value (edx): the byte, in dl.
;;     count (ebx): bytes.
;;
;; Saves:    ecx edi
;; Clobbers: eax, flags
B$FILLB         proc uses ecx edi

                mov     edi, eax
                movzx   eax, dl
                imul    eax, eax, 01010101h     ;; the byte four times
                mov     ecx, ebx
                cld
                shr     ecx, 2
                rep     stosd
                mov     ecx, ebx
                and     ecx, 3
                rep     stosb
                ret
B$FILLB         endp

;; B$CMPB - compare `count` bytes at `first` and `second`, as unsigned.
;;
;; By dwords until one differs, then the four bytes it is in.
;;
;; Args:
;;     first  (eax): ptr.
;;     second (edx): ptr.
;;     count  (ebx): bytes.
;;
;; Returns:  eax: -1, 0 or 1 as `first` sorts before, with or after `second`.
;; Saves:    ecx esi edi
;; Clobbers: flags
B$CMPB          proc uses ecx esi edi

                mov     esi, eax
                mov     edi, edx
                mov     ecx, ebx
                cld
                shr     ecx, 2                  ;; ZF set when no dwords: nothing differs yet
                repe    cmpsd
                je      @F
                sub     esi, 4
                sub     edi, 4
                mov     ecx, 4
                jmp     lastbytes
@@:
                mov     ecx, ebx
                and     ecx, 3
lastbytes:
                repe    cmpsb
                mov     eax, 0
                je      @F
                mov     eax, 1
                ja      @F
                mov     eax, -1
@@:
                ret
B$CMPB          endp
                end
