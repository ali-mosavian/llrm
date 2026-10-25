;; name: LL$OPEN, LL$CREATE, LL$READ, LL$SEEK, LL$WRITE_FILE, LL$CLOSE, LL$EXIT, LL$MORE, LL$CONSOLE_READ_KEY, LL$CONSOLE_KEY_READY, LL$DELETE, LL$RENAME, LL$MKDIR, LL$CHDIR, LL$RMDIR, LL$FIND_FIRST, LL$FIND_NEXT, LL$FREE_BYTES, LL$CURRENT_DIRECTORY, LL$CURRENT_DRIVE
;; desc: the OS layer shared by C, Nib and BASIC programs: x86-m32, DOS/32A
;;
;; args: the interface of runtime/shared/interface.toml, in cdecl32: arguments in dword slots from
;;       [esp+4]; ebx, esi and edi kept
;; retn: eax -> the result, or the DOS error code negated
;;
;; chng: oct/26 restyled, symbols renamed to LL$ [ali]
;; obs.: DOS/32A takes a flat DS:EDX buffer for the file calls, so none needs a transfer buffer.
;;       One object per group, as the m16 layer (OS_GROUPS).

;; The operating-system routines std.os declares (os.nib), in cdecl32: arguments in dword slots from
;; [esp+4], EBX, ESI and EDI kept, a result in EAX. DOS/32A takes a flat DS:EDX buffer for the file
;; calls, so none needs a transfer buffer.
.386
.model flat

;; Which groups of the interface this object holds. A library has one object per group, each
;; assembled with -DOS_GROUPS=<its bit>, so a program links only what it calls; assembled with no
;; definition the file holds every group.
G_CORE          equ     1
G_CONSOLE       equ     2
G_PROCESS       equ     4
G_FILES         equ     8
ifndef OS_GROUPS
OS_GROUPS       equ     15
endif

;; What DOS's device information (IOCTL 44h) says of a handle: bit 7 is a character device.
DEVICE_BIT      equ     80h
CTRL_Z          equ     1Ah

if OS_GROUPS and G_CORE
public LL$OPEN
public LL$CREATE
public LL$READ
public LL$SEEK
public LL$WRITE_FILE
public LL$CLOSE
public LL$EXIT
public LL$MORE
public LL$STACK_LOW
public LL$PSP
endif
if OS_GROUPS and G_CONSOLE
public LL$CONSOLE_READ_KEY
public LL$CONSOLE_KEY_READY
endif
if OS_GROUPS and G_FILES
public LL$DELETE
public LL$RENAME
public LL$MKDIR
public LL$CHDIR
public LL$RMDIR
public LL$FIND_FIRST
public LL$FIND_NEXT
public LL$FREE_BYTES
public LL$CURRENT_DIRECTORY
public LL$CURRENT_DRIVE
endif
if OS_GROUPS and G_PROCESS
public LL$COMMAND_LINE
public LL$ENVIRONMENT
public LL$EXECUTE
public LL$ENVIRONMENT_READ
public LL$ENVIRONMENT_WRITE
endif

;; HEAP_BYTES comes from os.toml, DOS_* from the OS's facts: the assembler is told both.

if OS_GROUPS and G_CORE
.data
;; The lowest ESP a checked function may reach (-fsanitize=stack). Start-up sets it.
LL$STACK_LOW    dd      0
;; The program's PSP selector, which the extender enters with in ES (start.asm keeps it).
LL$PSP          dw      0
;; The heap's next free byte and the end of its arena: set on the first LL$MORE, which asks the
;; extender for the arena.
heap_next       dd      0
heap_end        dd      0
;; The request being served and the arena size being tried, which DPMI 0501h's register use leaves in memory.
heap_ask        dd      0
heap_try        dd      0
endif
.code
if OS_GROUPS and G_CORE
;;::::::::::::::
;; LL$MORE(bytes: usize) -> *near mut u8: `bytes` more of the heap at its end, or 0 when it runs out.
;; The heap grows inside an arena taken from the extender (DPMI 0501h, allocate memory block): the first
;; call takes HEAP_BYTES, or the largest half of it the extender has, down to the request. When the arena
;; cannot hold a request the next is another arena (at least the request), and the grant is at an address
;; that need not follow the last one's: the heap chains arenas.
LL$MORE         proc
                mov     ecx, dword ptr [esp+4]
                mov     eax, heap_next
                test    eax, eax
                jz      take
                lea     edx, [eax + ecx]
                cmp     edx, heap_end
                ja      take
                mov     heap_next, edx
                ret
