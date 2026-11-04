# Adding a backend to gcc: TOY16 in 440 lines

TOY16 is an imaginary 16-bit CPU. Every listing below was built against gcc master (17.0.0) and its
output is real. The port lives in a local gcc fork (GPLv3, branch `spike/toy-tutorial`), not in this repository.

```
 C -> GIMPLE -> expand -> RTL -> combine -> IRA/LRA -> prologue/epilogue -> final -> .s
                  |        |        |           |              |             |
 toy.md:    define_expand  define_insn   (constraints)   expanders      output templates
 toy.cc:    hooks: ABI, address legality, print_operand        toy.h: macros: registers, classes, frame, asm syntax
```

Files: `config.sub` + `gcc/config.gcc` (register the target), `gcc/config/toy/{toy.h, toy.cc, toy.md, toy-protos.h, toy.opt, toy.opt.urls}`.
At build time `genemit/genrecog/genoutput/genextract` turn `toy.md` into C; `toy.cc` ends with
`struct gcc_target targetm = TARGET_INITIALIZER;`.

## The CPU

```
 r0..r6 general, r7 = sp, 16 bit, byte addressed.  Args r0..r3, result r0, every register clobbered by a call.
 mov ldi ld st | add sub and or xor addi neg not | cmp ra,rb ; b<cc> L | jmp call callr ret
```

Compare and branch are one gcc insn, so flags are never live between insns.

## Build

`config.sub`: add `toy` to the CPU list. `gcc/config.gcc`:

```
toy-*-*)
	target_has_targetm_common=no
	target_has_targetcm=no
	;;
```
```
configure --target=toy-unknown-elf --enable-languages=c --without-headers --disable-bootstrap \
  --disable-multilib --disable-libsanitizer --disable-libgomp --disable-lto --disable-plugin \
  --with-as=/usr/bin/true --with-ld=/usr/bin/true
make -j8 all-gcc                      # builds cc1 only; the assembler is external
```

## Run

```
cc1 -quiet -O2 x.c -o x.s             # add -dp: every line is annotated with its pattern (addhi3/0)
cc1 -fdump-rtl-expand x.c             # RTL after expand; -fdump-rtl-all for every pass
spike-tests/run.sh cc1                # every test at -O0 -O1 -O2 -Os -O3, -O2 diffed against a golden .s
```

## What to write, in order

1. **Registers and frame (`toy.h`).** `FIRST_PSEUDO_REGISTER 10` (r0..r6, sp, and two soft registers `argp`, `framep`),
   `FIXED_REGISTERS`, `CALL_USED_REGISTERS`, `enum reg_class`, `REG_CLASS_CONTENTS` (bit n = hard reg n),
   `ELIMINABLE_REGS` (both soft registers become `sp + constant`), plus sizes, `Pmode` and assembler syntax macros.
2. **Hooks (`toy.cc`).** `TARGET_LEGITIMATE_ADDRESS_P` (`[reg]`, `[reg+const]`), `TARGET_PRINT_OPERAND(_ADDRESS)`,
   `TARGET_FUNCTION_ARG(_ADVANCE)`, `TARGET_FUNCTION_VALUE`, `TARGET_LIBCALL_VALUE`; hooks are set with `#define TARGET_X toy_x`
   before `targetm`.
3. **Patterns (`toy.md`).** A move, `addhi3`, `jump`, `returner`, `prologue`/`epilogue` expanders, then the rest:

```
(define_insn "addhi3"
  [(set (match_operand:HI 0 "register_operand" "=r,r")           ; predicate, constraint per alternative
        (plus:HI (match_operand:HI 1 "register_operand" "%0,0")  ; % commutative, 0 = same register as operand 0
                 (match_operand:HI 2 "nonmemory_operand" "r,i")))]
  "" "@ add %0, %2
       addi %0, %2")                                              ; one template per alternative

(define_insn "cbranchhi4"                                         ; the name is how expand finds it
  [(set (pc) (if_then_else
              (match_operator 0 "ordered_comparison_operator"
               [(match_operand:HI 1 "register_operand" "r") (match_operand:HI 2 "register_operand" "r")])
              (label_ref (match_operand 3 "" "")) (pc)))]
  "" "cmp %1, %2\;b%C0 %l3")                                      ; %C0: our print_operand code, prints eq ne lt ... gtu
```

Anything the CPU lacks (multiply) becomes a libcall (`__mulhi3`) that you supply. Result for `sum` and a call:

```
_sum:  mov r1, r0 ; ldi r0, 0 ; cmp r1, r0 ; ble .L4 ; mov r2, r0
.L6:   add r0, r1 ; addi r1, -1 ; cmp r1, r2 ; bne .L6
.L4:   ret
_twice: mov r1, r0 ; call _add ; addi r0, 1 ; ret
```

## Three gotchas

**1. The frame is locals plus the outgoing-argument area.** With `ACCUMULATE_OUTGOING_ARGS 1`, arguments that do not fit in
registers are stored at `sp+0..` before the call. If the prologue and `INITIAL_ELIMINATION_OFFSET` use only
`get_frame_size ()`, that slot is also a local: the caller stores `&e` over `e` itself. Use
`get_frame_size () + crtl->outgoing_args_size` everywhere (prologue, epilogue, elimination). The 6502 port had this bug; it looked
like a miscompile of 32-bit arithmetic that only appeared under some register pressure, and the `-O1` add/sub failures went away with it.
Test: a function with a third argument after two 32-bit ones, and a local whose address it takes.

**2. `-O0` asks for a frame pointer.** `-fno-omit-frame-pointer` is the default at `-O0`. A CPU with no frame register leaves the
soft `framep` in the code and reload dies with `insn does not satisfy its constraints`. Either make a real register the
hard frame pointer, or set `flag_omit_frame_pointer = 1` in `TARGET_OPTION_OVERRIDE`, as TOY16 does.
Test at all five `-O` levels, not just `-O2`.

**3. gcc will not build until the required pieces exist, and says so one error at a time.**

| error | add |
|---|---|
| `zero-size array 'extract_paths'` (`insn-extract.cc`) | a real insn: an md with only `nop` is not enough |
| `ASM_OUTPUT_COMMON` / `ASM_OUTPUT_LOCAL` not declared | `ASM_OUTPUT_ALIGNED_COMMON` / `_LOCAL` |
| `merge_decl_attributes` not declared (`target-hooks-def.h`) | the full include list of a working port, `builtins.h` included |
| ICE in `default_function_value` | `TARGET_FUNCTION_VALUE`, `TARGET_LIBCALL_VALUE` |
| ICE in `invalid_rtx` (`insn-target-def.h`) | `jump` (even `return` uses `emit_jump`), then `call`, `prologue`, `epilogue` |
| `unrecognizable insn` | no pattern for this RTL: add one or a libcall |
| `unable to find a register to spill` | too few registers for the mode in use |

Also set `NO_FUNCTION_CSE`: otherwise two calls to the same symbol load its address into a register once and call through it.

## Not done

`char` and `long` (no QImode/SImode patterns), shifts, stack arguments, costs, peepholes, and any run on a simulator:
the listings above are checked by reading them.
