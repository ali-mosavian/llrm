;; name: gdfill.asm
;; desc: the box fill of the graphics screen, `op mem, imm` patched once per
;; primitive
;;
;; chng: oct/26 split from fill.asm [ali]
;; obs.: the patched bytes are in the code, written through cs; the registers
;; are C's; gdfill.inc has the structures

                .model  medium, pascal
                .386
                option  proc:private

                include gdfill.inc

.code
;; B$FSEL - set the box fill's operation and pattern, once per primitive.
;;
;; Args:
;;     operation (ax): 0 set, 1 and, 2 or, 3 xor.
;;     pattern   (dl): the byte to fill with.
;;
;; Saves:    bx
;; Clobbers: ax, flags
;; Patches:  the opcode, mod/rm and immediate of the box fill's loops.
B$FSEL          proc    public uses bx
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
                ;; the patched bytes are not in the prefetch queue
                jmp     short @F
@@:
                ret
B$FSEL          endp

;; B$FBOX - fill rows of a box with the operation B$FSEL set.
;;
;; A row is an edge byte (if `edges`), `middle` bytes, then an edge byte.
;; A row starts `steps[0]` after the one before it when that is even,
;; `steps[1]` when it is odd (`phase` says which the first is).
;;
;; Args:
;;     box (ax): ptr to a FillBox.
;;
;; Saves:    si di bp es
;; Clobbers: ax bx cx dx, flags
B$FBOX          proc    public uses si di bp es
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
;; es: mov dword ptr [di], imm32; the opcode and mod/rm patched
dword_op        db      66h, 26h, 0C7h, 05h
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
;; a set is UGL's hlinef: dwords, then the bytes left
@@set:
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
                ret
B$FBOX          endp

;; the opcode and the mod/rm of each operation, for a dword and for a byte

set_pattern     dd      0
set_flag        db      0
dword_ops       dw      05C7h, 2581h, 0D81h, 3581h
byte_ops        dw      05C6h, 2580h, 0D80h, 3580h
                end