take:
                push    ebx
                push    esi
                push    edi
;; DPMI 0501h returns the block's handle in SI:DI, so the request and the size being tried stay in memory.
                mov     heap_ask, ecx
                mov     edx, HEAP_BYTES
                cmp     edx, ecx
                jae     sized
                mov     edx, ecx
sized:
                mov     heap_try, edx
arena:
                mov     ebx, heap_try
                shr     ebx, 16
                mov     ecx, heap_try
                and     ecx, 0FFFFh
                mov     eax, DOS_DPMI_ALLOC
                int     DOS_DPMI_INT
                jnc     arena_got
                mov     edx, heap_try
                shr     edx, 1
                cmp     edx, heap_ask
                jb      arena_refused
                mov     heap_try, edx
                jmp     arena
arena_refused:
                pop     edi
                pop     esi
                pop     ebx
                xor     eax, eax
                ret
arena_got:
                shl     ebx, 16
                mov     bx, cx
                mov     eax, ebx
                add     ebx, heap_try
                mov     heap_end, ebx
                mov     edx, eax
                add     edx, heap_ask
                mov     heap_next, edx
                pop     edi
                pop     esi
                pop     ebx
                ret
LL$MORE         endp

;; The DOS file calls. Each returns what DOS does, or when DOS sets carry, its error code negated.

;;::::::::::::::
;; LL$OPEN(name: *far char, mode: u8) -> i16: a handle to an existing file.
LL$OPEN         proc
                mov     edx, dword ptr [esp+4]
                mov     al, byte ptr [esp+8]
                mov     ah, DOS_OPEN
                int     DOS_INT
                jmp     short checked
LL$OPEN         endp

;;::::::::::::::
;; LL$CREATE(name: *far char) -> i16: a handle to a new, empty file.
LL$CREATE       proc
                mov     edx, dword ptr [esp+4]
                xor     ecx, ecx
                mov     ah, DOS_CREATE
                int     DOS_INT
                jmp     short checked
LL$CREATE       endp

;;::::::::::::::
;; LL$READ(handle: i16, data: *near mut u8, count: usize) -> isize: bytes read, in one call: DOS/32A takes a
;; 32-bit count and returns one. Where DOS sets carry, the error code negated.
LL$READ         proc
                mov     ah, DOS_READ
                jmp     short transfer
LL$READ         endp

;;::::::::::::::
;; LL$SEEK(handle: i16, position: i32, origin: u8) -> i32: the absolute position, or the error code negated.
LL$SEEK         proc
                push    ebx
                movzx   ebx, word ptr [esp+8]
                mov     edx, dword ptr [esp+12]
                mov     ecx, edx
                shr     ecx, 16
                mov     al, byte ptr [esp+16]
                mov     ah, DOS_SEEK
                int     DOS_INT
                pop     ebx
                jc      short seek_failed
                movzx   eax, ax
                movzx   edx, dx
                shl     edx, 16
                or      eax, edx
                ret
seek_failed:
                movzx   eax, ax
                neg     eax
                ret
LL$SEEK         endp

;;::::::::::::::
;; LL$WRITE_FILE(handle: i16, data: *near u8, count: usize) -> isize: bytes written, as LL$READ.
LL$WRITE_FILE   proc
                mov     ah, DOS_WRITE
transfer::
                push    ebx
                movzx   ebx, word ptr [esp+8]
                mov     edx, dword ptr [esp+12]
                mov     ecx, dword ptr [esp+16]
                int     DOS_INT
                pop     ebx
                jnc     short transferred
                movzx   eax, ax
                neg     eax
transferred:
                ret
LL$WRITE_FILE   endp

;;::::::::::::::
;; LL$CLOSE(handle: handle) -> isize
LL$CLOSE        proc
                push    ebx
                movzx   ebx, word ptr [esp+8]
                mov     ah, DOS_CLOSE
                int     DOS_INT
                pop     ebx
                jc      short checked
                xor     eax, eax                ;; DOS leaves AX undefined on success
checked::
                jnc     short done
                movzx   eax, ax
                neg     eax
done:
                ret
LL$CLOSE        endp

endif

if OS_GROUPS and G_CONSOLE
;; The console's keyboard: standard input, so what DOS redirects it follows.

;;::::::::::::::
;; LL$CONSOLE_READ_KEY() -> u8: the keyboard's character, no echo (08h). From redirected
;; input it is the next byte read from the handle, or Ctrl-Z once there are none.
LL$CONSOLE_READ_KEY proc
                push    ebx
                push    ecx
                push    edx
                mov     ax, DOS_IOCTL * 256
                mov     ebx, DOS_STDIN
                int     DOS_INT
                jc      short from_handle
                test    dl, DEVICE_BIT
                jnz     short from_device
from_handle:
                push    eax                     ;; the byte lands here
                mov     ah, DOS_READ
                mov     ebx, DOS_STDIN
                mov     ecx, 1
                mov     edx, esp
                int     DOS_INT
                pop     edx
                jc      short end_of_input
                cmp     eax, 1
                jne     short end_of_input
                movzx   eax, dl
                jmp     short key_done
end_of_input:
                mov     eax, CTRL_Z
                jmp     short key_done
from_device:
                mov     ah, DOS_READ_KEY
                int     DOS_INT
                movzx   eax, al
key_done:
                pop     edx
                pop     ecx
                pop     ebx
                ret
LL$CONSOLE_READ_KEY endp

;;::::::::::::::
;; LL$CONSOLE_KEY_READY() -> bool: DOS's input status (0Bh), 0FFh or 0, as 1 or 0.
LL$CONSOLE_KEY_READY proc
                mov     ah, DOS_KEY_READY
                int     DOS_INT
                movzx   eax, al
                and     eax, 1
                ret
LL$CONSOLE_KEY_READY endp
endif

if OS_GROUPS and G_CORE
;;::::::::::::::
;; LL$EXIT(code: u8): ends the program.
LL$EXIT         proc
                mov     al, byte ptr [esp+4]
                mov     ah, DOS_EXIT
                int     DOS_INT
LL$EXIT         endp
endif

if OS_GROUPS and G_PROCESS
;; The program's PSP (the selector the extender gave ES at entry) holds the command tail (length at 80h, text from 81h) and the
;; environment's selector (at 2Ch); the environment is NUL-ended strings ended by one more NUL.
PSP_ENVIRONMENT equ     2Ch
PSP_TAIL        equ     80h

;;::::::::::::::
;; LL$COMMAND_LINE(data: *near mut u8, max: usize) -> isize: the command tail without its leading blanks, at most `max` bytes of
;; it, and the length copied.
LL$COMMAND_LINE proc
                push    ebx
                push    esi
                push    edi
                push    es
                mov     ax, LL$PSP
                mov     es, ax
                movzx   ecx, byte ptr es:[PSP_TAIL]
                mov     esi, PSP_TAIL + 1
skip_blank:
                test    ecx, ecx
                jz      short tail_copy
                cmp     byte ptr es:[esi], ' '
                jne     short tail_copy
                inc     esi
                dec     ecx
                jmp     short skip_blank
tail_copy:
                cmp     ecx, dword ptr [esp+28]
                jbe     short tail_fits
                mov     ecx, dword ptr [esp+28]
tail_fits:
                mov     edi, dword ptr [esp+24]
                mov     eax, ecx
                jecxz   short tail_done
copy_tail:
                mov     bl, es:[esi]
                mov     [edi], bl
                inc     esi
                inc     edi
                dec     ecx
                jnz     short copy_tail
tail_done:
                pop     es
                pop     edi
                pop     esi
                pop     ebx
                ret
LL$COMMAND_LINE endp

;;::::::::::::::
;; LL$ENVIRONMENT(index: usize, data: *near mut u8, max: usize) -> isize: the `index`th string of the environment (from 0), at
;; most `max` bytes of it, and its length; -1 past the last.
LL$ENVIRONMENT  proc
                push    ebx
                push    esi
                push    edi
                push    es
                mov     ax, LL$PSP
                mov     es, ax
                mov     ax, es:[PSP_ENVIRONMENT]
                mov     es, ax
                xor     esi, esi
                mov     ecx, dword ptr [esp+20]
next_string:
                cmp     byte ptr es:[esi], 0
                je      short no_string         ;; the closing NUL: past the last
                jecxz   short found
                dec     ecx
skip_string:
                mov     al, es:[esi]
                inc     esi
                test    al, al
                jnz     short skip_string
                jmp     short next_string
found:
                mov     ebx, esi
measure:
                cmp     byte ptr es:[esi], 0
                je      short measured
                inc     esi
                jmp     short measure
measured:
                mov     eax, esi
                sub     eax, ebx                ;; its length
                mov     ecx, dword ptr [esp+28]
                cmp     eax, ecx
                jbe     short string_fits
                mov     eax, ecx
string_fits:
                mov     ecx, eax
                mov     edi, dword ptr [esp+24]
                mov     esi, ebx
                jecxz   short environment_done
copy_string:
                mov     bl, es:[esi]
                mov     [edi], bl
                inc     esi
                inc     edi
                dec     ecx
                jnz     short copy_string
                jmp     short environment_done
no_string:
                mov     eax, -1
environment_done:
                pop     es
                pop     edi
                pop     esi
                pop     ebx
                ret
LL$ENVIRONMENT  endp

;;::::::::::::::
;; LL$ENVIRONMENT_READ(offset: usize, data: *near mut u8, max: usize) -> usize: the bytes from `offset`, at most `max`, and how many.
LL$ENVIRONMENT_READ proc
                push    ebx
                push    esi
                push    edi
                push    es
                mov     ax, LL$PSP
                mov     es, ax
                mov     ax, es:[PSP_ENVIRONMENT]
                mov     es, ax
                lsl     ebx, eax
                inc     ebx
                mov     esi, dword ptr [esp+20]
                sub     ebx, esi
                jbe     short read_none
                mov     ecx, dword ptr [esp+28]
                cmp     ecx, ebx
                jbe     short read_fits
                mov     ecx, ebx
read_fits:
                mov     edi, dword ptr [esp+24]
                mov     eax, ecx
                jecxz   short read_done
read_byte:
                mov     bl, es:[esi]
                mov     [edi], bl
                inc     esi
                inc     edi
                dec     ecx
                jnz     short read_byte
                jmp     short read_done
read_none:
                xor     eax, eax
read_done:
                pop     es
                pop     edi
                pop     esi
                pop     ebx
                ret
LL$ENVIRONMENT_READ endp

;;::::::::::::::
;; LL$ENVIRONMENT_WRITE(offset: usize, data: *near u8, count: usize) -> isize: `count` bytes into the block at `offset`; -1 when they do not fit.
LL$ENVIRONMENT_WRITE proc
                push    ebx
                push    esi
                push    edi
                push    es
                mov     ax, LL$PSP
                mov     es, ax
                mov     ax, es:[PSP_ENVIRONMENT]
                mov     es, ax
                lsl     ebx, eax
                inc     ebx
                mov     edi, dword ptr [esp+20]
                mov     ecx, dword ptr [esp+28]
                mov     eax, edi
                add     eax, ecx
                jc      short write_no_room
                cmp     eax, ebx
                ja      short write_no_room
                mov     esi, dword ptr [esp+24]
                jecxz   short write_ok
write_byte:
                mov     bl, [esi]
                mov     es:[edi], bl
                inc     esi
                inc     edi
                dec     ecx
                jnz     short write_byte
write_ok:
                xor     eax, eax
                jmp     short write_done
write_no_room:
                mov     eax, -1
write_done:
                pop     es
                pop     edi
                pop     esi
                pop     ebx
                ret
LL$ENVIRONMENT_WRITE endp
;;::::::::::::::
;; LL$EXECUTE(name: *near char, tail: *near char) -> isize: DOS's load-and-execute (4B00h) as the extender's real-mode call
;; (DPMI 0300h) runs it, over a block of DOS memory holding what DOS reads: the name, the tail, the parameter block (the
;; caller's environment, the tail, two blank FCBs). DOS/32A's own 4Bh takes a block layout it does not document.
EXEC_NAME       equ     0                       ;; at most 128 bytes, NUL-ended
EXEC_TAIL       equ     128                     ;; length byte, text, CR: at most 128 bytes
EXEC_BLOCK      equ     256                     ;; 14 bytes
EXEC_FCB        equ     272                     ;; two of 16 bytes
EXEC_PARAS      equ     19                      ;; 304 bytes
EXEC_CALL       equ     -16-52                  ;; from EBP
EXEC_VECTORS    equ     EXEC_CALL-12            ;; from EBP: INT 22h, 23h, 24h as offset and segment words
LL$EXECUTE      proc
                push    ebp
                mov     ebp, esp
                push    ebx
                push    esi
                push    edi
                push    es
                sub     esp, 52 + 12            ;; the real-mode call structure (EXEC_CALL), below it the real-mode terminate, break and critical-error vectors (EXEC_VECTORS)
                mov     ax, DOS_DPMI_DOS_ALLOC
                mov     bx, EXEC_PARAS
                int     DOS_DPMI_INT
                jc      exec_no_memory
                push    eax                     ;; the real-mode segment
                push    edx                     ;; the selector
                mov     es, dx
                xor     edi, edi
                mov     ecx, EXEC_PARAS * 4
                xor     eax, eax
                rep     stosd
                mov     esi, dword ptr [ebp+8]
                mov     edi, EXEC_NAME
                mov     ecx, 127
exec_copy_name:
                mov     al, [esi]
                test    al, al
                jz      short exec_name_done
                mov     es:[edi], al
                inc     esi
                inc     edi
                dec     ecx
                jnz     short exec_copy_name
exec_name_done:
                mov     esi, dword ptr [ebp+12]
                movzx   ecx, byte ptr [esi]
                add     ecx, 2
                cmp     ecx, 128
                jbe     short exec_tail_fits
                mov     ecx, 128
exec_tail_fits:
                mov     edi, EXEC_TAIL
                rep     movsb
                mov     eax, dword ptr [esp+4]  ;; the segment
                mov     word ptr es:[EXEC_BLOCK+4], ax
                mov     word ptr es:[EXEC_BLOCK+2], EXEC_TAIL
                mov     word ptr es:[EXEC_BLOCK+8], ax
                mov     word ptr es:[EXEC_BLOCK+6], EXEC_FCB
                mov     word ptr es:[EXEC_BLOCK+12], ax
                mov     word ptr es:[EXEC_BLOCK+10], EXEC_FCB + 16
                ;; the call structure, on the stack, with the registers 4Bh reads
                lea     edi, [ebp+EXEC_CALL]
                push    es
                push    ss
                pop     es
                mov     ecx, 13
                xor     eax, eax
                rep     stosd
                pop     es
                mov     eax, dword ptr [esp+4]
                mov     word ptr [ebp+EXEC_CALL+34], ax      ;; ES
                mov     word ptr [ebp+EXEC_CALL+36], ax      ;; DS
                mov     word ptr [ebp+EXEC_CALL+20], EXEC_NAME   ;; DX
                mov     word ptr [ebp+EXEC_CALL+16], EXEC_BLOCK  ;; BX
                mov     word ptr [ebp+EXEC_CALL+28], DOS_EXEC * 256 ;; AX
                mov     bl, 22h
exec_save_vector:
                mov     ax, DOS_DPMI_GET_VECTOR
                int     DOS_DPMI_INT
                movzx   eax, bl
                mov     word ptr [ebp+EXEC_VECTORS+eax*4-22h*4], dx
                mov     word ptr [ebp+EXEC_VECTORS+eax*4-22h*4+2], cx
                inc     bl
                cmp     bl, 25h
                jb      short exec_save_vector
                push    es
                push    ss
                pop     es
                lea     edi, [ebp+EXEC_CALL]
                mov     ax, DOS_DPMI_SIMULATE
                mov     bl, DOS_INT
                xor     bh, bh
                xor     ecx, ecx
                int     DOS_DPMI_INT
                pop     es
                pushfd
                mov     bl, 22h
exec_restore_vector:
                movzx   eax, bl
                mov     dx, word ptr [ebp+EXEC_VECTORS+eax*4-22h*4]
                mov     cx, word ptr [ebp+EXEC_VECTORS+eax*4-22h*4+2]
                mov     ax, DOS_DPMI_SET_VECTOR
                int     DOS_DPMI_INT
                inc     bl
                cmp     bl, 25h
                jb      short exec_restore_vector
                popfd
                jc      short exec_call_failed
                movzx   eax, word ptr [ebp+EXEC_CALL+28]
                test    byte ptr [ebp+EXEC_CALL+32], 1       ;; the flags' carry: DOS's error code is in AX
                jnz     short exec_failed
                xor     eax, eax
                jmp     short exec_free
exec_failed:
                neg     eax
                jmp     short exec_free
exec_call_failed:
                mov     eax, -1
exec_free:
                mov     ebx, eax
                pop     edx                     ;; the selector
                add     esp, 4
                mov     ax, DOS_DPMI_DOS_FREE
                int     DOS_DPMI_INT
                mov     eax, ebx
                jmp     exec_done
exec_no_memory:
                mov     eax, -8
exec_done:
                add     esp, 52 + 12
                pop     es
                pop     edi
                pop     esi
                pop     ebx
                pop     ebp
                ret
LL$EXECUTE      endp
endif

if OS_GROUPS and G_FILES
;;::::::::::::::
;; The file-name calls: DOS function in AH, the name at DS:EDX (the new name at ES:EDI for a rename, EDI kept); a
;; carry is the error code in AX, negated; 0 on success (DOS leaves AX undefined).
LL$DELETE       proc
                mov     edx, dword ptr [esp+4]
                mov     ah, DOS_DELETE
                jmp     short file_name_call
LL$DELETE       endp

LL$RENAME       proc
                push    edi
                mov     edx, dword ptr [esp+8]
                mov     edi, dword ptr [esp+12]
                mov     ah, DOS_RENAME
                int     DOS_INT
                pop     edi
                jmp     short file_name_checked
LL$RENAME       endp

LL$MKDIR        proc
                mov     edx, dword ptr [esp+4]
                mov     ah, DOS_MKDIR
                jmp     short file_name_call
LL$MKDIR        endp

LL$CHDIR        proc
                mov     edx, dword ptr [esp+4]
                mov     ah, DOS_CHDIR
                jmp     short file_name_call
LL$CHDIR        endp

LL$RMDIR        proc
                mov     edx, dword ptr [esp+4]
                mov     ah, DOS_RMDIR
file_name_call::
                int     DOS_INT
file_name_checked::
                jc      short file_name_failed
                xor     eax, eax
                ret
file_name_failed:
                movzx   eax, ax
                neg     eax
                ret
LL$RMDIR        endp

;;::::::::::::::
;; LL$FIND_FIRST(pattern: *near char, attrs: usize, entry: *near mut u8) -> isize: the DTA is the caller's `entry`.
LL$FIND_FIRST   proc
                mov     edx, dword ptr [esp+12]
                mov     ah, DOS_SET_DTA
                int     DOS_INT
                mov     ecx, dword ptr [esp+8]
                mov     edx, dword ptr [esp+4]
                mov     ah, DOS_FIND_FIRST
                jmp     short file_name_call
LL$FIND_FIRST   endp

;;::::::::::::::
;; LL$FIND_NEXT(entry: *near mut u8) -> isize
LL$FIND_NEXT    proc
                mov     edx, dword ptr [esp+4]
                mov     ah, DOS_SET_DTA
                int     DOS_INT
                mov     ah, DOS_FIND_NEXT
                jmp     short file_name_call
LL$FIND_NEXT    endp

;;::::::::::::::
;; LL$FREE_BYTES(drive: usize) -> i32: sectors per cluster x free clusters x bytes per sector; AX = FFFFh from
;; DOS is -15.
LL$FREE_BYTES   proc
                push    ebx
                mov     edx, dword ptr [esp+8]
                mov     ah, DOS_FREE_SPACE
                int     DOS_INT
                cmp     ax, 0FFFFh
                je      short free_failed
                movzx   eax, ax
                movzx   ecx, cx
                movzx   ebx, bx
                imul    eax, ecx
                imul    eax, ebx
                pop     ebx
                ret
free_failed:
                mov     eax, -15
                pop     ebx
                ret
LL$FREE_BYTES   endp

;;::::::::::::::
;; LL$CURRENT_DIRECTORY(drive: usize, data: *near mut char) -> isize
LL$CURRENT_DIRECTORY proc
                push    esi
                mov     edx, dword ptr [esp+8]
                mov     esi, dword ptr [esp+12]
                mov     ah, DOS_GET_DIRECTORY
                int     DOS_INT
                pop     esi
                jc      short directory_failed
                xor     eax, eax
                ret
directory_failed:
                movzx   eax, ax
                neg     eax
                ret
LL$CURRENT_DIRECTORY endp

;;::::::::::::::
;; LL$CURRENT_DRIVE() -> isize
LL$CURRENT_DRIVE proc
                mov     ah, DOS_GET_DRIVE
                int     DOS_INT
                movzx   eax, al
                ret
LL$CURRENT_DRIVE endp


endif

end
