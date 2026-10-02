# Open Watcom C front end: what it gathers and emits

Source: Open Watcom v2 commit `703e1ae2f`, the commit `toolchain/owshim/build.sh` builds. Paths are relative to `bld/`; `cc/c/` is the front end and `cg/` the code generator. Every cite is `file:function:line`, read from that tree. A row marked *not traced* was read only as far as it says.

Aim: the facts Open Watcom's front end gives its back end, so that every llrm frontend can state the same facts in HIR (rule 7).

## Headlines

- Open Watcom's front end gives its back end few language promises. The ones the back end uses to optimise harder: call class (`NO_MEMORY_READ/CHANGED`, `ABORTS`, `NORETURN`), `FE_CONSTANT` and read-only segments, `FE_VOLATILE`, exact aggregate sizes, the unroll count, signedness in the type (§1, "Facts that enable optimisation").
- It sends no `restrict`, no pointee `const`, no address-taken for `&x`, no purity beyond `#pragma aux nomemory`, no ranges. `FE_NOALIAS` and `CG_SYM_CONSTANT` exist in the interface and the C front end never sets them.
- Of what it does send, `llrm-c` drops the call-class bits `noreturn`, `aborts` and `nomemory`, the `__unaligned` mark, the option switches (`-oa`, `-ol`, `-ot`), and `modify` register lists; it refuses bit fields, `#pragma aux` register parameters and `__based` (§6).
- Quick BASIC drops `published`, so a BYREF polling loop compiles to an infinite loop on the default route (§7). Nib sends no pointer promises.
- The proposal (§8): one way to state a fact (`facts.state(subject, fact)`, one declaration table, one lowering, one typed reader in `llrm-mir`); facts are droppable promises, meaning stays in IR fields (`published` is retired in favour of `volatile`). Carriers nothing reads today (`range`, `nonnull`, unroll, `inline`) need a reader before their variant lands.

## 1. Interface inventory

### CG expression calls

| call | carries | emitted when | cite |
|---|---|---|---|
| CGInteger(val32, type) | 32-bit constant, cg_type | int/short/char/long/ulong constant leaf (`OPR_PUSHINT`) | cgen.c:PushConstant:761 |
| CGInteger (pointer const) | constant with pointer cg_type chosen from node flags | `TYP_POINTER` constant leaf | cgen.c:PushConstant:764 |
| CGInteger (helpers) | small consts: 0 (`TY_UNSIGNED`) for null seg; struct field offset (`TY_UNSIGNED`); element size (`TY_INTEGER`); 16 shift; try scope | null `PushSymSeg`; try-block field addr; array index scaling; `OPR_CONVERT_SEG`; SEH | cgen.c:PushSymSeg:366, TryFieldAddr:252, IndexOperator:718, EmitNodes:1477, SetTryScope:409 |
| CGInt64(val64, type) | 64-bit const | `TYP_LONG64`/`TYP_ULONG64` leaf | cgen.c:PushConstant:768 |
| CGFloat(string, type) | decimal text (from `flt->string`, or `ftoa` of binary form) + cg_type; no binary value | float/double/long double and imaginary consts | cgen.c:PushConstant:782 |
| CGFEName(sym, type) | FE symbol handle + its cg_type = address of the symbol (an lvalue name, not the value) | every `OPR_PUSHSYM`, `OPR_PUSHADDR`, function-call target, code-seg-of-CS | cgen.c:PushSym:582, PushSymAddr:613, InitFuncCall:940, PushSymSeg:369,377 |
| CGFEName(TrySym, refno) | try-block struct symbol with the try-block refno as its type | SEH field access | cgen.c:TryFieldAddr:251 |
| CGBackName(back, type) | back-end label handle (string literal, SEH table) as address | `OPR_PUSHSTRING` (type `TY_UINT_1`); SEH scope table (type `TY_POINTER`) | cgen.c:PushString:840, SetTryTable:400 |
| CGTemp(type) / CGTempName(temp, type) | back-end temporary; name = its address | return-value variable (`SYM_FUNC_RETURN_VAR`); saved `SS` (`TY_UINT_2`); `OPR_DUPE` temp (`TY_LONG_POINTER`) | cgen.c:CDoAutoDecl:1120, PushSym:580, PushSymSeg:372,374, EmitNodes:1450,1451,1453 |
| CGUnary(O_POINTS, name, type) | dereference: turns an address name into the value of `type` | rvalue of var (`PushSym`), of `.`/`->`/`[]` (`PushRValue`), of `*p` (`DoIndirection`), call result, return var, SEH fields | cgen.c:PushSym:592, PushRValue:638, DoIndirection:911, EmitNodes:1523, EndFunction:294 |
| CGUnary(op, name, type) | unary arith | `OPR_NEG`->O_UMINUS, `OPR_COM`->O_COMPLEMENT, `OPR_CONVERT`->O_CONVERT | cgen.c:EmitNodes:1371,1382,1462 |
| CGUnary(O_MATHFUNC, name, TY_DOUBLE) | math op from `node->u1.mathfunc`, always `TY_DOUBLE` | call to a recognised libm function when `-od` off and extensions on (`OPR_MATHFUNC`) | cgen.c:EmitNodes:1482; built at cexpr.c:2295 |
| CGUnary(O_PTR_TO_NATIVE / O_PTR_TO_FOREIGN, name, TY_POINTER) | __far16 <-> flat pointer conversion | 386 only: `->`, `[]`, `*` through __far16 pointer; `OPR_CONVERT_PTR` between far16 and flat | cgen.c:ArrowOperator:683, IndexOperator:705, DoIndirection:900, ConvertPointer:920,922 |
| CGUnary(O_STACK_ALLOC, size, TY_POINTER) | alloca size | RISC targets only (`OPR_ALLOCA`) | cgen.c:EmitNodes:1590 |
| CGUnary(O_VA_START, valist, TY_POINTER) | address of va_list | PPC only | cgen.c:GenVaStart:527 |
| CGBinary(op, l, r, type) | binary arith; type = result type | `+ - * / % \| & ^ >> <<`; pointer `+`/`-` uses the pointer cg_type | cgen.c:DoAddSub:734, EmitNodes:1338 |
| CGBinary(O_PLUS, base, off, ptrtype) | address = base + byte offset; type from `DataPointerType` | `.` and `->` (field offset), `[]` (scaled index) | cgen.c:DotOperator:659, IndexOperator:720 |
| CGBinary(O_TIMES, idx, CGInteger(elemsize), TY_INTEGER or TY_INT_4) | index scaled in the FE | `[]` when elem size != 1; `TY_INT_4` on 8086 for huge / huge-data | cgen.c:IndexOperator:717 (type at 710-714) |
| CGBinary(O_COMMA, l, r, type) | comma; struct result typed as pointer | `OPR_COMMA` | cgen.c:EmitNodes:1348 |
| CGBinary(O_CONVERT, off, seg, TY_LONG_POINTER) | two-operand convert = build far pointer from offset and segment | `OPR_FARPTR` (`seg :> off`, also from `__based`) | cgen.c:EmitNodes:1510 |
| CGBinary(mathop, l, r, TY_DOUBLE) | 2-arg math op (O_POW, O_ATAN2, O_FMOD) | `OPR_MATHFUNC2` | cgen.c:EmitNodes:1487; built cexpr.c:2299 |
| CGCompare(op, l, r, cmptype) | O_EQ..O_GE; type = operand compare type (not result) | `OPR_CMP`; op from `CC2CGOp[node->u1.cc]` | cgen.c:EmitNodes:1377; EndFinally:422 |
| CGFlow(O_FLOW_NOT, e, NULL) | logical not | `OPR_NOT` | cgen.c:EmitNodes:1387 |
| CGFlow(O_FLOW_AND/OR, l, r) | short-circuit && / \|\| | `OPR_AND_AND`, `OPR_OR_OR` | cgen.c:EmitNodes:1411 |
| CGChoose(test, t, f, type) | `?:`; struct result typed as pointer then `PushRValue` | `OPR_QUESTION` | cgen.c:EmitNodes:1398 |
| CGBitMask(addr, start, width, type) | bit field: address of the storage unit, start bit (byte), width (byte), storage type | `.`/`->` whose result type is `TYP_FIELD`/`TYP_UFIELD`; `_Bool` field forced to width 1 | cgen.c:DotOperator:667 (width 661-666) |
| CGAttr(name, CG_SYM_UNALIGNED) | unaligned access marker on an address | node carries `OPFLAG_UNALIGNED` (from `__unaligned` qualifier, `ctype.c:243`, `cexpr.c:196`) on var, `.`/`->`, `[]`, `*` | cgen.c:PushSym:585, DotOperator:670, IndexOperator:722, DoIndirection:905 |
| CGVolatile(name) | marks an address name volatile; applied BEFORE `O_POINTS` | node has `OPFLAG_VOLATILE`; also assign/compound/post-inc lvalue; var used in `#pragma aux`; SEH fields | cgen.c:PushSym:588, PushRValue:634, DoIndirection:909, EmitNodes:1312,1364,1422, PushSymAddr:621, TryFieldAddr:253 |
| CGVolatile (forced float) | same call, forced on float/double/long double even without qualifier | when `-op` (precise FP, `CompFlags.op_switch_used`) | cgen.c:ForceVolatileFloat:349-351 (used PushSym:590, PushRValue:636) |
| CGEval(name) | force evaluation order | only `OPR_DUPE` (based `__self` pointer: temp used on both sides) | cgen.c:EmitNodes:1454 |
| CGVarargsBasePtr(type) | base of vararg area | AXP and MIPS `va_start` only | cgen.c:GenVaStart:529,534 |

### CG assignment calls

| call | carries | emitted when | cite |
|---|---|---|---|
| CGAssign(lhs_addr, rhs, type) | store; type = result/lhs type; returns value | `=` on non-struct/union/complex | cgen.c:EmitNodes:1318 |
| CGLVAssign(lhs_addr, rhs, structtype) | struct/union/complex copy; returns an address, then `PushRValue` (so `O_POINTS` is added only if the node is an rvalue). No size argument: size is in the refno type | `=` when `IsStruct(result_type)` (struct, union, float/double/long double complex) | cgen.c:EmitNodes:1315-1316; IsStruct:972-984 |
| CGAssign(temp, expr, type) | store return value into return temp | `return expr;` | cgen.c:ReturnExpression:319 |
| CGAssign(field-of-try, ...) | SEH bookkeeping (scope table, scope index) | SEH functions | cgen.c:SetTryTable:401, SetTryScope:409 |
| CGAssign(dupe temp, ptr, TY_LONG_POINTER) | temp store | `OPR_DUPE` | cgen.c:EmitNodes:1452 |
| CGPreGets(op, lhs_addr, rhs, type) | `op=` compound; op = O_PLUS..O_LSHIFT mapped from `OPR_*_EQUAL`; type = result type | `+= -= *= /= %= ^= &= \|= >>= <<=` | cgen.c:EmitNodes:1366 |
| CGPostGets(op, lhs_addr, amount, type) | `x++`/`x--` as O_PLUS/O_MINUS with amount operand (pointer amount already scaled by FE) | `OPR_POSTINC`, `OPR_POSTDEC` | cgen.c:EmitNodes:1424 |
| CGLVAssign/CGAssign in va_start | pointer + offset stores | AXP / MIPS `va_start` | cgen.c:GenVaStart:530,532,536,538 |

Not emitted: `CGLVPreGets`. Pre-increment: no separate call site in cgen.c (`OPR_POSTINC/POSTDEC` only); how the FE builds `++x` was not traced. Bit-field stores are `CGAssign` to a `CGBitMask` name.

### CG statements and flow

| call | carries | emitted when | cite |
|---|---|---|---|
| CGDone(name) | discard value of a finished expression | any value still on the stack at the end of a statement thread; calls whose value is unused; SEH helper calls | cgen.c:EmitNodes:1604; CallTryRtn:239; ReturnExpression:319; EndFinally:426; GenVaStart:540 |
| CGTrash | - | never called (stub only) | cgstub.c:71 |
| CGControl(O_LABEL, NULL, lbl) | define label | `OPR_LABEL`; `OPR_CASE` only if `case_info->gen_label` | cgen.c:EmitNodes:1534,1538; EndFinally:427 |
| CGControl(O_GOTO, NULL, lbl) | unconditional jump | `OPR_JUMP` | cgen.c:EmitNodes:1542 |
| CGControl(O_IF_TRUE / O_IF_FALSE, cond, lbl) | conditional jump on an expression | `OPR_JUMPTRUE`, `OPR_JUMPFALSE`; SEH `_finally` test | cgen.c:EmitNodes:1546,1550; EndFinally:423 |
| CGSelInit / CGSelCase(sel, lbl, val64) / CGSelOther(sel, lbl) / CGSelect(sel, expr) | switch: one `CGSelCase` per case with value and label, one default, then select on the expression. Case order = `case_list` order. No range, no table-vs-search hint (`CGSelectRestricted` unused) | `OPR_SWITCH` | cgen.c:DoSwitch:963,966,968,969 |
| CGBigLabel(back) | non-local label (SEH `__except`/`__finally` handler entry) | `OPR_EXCEPT`, `OPR_FINALLY` (32-bit targets, `__SEH__`, cconst.h:41) | cgen.c:EmitNodes:1562 |
| CGReturn(expr_or_NULL, type) | only call site is function end (one return per function); `return e;` earlier assigned to the return temp. Void: `(NULL, type of return)`. Else value = `O_POINTS` of the return temp; type passed through `ReturnType` (promotes small ints via `FEParmType` when `CompFlags.returns_promoted`) | `OPR_FUNCEND` | cgen.c:EndFunction:289,295; ReturnType:205-211 |
| BENewLabel / BEFiniLabel | one handle per FE label index at function start / freed at end | `OPR_LABELCOUNT`; `OPR_FUNCEND`; SEH end-finally | cgen.c:DefineLabels:151; EndFunction:276; EndFinally:419,428 |
| Never called | CGBigGoto, CGWarp, CG3WayControl, CGIndex, CGDuplicate, CGType, CGCallback, CGPatchNode, CGSelRange, CGSelectRestricted, CGLVPreGets, CGFlow(O_FLOW_OUT), BEPatch*, BEAliasType | grep over `bld/cc/c/*.c` found none outside `cgstub.c` | - |

### CG functions and calls

| call | carries | emitted when | cite |
|---|---|---|---|
| CGProcDecl(sym, rettype) | function symbol + return cg_type (promoted via `ReturnType`). Call class, register convention, etc. are NOT arguments: back end pulls them with `FEAttr`/`FEAuxInfo` | `OPR_FUNCTION`; sets `CGSW_X86_FLOATING_SS` from `FLAG_FARSS` before | cgen.c:DoFuncDefn:1198 (1189-1195) |
| CGParmDecl(sym, type) | parameter symbol + cg_type (struct -> refno, array -> refno via `CGenType`) | once per parameter; `...` terminates the list; reversed order when aux `FECALL_GEN_REVERSE_PARMS` | cgen.c:CDoParmDecl:1151; DoFuncDefn:1236-1246 |
| CGLastParm() | end of parameter list | after parms | cgen.c:DoFuncDefn:1252 |
| CGAutoDecl(sym, type) | local variable + cg_type | per local of a block; static locals are emitted as data instead (no CGAutoDecl); SEH try struct with refno type | cgen.c:CDoAutoDecl:1123; DoFuncDefn:1256 |
| CGInitCall(target, rettype, sym) | target name (`CGFEName` for direct, expression for indirect), return cg_type, callee symbol (for indirect: the function-pointer variable's symbol, so the back end can read its aux info) | `OPR_FUNCNAME` (direct) / `OPR_CALL_INDIRECT` | cgen.c:InitFuncCall:943, InitIndFuncCall:954; CallTryRtn:237; EndFinally:425; CallTryUnwind:445 |
| CGAddParm(call, expr, type) | argument in source order, cg_type = node result type (struct -> refno, so struct by value is an argument of a struct type) | `OPR_PARM`; SEH helpers | cgen.c:EmitNodes:1530; CallTryRtn:238; CallTryUnwind:447 |
| CGCall(call) | complete call; result optionally wrapped in `O_POINTS` of the result type | `OPR_CALL` (rvalue flag decides `O_POINTS`) | cgen.c:EmitNodes:1521-1523 |
| inline calls | not a CG call: CG asks FE via `FEGenProc`, FE recursively emits callee body | call class has `FECALL_GEN_MAKE_CALL_INLINE` (`IsInLineFunc`: `FUNC_OK_TO_INLINE`, not in use, depth < `MAX_INLINE_DEPTH`) | cinfo.c:FEGenProc:274; cfeinfo.c:getCallClass:514; cgen.c:DoInLineFunction:1664-1689 |
| return type of struct | struct returns are plain: the return temp has the struct refno type, `CGAssign`'d then `CGReturn`'d as `O_POINTS` of that type | struct-returning function | cgen.c:ReturnExpression:317-319; EndFunction:292-295 |

### Types

#### cg_type codes the C front end uses (map `cdatatyp.h:41-73`, `CGDataType[]` cgen.c:97-101)

| C type | cg_type | note |
|---|---|---|
| `_Bool` | TY_UINT_1 | |
| `char` (signed) / `unsigned char` | TY_INT_1 / TY_UINT_1 | plain char resolved earlier (`TYP_PLAIN_CHAR` -> TY_INTEGER in table, never reaches CG normally) |
| `short` / `unsigned short` | TY_INT_2 / TY_UINT_2 | |
| `int` / `unsigned` | TY_INTEGER / TY_UNSIGNED | target-width; not TY_INT_4 |
| `long` / `unsigned long` | TY_INT_4 / TY_UINT_4 | |
| `__int64` / `unsigned __int64` | TY_INT_8 / TY_UINT_8 | |
| `float`, `double`, `long double` | TY_SINGLE, TY_DOUBLE, TY_LONG_DOUBLE | |
| `_Imaginary` types | same as real counterpart | imaginary identity lost at CG |
| `void` (value), `enum`, typedef, `...`, wchar | TY_INTEGER | enum takes its underlying type (`CGenType` 2025-2027) |
| data pointer | TY_POINTER / TY_NEAR_POINTER / TY_LONG_POINTER / TY_HUGE_POINTER | `PtrType` by `__near/__far/__huge` modifiers (x86); RISC always TY_POINTER; `__based` becomes TY_LONG_POINTER via FE expansion |
| code pointer / function | TY_CODE_PTR / TY_NEAR_CODE_PTR / TY_LONG_CODE_PTR | `CodePtrType` by `FLAG_FAR/NEAR` |
| struct / union / complex / array | refno >= TY_FIRST_FREE | created on first use with `BEDefType(refno, align, size)` |
| bit field | storage type of `field_type` | width/start go in `CGBitMask` |
| function (as type) | TY_DEFAULT | |
| `TY_UNKNOWN`, `TY_BOOLEAN`, `TY_PROC_PARM`, `TY_VA_LIST`, `TY_NEAR/LONG/HUGE_INTEGER`, `TY_HUGE_CODE_PTR` | never used by cc | grep of cc found none |

Type source: cgen.c:CGenType:1975-2032, PtrType:2035-2058, CodePtrType:544-563, DataPointerType:322-345.

| call | carries | emitted when | cite |
|---|---|---|---|
| BEDefType(refno, align, size) | opaque aggregate: alignment and byte size only; no field list, no member types | first `CGenType` of a struct/union/complex tag (refno cached in `tag->refno`), of an array (cached in `array->refno`); try-block struct (align 1, 28 bytes) | cgen.c:CGenType:1995,1999,2009; DoCompile:1945 |
| fresh refno per use | struct with a trailing zero-length array (`typ->object != NULL`) gets a new refno every call, not cached | flexible array member structs | cgen.c:CGenType:1989-1995 |
| array refno | size from `SizeOfArg`; element type not given | array declared/used | cgen.c:CGenType:2005-2012 |
| BETypeLength(TY_INTEGER) | FE asks CG for int size (segment alignment) | `SetSegs` | cinfo.c:SetSegs:627 |
| BEAliasType | never called | - | - |

#### cg_sym_attr (`cg.h:160-164`)

| value | emitted? | cite |
|---|---|---|
| CG_SYM_UNALIGNED | yes, via CGAttr on an address name | cgen.c:585,670,722,905 |
| CG_SYM_CONSTANT | never | grep found none |
| CG_SYM_VOLATILE | never (volatile goes through `CGVolatile`) | grep found none |

### Operators

| group | ops | used? | cite |
|---|---|---|---|
| binary | O_PLUS, O_MINUS, O_TIMES, O_DIV, O_MOD, O_AND, O_OR, O_XOR, O_RSHIFT, O_LSHIFT | yes (`CGOperator[]` from `copcodes.h:34-63`) | cgen.c:1338,734 |
| binary math | O_POW, O_ATAN2, O_FMOD | yes, TY_DOUBLE, only for recognised libm calls | cgen.c:1487; cmathfun.h:12-14 lines |
| unary | O_UMINUS, O_COMPLEMENT | yes | cgen.c:1371,1382 |
| unary math | O_LOG, O_COS, O_SIN, O_TAN, O_SQRT, O_FABS, O_ACOS, O_ASIN, O_ATAN, O_COSH, O_EXP, O_LOG10, O_SINH, O_TANH | yes, Intel builds only (`cmathfun.h` `#if _INTEL_CPU`) | cgen.c:1482 |
| conversion | O_CONVERT (1 operand: value convert to cg_type; 2 operands + TY_LONG_POINTER: build far pointer), O_PTR_TO_NATIVE, O_PTR_TO_FOREIGN | yes | cgen.c:1462,1474,1475,1510,683,900,920,922 |
| deref | O_POINTS | yes, the only way a value is read from memory | cgen.c:592,638,911 |
| compare | O_EQ, O_NE, O_GT, O_LE, O_LT, O_GE | yes | cgen.c:1377 (`copcond.h:32-37`) |
| flow | O_FLOW_AND, O_FLOW_OR, O_FLOW_NOT | yes | cgen.c:1387,1411 |
| flow | O_FLOW_OUT | no | - |
| assign | O_GETS | not as an argument; `CGAssign` implies it | - |
| misc | O_COMMA (via CGBinary) | yes | cgen.c:1348 |
| control | O_LABEL, O_GOTO, O_IF_TRUE, O_IF_FALSE | yes | cgen.c:1534,1542,1546,1550 |
| control | O_BIG_GOTO, O_INVOKE_LABEL, O_BIG_LABEL, O_LABEL_RETURN | no (`CGBigLabel` called instead of passing O_BIG_LABEL) | cgen.c:1562 |
| target | O_STACK_ALLOC, O_VA_START | RISC / PPC only | cgen.c:1590,527 |
| declaration ops | O_PROC, O_PARM_DEF, O_AUTO_DEF, O_PASS_PROC_PARM, O_DEFN_PROC_PARM, O_CALL_PROC_PARM, O_PRE_GETS, O_POST_GETS, O_SIDE_EFFECT, O_NOP, O_PARENTHESIS | no (O_PRE_GETS/POST_GETS are implied by `CGPreGets`/`CGPostGets`; FE passes the arithmetic op) | - |
| internal (`O_INTERNAL_*`, OP_*) | - | CG-internal, unreachable from FE | cgops.h:41-144 |

FE opcode -> cg_op table: `copcodes.h:34-131`; note `OPR_CMP`, `OPR_QUESTION`, `OPR_COLON` map to O_NOP and are dispatched by structure, not by table.

### How each construct is expressed

| subject | how it is expressed | when | cite |
|---|---|---|---|
| lvalue vs rvalue | lvalue = address-typed name (`CGFEName`, `O_PLUS` result); rvalue = `CGUnary(O_POINTS, addr, valuetype)`; `OPR_ADDROF`/`OPR_NOP` emit nothing | always; `OPFLAG_RVALUE` on the tree node decides whether `.`/`->`/`[]`/`*`/call add O_POINTS | cgen.c:PushRValue:631; EmitNodes:1504-1506 |
| bit field read | `CGBitMask(addr, start, width, storage type)` then `O_POINTS` with storage type | field access | cgen.c:DotOperator:667 |
| bit field write | `CGAssign(CGBitMask(...), rhs, type)`; no read-modify-write emitted by FE | field assignment | cgen.c:EmitNodes:1318 |
| struct copy | `CGLVAssign(dst_addr, src, refno)`; src is `O_POINTS` with refno type | `=` on struct/union/complex | cgen.c:1314-1316 |
| struct by value as argument / return | `CGAddParm(..., refno)`; return temp typed refno | calls / return | cgen.c:1530; 317-319 |
| struct in `?:` and `,` | result typed as pointer (`DataPointerType`), then `PushRValue` adds the O_POINTS | struct operands | cgen.c:1346-1349,1396-1400 |
| volatile | `CGVolatile` on the address name before `O_POINTS`; on the lvalue of assign/compound/post-inc; symbol-level via `FE_VOLATILE` in `FEAttr` | `OPFLAG_VOLATILE` (from `FLAG_VOLATILE`, cexpr.c:194); `#pragma aux` vars | cgen.c:588,634,909,1312,1364,1422; cinfo.c:FESymAttr:232-238 |
| unaligned | `CGAttr(addr, CG_SYM_UNALIGNED)`; no alignment argument | `__unaligned` qualified access path | cgen.c:585,670,722,905 |
| alignment of objects | `BEDefType` align arg; `DGAlign(GetTypeAlignment)` for static data (skipped when optimizing for size on x86; min 4 on RISC) | aggregate type creation; static data | cgendata.c:AlignIt:51-65 |
| far/near/huge data pointer | only the cg_type (TY_NEAR_POINTER/TY_LONG_POINTER/TY_HUGE_POINTER) on `O_PLUS` / `O_POINTS` / `CGFEName`; node flags `OPFLAG_NEARPTR/FARPTR/HUGEPTR/FAR16PTR` (cops.h:54-57) feed `DataPointerType` | x86 | cgen.c:DataPointerType:322-338 |
| huge index | index multiply uses `TY_INT_4` (8086 huge or huge-data default) | `[]` | cgen.c:IndexOperator:711-715 |
| far code pointers | TY_LONG_CODE_PTR / TY_NEAR_CODE_PTR on the function name, and in data initialisers | x86 | cgen.c:CodePtrType:549-555 |
| `__based` pointers | lowered in FE before CG: `OPR_FARPTR` (segment :> offset), or `OPR_ADD` of base var, `OPR_DUPE` for `__self`, `OPR_PUSHSEG` for `__segname`; CG sees `CGBinary(O_CONVERT, off, seg, TY_LONG_POINTER)` and/or `O_PLUS`. Segment value = `CGFEName(seg sym, TY_UINT_2)`, CS = `CGFEName(func, TY_LONG_CODE_PTR)`, SS = a saved `TY_UINT_2` temp | `__based(...)` deref | cexpr.c:BasedPtrNode:859-947, MakeFarOp:847-857; cgen.c:1443-1458,1507-1511, PushSymSeg:358-381 |
| pointer -> segment | `O_CONVERT` to TY_LONG_POINTER, `O_CONVERT` to TY_UINT_4, `O_RSHIFT` 16 | `OPR_CONVERT_SEG` | cgen.c:1474-1477 |
| `__far16` (386) | `O_PTR_TO_NATIVE` around pointers dereferenced or indexed; `O_PTR_TO_FOREIGN` when converting flat -> far16; function far16 pointers excluded | x86-32 | cgen.c:683,705,900,920-922 |
| varargs callee | no CG call for `va_start` on x86 (macro over the param area). Symbol gets `FE_VARARGS` via `FEAttr`; call class `FECALL_GEN_HAS_VARARGS|CALLER_POPS`; `...` terminates `CGParmDecl` list | x86 | cinfo.c:FESymAttr:223-224; cfeinfo.c:getCallClass:517-519; cgen.c:1244 |
| varargs RISC | `CGVarargsBasePtr` plus assigns (AXP/MIPS), `O_VA_START` (PPC) | `OPR_VASTART` | cgen.c:GenVaStart:509-541 |
| alloca | `O_STACK_ALLOC` on RISC only; on x86 it is an ordinary call | `__builtin_alloca` | cgen.c:1590 |
| math intrinsics | `CGUnary/CGBinary(O_SIN.., TY_DOUBLE)` replacing the libm call | only when not `-od` and extensions on; matched by name (`__SIN` or `sin`) and parm count | cexpr.c:2286-2303 |
| loop unroll | `BEUnrollCount(n)` whenever it changes between statements; `n` from `#pragma unroll` (255 = unlimited) | statement tree walk | cgen.c:GenOptimizedCode:1629-1631; cpragma.c:1440-1455 |
| address-taken / restrict / aliasing | not sent. Only `FE_ADDR_TAKEN` (pragma-used vars) and `FE_UNIQUE`; `FE_NOALIAS`, `FE_ONESEG`, `FE_COMMON` never set | - | cinfo.c:FESymAttr:200-264 |
| const | `FE_CONSTANT` in `FEAttr` (non-volatile `const` object) and placement in SEG_CONST2 unless reentrant; no per-access const info | - | cinfo.c:237-241, SymSegId:293-296 |
| noreturn | `FECALL_GEN_ABORTS`, `FECALL_GEN_NORETURN` in call class | `FLAG_ABORTS/NORETURN` | cfeinfo.c:getCallClass:505-510 |
| naked | `FE_NAKED` | `sym->attribs.naked` | cinfo.c:257-259 |
| inlining | FE decides and emits the callee body inline; CG only sees `DBBegBlock`/`DBEndBlock` around it | see section 4 | cgen.c:1226, 304 |

### Order of events

| step | call | cite |
|---|---|---|
| 1 | `BEInit( GenSwitches, TargetSwitches, OptSize, ProcRevision )`; result only tested `revision != 0 \|\| target != 0` | cgen.c:DoCompile:1924-1925 |
| 2 | P5 profiling (386): `AddSegName( "TI" ... )` | cgen.c:DoCompile:1926-1929 |
| 3 | `SetSegs()` = all `BEDefSeg` | cgen.c:DoCompile:1931, cinfo.c:SetSegs:616-681 |
| 4 | `BEStart()` | cgen.c:DoCompile:1932 |
| 5 | `EmitSegLabels()` (user segment symbols) | cgen.c:DoCompile:1933, cinfo.c:EmitSegLabels:683-702 |
| 6 | debug typedefs (`-d2/-d3`) | cgen.c:DoCompile:1934-1935 |
| 7 | `EmitSyms()`: zero/BSS data for uninitialised globals | cgen.c:DoCompile:1936, cgen.c:EmitSyms:1834-1855 |
| 8 | `EmitCS_Strings()`: `-zc` code-segment literals | cgen.c:DoCompile:1937 |
| 9 | `EmitDataQuads()`: all initialised static data | cgen.c:DoCompile:1941 |
| 10 | `PruneFunctions()`, `GenModuleCode()`: function bodies (CG*); string literals emitted lazily on first use; block-scope statics emitted when the function's locals are declared | cgen.c:DoCompile:1948-1949, cgen.c:Emit1String:824-832, cgen.c:CDoAutoDecl:1088-1098 |
| 11 | `FreeStrings`, `FiniSegLabels`, `BEAbort` if errors, `BEStop`, `FiniSegBacks`, `BEFini` | cgen.c:DoCompile:1950-1963 |

### DG static data

All 16 DG entries in the API: cgfuntab.h:116-131. `DGBlip` does not exist in this tree.

| call | carries | when bld/cc emits it | cite |
|---|---|---|---|
| `DGLabel(back_handle)` | define the label at the current segment position | (1) start of each initialised object, QDT_STATIC, after `BESetSeg(sym segid)` + `AlignIt` | cgendata.c:EmitDQuad:130-134 |
| | | (2) uninitialised object (BSS or zero-fill) | cgen.c:EmitSym:1040-1044 |
| | | (3) string literal, in SEG_CONST (or SEG_CODE/far seg) | cgen.c:EmitLiteral:810-820 |
| | | (4) `__segname` user segment symbol, in seg 10000+n | cinfo.c:EmitSegLabels:692-699 |
| | | (5) SEH exception table (SEG_DATA), P5 profile block (TI seg) | cgen.c:1044-... see cgen.c:489, cgen.c:1209 |
| `DGInteger(val, type)` | integer cell | char/bool as `TY_UINT_1`, short `TY_UINT_2`, int `TY_INTEGER`, long `TY_UINT_4`; a `Q_2_INTS_IN_ONE` quad is two calls | cgendata.c:EmitDQuad:137-172 |
| | | null/absolute pointer (no symbol): value, pointer cg_type | cgendata.c:EmitDQuad:220-221 |
| | | SEH table bytes; P5 profile header words | cgen.c:492-496, cgen.c:1210-1213 |
| `DGInteger64(u64, TY_UINT_8)` | 64-bit cell (signedness not passed) | `__int64` initialiser | cgendata.c:EmitDQuad:174-177 |
| `DGBytes(n, ptr)` | raw bytes | float (`(float)` of the double value, 4), double (8), long double (10 raw) | cgendata.c:EmitDQuad:196, 204, 209 |
| | | char-array literal body (`QDT_CONST`) and string-literal body | cgen.c:EmitBytes:792-796 |
| | | P5 profile function name | cgen.c:1214 |
| `DGIBytes(n, 0)` | n repeated bytes of value 0 | `QDT_CONSTANT` = zero-fill/padding, in chunks of 8K | cgendata.c:EmitZeros:43-49, EmitDQuad:248-249 |
| | | uninitialised object outside BSS (e.g. `const`, user seg) | cgen.c:EmitSym:1065-1067 |
| | | P5 profile name padding to 4 | cgen.c:1215-1218 |
| `DGUBytes(n)` | n uninitialised bytes | only `segid == SEG_BSS` objects | cgen.c:EmitSym:1050-1051 |
| `DGAlign(n)` | align position | before each object: `GetTypeAlignment(type)` when `OptSize == 0` (x86); RISC: max(align,4) always | cgendata.c:AlignIt:51-65 |
| | | wide string literal: `TARGET_SHORT` | cgen.c:EmitLiteral:815-817 |
| `DGFEPtr(sym, type, off)` | pointer to a FE symbol + addend | `QDT_POINTER/QDT_ID` with symbol (`&x`, `&x+4`, function name) | cgendata.c:EmitDQuad:217-224 |
| `DGBackPtr(back, segid, off, type)` | pointer to a back handle in segment | pointer to a string literal (off always 0) | cgen.c:EmitStrPtr:2061-2066 |
| | | SEH table: handler code label, `TY_CODE_PTR` | cgen.c:499 |
| `DGFloat`, `DGChar`, `DGString`, `DGTell`, `DGBackTell`, `DGSeek`, `DGCFloat` | | never called by bld/cc. `DGFloat` only in commented-out lines | cgendata.c:182-183, 202-203 |

How the quad list feeds those calls (cdinit.c builds, cgendata.c:EmitDataQuads:257-268 replays):

| topic | what cdinit does | cite |
|---|---|---|
| storage | initialiser is parsed into a doubly linked list of DATA_QUAD (type, flags, value); overwritable in place | cdinit.c:GenDataQuad:232-266 |
| start of object | `QDT_STATIC` quad carries the symbol; size 0 | cdinit.c:GenStaticDataQuad:1204-1213 |
| zero-fill | `ZeroBytes(n)` = `QDT_CONSTANT` quad. Struct/array start: zero whole object, seek back | cdinit.c:ZeroBytes:268-278, InitSymData:976-977, 998-999 |
| padding | no pad quads. Gaps stay as the zeroed `QDT_CONSTANT`; `RelSeekBytes` moves the cursor (splits quads). Union: rest zeroed after the member | cdinit.c:RelSeekBytes:280-306, InitStructUnion:872-873, 885-887, 908 |
| designated init | seek forward/back with `RelSeekBytes`; later quads overwrite earlier | cdinit.c:InitArray:803-813, GenDataQuad:237-256 |
| repeats/packing | equal adjacent ints fold into `Q_REPEATED_DATA` (emitted n times, loop in EmitDataQuads); two different ints fold into `Q_2_INTS_IN_ONE` | cdinit.c:StoreIValue:321-335, cgendata.c:EmitDataQuads:263-265 |
| bit fields | no bit-field concept reaches DG*. `InitBitField` reads the pending unit from the quad list, masks `field_start/width`, ORs value, stores the whole unit as an integer quad of the field's declared type (64-bit via `StoreIValue64`) | cdinit.c:LoadBitField:622-637, ResetBitField:639-651, InitBitField:653-697 |
| char arrays | `QDT_CONST` quad (points at the literal), then `ZeroBytes(size-len)`; chopped to size | cdinit.c:InitCharArray:1087-1123 |
| wchar arrays | one `QDT_SHORT` quad per char + zero tail | cdinit.c:InitWCharArray:1126-1168 |
| pointers | `AddrFold` to (symbol, offset) -> `QDT_ID`; string -> `QDT_STRING` (offset must be 0); integer constant -> plain int quad | cdinit.c:StorePointer:519-597 |
| pointer flags | `Q_NEAR_POINTER` (`__near`), `Q_FAR_POINTER` (`__far/__huge`, or 6-byte type), `Q_CODE_POINTER` (function pointer). Map to `TY_NEAR_POINTER`, `TY_LONG_POINTER`, `TY_CODE_PTR`, else `TY_POINTER` | cdinit.c:StorePointer:526-541, cgendata.c:GetDQuadPointerCGType:94-108 |
| floats | `StoreFloat` folds to binary `double`; `float` narrowed at emit | cdinit.c:StoreFloat:1171-1202, cgendata.c:180-199 |
| long double | `StoreFloat( TYP_DOUBLE, size )`: quad type DOUBLE, so `DGBytes(8)` while the slot is `size` (10). `QDT_LONG_DOUBLE` emit path exists but cdinit never creates it (grep). Read only, not run. | cdinit.c:InitSymData:1033-1037, cgendata.c:207-211 |
| 8086 | 64K chunking: bump segid (`++segid; BESetSeg`) when crossing 0x10000, except SEG_CONST/SEG_DATA | cgendata.c:EmitDQuad:119-127, 231-246, cgen.c:EmitSym:1053-1064 |
| auto array / struct initialiser | char/wchar array with braces under `auto_agg_inits`: a hidden static `.X` sym (`SYM_TEMP`, SC_STATIC) gets a `QDT_STATIC` quad + data, then struct-copy into the auto | cdinit.c:InitArrayVar:1519-1544, csym.c:GetNewDotSym:499-513 |
| initialised vs zero | `SYM_INITIALIZED` symbols skip EmitSym zero-fill and come via quads; else EmitSym emits zeros | cgen.c:EmitSym:1041, cdinit.c:VarDeclEquals:1654-1659 |
| string pool | literals hashed by (length, flags, bytes) and shared when toggle `reuse_duplicate_strings`; flags FAR (BIG_DATA and len > `DataThreshold`), CONST (`-zc`), WIDE | cstring.c:StringLeaf:272-319, cexpr.c:1681-1685 |
| literal segment | `STRLIT_FAR` -> `FarStringSegId`; `STRLIT_CONST` -> SEG_CODE; else SEG_CONST | cgen.c:StringSegment:799-808 |
| literal emit | lazily on first reference (expression push or `&"str"` in data); `EmitLiteral` switches segment and restores the old one; CONST literals emitted up front with `ref_count != 0` | cgen.c:Emit1String:824-832, EmitLiteral:810-821, DumpCS_Strings:863-871, EmitStrPtr:2061-2066 |
| order | BSS/zero objects (source symbol order) -> CS strings -> initialised quads (source order of initialisers) -> code | cgen.c:DoCompile:1936-1949 |

### Data, labels and segments as the CG calls see them

| call | carries | emitted when | cite |
|---|---|---|---|
| BEDefSeg(id, attrs, name, align) | segment id, `seg_attr` mask, name, alignment. Attr sets used: code `GLOBAL\|INIT\|EXEC` (+`GIVEN_NAME` for named text seg); const `BACK\|INIT\|ROM`; const2 `INIT\|ROM`; data `GLOBAL\|INIT`; bss `GLOBAL`; private `INIT\|PRIVATE` named `<module><n>_DATA`, align 16; user data `INIT\|GLOBAL`; based `INIT\|PRIVATE\|GLOBAL`; initfini `INIT\|GLOBAL`; initfini-TLS `INIT\|GLOBAL\|THREAD_LOCAL`; `#pragma code_seg` `GLOBAL\|INIT\|EXEC\|GIVEN_NAME`. EC-mode adds YIB/YI/YIE | once at start | cinfo.c:SetSegs:630-679 |
| BESetSeg(id) | select current segment | before emitting any data or label; returns previous id, FE restores it | cgen.c:EmitLiteral:814, EmitSym:1042, GenerateTryBlock:483,501; cgendata.c:EmitDQuad:123,132,239; cinfo.c:EmitSegLabels:697 |
| BENewBack(sym or NULL) / BEFiniBack / BEFreeBack | back-end address handle for a symbol (`FEBack`) or anonymous (strings, SEH table) | lazily per symbol; freed after compile | cinfo.c:FEBack:921; cgen.c:827,867,484; FreeSymBackInfo:169-170; FreeGblVars:1866 |
| DGLabel(back) | bind label to current data position | symbol data definitions, strings, seg labels | cgen.c:EmitSym:1044, EmitLiteral:818, GenerateTryBlock:489; cgendata.c:134; cinfo.c:698 |
| DGInteger(val, type) | integer datum. Initialisers use fixed types: TY_UINT_1 (char/bool), TY_UINT_2, TY_INTEGER (int), TY_UINT_4 (long); `Q_2_INTS_IN_ONE` packs two values | initialised data | cgendata.c:EmitDQuad:140-170 |
| DGInteger64(val, TY_UINT_8) | 64-bit datum | long long init | cgendata.c:176 |
| DGBytes(n, ptr) | raw bytes: float (host-converted to target float), double, long double as binary; string literal bytes; const char arrays | init data, strings | cgendata.c:196,204,209; cgen.c:EmitBytes:794 |
| DGFloat | not used (commented out: cgendata.c:183,203) | - | - |
| DGIBytes(n, 0) / DGUBytes(n) | zero fill (`DGIBytes` in 8K chunks) / BSS reservation | uninitialised statics/globals | cgendata.c:46,48; cgen.c:EmitSym:1051 |
| DGAlign(n) | align data | before static data and wide strings | cgendata.c:55,63; cgen.c:EmitLiteral:816 |
| DGFEPtr(sym, ptrtype, offset) | pointer datum = symbol address + offset | pointer/ID initialisers | cgendata.c:EmitDQuad:223 |
| DGBackPtr(back, seg, off, ptrtype) | pointer to a label | string-literal pointer initialisers; SEH table | cgen.c:EmitStrPtr:2065, GenerateTryBlock:499 |
| 8086 >64K objects | FE splits zero-fill into 64K chunks and advances to next segment id (`++segid`; `BESetSeg`) | huge data | cgen.c:EmitSym:1053-1063; cgendata.c:119-127,231-246 |
| BEInit(GenSwitches, TargetSwitches, OptSize, ProcRevision) | global switches (`CGSW_GEN_*`, `CGSW_X86_*`), size/time choice, CPU revision | once | cgen.c:DoCompile:1924 |
| BEStart / BEStop / BEAbort / BEFini / BEUnload | lifecycle; `BEAbort` if `ErrCount != 0` | once | cgen.c:1932,1955,1953,1963,1964 |

### FE queries and symbol attributes

The interface has 17 callbacks: cg/h/cgfertns.h:33-49. `FEStackModel`, `FETrashHere`, `FEStkSize`, `FEDbgInfo`, `FECodeBytes` do not exist at this commit (grep of cc/ and cg/h/). `FEPtrBase`/`FEPtrBaseOffset` are BE-internal functions in cg/intel/c/x86data.c:234,249, not callbacks.

| callback | answers | bld/cc answer | cite |
|---|---|---|---|
| `FEAttr(sym)` | `fe_attr` bits, see table below | `FESymAttr` | cinfo.c:FEAttr:278-284, FESymAttr:200-264 |
| `FEName(sym)` | source (unmangled) name | `sym->name`; `"*** NULL ***"` for SYM_NULL | cinfo.c:FEName:733-744 |
| `FEExtName(sym, req)` | `EXTN_BASENAME` -> `sym->name`; `EXTN_PATTERN` -> mangle pattern; `EXTN_PRMSIZE` -> bytes of params (-1 if variadic/not function), each rounded to `TARGET_INT`; `EXTN_IMPPREFIX` -> `"__imp_"` if NT else NULL; `EXTN_CALLBACKNAME` -> NULL | cfeinfo.c:FEExtName:805-821, GetParmsSize:713-749 |
| | pattern: function -> `inf->objname` (from aux/pragma) else `TS_CODE_MANGLE` (`"*_"` x86 OMF, `"*"` RISC); variadic stdcall/fastcall use cdecl pattern; data -> `VarNamePattern` else `TS_DATA_MANGLE` (`"_*"`/`"*"`); SEH helpers `"*"` | cfeinfo.c:GetNamePattern:751-795, langenv.h:75-76, 106-107 (comp_cfg/h) |
| `FESegID(sym)` | segment of symbol | function: `sym->seginfo->segid` (`alloc_text`) else SEG_CODE; imported far/big-code function whose address is taken: fresh negative id `import_segid--`. Data: `u.var.segid` if set; else FE_GLOBAL -> SEG_DATA; else SEG_CONST | cinfo.c:FESegID:863-907 |
| `FEBack(sym)` | back handle of symbol | created on demand with `BENewBack(sym)`, cached in `sym->u1.backinfo` | cinfo.c:FEBack:910-927 |
| `FEModuleName()` | module name | `-nm` value else source file name | cinfo.c:FEModuleName:847-853 |
| `FEMessage(msg, parm)` | see table below | | cinfo.c:FEMessage:747-844 |
| `FEMoreMem(size)` | may BE use more memory | always 0 | cmemmgr.c:FEMoreMem:444-450 |
| `FETrue()` | value of "true" for compares | 1 | cinfo.c:FETrue:856-860 |
| `FEGenProc(sym, call)` | BE asks FE to generate an inline function body now | `GenInLineFunc(sym)`; `call` ignored. BE side: inline.c:112 | cinfo.c:FEGenProc:267-275, cgen.c:GenInLineFunc:1699-1707, cg/c/inline.c:112 |
| `FELexLevel(sym)` | nesting depth | always 0 (C has no nested functions) | cinfo.c:FELexLevel:930-936 |
| `FEDbgType(sym)` | debug type of symbol | `DBType( sym->sym_type )` | cdebug.c:FEDbgType:410-415 |
| `FEDbgRetType(sym)` | debug return type | `DBType( fn return )` or `DBG_NIL_TYPE` if not a function | cdebug.c:FEDbgRetType:417-428 |
| `FEStackChk(sym)` | needs stack-check prolog | `sym->flags & SYM_CHECK_STACK` | cinfo.c:FEStackChk:1002-1010 |
| `FEGetEnv(name)` | env var (BE reads `TRQUIET`, `WCGMEMORY`, `WCGBLIPON`) | IDE env lookup unless `ignore_environment` | ideentry.c:FEGetEnv:160-170, cg/c/memmgt.c:99, cgmemmgr.c:191, doblips.c:167 |
| `FEParmType(func, parm, type)` | type a parameter is passed as | x86: `TY_INT_1/TY_UINT_1` -> `TY_INTEGER`; 386 and RISC also `TY_INT_2/TY_UINT_2` -> `TY_INTEGER`; 386 `__far16` function: all four -> `TY_INT_2`. AXP: ints/pointers -> `TY_INT_8`, `TY_UINT_1/2` -> `TY_UINT_8`, float/long double -> `TY_DOUBLE` | cinfo.c:FEParmType:940-998 |

#### fe_attr bits (`cg/h/cg.h:46-67`); derived in `cinfo.c:FESymAttr:200-264`

| bit | means (BE use) | C construct that sets it | cite |
|---|---|---|---|
| `FE_PROC` | symbol is a function (x86data.c:212, x86esc.c:130) | `SYM_FUNCTION`; also forces `FE_STATIC` | cinfo.c:221-222 |
| `FE_STATIC` | static storage: BE makes global memory operand. Absent -> BE makes stack temp (makeaddr.c:565 vs 585-599) | SC_FORWARD/SC_EXTERN, SC_NONE, SC_STATIC, every function. NOT SC_AUTO/SC_REGISTER/params (attr stays 0) | cinfo.c:206-222, cg/c/makeaddr.c:565 |
| `FE_GLOBAL` | externally visible definition (dataflo.c:193 forces memory; x86owl.c:381 import form) | SC_NONE (file-scope non-static def); SC_EXTERN/SC_FORWARD | cinfo.c:208-213 |
| `FE_IMPORT` | defined elsewhere | SC_EXTERN, SC_FORWARD (forward decl treated as import) | cinfo.c:206-210 |
| `FE_VISIBLE` | static but reachable by callees; blocks alias/scoreboard assumptions (conflict.c:92, dataflo.c:193, redefby.c:278) | SC_STATIC (any level) | cinfo.c:214-215 |
| `FE_INTERNAL` | block-scope static: referenced by segment offset, no symbol import form (x86omf.c:1390, x86owl.c:383) | SC_STATIC with `sym->level != 0`. File-scope `static` is not INTERNAL | cinfo.c:216-218 |
| `FE_VARARGS` | variadic function (used by AXP/MIPS prolog only; no x86 consumer found) | `VarFunc(sym)`: `...` in prototype, or name in the printf/scanf/exec/spawn/open/sopen hash table (ANSI subset unless extensions on) | cinfo.c:223-224, cfeinfo.c:VarParm:141-165, VarFunc:167-197 |
| `FE_UNIQUE` | function address must be unique; label status UNIQUE (optask.c:109) | only with `-ou` (`unique_functions`), and function is FE_GLOBAL or `SYM_ADDR_TAKEN` | cinfo.c:225-229, coptions.c:721-722 |
| `FE_MEMORY` | never keep in register (makeaddr.c:567, 587) | `#pragma aux` body uses it (`SYM_USED_IN_PRAGMA`); `SYM_TRY_VOLATILE`; `volatile` | cinfo.c:231-238 |
| `FE_ADDR_TAKEN` | address taken -> `USE_ADDRESS` (makeaddr.c:590, cse.c:97) | ONLY `SYM_USED_IN_PRAGMA`. C `&x` does not set it | cinfo.c:231-233 |
| `FE_VOLATILE` | volatile operand (makeaddr.c:572, 596) | `SYM_USED_IN_PRAGMA`, `SYM_TRY_VOLATILE`, `FLAG_VOLATILE` | cinfo.c:231-238 |
| `FE_CONSTANT` | read-only object (`VAR_CONSTANT`; survives calls; redefby.c:259) | `FLAG_CONST` and not volatile (volatile wins) | cinfo.c:237-241 |
| `FE_DLLIMPORT` | symbol comes from DLL (x86omf.c:2418, x86data.c:222) | `__declspec(dllimport)` and (FE_IMPORT or `-re`) | cinfo.c:243-247 |
| `FE_DLLEXPORT` | export (x86omf.c:2542) | `__declspec(dllexport)` and SC_NONE | cinfo.c:248-252 |
| `FE_THREAD_DATA` | TLS (x86omf.c:2423, 386tls.c:193) | `__declspec(thread)`; or `sym->attribs.rent` (`-re`) | cinfo.c:253-262 |
| `FE_NAKED` | no prolog/epilog (x86proc.c:975) | `sym->attribs.naked` | cinfo.c:257-259 |
| `FE_NOALIAS`, `FE_COMMON`, `FE_ONESEG`, `FE_UNALIGNED`, `FE_COMPILER` | | never set by C (no reference in FESymAttr). Unaligned goes through `CGAttr( name, CG_SYM_UNALIGNED )` per access; volatile also per access via `CGVolatile` | cinfo.c:200-264, cgen.c:584-588, 669-670 |

#### FEMessage (`fe_msg`, cg.h:180-202); bld/cc action in `cinfo.c:FEMessage:747-844`

| msg | bld/cc |
|---|---|
| `FEMSG_SYMBOL_TOO_LONG` | warning at sym location (754-763) |
| `FEMSG_BLIP` | `ConBlip` if IDE console and not quiet (764-770) |
| `FEMSG_INFO`, `_INFO_FILE`, `_INFO_PROC` | `NoteMsg` if IDE console and not quiet (771-779) |
| `FEMSG_CODE_SIZE` | "Code size: n" note (780-788) |
| `FEMSG_DATA_SIZE` | ignored (789-790) |
| `FEMSG_ERROR` | internal-error message (791-793) |
| `FEMSG_FATAL` | error, close files, `MyExit(1)` (794-798) |
| `FEMSG_BAD_PARM_REGISTER`, `_BAD_RETURN_REGISTER`, `_BAD_SAVE`, `_BAD_LINKAGE`, `_NO_SEG_REGS`, `_BAD_PEG_REG` | error naming the symbol (799-807, 829-840) |
| `FEMSG_SCHEDULER_DIED`, `_REGALLOC_DIED`, `_SCOREBOARD_DIED` | info "not enough memory to fully optimize" once per function, unless `-od` (808-817) |
| `FEMSG_PEEPHOLE_FLUSHED` | info once, unless `-od` (818-825) |
| `FEMSG_BACK_END_ERROR` | error with int code (826-828) |
| `FEMSG_WANT_MORE_DATA` | ignored (default, 841) |

#### FEAuxInfo (`aux_class`, cgaux.h:43-88 + x86auxc.h:33-45); handler `cfeinfo.c:FEAuxInfo:1079-1223`; unhandled requests return NULL (1219-1222)

`req_handle` is the symbol handle unless noted. `FindAuxInfoSym(s,r)` = `FEAuxInfo( FEAuxInfo(s,FEINF_AUX_LOOKUP), r )` (cgauxinf.h:34); C returns the handle itself for `FEINF_AUX_LOOKUP` (1103-1104).

| query | answer | cite |
|---|---|---|
| `FEINF_SOURCE_LANGUAGE` | `"c"` | cfeinfo.c:1089-1090, watcom/h/felang.h:34 |
| `FEINF_OBJECT_FILE_NAME` | `ObjFileName()` | cfeinfo.c:1099-1100 |
| `FEINF_SOURCE_NAME` | full path of first file | cfeinfo.c:1133-1134 |
| `FEINF_REVISION_NUMBER` | `II_REVISION` (9). BE never asks (no caller in cg/) | cfeinfo.c:1101-1102, cg.h:223 |
| `FEINF_NEXT_DEPENDENCY`, `_DEPENDENCY_TIMESTAMP`, `_DEPENDENCY_NAME` | include-file list (`-ad`): next FNAMEPTR, `&mtime`, full path | cfeinfo.c:1160-1167 |
| `FEINF_CALL_CLASS` | `call_class`: aux/pragma class + ABORTS (`FLAG_ABORTS`), NORETURN, DLL_EXPORT (`FLAG_EXPORT`), MAKE_CALL_INLINE (`IsInLineFunc`), CALLER_POPS+HAS_VARARGS (`VarFunc`); REVERSE_PARMS masked off | cfeinfo.c:getCallClass:491-526, 1135-1136 |
| `FEINF_CALL_CLASS_TARGET` | x86 `call_class_target`: aux class + FARSS, EMIT_FUNCTION_NAME (`-en`), FAR_CALL / INTERRUPT (`__far`+`__near`), LOAD_DS_ON_ENTRY (`__loadds`), THUNK_PROLOG, PROLOG_FAT_WINDOWS (16-bit Windows pascal/cdecl), PROLOG_HOOKS (`-ep`), EPILOG_HOOKS (`-ee`), GROW_STACK (`-sg`), TOUCH_STACK (`-st`). RISC: 0 | cfeinfo.c:getCallClassTarget:528-599, 1137-1138 |
| `FEINF_PARM_REGS` | `hw_reg_set[]` from aux; varargs without inline code get `DefaultVarParms`; SEH helpers `TryParms` | cfeinfo.c:1195-1213 |
| `FEINF_RETURN_REG` | `&inf->returns` | cfeinfo.c:1189-1191 |
| `FEINF_STRETURN_REG` (x86) | `&inf->streturn` | cfeinfo.c:1215-1217 |
| `FEINF_SAVE_REGS` | `&inf->save`; + `HW_SEGS` if `FLAG_SAVEREGS` (`-r`/`__saveregs`) | cfeinfo.c:1174-1188 |
| `FEINF_CALL_BYTES` | inline machine code (`#pragma aux ... = bytes`) or NULL | cfeinfo.c:1192-1194 |
| `FEINF_FREE_SEGMENT` | NULL | cfeinfo.c:1139-1140 |
| `FEINF_NEXT_LIBRARY`, `_LIBRARY_NAME` | index / name of default libs: clib if any main / pragma library / all-default-libs; math lib + emu lib if float used; plus `#pragma library` names | cfeinfo.c:NextLibrary:632-662, addDefaultLibs:601-630 |
| `FEINF_NEXT_IMPORT`, `_IMPORT_NAME` | extra undefined refs, see list below | cfeinfo.c:NextImport:990-1042 |
| `FEINF_NEXT_IMPORT_S`, `_IMPORT_NAME_S` | extra refs given as symbols (from `AddExtRefS`) | cfeinfo.c:NextImportS:1044-1077 |
| `FEINF_NEXT_ALIAS`, `_ALIAS_NAME`, `_ALIAS_SYMBOL`, `_ALIAS_SUBST_NAME`, `_ALIAS_SUBST_SYMBOL` | `#pragma alias` list | cfeinfo.c:NextAlias:664-711 |
| `FEINF_TEMP_LOC_NAME` | `TEMP_LOC_QUIT` (no fixed temp locations) | cfeinfo.c:1156-1157 |
| `FEINF_TEMP_LOC_TELL` | NULL | cfeinfo.c:1158-1159 |
| `FEINF_DBG_DWARF_PRODUCER` | `DWARF_PRODUCER_ID` | cfeinfo.c:1172-1173 |
| x86 `FEINF_STACK_SIZE_8087` | `Stack87` (8, or 4 for `-fpr` or NetWare 3/4) | cfeinfo.c:1092-1093, cmdlnx86.c:329-335 |
| x86 `FEINF_CODE_GROUP` / `FEINF_DATA_GROUP` | `GenCodeGroup` (`-g`) / `DataSegName` (`-nd`) | cfeinfo.c:1094-1097 |
| x86 `FEINF_PROEPI_DATA_SIZE` | `ProEpiDataSize` | cfeinfo.c:1106-1107 |
| x86 `FEINF_DBG_PREDEF_SYM` | `SymDFAbbr` (`__DFABBREV`, `-hda/-hdg`) | cfeinfo.c:1108-1109, cmdlnx86.c:524, 528 |
| x86 `FEINF_P5_CHIP_BUG_SYM` | `SymChipBug`. No BE caller found by grep | cfeinfo.c:1110-1111 |
| x86 `FEINF_CODE_LABEL_ALIGNMENT` | 3 bytes `{2,1,1}`; `[1]=TARGET_INT` if `OptSize==0`. BE uses it only below 386 (386/486 return fixed values first) | cfeinfo.c:1112-1120, cg/intel/c/x86enc2.c:DepthAlign:134-158 |
| x86 `FEINF_CLASS_NAME` | `SegClassName(segid)`: SEG_CODE -> `CodeClassName`; user seg class (empty -> `FAR_DATA` if based or name ends DATA, `CODE` if ends TEXT); `alloc_text` seg class; any other id -> `"FAR_DATA"` if `-nd` set, else NULL | cfeinfo.c:1121-1122, cinfo.c:SegClassName:505-553 |
| x86 `FEINF_USED_8087` | side effect: `pgm_used_8087 = true`; NULL | cfeinfo.c:1123-1125 |
| x86 `FEINF_PEGGED_REGISTER` | `&useg->pegged_register` for `__based(__segname("_ES:x"))`-style segs; NULL otherwise | cfeinfo.c:1169-1170, cinfo.c:SegPeggedReg:591-601, AddSeg:424-449 |
| 386 `FEINF_P5_PROF_DATA` / `_P5_PROF_SEG` | `FunctionProfileBlock` back handle / `FunctionProfileSegId` | cfeinfo.c:1126-1131 |
| not handled (NULL): `FEINF_SHADOW_SYMBOL`, `_DEFAULT_IMPORT_RESOLVE`, `_UNROLL_COUNT`, `_DBG_PCH_SYM`, `_DBG_SYM_ACCESS`, `_IMPORT_TYPE`, `_CONDITIONAL_IMPORT`, `_NEXT_CONDITIONAL`, `_CONDITIONAL_SYMBOL`, `_CLASS_APPENDED_NAME`, `_VIRT_FUNC_*`, `_FREE_AUX_REQ1` | BE callers exist for most (e.g. tree.c:1525, x86omf.c:2429, dfsyms.c:597, 1028, cvsyms.c:408, bldcall.c:137); `_UNROLL_COUNT` and `_FREE_AUX_REQ1` have none. C uses `BEUnrollCount()` instead | cfeinfo.c:1219-1222 |

Extra imports (`addDefaultImports`, cfeinfo.c:823-987), queried through `FEINF_NEXT_IMPORT`; needs `emit_targimp_symbols` (1013-1014), and `emit_library_names` for the float/code-model ones:

| condition | symbol(s) |
|---|---|
| `main`/`WinMain`/DLL main seen (`-bd/-bw/-bg/-bc` pick) | `__DLLstart_`[`w`], `_wstart_`[`w`]/`_cstart_`[`w`] (`w` = `has_wchar_entry`) (832-875) |
| float used | `_fltused_`; `+ _fltused_80bit_` if `-fld` (878-884) |
| 8086, any statement | `_big_code_` or `_small_code_` (885-892) |
| 87 used, emulator | `__init_87_emulator` (16-bit) / `__init_387_emulator` (32-bit) (894-902) |
| 87 used, not FPC | `__old_8087` if `Stack87==4` else `__8087` (903-909) |
| RISC float | `_fltused_` (912-920) |
| `main` with parameters | `__argc`/`__wargc` (16-bit, or 32-bit register convs), `_argc`/`_wargc` (32-bit stack), RISC `_argc` (922-950) |
| `-bw` | `__init_default_win` (954-956) |
| NetWare | `__WATCOM_Prelude` (961-967) |
| `-et` / `-etp` | `__p5_profile` / `__new_p5_profile` (971-985) |

FindInfo (which aux record answers): default record by calling-convention flag (`GetLangInfo`, 290-312); `#pragma aux` by name (`InfoLookup`, 331-403, also `_inline_`/intrinsic tables `IF_Lookup` 217-287); typedef'd aux (441-453); `__far16` override (454-464); STOSB/`finally`/`tryfini` special syms (420-433).

### Back-end configuration: switches and segments

#### BEInit arguments (cgen.c:DoCompile:1924) and return

| arg / ret | value | set where | cite |
|---|---|---|---|
| `cg_switches` | `GenSwitches` | see table below | |
| `cg_target_switches` | `TargetSwitches` | x86: cmdlnx86.c; RISC: cmdlnrsc.c | |
| `uint OptSize` | 0 = `-ot`, 100 = `-os`, 50 default | | coptions.c:467-478 |
| `proc_revision` | `ProcRevision`: CPU 0-6, FPU level, FPU_EMU bit, Weitek | x86 `-0..-6`, `-fp2/3/5/6`, `-fpi/-fpi87/-fpc` | cmdlnx86.c:192-324, cg/intel/h/cgx86swi.h:37-62 |
| return `cg_init_info{revision,target}` | C only checks not both 0; does not compare to `II_REVISION` (9) or `II_TARG_*` | | cg.h:207-223, cgen.c:1925 |
| other BE calls | `BETypeLength(TY_INTEGER)` (default alignment), `BEUnrollCount(n)` per statement when it changes, `BEDefType(ref,align,size)` for each struct/union/array type (refno from `NewRefno`; `BEAliasType` unused) | | cinfo.c:627, cgen.c:1629-1632, 1995-1999, 2009 |

#### cg_switches (cg/h/cgswitch.h:39-71)

Initial: `GenSwitches = CGSW_GEN_MEMORY_LOW_FAILS` (cmdlnx86.c:74, cmdlnrsc.c:56); cdata.c:142 zeroes before.

| bit | set by | cite |
|---|---|---|
| `OBJ_ENDIAN_BIG` | RISC `-ebe/-ele` | cmdlnrsc.c:229-232 |
| `OBJ_COFF` / `OBJ_ELF` | default by target (RISC); x86 386 `-eoc/-eoe`; `-eoo` clears both | cmdlnrsc.c:178-182, 235-238; cmdlnx86.c:554-561 |
| `DLL_RESIDENT_CODE` | `-bd` | coptions.c:611-613 |
| `POSITION_INDEPENDANT` | `CompFlags.rent` (`-re`) | cgen.c:DoCompile:1921-1923 |
| `MICROSOFT_COMPATIBLE`, `FORTRAN_ALIASING` | never set by bld/cc (grep) | |
| `ECHO_API_CALLS` | DEVBUILD `-lc`; toggle dump_cg | coptions.c:681-684, cgen.c:1916-1919 |
| `SUPER_OPTIMAL` | `-oh` | coptions.c:697-698 |
| `FPU_ROUNDING_OMIT` / `_INLINE` | `-zro` / `-zri` | cmdlnx86.c:346-351 |
| `FLOW_REG_SAVES` | `-ok` | coptions.c:703-704 |
| `BRANCH_PREDICTION` | `-ob`; `-ox` | coptions.c:689-690, 449 |
| `DBG_PREDEF` | `-hda`, `-hdg` (with `DBG_DF`) | cmdlnx86.c:522-528 |
| `NULL_DEREF_OK` | `-oz` | coptions.c:724-725 |
| `FP_UNSTABLE_OPTIMIZATION` | `-on` | coptions.c:712-713 |
| `MEMORY_LOW_FAILS` | default; cleared by `-oo` | coptions.c:715-716 |
| `INS_SCHEDULING` | `-or`; `-ox` | coptions.c:718-719, 455 |
| `LOOP_OPTIMIZATION` | `-ol`, `-ol+`, `-ox` | coptions.c:706-710, 452 |
| `LOOP_UNROLLING` | `-ol+` | coptions.c:709-710 |
| `DBG_TYPES`, `DBG_LOCALS`, `DBG_NUMBERS` | `-d2`, `-d3`, `-d1+` set all three; `-d1` only NUMBERS | coptions.c:520-549 |
| `RELAX_ALIAS` | `-oa` | coptions.c:686-687 |
| `DBG_CV` / `DBG_DF` | `-hc` / `-hd*` (default DWARF; Watcom format `-hw` sets neither) | cmdlnx86.c:515-537 |
| `NO_OPTIMIZATION` | `-od`; `-d2`, `-d3` always force it (`-d3` twice) ; cleared by `-ox`, `-ot`, `-os` | coptions.c:458-459, 528, 541, 448, 470, 474 |
| `I_MATH_INLINE` | `-om`; `-ox`; `CmdSysSetMaxOptimization` | coptions.c:450, cmdlnx86.c:614-615, 1226-1229 |
| `NO_CALL_RET_TRANSFORM` | `-oc` | cmdlnx86.c:605-606 |

#### cg_target_switches, x86 (cg/intel/h/x86swi.h:33-58)

Initial: 16-bit `0`; 32-bit `CGSW_X86_USE_32` (cmdlnx86.c:75-79).

| bit | set by | cite |
|---|---|---|
| `EZ_OMF` | 386 `-ez` | cmdlnx86.c:580-581 |
| `BIG_DATA`, `BIG_CODE`, `CHEAP_POINTER`, `FLAT_MODEL` | memory model: ms=cheap; mm=big code+cheap; mc=big data+cheap; ml=both+cheap; mh (16-bit)=both, not cheap; mf (32-bit, default except NetWare)=flat+cheap. Mask cleared then OR'd at end | cmdlnx86.c:986-1025, 1142-1143 |
| `FLOATING_SS` | `-zu` | cmdlnx86.c:1041-1043 |
| `FLOATING_ES` | not flat | cmdlnx86.c:1047-1049 |
| `FLOATING_DS` | big data; cleared by 16-bit Windows, `-zdl`; `-zdp/-zdf` | cmdlnx86.c:1053-1088 |
| `FLOATING_FS` | CPU>=386 and (16-bit, or not flat, or 32-bit Windows); `-zfp/-zff`; cleared if CPU<386 | cmdlnx86.c:1095-1119, 1145-1149 |
| `FLOATING_GS` | CPU>=386; `-zgp/-zgf` | cmdlnx86.c:1123-1140 |
| `USE_32` | 32-bit compiler | cmdlnx86.c:79 |
| `INDEXED_GLOBALS` | 386 `-xgv` | cmdlnx86.c:643-644 |
| `WINDOWS` | 16-bit Windows target | cmdlnx86.c:438-441 |
| `CHEAP_WINDOWS` | cheap-Windows target / `-zW` | cmdlnx86.c:434-436 |
| `SMART_WINDOWS` | 16-bit `-zws`, `-zWs` | cmdlnx86.c:111-118 |
| `CONST_IN_CODE` | `-zc`; cleared for ms/mm models | cmdlnx86.c:647-650, 990, 998 |
| `NEED_STACK_FRAME` | `-of`, `-of+` | cmdlnx86.c:608-613 |
| `LOAD_DS_DIRECTLY` | 386 `-zdl` | cmdlnx86.c:1060-1061 |
| `P5_PROFILING` (+`_CTR0`), `NEW_P5_PROFILING`, `STATEMENT_COUNTING` | 386 `-et`, `-et0`, `-etp`, `-esp` | cmdlnx86.c:567-579 |
| `P5_DIVIDE_CHECK` | `-fpd` | cmdlnx86.c:340-341 |
| `GEN_FWAIT_386` | `-zfw` | cmdlnx86.c:343-344 |
| `FLOATING_*` cleared when CPU<386 | (FS, GS) | cmdlnx86.c:468-472 |
| `GENERIC_TLS`, `NULL_SELECTOR_BAD` | never set by bld/cc (grep) | |

#### BEDefSeg (`void BEDefSeg(segment_id, seg_attr, name, align)`, cgfuntab.h:48; seg_attr cg.h:166-177); all in `cinfo.c:SetSegs:616-681`

`SegAlign(a)` = 1 if `-zp`-style `unaligned_segs`, else `a` (604-614). `SegAlignment[id]` starts at `TARGET_INT` (1019) and grows to the max `GetTypeAlignment` of symbols placed there when `OptSize == 0` (1033-1045).

| segment id | attr | name (x86 OMF / RISC) | align | cite |
|---|---|---|---|---|
| `SEG_CODE`=1 | `GLOBAL\|INIT\|EXEC` (+`GIVEN_NAME` with `-nt`) | `_TEXT` / `.text` or `-nt` name | `BETypeLength(TY_INTEGER)` if `OptSize==0` else 1 | 629-633 |
| `SEG_CONST`=2 | `BACK\|INIT\|ROM` (BE may place its own data here) | `CONST` / `.const` | `SegAlignment` | 634 |
| `SEG_CONST2`=3 | `INIT\|ROM` | `CONST2` / `.const2` | `SegAlignment` | 635 |
| `SEG_DATA`=4 | `GLOBAL\|INIT` | `_DATA` / `.data` | `SegAlignment` | 636 |
| `SEG_YIB/YI/YIE` | `GLOBAL\|INIT`, only with `-ec` | `YIB/YI/YIE` / `.rtl$yib..` | 2 | 637-641 |
| `SEG_BSS` | `GLOBAL` (no INIT), only if any symbol went to BSS | `_BSS` / `.bss` | `SegAlignment` | 642-644 |
| private ids `FIRST_PRIVATE_SEGMENT`(14) .. `SegmentNum-1` | `INIT\|PRIVATE` | `<module><id>_DATA` | `SegAlign(16)` | 645-654 |
| user segs, id 10000+ | see next | | | 655-676 |
| `#pragma alloc_text` segs, id `++SegmentNum` | `GLOBAL\|INIT\|EXEC\|GIVEN_NAME` | as given | like code | 677-680 |
| `SEG_THREAD_BEG/THREAD/THREAD_END`, `SEG_STACK` | never passed to `BEDefSeg`. THREAD ids unused by cc (TLS uses user segs); `SEG_STACK` is a pointer-segment tag only | | | csegid.h:43-47; cexpr.c:578, 793 |

User segments (`userSegments`, ids from `FIRST_USER_SEGMENT` = 10000, cinfo.c:48, 1017):

| segtype | made by | attr | align | cite |
|---|---|---|---|---|
| `SEGTYPE_DATA` | `#pragma data_seg` | `INIT\|GLOBAL` | `TARGET_INT` | 663-665 |
| `SEGTYPE_BASED` | `__based(__segname("x"))` | `INIT\|PRIVATE\|GLOBAL` | `TARGET_INT` | 666-668 |
| `SEGTYPE_INITFINI` | segs named `TI/XI/YI` (+ `*IB`, `*IE` added as a triple) | `INIT\|GLOBAL` | 1 | 393-480, 669-671 |
| `SEGTYPE_INITFINITR` | TLS `.tls`, `.tls$`, `.tls$ZZZ` | `INIT\|GLOBAL\|THREAD_LOCAL` | 1 | 101-103, 672-674 |
| `SEGTYPE_CODE` | `#pragma code_seg` | no `BEDefSeg` (`#if 0`); uses `SetFuncSegment` -> `LkSegName` | | 658-662, 492-502 |

Predefined names `_CODE,_CONST,_DATA,_STACK` map to SEG_CODE/CONST/DATA/STACK (cinfo.c:77-82, AddSeg:419-423). Names with `:` split a register peg (424-449).

#### Segment id assignment per symbol

| rule | cite |
|---|---|
| auto/register/typedef -> `SEG_NULL` | cinfo.c:AssignSeg:116-122 |
| non-extern, uninitialised: `SetSegment`; `SEG_DATA` -> `SEG_BSS` and `bss_segment_used`; `SetSegAlign` | cinfo.c:AssignSeg:123-133 |
| extern far/huge: fresh negative id `import_segid--`; extern near with `-nd`: `import_near_segid` (-1) | cinfo.c:AssignSeg:134-138, ImportNearSegIdInit:1047-1050, ImportSegIdInit:1052-1059 |
| `SymSegId`: function -> SEG_CODE; `FE_CONSTANT` and not `-re` -> SEG_CONST2; else SEG_DATA | cinfo.c:286-299 |
| `SetSegment`, 16-bit: far const (or static temp) under `-zc` -> SEG_CODE | cinfo.c:308-315 |
| `SetSegment`, 32-bit non-`-re`, flat or far: const under `-zc` -> SEG_CODE; any static temp (hidden `.X` init sym, etc.) -> SEG_CODE | cinfo.c:317-329 |
| far/huge data: first-fit into `SegListHead` list of private segs (16-bit: by remaining size; oversized bump `SegmentNum`); 32-bit: one list head | cinfo.c:SetSegment:331-371 |
| far decision: `BIG_DATA` model, no model keyword, size > `DataThreshold` (`-zt`), or unsized extern array, or const under `-zc`; >64K becomes huge (16-bit) | cinfo.c:SetFarHuge:142-198 |
| file-scope default via `FESegID` if `u.var.segid` is SEG_NULL | cinfo.c:FESegID:897-902 |

### Facts that enable optimisation

What the FE tells the BE that permits harder optimisation, and what it is used for. "BE use" cites were read in cg/.

| fact | how bld/cc states it | what it enables (BE use) | cite |
|---|---|---|---|
| object is read-only | `FE_CONSTANT` (`const`, not volatile) on a static | `VAR_CONSTANT`: value survives calls (`CST_OK_ACROSS_CALLS`), loads are redundant-load/CSE candidates, never "redefined by" | cinfo.c:237-241, cg/c/makeaddr.c:578, conflict.c:92, redefby.c:259 |
| object lives in a ROM segment | `SEG_CONST`/`SEG_CONST2` defined `ROM`; const statics go to CONST2 unless `-re` | `AskNameIsROM` -> same "constant" treatment for any symbol in a ROM segment | cinfo.c:634-635, 293-296, cg/c/redefby.c:261 |
| block-scope static is private | `FE_INTERNAL` | referenced by segment offset without symbol (cheaper fixups, no public name) | cinfo.c:216-218, cg/intel/c/x86omf.c:1390 |
| file-scope `static` not visible outside | `FE_STATIC` without `FE_GLOBAL`; `FE_VISIBLE` also on | with `-oa` (`RELAX_ALIAS`) a static that is neither `FE_GLOBAL` nor `FE_VISIBLE` counts as call-safe. Because C sets `FE_VISIBLE` on all `static`, this path is never taken for C statics | cinfo.c:214-215, cg/c/conflict.c:92 |
| globals may be touched by any callee | `FE_GLOBAL` / `FE_VISIBLE` | unless `-oa`, all such memory names forced to memory (`USE_MEMORY\|USE_ADDRESS`). So `-oa` is what un-forces them | cg/c/dataflo.c:188-198, coptions.c:686-687 |
| auto is a register candidate | `attr == 0` for SC_AUTO/REGISTER/params: BE builds a stack temp, enregisterable | makeaddr.c:585-599 | cinfo.c:206-222 |
| must stay in memory | `FE_MEMORY` (volatile, try-volatile, used by pragma aux) | `NEEDS_MEMORY\|USE_MEMORY` | cinfo.c:231-238, makeaddr.c:567, 587 |
| volatile | `FE_VOLATILE` on the symbol, and `CGVolatile(name)` on each access (`OPFLAG_VOLATILE`); float temps forced volatile by `ForceVolatileFloat` when needed | blocks CSE/scoreboard/dead-store | cinfo.c:237-238, cgen.c:587-590, cg/c/cse.c:94-95 |
| unaligned access | `CGAttr(name, CG_SYM_UNALIGNED)` per access (packed struct members), never `FE_UNALIGNED` | BE must use unaligned ops; absence = aligned ops | cgen.c:584-586, 669-671 |
| address taken | only via pragma aux (`FE_ADDR_TAKEN`). `SYM_ADDR_TAKEN` kept FE-side (used for FE_UNIQUE, import seg ids, debug info) | `USE_ADDRESS`; CSE skips such temps. How the BE learns of C `&x` otherwise was not traced | cinfo.c:231-233, 225-229, cg/c/cse.c:96-98 |
| function never returns | `FECALL_GEN_ABORTS` (`__declspec(noreturn)`-class `FLAG_ABORTS`, aux `aborts`), `FECALL_GEN_NORETURN` (`FLAG_NORETURN`, `FunctionAborts`) | `ROUTINE_NEVER_RETURNS_*` routine attr (x86reg.c:88-93); further use at x86enc2.c:388, 412 not read | cfeinfo.c:491-526, 471-489, cg/intel/c/x86reg.c:88-93, x86enc2.c:388, 412 |
| call writes no memory | `FECALL_GEN_NO_MEMORY_CHANGED` from `#pragma aux ... modify nomemory` | `CALL_WRITES_NO_MEMORY`: memory values survive the call (scoreboard, loop-invariant load hoisting, dead stores) | cpragx86.c:924-926, cg/intel/c/x86reg.c:110-112, cg/c/redefby.c:272-273, loopopts.c:547, scblock.c:191 |
| call reads no memory | `FECALL_GEN_NO_MEMORY_READ` from `#pragma aux ... parm nomemory` (pragma only; C has no `pure`/`const` function attribute) | `ROUTINE_READS_NO_MEMORY`: stores before the call need not be flushed | cpragx86.c:820-822, x86reg.c:113-115 |
| exact register clobber set | `FECALL_X86_MODIFY_EXACT` (`modify exact`, and all inline-table functions) | sets `ROUTINE_MODIFY_EXACT` (x86reg.c:107-109); effect not read | cpragx86.c:921-923, cfeinfo.c:381, cg/intel/c/x86reg.c:107-109 |
| register convention | `FEINF_PARM_REGS`, `FEINF_RETURN_REG`, `FEINF_SAVE_REGS`, `FEINF_STRETURN_REG`, `FEINF_CALL_BYTES` (inline asm bytes) | arguments/results in registers; callee-saved sets let values live across calls; inline bytes = no call at all | cfeinfo.c:1174-1217 |
| call inlining | `FECALL_GEN_MAKE_CALL_INLINE` (from `IsInLineFunc`) + `FEGenProc` callback | BE calls back; FE emits the callee body in place. Depth-limited (`MAX_INLINE_DEPTH`); FE decides eligibility (`FUNC_OK_TO_INLINE`, cleared by string literals) | cfeinfo.c:514-516, cinfo.c:267-275, cgen.c:1699-1726, cstring.c:326-328 |
| caller pops / varargs | `FECALL_GEN_CALLER_POPS`, `HAS_VARARGS`, `FE_VARARGS` | correct stack cleanup; lets BE skip callee pops | cfeinfo.c:517-519, cinfo.c:223-224 |
| function address must be unique | `FE_UNIQUE` (`-ou`) | label status `UNIQUE` (optask.c:109); meaning of UNIQUE in the BE not read | cinfo.c:225-229, cg/c/optask.c:109 |
| loop unroll factor | `#pragma unroll(n)` -> `BEUnrollCount(n)` per statement (255 = default max). Not via `FEINF_UNROLL_COUNT` | drives `unroll.c` when `LOOP_UNROLLING` (`-ol+`) | cpragma.c:1440-1455, cgen.c:1629-1632, cg/c/unroll.c:303 |
| unsigned/signed/float/width | exact `cg_type` for every operand from `CGDataType[]`: `TY_UINT_1/INT_1 ... UINT_8`, `TY_SINGLE/DOUBLE/LONG_DOUBLE`; bool = `TY_UINT_1`; bit-field via `CGBitMask(name,start,width,type)` | correct widening, sign-aware compares, bit-field extract as mask/shift | cdatatyp.h:41-57 (cc/h), cgen.c:CGenType:2014-2017, 660-668 |
| pointer kind | `TY_POINTER`, `TY_NEAR_POINTER`, `TY_LONG_POINTER`, `TY_HUGE_POINTER`, `TY_CODE_PTR` from `__near/__far/__huge` and model | avoids segment-register loads for near pointers | cgen.c:PtrType:2035-2057 |
| aggregate size + alignment | `BEDefType(refno, align, size)` for every struct/union/array; `DGAlign` before each object | BE knows alignment of aggregates for aligned moves/SIMD-ish string ops | cgen.c:1995-1999, 2009, cgendata.c:AlignIt:51-65 |
| segment alignment grown to need | `SegAlignment[id]` = max alignment of symbols placed there, only when `OptSize==0` | segment starts aligned for widest member | cinfo.c:SetSegAlign:1033-1045, SetSegs:634-644 |
| data size threshold / near-far | `FLAG_FAR/HUGE` set by `SetFarHuge` from `DataThreshold` | near data stays near -> short addressing; `FE_ONESEG` never set so BE assumes huge for private/floating-DS segs | cinfo.c:142-198, cg/intel/c/x86segs.c:256-262 |
| floating segment registers | `FLOATING_DS/ES/FS/GS/SS` target switches, `FEINF_PEGGED_REGISTER` | BE may keep DS/ES/FS/GS pegged or free; saves reloads | cmdlnx86.c:1041-1140, cfeinfo.c:1169-1170 |
| memory model | `BIG_DATA/BIG_CODE/FLAT_MODEL/CHEAP_POINTER` | near vs far code/data; flat drops segment ops | cmdlnx86.c:986-1025 |
| CPU/FPU revision | `ProcRevision` CPU 386..686, FPU 387/586/686, inline vs emu | instruction selection, scheduling, inline FPU | cmdlnx86.c:192-324 |
| FPU stack depth | `FEINF_STACK_SIZE_8087` (8, or 4) | how many values the BE may keep on the FPU stack | cfeinfo.c:1092-1093, cg/intel/c/i87exp.c:1186 |
| code label alignment | `FEINF_CODE_LABEL_ALIGNMENT` | loop/proc alignment, used only pre-386 | cfeinfo.c:1112-1120, x86enc2.c:134-158 |
| unsafe-FP and rounding | `FP_UNSTABLE_OPTIMIZATION` (`-on`), `FPU_ROUNDING_OMIT/INLINE` (`-zro/-zri`), `I_MATH_INLINE` (`-om`) | algebraic FP rewrites, no rounding-mode fixups, inline math intrinsics | coptions.c:712-713, cmdlnx86.c:346-351, 614-615 |
| objective/size | `OptSize` 0/50/100 | time vs size heuristics | coptions.c:467-478 |
| loop / schedule / branch / flow | `LOOP_OPTIMIZATION`, `LOOP_UNROLLING`, `INS_SCHEDULING`, `BRANCH_PREDICTION`, `FLOW_REG_SAVES`, `SUPER_OPTIMAL`, `NULL_DEREF_OK` (`-oz`; BE use not read) | enables each pass | coptions.c:689-725 |
| stack-check elision | `FEStackChk` | BE asks per procedure (x86proc.c:319); FE answers from `SYM_CHECK_STACK` | cinfo.c:1002-1010, cdecl1.c:123 |
| dead functions | FE-side `PruneFunctions` before codegen; only used static functions reach the BE | fewer functions compiled | cgen.c:1783-1816 |
| zero data placement | uninitialised globals to `SEG_BSS` with `DGUBytes` (no file bytes); initialised zero data stays in DATA | no file bytes for BSS | cinfo.c:123-131, cgen.c:1050-1051 |
| string sharing | literals deduped (`reuse_duplicate_strings`), emitted once in SEG_CONST (ROM, BE may add its own data: `BACK`) | less data; BE addresses literal via `CGBackName` of literal as constant address | cstring.c:297-319, cinfo.c:634 |
| debug build cost | `-d2`/`-d3` force `NO_OPTIMIZATION` | FE asks for no optimisation when debugging | coptions.c:528, 541 |

Facts the FE does NOT give the BE (grep: no producer in bld/cc): restrict/no-alias (`FE_NOALIAS`), function purity beyond the two pragma flags, pointee constness, `FE_ADDR_TAKEN` for plain C `&x`, `FE_ONESEG`, `FE_COMMON`, loop trip counts, branch probabilities (only `-ob` global switch), value ranges.

### What the Open Watcom back end does with each fact

Paths in this subsection: `c/`, `intel/`, `risc/`, `h/` are under `cg/`; `cc/` is `cc/c/`.

#### Symbol attributes (`FEAttr`)

| fact | consumed at | what it enables | C FE | cite |
|---|---|---|---|---|
| FE_STATIC | `if( attr & FE_STATIC )` picks a global memory operand; otherwise a stack temp | autos (attr 0) become N_TEMP: register-allocatable, USE_ADDRESS tracked. Statics become N_MEMORY. | yes (SC_STATIC, SC_NONE, SC_EXTERN) | c/makeaddr.c:MakeAddrName:565 |
| FE_GLOBAL, FE_VISIBLE | `FEAttr & (FE_VISIBLE\|FE_GLOBAL)` with `_IsntModel(RELAX_ALIAS)` forces USE_MEMORY\|USE_ADDRESS | Without -oa every global/file-static is pinned to memory. With -oa they may be enregistered. | FE_GLOBAL yes; FE_VISIBLE only on SC_STATIC | c/dataflo.c:CheckGlobals:189-194; cc/cinfo.c:FESymAttr:217 |
| FE_GLOBAL, FE_VISIBLE | `(attr & (FE_GLOBAL\|FE_VISIBLE))==0 && RELAX_ALIAS` sets CST_OK_ACROSS_CALLS | A memory name that is neither global nor visible may stay in a register across calls. C never produces such an N_MEMORY name (statics are FE_VISIBLE), so this fires only for FE_CONSTANT in C. | no (see note) | c/conflict.c:AddConflictNode:92 |
| FE_VISIBLE | `have_call`: only `FEAttr & FE_VISIBLE` CG_FE memory is set VU_VARIANT | Loop-invariant hoisting of non-visible memory across a call. Same C caveat. | partial | c/loopopts.c:MarkInvariants:610 |
| FE_VISIBLE | `_IsModel(FORTRAN_ALIASING)`: `(attr & FE_VISIBLE)==0` returns MB_FALSE | A call cannot touch a non-visible name. Fortran only. | no | c/redefby.c:VisibleToCall:275-278 |
| FE_CONSTANT | `attr & FE_CONSTANT` sets VAR_CONSTANT | Name is never redefined. `NameIsConstant` makes `ReDefinedBy` return MB_FALSE for any instruction or call: CSE, copy propagation, scheduling across calls/stores, scoreboard reuse. | yes (`const`, not volatile) | c/makeaddr.c:MakeAddrName:578; c/redefby.c:NameIsConstant:246-259,ReDefinedBy:373 |
| FE_CONSTANT | `NameIsConstant(op)` skips `_SetLoopUsage(VU_VARIANT)` | Loads of const memory stay loop-invariant across calls and pointer stores (hoisted). | yes | c/loopopts.c:MarkInvariants:600,606 |
| FE_CONSTANT | `NameIsConstant(conf->name)` clears need_store | No store-back of a register-cached const. | yes | c/loadstor.c:280 |
| FE_CONSTANT | `attr & FE_CONSTANT` sets OK_ACROSS_CALLS | Const memory value kept in a register across calls. | yes | c/conflict.c:AddConflictNode:92 |
| FE_CONSTANT (via segment) | `AskNameIsROM` fallback in `NameIsConstant` | Any name in a ROM segment counts as constant even without FE_CONSTANT (string literals in SEG_CONST are BACK\|INIT\|ROM). See section 6. | yes | c/redefby.c:NameIsConstant:261 |
| FE_VOLATILE | `attr & FE_VOLATILE` sets VAR_VOLATILE\|NEEDS_MEMORY\|USE_MEMORY (static and auto) | Restricts only. Name never enregistered, never CSE'd, never deleted, never invariant. | yes (`volatile`, pragma-used, try-block vars) | c/makeaddr.c:MakeAddrName:572,596; cc/cinfo.c:FESymAttr:232-238 |
| FE_VOLATILE consumers | `VAR_VOLATILE` tests: ZapsTheOp returns MB_TRUE; CSE skips; InsDead keeps; scoreboard class SC_N_VOLATILE; loops skip | as above | - | c/redefby.c:ZapsTheOp:151-163; c/cse.c:ReCalcAddrTaken:94,DoArithOps:986; c/insdead.c:InitVisitedTemps:58,VolatileIns:156; c/scinfo.c:ScoreInfo:318,325; c/loopopts.c:MarkInvariants:525,532; c/regalloc.c:323 |
| FE_MEMORY | `attr & FE_MEMORY` sets NEEDS_MEMORY\|USE_MEMORY | Restricts only. `AddOne` returns NULL for USE_MEMORY names so no conflict node, no register. | yes, only with volatile/pragma/try | c/makeaddr.c:MakeAddrName:567,587; c/conflict.c:AddOne:105 |
| FE_ADDR_TAKEN | `attr & FE_ADDR_TAKEN` sets USE_ADDRESS on a local | Marks a local as reachable by pointer stores and calls. | only for pragma-used symbols (SYM_USED_IN_PRAGMA) | c/makeaddr.c:MakeAddrName:590; cc/cinfo.c:FESymAttr:232 |
| FE_ADDR_TAKEN | `ReCalcAddrTaken` clears USE_ADDRESS on every temp that lacks FE_ADDR_TAKEN, then `FindReferences` re-derives it from OP_LA | After copy propagation removes an address load, the temp becomes register-allocatable again. The BE derives USE_ADDRESS itself from OP_LA (`Use(name, USE_ADDRESS)`), so the FE bit is only needed for pragma-visible addresses. | see left | c/cse.c:ReCalcAddrTaken:97-101; c/varusage.c:SearchDefUse:387-391 |
| USE_ADDRESS (derived) | tested in alias rules: a pointer store zaps a temp only `if( op->v.usage & USE_ADDRESS )`; `ScoreStomp`; tail recursion bails if any temp has it; loops set VU_VARIANT | Non-address-taken locals are immune to pointer stores and calls: they live in registers across both. | derived in BE | c/redefby.c:ZapsTemp:128,ZapsIndexed:189,VisibleToCall:296; c/scinfo.c:ScoreStomp:94; c/trecurse.c:ScaryOperand:288; c/loopopts.c:MarkInvariants:571,595 |
| FE_NOALIAS | no test anywhere; only printed by the API echo | nothing | no | c/cg.c:334 (echo only; grep of c/ intel/ h/ risc/ finds no other use) |
| FE_UNIQUE | `attr & FE_UNIQUE` sets label status UNIQUE | Restricts: UniqueLabel blocks aliasing two function labels; one pad byte added. No speedup. | only with -ou | c/optask.c:AskForLabel:109; c/optlbl.c:UniqueLabel:143; c/optmain.c:161 |
| FE_INTERNAL | public/export decision only | none | yes (local statics) | intel/c/x86omf.c:1390; intel/c/x86owl.c:383 |
| FE_ONESEG | `attr & FE_ONESEG` in NamePtrType: returns TY_POINTER instead of TY_HUGE_POINTER for names in private segments | 16-bit: avoids segment arithmetic on pointers into that object. | no | intel/c/x86segs.c:NamePtrType:259 |
| FE_NAKED | `(attr & FE_NAKED)==0` around prolog/epilog; object.c emits only pragma calls | no prolog/epilog generated | yes | intel/c/x86proc.c:975,1005,1215; c/object.c:137 |
| FE_IMPORT | data emit skipped; call through import | emission only | yes | c/dg.c:208,541,552,1246; intel/c/x86enc2.c:392,402 |
| FE_VARARGS | no reader in bld/cg | nothing | yes | grep found none |
| FE_UNALIGNED | sets VAR_UNALIGNED; header says "no longer used" | nothing: VAR_UNALIGNED has no reader | no | c/makeaddr.c:MakeAddrName:575; h/name.h:80 |
| FE_PROC, FE_COMMON, FE_COMPILER, FE_THREAD_DATA, FE_DLLIMPORT, FE_DLLEXPORT | symbol classification for object emission, TLS, import thunks | no optimisation | partly | intel/c/x86omf.c:2415-2423,2541-2546; intel/386/c/386tls.c:193 |
| FE_UNINITIALIZED | not defined in this tree's cg.h | - | - | h/cg.h (fe_attr list) |

#### Call-class bits

Mapping path on x86: `x86reg.c` turns cclass into ROUTINE_* attrs; `x86call.c:BGCall` turns those into per-call `call_flags`.

| fact | consumed at | what it enables | C FE | cite |
|---|---|---|---|---|
| FECALL_GEN_NO_MEMORY_CHANGED -> CALL_WRITES_NO_MEMORY | `cclass & NO_MEMORY_CHANGED` -> ROUTINE_MODIFIES_NO_MEMORY -> call flag | Call is not a store: scoreboard keeps memory values (`MemChanged(..., (flags & WRITES_NO)==0)`); `VisibleToCall(modifies)` returns MB_FALSE; loop `MemChangedInLoop` stays false so loads hoist across the call; no reload/store around it (`savcode.h`); `BLK_CONTAINS_CALL` not set when both flags present. | only `#pragma aux ... modify nomemory` | intel/c/x86reg.c:110; intel/c/x86call.c:BGCall:199-203; c/scblock.c:DoScore:190-191; c/redefby.c:VisibleToCall:272-274; c/loopopts.c:MarkInvariants:546-548; c/loadstor.c:CheckRefs:99-100; c/varusage.c:SearchDefUse:377-378; h/savcode.h:165,173; cc/cpragx86.c:925 |
| FECALL_GEN_NO_MEMORY_READ -> CALL_READS_NO_MEMORY | `cclass & NO_MEMORY_READ` -> ROUTINE_READS_NO_MEMORY -> flag | Stores need not be flushed before the call; scheduler may move stores across it (`InsOrderDependant` skips the visibility test); no store-before-call in save code. | only `#pragma aux ... parm nomemory` | intel/c/x86reg.c:113; intel/c/x86call.c:BGCall:206; c/inssched.c:InsOrderDependant:440; c/varusage.c:377; h/savcode.h:174; cc/cpragx86.c:821 |
| FECALL_GEN_ABORTS | -> ROUTINE_NEVER_RETURNS_ABORTS | Caller: implies CALL_WRITES_NO_MEMORY; call emitted as `JMP` (no return address); `OC_NORET` marker follows. Callee: no return address on stack, no saved-register pushes, no epilog. No block-graph pruning after the call was found (no ABORTS/NORETURN test in c/). | yes (`#pragma aux aborts`, FLAG_ABORTS) | intel/c/x86reg.c:88; intel/c/x86call.c:BGCall:199,209; intel/c/x86enc2.c:GenCall:388-395,412; intel/c/x86proc.c:returnAddressStackSize:914, GenProlog:1068, 1216; cc/cfeinfo.c:getCallClass:505 |
| FECALL_GEN_NORETURN | -> ROUTINE_NEVER_RETURNS_NORETURN | Same as ABORTS for the caller side except the call stays a CALL. `OC_NORET` is a transfer instruction: peephole drops code after it (`IsolatedCode`), merges identical tails (`ComTail(NoRetList)`). | yes (`__declspec(noreturn)`) | intel/c/x86call.c:BGCall:199,212; intel/c/x86enc2.c:GenCall:412,GenCallIndirect:451; c/optins.c:307,378; c/optcom.c:TransformJumps:96; cc/cfeinfo.c:getCallClass:508 |
| FECALL_GEN_MAKE_CALL_INLINE | `call_inline` test | No call node is built: pragma byte sequence emitted in line, no call zap, no arg spill. Symbol is not emitted as a function. | yes (IsInLineFunc) | c/tree.c:2323; intel/c/x86omf.c:3394; cc/cfeinfo.c:getCallClass:515 |
| FECALL_GEN_PARMS_BY_ADDRESS | parm tree copied to a temp and its address passed | FORTRAN by-reference; nothing for C | no | c/tree.c:2336 |
| FECALL_GEN_SETJMP_KLUGE | `CALL_IS_SETJMP` in RISC only | AXP scheduler refuses to move registers past setjmp. Not implemented on x86. | no (C flags locals FE_VOLATILE via SYM_TRY_VOLATILE instead) | risc/axp/c/axpreg.c:102; c/redefby.c:VisibleToCall:296-305 (`#if AXP`); cc/cinfo.c:FESymAttr:235 |
| FECALL_GEN_HAS_VARARGS | only read in PPC | nothing on x86/386 | yes | risc/ppc/c/ppcreg.c:132 |
| FECALL_GEN_CALLER_POPS | clears ROUTINE_REMOVES_PARMS -> no CALL_POPS_PARMS; explicit `ADD SP` after the call | A caller-pops call with no OC_ATTR_POP can be turned into a tail `JMP` (`RetAftrCall`); callee-pops calls cannot. Also stack depth tracking. | yes (varargs, `__cdecl`) | intel/c/x86reg.c:85; intel/c/x86call.c:BGCall:193,268; c/optpush.c:RetAftrCall:60-62,69; intel/c/x86proc.c:1149 |
| FECALL_GEN_REVERSE_PARMS | argument order reversed | ABI only | `#pragma aux reverse` | c/tree.c:872 |
| FECALL_GEN_DLL_EXPORT | emission | none | yes | intel/c/x86omf.c:3453 |

Call-flag gate: an `OP_CALL` lacking either NO_MEMORY flag (i.e. unless both are present) makes `SearchDefUse` call `UseDefGlobals`, i.e. every global is treated as used and defined (c/varusage.c:SearchDefUse:375-380).

#### Register lists

| fact | consumed at | what it enables | C FE | cite |
|---|---|---|---|---|
| SAVE_REGS (callee preserves) | `state->modify = FULL minus *pregs`; `CallZap` = modify (+ parm regs + return reg + full-register widening unless MODIFY_EXACT) -> `call_ins->zap` | Registers outside zap survive the call: liveness, allocation and the scoreboard treat only the zap set as killed. Precise `modify [..]` lists on pragma functions keep more values in registers. `FECALL_X86_MODIFY_EXACT` drops the widening. | yes: list from the aux (`inf->save`); no code found that derives it from callee bodies | intel/c/x86reg.c:64-68,CallZap:259-270; c/bldcall.c:AssgnParms:887; c/scblock.c:DoScore:186-192; c/regalloc.c:787,853; c/liveinfo.c:FlowConflicts:300; cc/cfeinfo.c:1174-1188,381 |
| SAVE_REGS / modify for the routine being compiled | `MustSaveRegs` = FULL minus `CurrProc->state.modify` minus return/parm regs | Registers the routine declares it may clobber are not pushed/popped; allocator charges push+pop cost only for registers in must_save. | yes | intel/c/x86reg.c:MustSaveRegs:278-296; c/regalloc.c:CountRegMoves:553; intel/c/x86regsv.c:110 |
| SAVE_REGS -> SP | `!HW_Ovlap(*pregs, StackReg())` | Calls that modify SP force arguments to be computed before pushing (`MakeSPSafe`). | yes | c/tree.c:FunctionModifiesSP:2220 |
| PARM_REGS | copied into `state->parm.table`; `ParmReg` hands out registers by type class | Arguments in registers: no store/load through the stack; `#pragma aux parm [..]` picks exact registers. | yes (`inf->parms`, `DefaultVarParms` for varargs) | intel/c/x86reg.c:167-178; intel/c/x86parm.c:ParmReg:53; cc/cfeinfo.c:1195-1213 |
| RETURN_REG / STRETURN_REG | `FECALL_X86_SPECIAL_RETURN` picks the pragma register | Result returned in any register set, e.g. a pair, without moves. | yes | intel/c/x86reg.c:187,199 |
| FEINF_CALL_BYTES | non-NULL -> inline byte sequence, call treated as in-line | see MAKE_CALL_INLINE; also keeps FE_NAKED bodies to pragma calls | yes | c/tree.c:2321; c/object.c:143 |
| FECALL_X86_FAR_CALL etc. | far/near call, `RETF` | ABI; `AssgnParms` far flags | yes | c/bldcall.c:449 |
| FEINF_CODE_LABEL_ALIGNMENT | `AlignArray` from FE; `OptForSize>0` returns 1 | Loop/proc label alignment (16 on 486+, time mode only) | yes: `{2,1,1}`, `[1]=TARGET_INT` when OptSize==0 | intel/c/x86enc2.c:134-142; cc/cfeinfo.c:1112-1119 |

#### Volatile, unaligned, alignment, aggregate size

| fact | consumed at | what it enables | C FE | cite |
|---|---|---|---|---|
| CGVolatile (TF_VOLATILE) | TF_VOLATILE -> FL_VOLATILE on the address node -> X_VOLATILE on the index operand | Volatile dereference (through pointer) is never CSE'd, scheduled around, deleted or hoisted. Restricts. | yes (OPFLAG_VOLATILE; also float ops when `op_switch_used`) | c/tree.c:1592-1594; c/addrfold.c:167; c/redefby.c:ZapsTheOp:159,IsVolatile:397; cc/cgen.c:588,633,641,908,1311 |
| CGAttr(CG_SYM_VOLATILE) | same as CGVolatile | same | - | c/tree.c:TGAttr:1297 |
| CGAttr(CG_SYM_CONSTANT) | sets TF_CONSTANT | nothing: TF_CONSTANT is never read. FL_CONSTANT (-> X_CONSTANT) is never set anywhere, X_CONSTANT is never read. | no | c/tree.c:TGAttr:1300; h/tree.h:45; c/addrfold.c:170 |
| CGAttr(CG_SYM_UNALIGNED) | `alignment = 1` -> `X_ALIGNED_1` / `m.alignment` | x86: nothing reads it. RISC: `rscver.c` decides unaligned load/store sequences from it. | yes (OPFLAG_UNALIGNED, packed) | c/tree.c:TGAttr:1303; c/addrfold.c:173-191; risc/c/rscver.c:148-164 |
| CGAlign | sets `u1.t.alignment` | same as UNALIGNED (RISC only) | no | c/tree.c:TGAlign:1317 |
| BEDefType align | `TypeDef` keeps `align` only `#if _TARGET_RISC` (x86: `(void)align`); `ParmAlignment` returns 1 on x86 | x86: aggregate alignment has no effect on code. RISC: alignment of tree nodes of user types. | yes (`BEDefType(dtype, align, size)`) | c/types.c:TypeDef:205-212; intel/c/x86parm.c:ParmAlignment:45; c/tree.c:1582-1587; cc/cgen.c:1995 |
| BEDefType size | `TypeClass` -> `MapStruct(length)`: length 1/2/4 becomes U1/U2/U4 | A 1/2/4-byte struct local is a scalar temp (register candidate, single mov). Other sizes are XX: memory only, moved by `rep movs` or mov runs (`OptForSize>50` changes the choice). | yes | c/typemap.c:TypeClass:87-108; intel/386/c/386ptype.c:MapStruct:105-119; intel/c/x86split.c:280-290 |

#### Unroll count and signedness

| fact | consumed at | what it enables | C FE | cite |
|---|---|---|---|---|
| BEUnrollCount / `#pragma unroll(n)` | `UnrollValue` stored in the block started by the next label; `Head->unroll_count` read by `UnrollCount` | n>0 forces unrolling of that loop even without -ol+ (`CGSW_GEN_LOOP_UNROLLING` is only tested when count==0). Still needs -ol (TransLoops runs only under LOOP_OPTIMIZATION) and no -od. n=255 from the pragma means "max". | yes (cgen.c:1629-1631) | c/bldins.c:BGGenCtrl:382,BGUnrollCount:470; c/unroll.c:UnrollCount:301-303,UnRoll:1127; c/generate.c:PreOptimize:164-184; cc/cgen.c:GenOptimizedCode:1629; cc/cpragma.c:1437-1455 |
| Auto unroll (count 0) | `LOOP_UNROLLING` set (-ol+) and `OptForSize==0` and no switch in loop body; uses BLK_ITERATIONS_KNOWN | Unroll small counted loops fully or by a divisor of the trip count. | - | c/unroll.c:UnrollCount:303-320 |
| cg_type signed vs unsigned (I4 vs U4) | compare ops carry signed/unsigned via type class; `CheckCmpRange` folds compares of a narrow-source value against out-of-range constants using the original type; `a % 2^k` folded with a sign fix for signed types | Range-based compare folding. No "signed overflow is undefined" assumption found: `CalcFinalValue` computes trip counts from constants and rejects "wraps or exits immediately"; `DangerousTypeChange` refuses to swap induction vars of different signedness unless pointer-like; `ConstOverflowsType` bails on overflow. | yes | c/treefold.c:CheckCmpRange:112,1425-1441,1036-1056; c/loopopts.c:CalcFinalValue:2728-2753,DangerousTypeChange:2840-2854,ConstOverflowsType:2857-2934 |
| Pointer arithmetic type | `PointerOk` (PT, CP, U2 indexed temp) | induction-variable replacement allowed across signedness for pointers | yes | c/loopopts.c:PointerOk:2814-2832 |

#### Segment placement

| fact | consumed at | what it enables | C FE | cite |
|---|---|---|---|---|
| seg_attr ROM on a segment | `AskNameIsROM(sym,class)` -> `seg_is_rom` -> `rec->rom` from `seg->attr & ROM` | Names in ROM segments are constant for all alias queries (`NameIsConstant`, section 1). | SEG_CONST = BACK\|INIT\|ROM, SEG_CONST2 = INIT\|ROM | c/redefby.c:NameIsConstant:261; intel/c/x86omf.c:askNameIsROM:3438-3441,rec->rom:853-854; cc/cinfo.c:634-635 |
| const data goes to SEG_CONST2 | `SymSegId`: FE_CONSTANT and not `CompFlags.rent` -> SEG_CONST2 | makes const globals ROM; with `rent` (-zr) they stay in SEG_DATA but keep FE_CONSTANT | yes | cc/cinfo.c:SymSegId:290-296 |
| BACK segments | BE may place its own data (jump tables, FP constants) | - | yes | cc/cinfo.c:634 |
| CGSW_GEN_* ROM/ROMable | no switch for ROM code; nothing found | - | - | grep of cgswitch.h |

#### Switch lowering

| fact | consumed at | what it enables | C FE | cite |
|---|---|---|---|---|
| Case list (value, label), any order | `SortNodeList` sorts twice (signed, unsigned); `MergeListEntries` joins consecutive values with the same label into ranges | Dense and clustered cases collapse to ranges before costing. | `CGSelCase` per case, unsorted, no ranges, default via `CGSelOther` | c/bldsel.c:SortNodeList:140,MergeListEntries:165-190; cc/cgen.c:DoSwitch:963-969 |
| Choice of method | `BGSelect` computes ScanCost, JumpCost, DistinctIfCost (binary search) for signed and unsigned orderings and takes the cheapest (`cost <= best`) | jump table, scan table (`repne scas`), or binary if-tree | C uses `CGSelect` = all three allowed. `CGSelectRestricted`/`CG_SWITCH_*` never passed | c/bldsel.c:BGSelect:574-640; h/cg.h:CG_SWITCH_*; cc/cgen.c:969 |
| Cost model | JumpCost needs `num_cases>=MIN_JUMPS(4)` and `range>=4`; ScanCost needs `>=MIN_SVALUES(7)` (or 5 for 4-byte); `Balance(size,time)` blends by `OptForSize` with a floor of 25 | -os favours tables/scan (smaller); -ot favours binary search/table by time. | uses OptSize 0/50/100 | intel/c/x86sel.c:58-59,109-121,126-190 |
| Switch expression | `node + 0` temp inserted so a volatile selector is read once | correctness | - | c/bldsel.c:BGSelect:650-660 |
| Case frequency / profile | no input exists in the API | nothing | no | h/cg.h, c/bldsel.c |

#### Generic switches

| switch | consumed at | what it changes | cite |
|---|---|---|---|
| NO_OPTIMIZATION | `_IsModel(NO_OPTIMIZATION)` | `PreOptimize` and `PostOptimize` skip every optimisation pass; per-statement code generation (`BlockByBlock`); no FEMessage for peephole flush; peephole `optins/optrel/optcom` skip; frame uses `base_adjust=0`; all FE-named memory gets USE_MEMORY | c/generate.c:PreOptimize:152,PostOptimize:219,236,282,Generate:663; c/namelist.c:398; c/optins.c:152,287-379; c/optrel.c:135; c/trecurse.c:431; intel/c/x86proc.c:1073 |
| LOOP_OPTIMIZATION (-ol) | gate in PreOptimize | `TransLoops`, `LoopInvariant`, `CommonInvariant`, `IndVars` (strength reduction), `ReConstFold`, `LoopEnregister`; `SplitVars` later; UnRoll | c/generate.c:PreOptimize:160-196,Generate:726 |
| LOOP_UNROLLING (-ol+) | `UnrollCount` when no pragma | automatic unrolling | c/unroll.c:303 |
| INS_SCHEDULING (-or) | gate on `Schedule()` | instruction scheduling, then a second `PeepOpt` | c/generate.c:PostOptimize:287 |
| RELAX_ALIAS (-oa) | see "Alias" below | pointer stores stop killing globals and locals without an address; globals may be enregistered | c/conflict.c:92,165; c/redefby.c:ZapsMemory:90-93,ZapsIndexed:208-210; c/scinfo.c:ScoreStomp:88; c/dataflo.c:189; intel/c/i87sched.c:CheckTemp:621 |
| FORTRAN_ALIASING | alias rules | pointer derefs get a base from the pointer variable (`TNFindBase`), calls only touch visible names, extra NOPs record by-ref arg modification. C never sets it. | c/redefby.c:86,204,275; c/tree.c:1434,1501; c/bldcall.c:635; c/loopopts.c:560,593; c/inssched.c:364; c/generate.c:397; c/breakrtn.c:86 |
| NULL_DEREF_OK (-oz) | clears two folds | when NOT set: `PropNullInfo` uses a dereference as proof the pointer is non-null (folds later `p==0`); `&object != 0` folds to true | c/nullprop.c:492-494; c/treefold.c:1437-1446 |
| FP_UNSTABLE_OPTIMIZATION (-on) | | `x/c` -> `x*(1/c)` for any c (else only powers of 2), CSE of reciprocals | c/cse.c:OkToInvert:758; c/treefold.c:908 |
| FPU_ROUNDING_OMIT / INLINE | | omit or inline the FP rounding-mode save/restore around float-to-int | intel/c/i87exp.c:449-475 |
| I_MATH_INLINE (-om) | | inline 8087 math (sin, sqrt..) | intel/c/i87exp.c:890,901; intel/c/i87opt.c:373; c/loopopts.c:721 |
| SUPER_OPTIMAL (-oh) | | extra scoreboard tracking of register halves, deeper move counting in register allocation (slow) | c/scinfo.c:161,213,247; c/regalloc.c:532,539 |
| FLOW_REG_SAVES (-ok) | | push/pop of callee-saved registers placed on the flow (dominator) path, not always in prolog/epilog | c/flowsave.c:FlowSave:307; intel/c/x86proc.c:708-712 |
| BRANCH_PREDICTION (-ob) | gate in `SortBlocks`; also off if `OptForSize>50` | lays out blocks for static branch prediction | c/object.c:SortBlocks:810-814 |
| NO_CALL_RET_TRANSFORM | | disables call+ret -> jmp (tail call) | c/optpush.c:RetAftrCall:52 |
| MEMORY_LOW_FAILS | | when set, `ChkMemLimit` returns false (no peephole-queue flush under memory pressure); -oo clears it so the queue may be flushed. `AddCacheRegs` returns unless it is set. | c/memlimit.c:80; intel/c/x86proc.c:AddCacheRegs:863 |
| MICROSOFT_COMPATIBLE, POSITION_INDEPENDANT, DLL_RESIDENT_CODE | | code shape/ABI, RISC splitting | risc/axp/c/axpsplit.c:673; intel/c/x86proc.c:69 |
| DBG_LOCALS (-d2) | | disables the `a % 2^k` rewrite (keeps temps visible) | c/treefold.c:1036 |

#### Options to switches

| option | sets |
|---|---|
| -ox | clears NO_OPTIMIZATION; BRANCH_PREDICTION, I_MATH_INLINE, LOOP_OPTIMIZATION, INS_SCHEDULING; FE inlining (threshold 20); no stack check. Not -oa, -oh, -ok, -on, -oz, -ol+. |
| -od | NO_OPTIMIZATION |
| -ot / -os / default | OptSize 0 / 100 / 50, passed to `BEInit` as `OptForSize` (-ot and -os also clear NO_OPTIMIZATION) |
| -oa | RELAX_ALIAS |
| -ob | BRANCH_PREDICTION |
| -oh | SUPER_OPTIMAL |
| -ok | FLOW_REG_SAVES |
| -ol / -ol+ | LOOP_OPTIMIZATION / plus LOOP_UNROLLING |
| -on | FP_UNSTABLE_OPTIMIZATION |
| -oo | clears MEMORY_LOW_FAILS |
| -or | INS_SCHEDULING |
| -oz | NULL_DEREF_OK |
| -ou | FE sets FE_UNIQUE on functions |
| default (no -o) | GenSwitches starts as MEMORY_LOW_FAILS only (cmdlnx86.c:74); -d1/-d2 variants set NO_OPTIMIZATION (coptions.c:524,528,541). Whether plain default runs the optimiser: not traced. |

Cites: cc/coptions.c:444-478,683-725; cc/cgen.c:1924 (`BEInit(GenSwitches, TargetSwitches, OptSize, ...)`); c/intrface.c:BEInitCg:129.

#### Size against time

| threshold | consumed at | effect |
|---|---|---|
| >0 | c/unroll.c:UnrollCount:305; c/loopopts.c:2392; intel/c/x86ldstr.c:362; intel/c/x86enc2.c:138 | no auto unroll, no small-loop unroll, no Pentium load/store pairing pass, no code alignment |
| >=50 / >50 | c/inssched.c:261; c/cse.c:625; c/encode.c:64,82; c/optrel.c:263; c/object.c:814; intel/c/x86mul.c:45; intel/c/x86split.c:284; intel/c/x86ldstr.c:431; intel/c/i87opt.c:503,513; intel/c/x86proc.c:496,630,634,799,865; intel/i86/c/i86opseg.c:119 | prefer short encodings, MUL over shift/add, `leave`, no hoisting out of switches, no label alignment, no branch-layout, no EBP freeing |
| <50 / <25 | c/optpull.c:143; c/optcom.c:245; c/loopopts.c:3524 | clone code into jump targets, allow jump transformations, loop inversion when pre-header precedes |
| ==100 | intel/c/x86enc.c:1606 | stack touch uses push/pop |

#### Back-end passes and what each needs

Order from `PreOptimize`/`PostOptimize`/`Generate` (c/generate.c:152-300,640-740).

| pass | where | inputs from the FE |
|---|---|---|
| Move/address constant propagation (`MakeMovAddrConsts`, `KillMovAddrConsts`) | c/addrcnst.c:43 | none |
| `PushPostOps` (untangle `*p++`) | c/optimize.c:351 | none |
| `DeadTemps`, `InsDead` | c/optimize.c:209; c/insdead.c:378 | VAR_VOLATILE, USE_ADDRESS (insdead.c:58) |
| `CommonSex`: copy/constant propagation, CSE, `LoadAddr`, reciprocal, invariant exprs | c/cse.c:1587 (loop: `ReCalcAddrTaken`, `DoPropagateMoves`, `PropagateExprs`) | alias rules (`ReDefinedBy`): FE_CONSTANT/ROM, USE_ADDRESS, VAR_VOLATILE, call flags, RELAX_ALIAS, FP_UNSTABLE |
| `SetOnCondition` (setcc) | intel/c/386setcc.c:191 | none |
| `BlockTrim`, `AxeDeadCode` | c/blktrim.c:493; c/optimize.c:274 | none |
| Loop invariant motion (`LoopInvariant`, `CommonInvariant`) | c/loopopts.c:3251,965 | FE_CONSTANT, FE_VISIBLE, call flags, USE_ADDRESS, volatile, RELAX_ALIAS indirectly via ZapMemory |
| Induction variables / strength reduction / loop inversion (`IndVars`, `TransLoops`, `Induction`) | c/loopopts.c:3696,3703,3676 | type class signedness, constants, OptForSize |
| Unrolling (`UnRoll`) | c/unroll.c:1093 | BEUnrollCount, LOOP_UNROLLING, OptForSize |
| Loop register caching (`LoopEnregister`, `LoopRegInvariant`) | c/loopopts.c:3265,3258 | alias rules; RELAX_ALIAS for memory names |
| `MulToShiftAdd` | c/multiply.c:232 | OptForSize (x86mul.c cost) |
| `PropNullInfo` | c/nullprop.c:483 | NULL_DEREF_OK off |
| Tail recursion | c/trecurse.c:410 (generate.c:713) | no USE_ADDRESS temps; not BlockByBlock |
| `SplitVars` | c/splitvar.c (generate.c:726) | LOOP_OPTIMIZATION |
| `ConstToTemp`/`MemConstTemp` (constant caching; file header says purpose unknown) | c/cachecon.c:168,200 | none seen |
| `AddCacheRegs` (386: ESP frame, frees EBP) | intel/c/x86proc.c:857-899 | MEMORY_LOW_FAILS set, OptForSize<=50, no FLOATING_DS/SS, not Windows, `lex_level==0` |
| Register allocation | c/regalloc.c; conflicts c/conflict.c | zap sets, must-save, USE_MEMORY/NEEDS_MEMORY, parm/return regs, SUPER_OPTIMAL |
| Load/store placement (`LdStAlloc`, `LdStCompress`) | intel/c/x86ldstr.c:405,647; c/loadstor.c | call flags, NameIsConstant, USE_ADDRESS |
| Scoreboard (`Score`): redundant load/move elimination | c/scmain.c:267; c/scblock.c:DoScore:130 | `ScoreStomp` alias rule (RELAX_ALIAS, USE_ADDRESS), call zap, WRITES_NO_MEMORY, SC_N_VOLATILE |
| `Conditions` (drop redundant compares) | c/condcode.c:348 | none |
| Scheduler | c/inssched.c:1067 | INS_SCHEDULING, call flags, `ReDefinedBy`, FORTRAN_ALIASING |
| Peephole (`PeepOpt`, plus `optins/optcom/optpull/optrel` on the object queue): jump threading, tail merge, code cloning, call+ret->jmp | c/peepopt.c:710; c/optcom.c; c/optpull.c; c/optpush.c:46 | OptForSize, NO_CALL_RET_TRANSFORM, CALLER_POPS, UNIQUE labels |
| Flow-based register save placement | c/flowsave.c:289 | FLOW_REG_SAVES, dominator info |
| FP optimisation (`FPExpand`, `FPOptimize`, 87 scheduling) | intel/c/i87opt.c:812; intel/c/i87sched.c:621 | RELAX_ALIAS, I_MATH_INLINE, ROUNDING, FP_UNSTABLE |
| Tree folding (`ConstFold`) | c/treefold.c | type signedness, FP_UNSTABLE, NULL_DEREF_OK |
| Switch lowering | c/bldsel.c:574; intel/c/x86sel.c | CGSelCase list, OptForSize |
| Operand overlap (`overlap.c`) | c/overlap.c:99 | not alias analysis: decides whether a result operand overlaps an operand inside one instruction (register allocation/peephole). No FE input. |
| Alias analysis proper | c/redefby.c (`ReDefinedBy`, `ZapsMemory`, `ZapsTemp`, `ZapsIndexed`, `VisibleToCall`) | see "Alias rules" below |

#### Alias rules (c/redefby.c), summarised

| query | answer | needs |
|---|---|---|
| store through pointer with no known base vs N_MEMORY name | kills it if `USE_ADDRESS`; else kills it unless RELAX_ALIAS (Fortran: never) | -oa to spare globals; USE_ADDRESS (derived) |
| store through pointer vs N_TEMP | kills only if temp is `USE_ADDRESS` | BE-derived |
| store through pointer with a fake base (`&a+i`) vs another named object | distinct symbol -> no kill | `TNFindBase` (tree.c:1470-1510); for C only bases from `&object` arithmetic (pointer variables need FORTRAN_ALIASING) |
| store to name vs same name | `TempsOverlap` byte-range test | BEDefType size, offsets |
| call vs N_MEMORY | kills unless WRITES_NO_MEMORY; Fortran also spares non-visible | call class |
| call vs N_TEMP | kills only if `USE_ADDRESS` | BE-derived |
| anything vs FE_CONSTANT / ROM name | never | FE_CONSTANT or ROM segment |
| anything vs volatile | always | FE_VOLATILE / CGVolatile |

#### Accepted by the back end, never sent by the C front end

| fact | status in BE | what supplying it would enable | grep evidence |
|---|---|---|---|
| FE_NOALIAS | defined; no reader | would allow skipping `USE_ADDRESS`-style kills for that name; needs a new reader | cc: no match; cg: only c/cg.c:334 echo |
| CG_SYM_CONSTANT (CGAttr) | sets TF_CONSTANT, which nothing reads; FL_CONSTANT never set | would mark a dereference (e.g. `const T *p; *p`) as invariant via X_CONSTANT; needs readers for X_CONSTANT in `ReDefinedBy`/loops | cc: no CG_SYM_CONSTANT; cg: tree.h:45, tree.c:1301, addrfold.c:170 |
| FE_ONESEG | used for 16-bit pointer type | cheaper pointers to objects known to fit one segment | cc: no match |
| FE_ADDR_TAKEN (general) | used (USE_ADDRESS seed, ReCalcAddrTaken) | C sets it only for pragma symbols; `SYM_ADDR_TAKEN` exists in cc but is not forwarded. BE re-derives from OP_LA, so little lost | cc/cinfo.c:232 only |
| FE_COMMON, FE_COMPILER | emission | COMDAT/inline-function dedup | cc: no match |
| FE_VISIBLE on non-static, FE_GLOBAL absence | see section 1 | `conflict.c:92` and `loopopts.c:610` "not visible" cases never hit in C | cc/cinfo.c:FESymAttr |
| FORTRAN_ALIASING | full alias model | pointer/parameter bases (`restrict`-like); C has `FLAG_RESTRICT` parsed (ctype.c:238-240) but nothing is emitted | cc: no CGSW_GEN_FORTRAN_ALIASING |
| FECALL_GEN_NO_MEMORY_READ / CHANGED | used | only via `#pragma aux nomemory`; no inference from function bodies, no `__attribute__((pure/const))` | cc/cpragx86.c:821,925 |
| FECALL_GEN_PARMS_BY_ADDRESS, SETJMP_KLUGE | used (Fortran / RISC) | - | cc: no match |
| CGSelRange, CGSelectRestricted, CG_SWITCH_* | used | range cases from GNU `case a ... b`, or forcing a method | cc/cgen.c:963-969 uses only CGSelCase |
| CGAlign | used on RISC | - | cc: no match |
| FEINF_SAVE_REGS from callee analysis | list comes from aux declaration | per-callee real clobber sets (interprocedural register allocation) | cc/cfeinfo.c:1174 |
| Case frequency, branch probabilities, hot/cold | no API | - | h/cg.h |
| Signed-overflow-undefined (no-wrap) flag | no API; BE never assumes it | would let `IndVars`/`CalcFinalValue` drop wrap guards and widen trip-count analysis | c/loopopts.c:2728-2753 |

### Surprises in the CG calls

1. Many API calls are never used: no `CGIndex`, `CGWarp`, `CGSelRange`, `CGBigGoto`, `CGTrash`, `CGType`, `CGDuplicate`, `CGCallback`, `CGPatchNode`, `CGLVPreGets`, `BEAliasType`. Array indexing is explicit `O_TIMES` by element size plus `O_PLUS` (cgen.c:717,720).
2. There is exactly one `CGReturn`, at function end (cgen.c:289,295); `return e;` only assigns to a return temp (cgen.c:319).
3. Structs/arrays/complex are opaque `BEDefType(refno, align, size)` with no fields; a struct with a trailing zero-length array gets a new refno on every use (cgen.c:1989-1995). Struct values are `O_POINTS` typed by refno, copy is `CGLVAssign` with no size argument (cgen.c:1315).
4. Volatile is forced onto every float/double/long double access under `-op` (cgen.c:349-351); `CG_SYM_CONSTANT` and `CG_SYM_VOLATILE` are never used, and const reaches CG only as `FE_CONSTANT` plus segment placement (cinfo.c:237-241, 294-295).
5. `__based`, `__far16` and far-pointer building are lowered in the FE to ordinary `O_CONVERT` (two operands: offset, segment), `O_PLUS`, `O_PTR_TO_NATIVE`; call class, registers, and inline decisions never appear as CG call arguments - the back end pulls them through `FEAuxInfo`/`FEAttr`.

### Surprises in the DG and FE interface

1. `FE_ADDR_TAKEN` is set only by `#pragma aux` use; a C `&x` on an auto never reaches the BE through `FEAttr` (cinfo.c:231-233). `FE_NOALIAS/COMMON/ONESEG/UNALIGNED/COMPILER` are never set; autos have `attr == 0`.
2. Only 17 FE callbacks exist (cgfertns.h:33-49); `FEMoreMem`=0, `FELexLevel`=0, `FETrue`=1. No `FEStackModel/FETrashHere/FEStkSize/FEDbgInfo/FECodeBytes`.
3. Padding and bit fields never reach DG*: padding is zeroed `DGIBytes` runs; a bit-field initialiser is read-modified-written in the quad list and emitted as one whole-unit `DGInteger`. `DGUBytes` is used only for `SEG_BSS`. Of 16 DG calls, bld/cc uses 9 (`DGLabel, DGInteger, DGInteger64, DGBytes, DGIBytes, DGUBytes, DGAlign, DGFEPtr, DGBackPtr`).
4. A `-fld` `long double` initialiser goes through `StoreFloat(TYP_DOUBLE)`, so the emit path writes 8 bytes (`DGBytes(TARGET_DOUBLE)`) for a 10-byte slot; `QDT_LONG_DOUBLE` is emitted by cgendata.c:207 but cdinit never produces it (read, not run).
5. `BEInit`'s result is only tested for non-zero (revision/target never compared), `SC_FORWARD` is reported as import, and on 32-bit flat/far targets every hidden static temp (`.X` aggregate-init copies) lands in `SEG_CODE` (cinfo.c:323-326).

## 2. Worked out and not passed on

### Constant folding

| fact | computed where | passed on? | re-derived by CG? | cite |
|---|---|---|---|---|
| int +,-,*,/,%,>>,<<,\|,&,^,&&,\|\|, unary -,~,! on two constant leaves | `DoConstFold` after children (postfix), at `AddStmt` and in `RelOp`/`BoolConv`/`BracketExpr` | yes: one `OPR_PUSHINT` leaf | yes: `TGBinary` runs `BinFold` (`FoldPlus`, `FoldTimes`, ...) on every node again | cfold.c:DoConstFold:1288, DoOp32:44, DoSignedOp:204, DoUnSignedOp:263; cstmt.c:AddStmt:151; cg tree.c:TGBinary:753, treefold.c:FoldPlus:494 |
| 64-bit int folding | same, separate routines | yes: `OPR_PUSHINT` with 64-bit const_type | yes (CG folds 64-bit consts via `CF*`) | cfold.c:DoSignedOp64:459, DoUnSignedOp64:402 |
| div/mod by constant 0 | folds to 0 and warns | folded 0 passed | n/a | cfold.c:DoOp32:44 (DIV/MOD right==0), CheckOpndValues:1160 |
| shift by negative or >= width | warning only; host `<<`/`>>` used for the value | folded value passed | n/a | cfold.c:CheckOpndValues:1160, DoOp32:44 |
| int comparisons of two constants | folded to int 0/1 | yes: `OPR_PUSHINT` | yes: `FoldCompare` | cfold.c:DoSignedOp:204 (OPR_CMP), DoUnSignedOp:263; cg treefold.c:FoldCompare:1349 |
| float +,-,*,/,unary -, and comparisons | `DoFloatOp` in host long double soft-float (`__FLDA` etc.) | yes: `OPR_PUSHFLOAT` holding binary `ld`; emitted to CG as a decimal string via `ftoa` | yes: CG re-parses string, folds again | cfold.c:DoFloatOp:664, cgen.c:PushConstant:744 |
| float op result rounding | operands rounded to float/double by `MakeBinaryFloat`; the result is stored as long double, not re-rounded to the expression type | string passed is the long-double value | CG converts to target type on `CGFloat` (not verified how) | cfold.c:MakeBinaryFloat:614, DoFloatOp:664 (tail) |
| float ops not folded: `%` n/a, `OPR_MATHFUNC` (sin, sqrt, ...) | never folded | `OPR_MATHFUNC` node | CG `FoldSqrt`/`FoldLog` only | cfold.c:FoldableTree:996; cg tree.c:TGUnary:773 |
| `OPR_CONVERT` of constant | folded in place (`CastConstNode`), type set | yes: constant leaf | n/a | cfold.c:FoldableTree:996 (OPR_CONVERT case), CastConstNode:921, CastConstValue:860 |
| `,` with constant left; `?:` with constant cond; `&static ? a : b` (cond replaced by 1) | folded to the chosen arm | yes: only the chosen arm exists | yes for cond constants in flow | cfold.c:FoldableTree:996, FoldQuestionTree:963; cexpr.c:TernOp:2661 |
| `&&` / `\|\|` with constant operand | constant-true/false operand dropped, result is the other operand; constant-decided short circuit skips parsing right side codegen via `SizeOfCount` | yes: tree shrinks; `OPR_OR_OR` has a label index | yes (`FoldFlAnd`/`FoldFlOr`) | cmath.c:FlowOp:968; cexpr.c:OrOr:2524, AndAnd:2539; cg treefold.c:FoldFlAnd:1155 |
| offsetof pattern `&((T*)0)->f` | folded to `UINT` constant | yes | n/a | cfold.c:FoldableTree:996 (OPR_ADDROF case) |
| far pointer `seg :> off` of constants (8086 only) | folded to 64-bit pointer const | yes | no | cfold.c:FoldableTree:996 (OPR_FARPTR, `#if _CPU == 8086`) |
| address constants (`&sym + k`) in expressions | not folded by cfold (leaves are only PUSHINT/PUSHFLOAT) | `OPR_ADD` of `PUSHADDR` and `PUSHINT` goes to CG | CG (not checked which routine folds sym+k) | cops.h:IsConstLeaf:33 |
| address constants in static initialisers | `AddrFold` walks ADD/SUB/INDEX/DOT/ARROW/ADDROF/CONVERT into sym+offset | yes: data quad with sym handle and offset | no: CG sees `DGFEPtr(sym, type, offset)` | cdinit.c:AddrFold:372; cgendata.c:EmitDQuad (QDT_POINTER) |
| sizeof(x), array dimension, case value, bit width, enum value | `ConstExprAndType` needs a `PUSHINT` | yes: constants | n/a | cexpr.c:ConstExprAndType:1174; cdecl2.c:ArrayDecl:1319 |
| meaningless compare ("always true/false", unsigned vs 0) | value/type range check, warning only | no: compare node stays | CG `FoldCompare` may fold by range (not checked) | cmath.c:IsMeaninglessCompare:622, RelOp:844 |

### Integral promotions and usual arithmetic conversions

| fact | computed where | passed on? | re-derived by CG? | cite |
|---|---|---|---|---|
| common type of binary op | `BinResult[][]` table; `PUS` = int or uint by target; result forced `>= INT` in `BinOp` only | yes: node `expr_type`/`result_type` becomes the `cg_type` of `CGBinary` | n/a | cmath.c:BinResult:256, BinOp:1374, cgen.c:EmitNodes:1274 (OPR_MUL..LSHIFT) |
| conversion nodes for operands of binary ops | `Convert()` is a macro that returns its operand: no `OPR_CONVERT` inserted | NOT passed: operands keep their own types | YES: `TGBinary` -> `BinResult` converts both operands to the result type | cmath.c:553 (`#define Convert ... opnd`), cg tree.c:BinResult:500, TGBinary:753 |
| shift: left operand type only (right not converted) | `ShiftResult[]` | result type = promoted left type | CG | cmath.c:ShiftOp:1613 |
| compare type | `BinExprType` of the operands, so two `char`s compare as `char` (not promoted to int); ptr vs ptr uses the larger pointer | yes: `CGCompare(..., compare_type)` | CG converts operands | cmath.c:RelOp:844; cgen.c:EmitNodes (OPR_CMP) |
| compound assign `op=` | result type = type of lvalue; RHS not converted to the wider common type | `CGPreGets(op, lv, rhs, lvalue type)` | CG converts RHS to lvalue type; the C rule (compute in common type, then narrow) is not expressed in the tree (read from code; not tested) | cmath.c:BinOp:1374 (`result_type = op1_type`), AddOp:1210; cgen.c:EmitNodes (OPR_PLUS_EQUAL...) |
| unary `-`, `~` promotion | `SubResult[t][t]` | result type on node | CG `TGConvert` | cmath.c:UMinus:1992; cg tree.c:TGUnary:773 |
| explicit casts and assignment/arg/return conversions | `CnvOp`, `FixupAss`, `AsgnOp`: `OPR_CONVERT` when types differ (or `OPR_CONVERT_PTR` for far16/foreign ptrs, `OPR_CONVERT_SEG`) | yes: `CGUnary(O_CONVERT, .., result_type)` | CG folds consts (`FoldCnvRnd`), picks machine ops | cmath.c:CnvOp:1737, FixupAss:1929, AsgnOp:1484; cgen.c:EmitNodes (OPR_CONVERT) |
| conversion class table (P2P, P2A, A2P, S2B, ...) | `CnvTable[from][to]` | lost; only the resulting convert node | n/a | cmath.c:CnvOp:1737 |
| `_Bool` conversion | `x ? 1 : 0` tree built in FE | yes as `?:` | CG | cmath.c:BoolConv:793 |
| default argument promotion (float->double, near->far ptr) | only when no prototype | `OPR_CONVERT` | CG | cexpr.c:GenNextParm:1971 |
| `char`/`short` parameters are widened to `int` for the call | CG hook | n/a (CG asks) | FE answers via `FEParmType` | cinfo.c:FEParmType:940 |
| pointer + int scaling | FE multiplies index by `sizeof(*p)` (`MulByConst`; folded if const) | yes: already scaled `OPR_MUL`/`PUSHINT` | CG sees byte offsets, not element size | cmath.c:MulByConst:1013, AddOp:1210 |
| pointer - pointer | FE emits SUB then `>>k` or `/size` | yes | no | cmath.c:PtrSubtract:1068 |
| array index | `OPR_INDEX` node; scaling by element size done in CG driver | `CGBinary(O_TIMES, idx, size)`, `O_PLUS` | CG | cgen.c:IndexOperator:693 |
| array-to-pointer, function-to-pointer decay | `TakeRValue` wraps `OPR_ADDROF` (no-op in cgen) | address value | n/a | cexpr.c:TakeRValue:531; cgen.c:EmitNodes (OPR_ADDROF: break) |
| lvalue vs rvalue | `OPFLAG_RVALUE` on DOT/ARROW/INDEX/POINTS/CALL; cgen adds `O_POINTS` load | yes: `CGUnary(O_POINTS)` | CG | cexpr.c:TakeRValue:531; cgen.c:PushRValue:627 |

### sizeof, offsetof, struct layout

| fact | computed where | passed on? | re-derived by CG? | cite |
|---|---|---|---|---|
| `sizeof` value | `SizeOfArg` -> `TypeSize`; replaced by `UINT` leaf at parse time; operand freed, `SizeOfCount` suppresses warnings/codegen | yes: constant only | no | cexpr.c:SizeofOp:2776, TC_SIZEOF in GetExpr:1252; csizeof.c:SizeOfArg:34 |
| struct/union size | sum with `_RoundUp(size, worst_alignment)` | `BEDefType(refno, align, size)`: opaque sized type | no: CG knows size and alignment only | ctype.c:GetFields:1022; cgen.c:CGenType:1975 |
| field offsets | `FieldAlign` per field; offset stored in field | yes: integer leaf in `OPR_DOT`/`OPR_ARROW` right child; nested `.` offsets are summed into one node | no: CG does `O_PLUS base, offset`, no field identity | ctype.c:FieldAlign:892; cexpr.c:DotOp:1060 (sum at 1096), ArrowOp:1114; cgen.c:DotOperator:649 |
| `-zp` / `#pragma pack` | `PackAmount` caps each field's alignment (`align > pack` => pack) and is part of `worst_alignment` | only via resulting offsets, struct size and the `align` given to `BEDefType` | no | ctype.c:FieldAlign:892; cmdlnx86.c:77-82 (defaults) |
| alignment of a packed field's address | implicit in offset | not passed (only `__unaligned` => `CG_SYM_UNALIGNED`) | CG assumes the type's natural alignment | cgen.c:DotOperator:649 |
| bitfield layout (unit, start, width) | `GetFields` packs into storage unit of the declared type; start/width into `TYP_FIELD` type (`field_start`, `field_width`) | yes: `CGBitMask(addr, start, width, type)`; `_Bool` width forced 1 | no: CG does the mask/shift | ctype.c:GetFields:1022, EnumFieldType:838; cgen.c:DotOperator:649 |
| bitfield type promotion | field type is `TYP_FIELD`/`TYP_UFIELD` with signedness | `cg_type` via `CGDataType[field_type]` | CG | cgen.c:CGenType:1975 |
| struct alignment | `tag->alignment = worst_alignment` | yes: `BEDefType` align arg; `AlignIt` for data | no | ctype.c:GetTypeAlignment:871; cgen.c:CGenType:1975 |
| union members all offset 0 | `next_offset = start` per member | offsets only | no | ctype.c:GetFields:1022 |
| flexible/zero-length last array | `typ->object != NULL` makes a fresh refno per use | sized type | no | cgen.c:CGenType:1975 |
| array of unknown dim sized from initialiser | `StaticInit` re-types symbol | size only | no | cdinit.c:StaticInit:1215 |

### Evaluation order and sequence points

| fact | computed where | passed on? | re-derived by CG? | cite |
|---|---|---|---|---|
| operand order | tree is walked left, right, node; nodes become CG calls in that order | CG builds one expression tree per statement; order is fixed only where CG semantics say so | CG is free to reorder unsequenced operand evaluation inside one tree | treewalk.c:WalkExprTree:37; cgen.c:EmitNodes:1274 |
| `,` | `OPR_COMMA` node; left made rvalue | `CGBinary(O_COMMA)` | CG keeps sequencing | cexpr.c:GetExpr:1252 (TC_COMMA); cgen.c:EmitNodes (OPR_COMMA) |
| `&&`, `\|\|` | `OPR_AND_AND/OR_OR` with a label index; operands forced boolean by `BoolExpr` (`!= 0` compare) | `CGFlow(O_FLOW_AND/OR)` | CG builds control flow | cmath.c:FlowOp:968; cexpr.c:BoolExpr:2554; cgen.c:EmitNodes |
| `?:` | `OPR_QUESTION`/`OPR_COLON`, 2 labels reserved; struct arms via pointers | `CGChoose` | CG | cexpr.c:TernOp:2661; cgen.c:EmitNodes (OPR_QUESTION) |
| `!x` | `OPR_NOT` | `CGFlow(O_FLOW_NOT)` | CG | cexpr.c:NotOp:2589 |
| `i++`, `i--` | `OPR_POSTINC/DEC` | `CGPostGets` | CG picks when the update happens (unspecified by C) | cgen.c:EmitNodes (OPR_POSTINC) |
| function argument order | parms chained in tree order, reversed for right-to-left conventions (`ParmsToBeReversed`); first parm built last when reversed | `CGAddParm` order = tree order | CG may reorder side-effect-free parts; FE makes no sequence-point claim | cexpr.c:GenFuncCall:2193 (2244), GenNextParm:1971; cfeinfo.c:ParmsToBeReversed:314 |
| `-wo`, `-ec` | no such handling found (searched cexpr.c, cmath.c, cgen.c for order options) | n/a | n/a | not found |
| statement end | value left on the CG stack is discarded | `CGDone(PopCGName())` at end of `EmitNodes` if the stack is non-empty | CG | cgen.c:EmitNodes:1603 |
| `CGEval` | used only for `OPR_DUPE` (far16 temp used on both sides) | once | n/a | cgen.c:EmitNodes:1454 |
| `CGTrash` | defined only as a stub; FE never calls it | no | n/a | cgstub.c:71 |
| volatile access | `OPFLAG_VOLATILE` from pointer/symbol cv bits | `CGVolatile(name)` on each access | CG will not cache or drop it | cgen.c:PushSym:565, PushRValue:627, EmitNodes (assign ops) |
| "useful side effect" / meaningless statement | `CompFlags.meaningless_stmt`, `useful_side_effect` | no | no | cexpr.c:GetExpr:1252; cstmt.c:CheckUseful:93 |

### Inlining and intrinsics

| fact | computed where | passed on? | re-derived by CG? | cite |
|---|---|---|---|---|
| function is inlinable | `FUNC_OK_TO_INLINE` set if `-oe`/`inline` toggle or `inline` keyword, not `naked`, not `main`; cleared by: body node count > `Inline_Threshold`, varargs, any string literal, any local static ref, address of local in pragma, `_try` | as `FECALL_GEN_MAKE_CALL_INLINE` in `FEAuxInfo(CALL_CLASS)` | no: CG asks, FE decides | cstmt.c:GenFunctionNode:177 (190), Statement:1188 (1475-1479), cstring.c:StringLeaf:272 (327), csym.c:931, cstmt.c:TryStmt:880 (887); cfeinfo.c:getCallClass:491 (514); cgen.c:IsInLineFunc:1709 |
| inline expansion itself | FE re-walks the callee's saved statement tree inside the caller via `FEGenProc` -> `GenInLineFunc` | yes: CG API calls (nesting depth `MAX_INLINE_DEPTH`) | CG manages inline frames (`BGStartInline`) | cinfo.c:FEGenProc:267; cgen.c:DoInLineFunction:1664; cg inline.c:BGStartInline:65 |
| function never needs emitting | `PruneFunctions` marks from non-static and address-taken statics; callees reached via `ScanFunction` (simulating inlining) | unmarked bodies skipped in `GenOptimizedCode` | no | cgen.c:PruneFunctions:1783, ScanFunction:1728, GenOptimizedCode:1608 (1640) |
| math intrinsics (`sin`, `sqrt`, `pow`, ...) | `GenFuncCall` matches `__SIN`-style name, or plain name if `#pragma intrinsic`; only with optimisation and extensions on | `OPR_MATHFUNC/2` -> `CGUnary/CGBinary(O_SIN..., TY_DOUBLE)` | CG emits x87 sequence | cexpr.c:GenFuncCall:2193 (2288-2304), MathFuncs:54; cmathfun.h; cgen.c:EmitNodes (OPR_MATHFUNC) |
| `#pragma intrinsic` | sets `SYM_INTRINSIC` | used in name match and `InfoLookup` | no | cpragma.c:pragIntrinsic:1322; cfeinfo.c:InfoLookup:331 |
| `_inline_xxx` / intrinsic string and x87 byte-sequence functions | `IF_Lookup` picks code bytes per model/CPU/`-os` | yes: aux info with `code`, `parms`, `returns`, `save` | CG splices bytes | cfeinfo.c:InfoLookup:331, IF_Lookup:217 |
| `-oi` | sets `CompFlags.inline_functions`, defines macro `__OI`; headers then use `#pragma intrinsic`. No FE logic tested on the flag beyond that (`cmodel.c:254` not read) | via pragma | no | coptions.c:701; cmdlnx86.c:830 |
| `_inline_strcmp` with literal | replaced by memcmp call with length parm | call | no | cexpr.c:GenFuncCall:2193 |
| `alloca` | `__builtin_alloca` becomes `OPR_ALLOCA` only `#if _RISC_CPU`; x86 path has no FE handling in cc | RISC: `O_STACK_ALLOC` | x86: a normal call (library/pragma) | cexpr.c:GenAllocaNode:2128; cgen.c:EmitNodes:1588 |
| `FEAttr` bits sent | `FE_PROC/GLOBAL/IMPORT/STATIC/VISIBLE/INTERNAL/VARARGS/UNIQUE/MEMORY/VOLATILE/CONSTANT/NAKED/THREAD_DATA/DLL*`. `FE_ADDR_TAKEN` only for `SYM_USED_IN_PRAGMA` | see list | address-taken for ordinary locals is re-derived by CG | cinfo.c:FESymAttr:200, FEAttr:278 |
| `SYM_ADDR_TAKEN` on locals | set by `&x`, used by FE only for debug info (`emit_extra_info`) | no | yes | cgen.c:CDoAutoDecl:1074 (1101) |
| call class flags | `FECALL_GEN_ABORTS/NORETURN/CALLER_POPS/HAS_VARARGS/DLL_EXPORT/MAKE_CALL_INLINE`, x86 far/interrupt/farss/etc. | yes via `FEAuxInfo` | no | cfeinfo.c:getCallClass:491, getCallClassTarget:528 |

### String literals

| fact | computed where | passed on? | re-derived by CG? | cite |
|---|---|---|---|---|
| concatenation, escapes, wide flag | escapes removed, then concatenated; any wide piece makes all wide | bytes | no | cstring.c:GetLiteral:190 |
| duplicate pooling | hash + length + flags + `memcmp`; toggle `reuse_duplicate_strings` (default on). Exact duplicates only; no tail merging | one `STR_HANDLE` per distinct string | no | cstring.c:StringLeaf:272 (297); cmodel.c:329 |
| read-only placement | all literals go to `SEG_CONST`; code segment if `STRLIT_CONST` (`-zc`); far segment if big-data and length > `DataThreshold` | `BESetSeg(StringSegment)` | no | cgen.c:StringSegment:799, EmitLiteral:810; cstring.c:StringLeaf:272 (283) |
| const data symbols (non-string) | `FE_CONSTANT` -> `SEG_CONST2` unless `-zc`-rent | segment choice; attr to CG | CG uses `FE_CONSTANT` for call-crossing liveness | cinfo.c:SymSegId:286; cg conflict.c:92 |
| literal emitted only if used | emitted lazily at first `PushString`/initialiser use; `ref_count` counted | yes | no | cgen.c:Emit1String:824, PushString:834 |
| string as array initialiser (`char a[]="x"`) | copied into data quads, not pooled | bytes | no | cdinit.c:InitCharArray:1087 |
| `*"abc"` constant index | folded to char constant | yes | yes | cexpr.c:TakeRValue:531 |
| function using a string literal is not inlined | flag cleared | hidden from CG | n/a | cstring.c:StringLeaf:272 (327) |
| string identity (`"a" == "a"`) | pooled => same address; language allows either | no | no | cstring.c:StringLeaf:272 |

### Static initialisers, data

| fact | computed where | passed on? | re-derived by CG? | cite |
|---|---|---|---|---|
| static init order / layout | parsed into a linear list of `DATA_QUAD`s (`QDT_*`) with zero-fill runs (`QDT_CONSTANT`), repeat counts, bitfield merging; emitted after all symbols, before functions | bytes via `DGInteger`/`DGBytes`/`DGFEPtr`/`DGBackPtr`/`DGUBytes` in one pass | no: CG sees raw data | cdinit.c:StaticInit:1215, InitSymData:938; cgendata.c:EmitDQuad, EmitDataQuads:253; cgen.c:DoCompile:1901 |
| constant-ness of initialiser | must fold to `PUSHINT/PUSHFLOAT` or `AddrFold` sym+offset; else `ERR_NOT_A_CONSTANT_EXPR` | error | n/a | cdinit.c:StoreInt64:599, AddrFold:372 |
| all-zero data goes to BSS | only for symbols with no initialiser (`SYM_INITIALIZED` unset): zero bytes emitted (`DGUBytes` if `SEG_BSS`, else `EmitZeros`). `CompFlags.non_zero_data` is set but never read | partly | no | cgen.c:EmitSym:1023; cdinit.c (writes only) |
| auto aggregate init | split into element assignments / `SimpleStruct` path / memcpy-style assign | ordinary expression trees | CG | cdinit.c:VarDeclEquals:1648, InitArrayVar:1483, InitStructUnionVar:1379 |
| function-local statics | segment chosen at declaration; storage emitted when CG enters the block (`OPR_NEWBLOCK`) via `CDoAutoDecl`; init through `StaticInit` | global data with FE label | no | cgen.c:CDoAutoDecl:1074; cdecl2.c:VarDecl:324 |
| unused static variables | warning only; `EmitSyms` emits every global symbol (no liveness test) | emitted | not checked whether CG/linker drops them | csym.c:CheckDefined:636; cgen.c:EmitSyms:1834 |

### Dead code and control flow

| fact | computed where | passed on? | re-derived by CG? | cite |
|---|---|---|---|---|
| unreachable statement after `return/break/continue/goto`-like jump | `DeadCode` counter (1 after `Jump`, `break`, `continue`, `switch` head, noreturn call; reset at labels/case) | warning `ERR_DEAD_CODE` only; the statement is still added to the tree | YES: `DeadBlocks`, `AxeDeadCode` | cstmt.c:DeadMsg:488, Jump:285, Statement:1188; cg blktrim.c:DeadBlocks:462, optimize.c:AxeDeadCode:274 |
| constant `if`/`while`/`for`/`do` condition | `JumpFalse`/`JumpTrue` on a `PUSHINT`: no test emitted; true-always loop drops break label; false-always becomes unconditional `OPR_JUMP` | yes: jump or nothing; body still emitted | CG removes the unreachable body (`DeadBlocks`) | cstmt.c:JumpFalse:298, JumpTrue:317, Statement:1188 (T_WHILE/T_FOR), ForStmt:707 |
| `?:` / `&&` / `\|\|` with constant operand | see section 1 | arm only | yes | cfold.c:FoldQuestionTree:963 |
| constant `switch` selector | not folded | `OPR_SWITCH` with constant operand | CG may fold (not checked) | cstmt.c:SwitchStmt:996 |
| unused static function | `PruneFunctions` | body skipped | no | cgen.c:PruneFunctions:1783 |
| unused label / goto target | labels counted, `OPR_LABELCOUNT` sent | labels created with `BENewLabel` | CG drops unreferenced ones | cgen.c:DefineLabels:143 |
| return at end / single exit | `end_of_func_label`; return in inner block -> `Jump` to it | yes | CG | cstmt.c:Statement:1188 |
| noreturn call | `FLAG_ABORTS`/`FLAG_NORETURN` (`__declspec(noreturn)`, `#pragma aux aborts`) sets `pending_dead_code` (FE dead-code warning, suppresses missing-return warning) | `FECALL_GEN_ABORTS`/`NORETURN` in call class | CG uses it: no return path, no save/restore, no code after | cfeinfo.c:FunctionAborts:471, getCallClass:491; cexpr.c:GenFuncCall:2193 (2334); cg x86reg.c:88, x86enc2.c:388 |
| loop unroll pragma | `UnrollCount` per statement | `BEUnrollCount` | CG | cgen.c:GenOptimizedCode:1608 |

### Flow-dependent diagnostics (all FE-local, none passed)

| diagnostic | how computed | passed on? | cite |
|---|---|---|---|
| missing return value | `return_info` + `DeadCode == 0` at function end; syntactic, not a CFG; skipped for `naked` | no | cstmt.c:Statement:1188 (1432-1440), CheckRetValue:446 |
| unreachable code | `DeadCode` flag as above, one message per region (`DeadCode = 2`) | no | cstmt.c:DeadMsg:488 |
| unused variable / parameter / static | `SYM_REFERENCED` bit set on any rvalue use or address-take, checked at scope end | no | csym.c:CheckReference:578, CheckDefined:625; cexpr.c:TakeRValue:531 |
| use before assignment | `SYM_ASSIGNED` set by assignment, `&x`, array/struct decay, `.`; warn on first rvalue read of a local when not yet set in source order; disabled once any label is dropped (`label_dropped`); not for static/extern/arrays | no | cexpr.c:TakeRValue:531 (630-640), DotOp:1060, cmath.c:SetSymAssigned:1452; csym.c:CheckReference:578 (593) |
| assignment in condition | constant RHS only (`if (x = 1)`) | no | cexpr.c:BoolExpr:2554, NotOp:2589 |
| statement with no effect | `meaningless_stmt` / `useful_side_effect` flags | no | cstmt.c:CheckStmtExpr:103, cexpr.c:GetExpr:1252 |
| always-true/false compare | range check on constant operand | no | cmath.c:IsMeaninglessCompare:622 |
| shift too big, div by zero, constant too big | constant operand checks | no | cfold.c:CheckOpndValues:1160; cmath.c:BinOp:1374 |
| pointer/const/type mismatch | `CheckParmAssign`, `CompatiblePtrType`, `CheckConst` | no | ccheck.c:CheckParmAssign:853; cmath.c:CheckConst:1563 |
| duplicate case | sorted insert detects equality | no | cstmt.c:AddCaseLabel:775 |

### const / volatile / restrict / unaligned

| fact | tracked where | reaches CG? | re-derived by CG? | cite |
|---|---|---|---|---|
| `const` on object | symbol `mods` (`FLAG_CONST`); pointer `decl_flags` hold the pointee's cv; tree `OPFLAG_CONST` | symbols only: `FE_CONSTANT` (if not volatile) and `SEG_CONST2` placement. Lvalue `OPFLAG_CONST` is used only for the "modify const" error, never sent | no | cinfo.c:FESymAttr:200, SymSegId:286; cmath.c:CheckConst:1563; cexpr.c:OpFlags:186 |
| `volatile` | same bits; `SYM_TRY_VOLATILE` for locals in `_try`; `SYM_USED_IN_PRAGMA` | yes: `FE_VOLATILE`/`FE_MEMORY` on symbol; `CGVolatile` on each access; float temporaries forced volatile by `ForceVolatileFloat` | no | cinfo.c:FESymAttr:200; cgen.c:PushSym:565, ForceVolatileFloat:347; cstmt.c:MarkTryVolatile:868 |
| `restrict` | parsed to `FLAG_RESTRICT` in `TypeQualifier`; only `cdump.c` reads it | NO | CG has only `CGSW_GEN_RELAX_ALIAS` | ctype.c:TypeQualifier:223 (238); cdump.c:207; cg conflict.c:92 |
| `__unaligned` | `FLAG_UNALIGNED` -> `OPFLAG_UNALIGNED` | yes: `CGAttr(name, CG_SYM_UNALIGNED)` | no | cgen.c:PushSym:565, DotOperator:649 |
| qualifiers on non-pointer types | not in the type node; only in `sym->mods` and op flags (qualified typedefs use dummy typedef nodes) | see above | n/a | ctype.c:AdjModsTypeNode:1003, MkPtrNode:1504 |
| qualified-pointer merging in `?:` | `MergedType` ORs cv bits | result type | n/a | cmath.c:MergedType:2137 |

### Switch lowering

| fact | computed where | passed on? | re-derived by CG? | cite |
|---|---|---|---|---|
| case values sorted, duplicates rejected | sorted linked list insert | yes: `CGSelCase` in ascending order, one call per value | - | cstmt.c:AddCaseLabel:775; cgen.c:DoSwitch:957 |
| consecutive `case a: case b:` share a label | `gen_label=false` reuses label | labels | - | cstmt.c:AddCaseLabel:775 |
| case ranges (`CGSelRange`) | FE does not use it; no GNU range syntax found in cc | no | YES: CG merges adjacent same-label entries (`MergeListEntries`) and picks jump table / binary search / if-chain | cg bldsel.c:MergeListEntries:165, cg.c:CGSelRange:521 |
| low, high, count of cases | kept in `SWITCHDEFN` (`low_value`, `high_value`, `number_of_cases`); never read after init in cc | NO (dead) | YES | cstmt.c:SwitchStmt:996, AddCaseLabel:775; cops.h:111-114 |
| default label | `sw->default_label`, or break label | `CGSelOther` | - | cstmt.c:EndSwitch:1036; cgen.c:DoSwitch:957 |
| selector type | checked integer; no promotion to `int` inserted (selector has its own type) | `CGSelect(table, expr)` | CG | cstmt.c:SwitchStmt:996 |
| table vs tree density decision | none in FE | no | CG | cg bldsel.c |

### Pointer alias information

| fact | computed where | passed on? | re-derived by CG? | cite |
|---|---|---|---|---|
| pointee type (type-based alias) | full C type in FE | NO: pointers reach CG as `TY_POINTER/NEAR/LONG/HUGE`; aggregates as opaque refno with size+align; loads typed by scalar `cg_type` | CG has none; it does its own address-name analysis | cgen.c:CGenType:1975, PtrType:2035 |
| `restrict` | parsed | NO | no | ctype.c:TypeQualifier:223 (238) |
| address-taken locals | `SYM_ADDR_TAKEN` | not as `FE_ADDR_TAKEN` (pragma only); debug info only | yes | cinfo.c:FESymAttr:200; cgen.c:CDoAutoDecl:1074 |
| `-oa` (relax alias) | `CGSW_GEN_RELAX_ALIAS` | yes: a global switch | CG: non-visible, non-global symbols OK across calls | coptions.c:685; cg conflict.c:92 |
| `const` symbol never modified | `FE_CONSTANT` | yes | CG: OK across calls, `makeaddr.c:578` | cinfo.c:FESymAttr:200; cg makeaddr.c:578, redefby.c:259 |
| field identity of `p->f` | field resolved to offset | NO | no | cexpr.c:DotOp:1060, ArrowOp:1114 |
| struct/union distinction, member types | `TYP_STRUCT/UNION` | only size/align via `BEDefType` | no | cgen.c:CGenType:1975 |
| stack vs data pointer class (`SEG_STACK` ptr for `&auto`, `-zu` floating SS) | `PtrNode(..., SEG_STACK)` | as far/near flag | CG | cexpr.c:TakeRValue:531 |
| which calls may modify which data | none (no mod/ref) | no | CG assumes any call may modify visible data | - |

### Other

| item | finding | cite |
|---|---|---|
| variable-length arrays | not supported: array dimension must fold to a constant (`ERR_NOT_A_CONSTANT_EXPR`/`ERR_INVALID_DIMENSION`) | cdecl2.c:ArrayDecl:1319; cexpr.c:ConstExprAndType:1174 |
| `volatile`/`setjmp` | `_try` marks all locals `SYM_TRY_VOLATILE` | cstmt.c:MarkTryVolatile:868 |
| register / storage hints | `SC_REGISTER` only affects `&` checks and far-SS; no `FE_` bit sent. `sym.weight` via `IncSymWeight` computed but only for FE use (not checked if read by cgen) | cexpr.c:IncSymWeight:470 |
| line numbers / unroll count | `src_loc` per statement | `DBSrcCue`, `BEUnrollCount` | cgen.c:GenOptimizedCode:1608 |
| `sizeof` of expression | operand parsed with `SizeOfCount++` so no warnings/codegen, operand freed | cexpr.c:GetExpr:1252 (TC_SIZEOF) |

### What a back end could optimise harder with

"Usable promise" = FE keeps the fact in a form a pass can rely on and hands it over.

| fact | FE keeps it as a usable promise? | what reaches CG | cite |
|---|---|---|---|
| `restrict` | kept as a type bit, never enforced or used | nothing | ctype.c:TypeQualifier:223 (238); cdump.c:207 |
| `const` object | yes for symbols: `FE_CONSTANT` (sym not volatile), placed in `SEG_CONST2`; the promise is used by CG (value survives calls, no redefinition). No for `const T *` lvalues (`OPFLAG_CONST` only checks stores) | `FE_CONSTANT` on symbols only | cinfo.c:FESymAttr:200, SymSegId:286; cg conflict.c:92, makeaddr.c:578; cmath.c:CheckConst:1563 |
| `volatile` | yes, strict | `FE_VOLATILE`, `CGVolatile` per access | cinfo.c:FESymAttr:200; cgen.c:PushSym:565 |
| noreturn / aborts | yes, for functions by declspec, `#pragma aux aborts`, and known abort names | `FECALL_GEN_ABORTS/NORETURN`; CG uses it (no return code, no saves) | cfeinfo.c:getCallClass:491; cg x86reg.c:88, x86enc2.c:388 |
| pure / no-memory calls | only via `#pragma aux ... nomemory` (parm and modify forms); no C-level `pure`/`const` function attribute found | `FECALL_GEN_NO_MEMORY_READ/CHANGED` -> `ROUTINE_READS_NO_MEMORY`/`MODIFIES_NO_MEMORY` | cpragx86.c:820, 924; cg x86reg.c:110-114 |
| sequence-point / operand order | no: FE emits one left-to-right tree; unspecified-order operands are not marked, and no "these two side effects are sequenced" vs "unsequenced" distinction. Only `,` `&&` `\|\|` `?:` have CG ops | tree order; `O_COMMA`, `O_FLOW_*`, `CGChoose` | treewalk.c:WalkExprTree:37; cgen.c:EmitNodes:1274 |
| known non-null pointer | no: only one fold (`&static ? :` cond replaced by 1). `&x`, string literal, `this`-like never marked non-null; null-pointer-constant knowledge is lost after compare folding | nothing | cfold.c:FoldableTree:996 (OPR_QUESTION) |
| folded constants | yes, fully (int, 64-bit, float via decimal string); CG also re-folds | constant leaves | cfold.c:DoConstFold:1288; cgen.c:PushConstant:744 |
| value ranges (`x & 0xff`, enum range, bit-field width, bool) | no: FE checks ranges for warnings only (`IsMeaninglessCompare`, `CheckAssignRange`); bitfield width does reach CG as `CGBitMask` | bitfield mask only | cmath.c:IsMeaninglessCompare:622; ccheck.c:CheckAssignRange:782; cgen.c:DotOperator:649 |
| signed overflow is undefined | no: signedness is in the type (`cg_type` signed vs unsigned) but nothing says "no wrap" | signed `cg_type` | cgen.c:CGenType:1975 |
| dead code | partly: FE drops tests on constant conditions and arms of `?:` `&&` `\|\|`; unreachable statements still sent; unused static functions dropped | pruned tree | cstmt.c:JumpFalse:298; cgen.c:PruneFunctions:1783; DeadBlocks in cg blktrim.c:462 covers the rest |
| switch sorted values | yes: sorted, unique, shared labels; but no range form (`CGSelRange` unused) and low/high/count kept in FE but unused | sorted `CGSelCase` list | cstmt.c:AddCaseLabel:775; cgen.c:DoSwitch:957; cg bldsel.c:165 |
| alignment of objects | yes for struct/array types and data symbols | `BEDefType(refno, align, size)`, `AlignIt`, `SetSegAlign` | cgen.c:CGenType:1975; cinfo.c:SetSegAlign:1033 |
| alignment of a pointer's target | no: pointer type carries no alignment; only "unaligned" is a yes-flag | `CG_SYM_UNALIGNED` only | cgen.c:DotOperator:649 |
| `__unaligned` | yes, as a negative promise | `CGAttr(CG_SYM_UNALIGNED)` | cexpr.c:OpFlags:186; cgen.c:PushSym:565 |
| packed layout (`-zp`, pack pragma) | offsets only; the fact that a field is under-aligned is not passed | offsets | ctype.c:FieldAlign:892 |
| type-based aliasing (pointee type, struct field identity) | kept in FE types only; CG sees scalar `cg_type` and opaque sized aggregates | none | cgen.c:CGenType:1975, PtrType:2035 |
| address-taken locals | `SYM_ADDR_TAKEN` exists; not sent (except pragma use) | debug info only | cgen.c:CDoAutoDecl:1074 (1101) |
| call never inlined / inlinable | FE decides and answers CG | `FECALL_GEN_MAKE_CALL_INLINE`, body via `FEGenProc` | cfeinfo.c:getCallClass:491; cinfo.c:FEGenProc:267 |
| unused static data | warns; emits anyway | emitted | cgen.c:EmitSyms:1834 |
| string literal read-only and pooled | yes | `SEG_CONST`, one copy | cgen.c:StringSegment:799; cstring.c:StringLeaf:272 |
| loop trip counts / unroll | only `#pragma unroll` count | `BEUnrollCount` | cgen.c:GenOptimizedCode:1608 |
| whole-module view (call graph, all bodies) | yes inside FE (`FirstStmt` list, `ScanFunction`); not given to CG, which compiles function by function | per-function CG streams | cgen.c:PruneFunctions:1783, GenModuleCode:1818 |

## 3. Flags

### How switches reach the back end

| Channel | What it carries | Cite |
|---|---|---|
| `BEInit( GenSwitches, TargetSwitches, OptSize, ProcRevision )` | all -o*, -d*, -m*, -z* bits, size/time, cpu/fpu | cgen.c:DoCompile:1924 |
| `GenSwitches` (`cg_switches`, target independent) | bit list | bld/cg/h/cgswitch.h:39-69 |
| `TargetSwitches` (`cg_target_switches`, x86) | bit list | bld/cg/intel/h/x86swi.h:33-58 |
| `ProcRevision` | cpu (mask 0xf), fpu level (0xf0), emu bit 0x80 | bld/cg/intel/h/cgx86swi.h:38-90 |
| `FEAuxInfo(handle, FEINF_*)` | per-symbol call info, segments, libs, imports | cfeinfo.c:FEAuxInfo:1079-1223 |
| `FEAttr` | per-symbol `fe_attr` bits | cinfo.c:FEAttr:278, FESymAttr:200-264 |
| `FESegID`, `FEStackChk`, `FEParmType`, `FEExtName`, `FEName` | segment, stack check, arg widening, name mangling | cinfo.c:863, 1002, 969; cfeinfo.c:805 |
| Switch init | `GenSwitches = CGSW_GEN_MEMORY_LOW_FAILS`; 386 `TargetSwitches = CGSW_X86_USE_32`; `DataThreshold = TARGET_INT_MAX`; `Inline_Threshold = 20` | cmdlnx86.c:CmdSysInit:74-84 |
| Globals | GenSwitches, TargetSwitches, ProcRevision, DataThreshold, Inline_Threshold, OptSize, Stack87 | h/cvars.h:246,247,249,256,258,302,368 |
| Order of processing | `AnalyseAnyTargetOptions` (generic) then `CmdSysAnalyse`: target system, arch, debug format, gen switches, memory model, default call conv, final target | coptions.c:1029-1030; cmdlnx86.c:CmdSysAnalyse:1212-1224 |
| PCH check | PCH invalid if GenSwitches or TargetSwitches differ | pchdr.c:1755 |

### -o* (optimization)

| Flag | Sets | Back end told | Cite |
|---|---|---|---|
| -od | `CGSW_GEN_NO_OPTIMIZATION` | BE no-opt. FE also: no `SYM_OK_TO_RECURSE`; autos/parms get debug info; no strcmp->memcmp and no math-func intrinsic rewrite | coptions.c:AnalyseAnyTargetOptions:458-460; cdecl1.c:113; cgen.c:1086,1153; cexpr.c:2265,2286 |
| -ox (x86) | clears NO_OPT; `CGSW_GEN_BRANCH_PREDICTION`, `CGSW_GEN_I_MATH_INLINE`, `data->oi=true`, `CGSW_GEN_LOOP_OPTIMIZATION`, `Inline_Threshold=20`, `TOGGLE(inline)=true`, `CGSW_GEN_INS_SCHEDULING`, `TOGGLE(check_stack)=false` | = -ob -om -oi -ol -oe=20 -or -s | coptions.c:447-457 |
| -ob | `CGSW_GEN_BRANCH_PREDICTION` | BE branch prediction | coptions.c:689-691 |
| -oa | `CGSW_GEN_RELAX_ALIAS` | BE relaxed aliasing | coptions.c:686-688 |
| -oc | `CGSW_GEN_NO_CALL_RET_TRANSFORM` | BE keeps call+ret, no call->jmp | cmdlnx86.c:SetGenSwitches:605-607 |
| -oe[=n] | `Inline_Threshold=n`, `TOGGLE(inline)=true` | FE only: sets `FUNC_OK_TO_INLINE` on functions, cleared if node count > n; BE sees `FECALL_GEN_MAKE_CALL_INLINE` and the FE emits the body at the call through `FEGenProc`->`GenInLineFunc` | coptions.c:693-696; cstmt.c:186-191, 1473-1477; cfeinfo.c:514-515; cinfo.c:267-275 |
| -of / -of+ | `CGSW_X86_NEED_STACK_FRAME`; -of+ also `WatcallInfo.cclass_target \|= FECALL_X86_GENERATE_STACK_FRAME` | BE traceable frame (-of), frame always for watcall (-of+) | cmdlnx86.c:608-613, 488-490 |
| -oh | `CGSW_GEN_SUPER_OPTIMAL` | BE expensive opts | coptions.c:697-699 |
| -oi | `CompFlags.inline_functions` only | BE not told. FE predefines `__INLINE_FUNCTIONS__`; headers then `#pragma intrinsic`. Intrinsic call resolves to `InlineInfo` with byte code | coptions.c:700-702; cmodel.c:254-256; cfeinfo.c:InfoLookup:347-399 |
| -oi+ | not in option table (grep of gml/options.gml: no hit) | - | - |
| -ok | `CGSW_GEN_FLOW_REG_SAVES` | BE prolog/epilog in flow graph | coptions.c:703-705 |
| -ol | `CGSW_GEN_LOOP_OPTIMIZATION` | BE loop opts | coptions.c:706-708 |
| -ol+ | LOOP_OPTIMIZATION + `CGSW_GEN_LOOP_UNROLLING` | BE loop unrolling | coptions.c:709-711 |
| -om | `CGSW_GEN_I_MATH_INLINE`; macro `__SW_OM` | BE inline math. Also `CmdSysSetMaxOptimization` sets it | cmdlnx86.c:614-616, 690-692, 1226-1229 |
| -on | `CGSW_GEN_FP_UNSTABLE_OPTIMIZATION` | BE unstable FP opts | coptions.c:712-714 |
| -oo | clears `CGSW_GEN_MEMORY_LOW_FAILS` | BE continues when low on memory | coptions.c:715-717 |
| -op | `CompFlags.op_switch_used` | FE wraps float/double/long double names in `CGVolatile` (forces FP results to memory). -fpc clears it | cmdlnx86.c:617-619, 319-320; cgen.c:ForceVolatileFloat:349-353 |
| -or | `CGSW_GEN_INS_SCHEDULING` | BE instruction scheduling | coptions.c:718-720 |
| -os | `OptSize=100`; clears NO_OPT | BE size bias (BEInit arg). FE: no `DGAlign` of data, code seg align 1, size-variant inline tables (`SInline_Functions`), code label alignment stays 1 | coptions.c:472-475; cgendata.c:AlignIt:54; cinfo.c:627, 1039; cfeinfo.c:IF_Lookup:228-250, 1114-1118 |
| -ot | `OptSize=0`; clears NO_OPT | BE time bias. FE: `DGAlign` data, code seg align = int size, `SegAlignment` raised, label alignment = int size | coptions.c:468-471; cgendata.c:54-56; cinfo.c:627, 1039-1044; cfeinfo.c:1116-1117 |
| (neither) | `OptSize=50` | BE default bias; FE treats as not-0 not-100 | coptions.c:476-478 |
| -ou | `CompFlags.unique_functions` | `FE_UNIQUE` on global or address-taken functions. -za and -zA also set it | coptions.c:721-723, 504; cinfo.c:225-229 |
| -oz | `CGSW_GEN_NULL_DEREF_OK` | BE: NULL is valid memory | coptions.c:724-726 |
| -ox note | -d2 and -d3 set `data->oi=false` and NO_OPT, so `-d2 -ox` loses -oi | - | coptions.c:528-529, 541-542 |

### Floating point

| Flag | Sets | Back end told | Cite |
|---|---|---|---|
| -fp2 / -fp287 | `SET_FPU_LEVEL(FPU_87)` (16-bit default) | `ProcRevision` fpu level | cmdlnx86.c:289-294 |
| -fp3 / -fp387 | `FPU_387` (386 default) | ditto | cmdlnx86.c:283-288 |
| -fp5 | `FPU_586` | ditto, P5 scheduling | cmdlnx86.c:280-282 |
| -fp6 | `FPU_686` | ditto | cmdlnx86.c:277-279 |
| -fpi (default) | `SET_FPU_EMU`; on 386 Windows (`TS_WINDOWS`) falls to fpi87 | BE emits emulator fixups (FWAIT patch bytes); FE adds extref `__init_87_emulator` / `__init_387_emulator` and lib `8emu87`/`8emu387` | cmdlnx86.c:303-309; cfeinfo.c:896-902; cmdlnx86.c:1169-1173, 1201-1205 |
| -fpi87 | `SET_FPU_INLINE` | BE inline 8087, no emulation; lib `8noemu*` | cmdlnx86.c:314-316, 1172, 1204 |
| -fpc | `SET_FPU_FPC` (`FPU_NONE`), `op_switch_used=false` | BE calls FP library, no 8087 code; libs `5math*`; no `__8087` extref | cmdlnx86.c:317-320, 1164-1167, 1188-1194; cfeinfo.c:903 |
| -fpr | `Stack87=4` | `FEINF_STACK_SIZE_8087` returns 4 (not 8); extref `__old_8087` instead of `__8087`. NetWare sets 4 too | cmdlnx86.c:329-335; cfeinfo.c:1092-1093, 904-908 |
| -fpd | `CGSW_X86_P5_DIVIDE_CHECK` | BE FDIV bug check (`FEINF_P5_CHIP_BUG_SYM` = `SymChipBug`) | cmdlnx86.c:340-342; cfeinfo.c:1110-1111 |
| -fld | `CompFlags.use_long_double` | FE: `long double` stays 10-byte `TYP_LONG_DOUBLE` (`TY_LONG_DOUBLE`), else maps to double; `L` suffix constants; extref `_fltused_80bit_` | cmdlnx86.c:337-339; ctype.c:316-320; cscan.c:411-416; cfeinfo.c:880-882; cdatatyp.h:54 |
| -zfw | `CGSW_X86_GEN_FWAIT_386` | BE emits FWAIT on 386+ | cmdlnx86.c:343-345 |
| -zri | `CGSW_GEN_FPU_ROUNDING_INLINE` | BE inline rounding | cmdlnx86.c:347-349 |
| -zro | `CGSW_GEN_FPU_ROUNDING_OMIT` | BE omits rounding calls | cmdlnx86.c:350-352 |

### CPU and calling convention defaults

| Flag | Sets | Back end told | Cite |
|---|---|---|---|
| -0 .. -6 (16-bit) | `SET_CPU(CPU_86..CPU_686)`; default 8086 | `ProcRevision` cpu; inline asm assembler `AsmEnvInit` cpu | cmdlnx86.c:198-220; cpragx86.c:PragmaInit:95-124 |
| -3r -4r -5r -6r (386) | `CompFlags.register_conventions=true`, cpu 386..686; -6r is the default | watcall register args; lib model `r`; `__3R__` | cmdlnx86.c:233-258, 1176-1179 |
| -3s -4s -5s -6s | cpu set, `register_conventions=false` | `SetAuxStackConventions`: WatcallInfo becomes caller-pops, `FECALL_X86_NO_8087_RETURNS`, parms=stack (MetaWareParms), save minus eax/ecx/edx/flts, objname `*`. NetWare forces this | cmdlnx86.c:230-231, 264-270, 491-494; callinfo.c:SetAuxStackConventions:384-402 |
| -ecc | `DftCallConv=&CdeclInfo` | default aux for unqualified funcs | cmdlnx86.c:SetDftCallConv:367-369 |
| -ecd | `&StdcallInfo` | ditto | cmdlnx86.c:370-372 |
| -ecf | `&FastcallInfo` | ditto | cmdlnx86.c:373-375 |
| -eco | `&OptlinkInfo` | ditto | cmdlnx86.c:376-378 |
| -ecp | `&PascalInfo` | ditto | cmdlnx86.c:379-381 |
| -ecr | `&FortranInfo` | ditto | cmdlnx86.c:382-384 |
| -ecs | `&SyscallInfo` | ditto | cmdlnx86.c:385-387 |
| -ecw / default | `&WatcallInfo` | ditto | cmdlnx86.c:388-391 |
| Use of DftCallConv | `DefaultInfo = *DftCallConv`; `FindInfo` falls back to `GetLangInfo(sym->mods)` -> `DefaultInfo` | per-symbol `aux_info` | callinfo.c:SetDefaultAuxInfo:452-459; cfeinfo.c:290-312, 465-467 |
| -zz (386) | `use_stdcall_at_number=false` | `__stdcall` objname `_*` instead of `_*#` | cmdlnx86.c:663-665; callinfo.c:224-228 |
| -r | `save_restore_segregs` | WatcallInfo keeps DS/ES/FS/GS in `save` (else floating ones removed); `modify` lists do not strip seg regs | cmdlnx86.c:620-622, 474-487; cpragx86.c:195 |
| -ri | `CompFlags.returns_promoted` | return type widened via `FEParmType` (char/short -> int; 386 also 16-bit) | cmdlnx86.c:628-630; cgen.c:ReturnType:205-211; cinfo.c:969-998 |

### Memory model and segment registers

| Flag | Sets | Back end told | Cite |
|---|---|---|---|
| -ms | `CHEAP_POINTER`; strings not in code | small; near code and data | cmdlnx86.c:Define_Memory_Model:987-992 |
| -mm | `BIG_CODE\|CHEAP_POINTER`; `WatcallInfo.cclass_target\|=FAR_CALL`; `CodePtrSize=far` | far calls; strings not in code | cmdlnx86.c:993-1000 |
| -mc | `BIG_DATA\|CHEAP_POINTER`; `DataPtrSize=far` | far data, near code | cmdlnx86.c:1001-1005 |
| -ml | `BIG_CODE\|BIG_DATA\|CHEAP_POINTER`; far call | large | cmdlnx86.c:1006-1012 |
| -mh (16-bit) | `BIG_CODE\|BIG_DATA` (no CHEAP_POINTER) | huge; `IsHugeData()` | cmdlnx86.c:1014-1020; h/cconst.h:64 |
| -mf (386, default) | `FLAT_MODEL\|CHEAP_POINTER`; lib model `s` | flat; ES not floating; `Flat()` inline alternates; const/static temps to code seg | cmdlnx86.c:1022-1025, 1047-1049; cfeinfo.c:201-215, 251-258; cinfo.c:317-327 |
| Default model | 16-bit ms; 386 mf, NetWare ms | - | cmdlnx86.c:974-985 |
| -mt | not in option table | - | gml/options.gml listing: only mc mf mh ml mm ms |
| Model -> types | pointers in model become `TY_POINTER`; explicit `__near`/`__far`/`__huge` become `TY_NEAR_POINTER`/`TY_LONG_POINTER`/`TY_HUGE_POINTER`; func ptrs via `CodePtrType` | BE data type per pointer | cgen.c:PtrType:2035-2058, DataPointerType:322-337; cgendata.c:GetDQuadPointerSize:67-92; cmath.c:PointerClass:503-537 |
| Model -> var placement | big data: size>`DataThreshold`, unsized extern array, or const with -zc, gets `FLAG_FAR`; far vars get private segments | `FESegID` returns per-var segid | cinfo.c:SetFarHuge:161-185, SetSegment:331-371; cdecl2.c:436-448 |
| -zdp | clears FLOATING_DS | DS pegged to DGROUP | cmdlnx86.c:1075-1078 |
| -zdf | sets FLOATING_DS | DS floats; inline tables `DF_`/`BigData_` | cmdlnx86.c:1079-1082; cfeinfo.c:232-240, 263-270 |
| -zdl (386) | `CGSW_X86_LOAD_DS_DIRECTLY`, DS not floating | BE loads DS directly, no runtime call | cmdlnx86.c:1060-1063 |
| Big data default | `FLOATING_DS` when BIG_DATA; 16-bit Windows pegs DS | - | cmdlnx86.c:1053-1055, 1069-1072 |
| -zfp / -zff | clear/set FLOATING_FS (cpu>=386) | FS pegged or floating; `__SW_ZFP/ZFF` | cmdlnx86.c:1095-1119, 758-762 |
| -zgp / -zgf | clear/set FLOATING_GS | GS pegged or floating | cmdlnx86.c:1123-1140, 763-767 |
| -zu | `CompFlags.zu_switch_used`; `FLOATING_SS` | SS != DGROUP. FE makes `&auto` and auto vars far; else per function `FLAG_FARSS` toggles FLOATING_SS; `__interrupt` forces FARSS | cmdlnx86.c:659-661, 1041-1043; cdecl1.c:126-132; cdecl2.c:395, 196-198; cexpr.c:588-590; cgen.c:1189-1195, 1912-1914; cfeinfo.c:551-553 |
| -zt[=n] | `DataThreshold=n` (>TARGET_INT_MAX -> 256) | big-data objects over n go to far segments; strings over n `STRLIT_FAR` | cmdlnx86.c:656-658; coptions.c:169-175; cinfo.c:172; cstring.c:282-288 |
| -zc | `strings_in_code_segment`, `zc_switch_used`, `CGSW_X86_CONST_IN_CODE`; forced off for ms/mm | string literals and (far) const emitted in `SEG_CODE`; BE FP consts in code | cmdlnx86.c:647-651, 989-990; cgen.c:EmitCS_Strings:874-883; cexpr.c:1681; cinfo.c:174, 309-327 |
| -zm | `multiple_code_segments`, `zm_switch_used` | one text segment per function (name = func name in big code) | cmdlnx86.c:652-655; cdecl1.c:187-200; csym.c:670-673 |
| -nt=x | `TextSegName` | BE code segment name (`GIVEN_NAME`) | cmdlnx86.c:602-604; cinfo.c:629-633 |
| -nd=x | `DataSegName`, `ImportNearSegIdInit` | `FEINF_DATA_GROUP`; class `FAR_DATA` | cmdlnx86.c:590-598; cfeinfo.c:1096-1097; cinfo.c:549-551 |
| -nc=x | `CodeClassName` | class of code seg | cmdlnx86.c:587-589; cinfo.c:512-514 |
| -g=x | `GenCodeGroup` | `FEINF_CODE_GROUP` | cmdlnx86.c:584-586; cfeinfo.c:1094-1095 |
| -nm=x | `ModuleName` | `FEModuleName` | cmdlnx86.c:599-601; cinfo.c:847-853 |
| -xbsa | `unaligned_segs` | BE segment alignment 1 | coptions.c:791-793; cinfo.c:SegAlign:604-614 |
| -zp=n | `PackAmount`, `GblPackAmount` (1,2,4,8,16); default 2 (16-bit), 8 (386) | struct field offsets and size in FE; BE gets `BEDefType(ref, align, size)` | coptions.c:819-822, 163-166; cmdlnx86.c:75-82; ctype.c:FieldAlign:892-905; cgen.c:1994-1999 |
| -zpw | `slack_byte_warning` | FE warning only | coptions.c:823-825 |
| -xgv (386) | `CGSW_X86_INDEXED_GLOBALS` | BE indexed globals (PIC-like; header says faulty) | cmdlnx86.c:643-645; x86swi.h:44 |
| -re (386) | `CompFlags.rent` -> `GenSwitches\|=CGSW_GEN_POSITION_INDEPENDANT` at `BEInit` | rent data: `FE_DLLIMPORT` on dllimport, no `SEG_CONST2` for const, `FE_THREAD_DATA` on function/var marked rent | cmdlnx86.c:624-626; cgen.c:1921-1924; cinfo.c:244, 260-262, 293, 317; cdecl2.c:349-353 |

### Stack, profiling, hooks, windows, target

| Flag | Sets | Back end told | Cite |
|---|---|---|---|
| -s | `TOGGLE(check_stack)=false` | `SYM_CHECK_STACK` not set on functions; `FEStackChk` returns 0. -ox also sets it | coptions.c:770-772; cdecl1.c:122-123; cinfo.c:1002-1010 |
| -sg | `sg_switch_used` | `FECALL_X86_GROW_STACK` on every call class | cmdlnx86.c:631-633; cfeinfo.c:587-589 |
| -st | `st_switch_used` | `FECALL_X86_TOUCH_STACK` | cmdlnx86.c:634-636; cfeinfo.c:590-592 |
| -en | `emit_names` | `FECALL_X86_EMIT_FUNCTION_NAME` on functions | coptions.c:633-635; cfeinfo.c:554-556 |
| -ep[=n] | `ep_switch_used`, `ProEpiDataSize=n` | `FECALL_X86_PROLOG_HOOKS`; `FEINF_PROEPI_DATA_SIZE` | coptions.c:636-639; cfeinfo.c:581-583, 1106-1107 |
| -ee | `ee_switch_used` | `FECALL_X86_EPILOG_HOOKS` | cmdlnx86.c:550-552; cfeinfo.c:584-586 |
| -ec | `ec_switch_used` | defines coverage segments `SEG_YIB/YI/YIE` | cmdlnx86.c:547-549; cinfo.c:637-641 |
| -et | `CGSW_X86_P5_PROFILING` | BE RDTSC profiling; FE emits per-function profile block in `FunctionProfileSegId`, extref `__p5_profile`, `FEINF_P5_PROF_DATA/SEG` | cmdlnx86.c:568-570; cgen.c:1199-1220, 1926-1930; cfeinfo.c:971-975, 1127-1130 |
| -et0 | P5_PROFILING + `P5_PROFILING_CTR0` | RDPMC | cmdlnx86.c:574-576 |
| -etp | `CGSW_X86_NEW_P5_PROFILING` | new profiler, extref `__new_p5_profile` | cmdlnx86.c:577-579; cfeinfo.c:980-985 |
| -esp | `CGSW_X86_STATEMENT_COUNTING` | BE statement counting | cmdlnx86.c:571-573 |
| -ez | `CGSW_X86_EZ_OMF` | PharLap EZ-OMF | cmdlnx86.c:580-582 |
| -zw | target WINDOWS; `TS_WINDOWS` sets `CGSW_X86_WINDOWS` (16-bit) | Win16 prologs; `FECALL_X86_PROLOG_FAT_WINDOWS` on `__pascal`/`__cdecl` funcs; DS pegged; WinMain startup ref | cmdlnx86.c:104-106, 438-442, 1069-1072; cfeinfo.c:543-550, 855-856 |
| -zW | CHEAP_WINDOWS: `CGSW_X86_CHEAP_WINDOWS` | cheaper prologs, callbacks only | cmdlnx86.c:108-110, 434-437 |
| -zws / -zWs | adds `CGSW_X86_SMART_WINDOWS` | smart callbacks, DS==SS | cmdlnx86.c:111-118 |
| -bt=os | `TargetSystem` | target macros, `FEExtName EXTN_IMPPREFIX` (`__imp_` only NT), NetWare stack conv, Stack87 | cmdlnx86.c:97-99, 155-188; cfeinfo.c:815-816 |
| -bd | `bd_switch_used`; `CGSW_GEN_DLL_RESIDENT_CODE` | BE DLL-resident code; extref `__DLLstart_`; lib `clibdl` | coptions.c:611-614; cfeinfo.c:835-853; cmdlnx86.c:1157-1162 |
| -bc / -bg / -bw | `bc/bg/bw_switch_used` | pick startup extref (`_cstart_`, `_wstart_`); -bw adds `__init_default_win` | coptions.c:608-623; cfeinfo.c:823-875, 954-956 |
| -bm | `bm_switch_used`; `_MT` | MT lib (`clibmt`) | cmdlnx86.c:867-870, 1154-1156 |
| -br (386) | `br_switch_used`; `_DLL` | DLL CRT libs (`clb?dll`) | cmdlnx86.c:543-545, 880-883, 1183-1185 |
| -eoo / -eoe / -eoc | clear / set `CGSW_GEN_OBJ_ELF` / `CGSW_GEN_OBJ_COFF` | BE object format | cmdlnx86.c:553-563 |
| -hc / -hd / -hda / -hdg / -hw | `CGSW_GEN_DBG_CV` / `DBG_DF` / `DBG_DF\|DBG_PREDEF` (+`__DFABBREV`) / Watcom (none) | BE debug format; `FEINF_DBG_PREDEF_SYM` | cmdlnx86.c:SetDebugInfoFormat:515-537; cfeinfo.c:1108-1109 |
| -zl, -zld, -zls, -zlf | `emit_library_names=false`, `emit_dependencies=false`, `emit_targimp_symbols=false`, `emit_all_default_libs` | `FEINF_NEXT_LIBRARY`, `FEINF_NEXT_DEPENDENCY`, `FEINF_NEXT_IMPORT` return nothing/extra | coptions.c:807-818; cfeinfo.c:616-629, 1013-1014, 1051-1052, 1160-1163 |

### Debug (-d*)

| Flag | Sets | Back end told | Cite |
|---|---|---|---|
| -d0 | nothing | - | coptions.c:551-552 |
| -d1 | `CGSW_GEN_DBG_NUMBERS` | line numbers | coptions.c:548-550 |
| -d1+ | NUMBERS\|TYPES\|LOCALS; `debug_info_some` | locals + types; keeps optimization; extra info for addr-taken/agg locals | coptions.c:544-547; cgen.c:1126-1129 |
| -d2 | NUMBERS\|TYPES\|LOCALS; `CGSW_GEN_NO_OPTIMIZATION`; `oi=false` | no-opt | coptions.c:534-543 |
| -d2~ | as -d2 + `no_debug_type_names` | - | coptions.c:531-533 |
| -d3 | as -d2 + `dump_types_with_names`; NO_OPT | unreferenced types emitted (`EmitDBType` if DBG_TYPES) | coptions.c:520-530; cgen.c:1934-1935 |
| -d9 | `use_full_codegen_od` | full codegen in -od | coptions.c:517-519 |
| DBG_LOCALS use | `DBLocalSym`, `DBModSym`, `DBBegBlock` calls | BE debug API | cgen.c:1132,1158,1222-1227 |

### Language flags that change what the BE sees

| Flag | Sets | Back end told | Cite |
|---|---|---|---|
| -j | `SetSignedChar()` (plain char = `TYP_CHAR`), `CompFlags.signed_char` | plain char `TY_INT_1` not `TY_UINT_1`; char constants sign-folded; `__CHAR_SIGNED__`. Default is unsigned: `SetPlainCharType(TYP_UCHAR)` | coptions.c:677-680; ctype.c:131, 217-220; cscan.c:1498-1510; cdatatyp.h:42-43; cmodel.c:266-268 |
| -ei | `make_enums_an_int`, `original_enum_setting=true` | enum base type at least int (size 4 on 386, 2 on 16-bit) | coptions.c:561-564; cenum.c:206-210 |
| -em | `make_enums_an_int=false` | minimal int type (starts at `ENUM_S8`) | coptions.c:565-568; cenum.c:206-210 |
| default enums | coptions.c says "default set in CmdSysInit()" but cmdlnx86.c:CmdSysInit:70-88 does not assign it; assignment not found in the files read | - | coptions.c:569-571; cmdlnx86.c:70-88 |
| -ze / -za / -zA | `extensions_enabled`, `non_iso_compliant_names_enabled`; -za/-zA also `unique_functions`; -zA `strict_ANSI` | FE keywords (`__near` etc. vs `_near`); math intrinsic rewrite needs extensions | coptions.c:497-514; cexpr.c:2286-2287 |
| -za99, -zastd | `CompVars.cstd` | FE only | coptions.c:483-496 |
| -zev | `unix_ext` | void arithmetic, FE only | coptions.c:800-802 |
| -aa, -ai, -aq | `auto_agg_inits`, `no_check_inits`, `no_check_qualifiers` | FE only | coptions.c:577-607 |
| -wo (16-bit) | `using_overlays` | warn on static func address | cmdlnx86.c:637-641; csym.c:666-668 |
| FE-only (no BE effect found) | -w*, -we, -wcd, -wce, -wpx, -e, -ef, -eq, -fh, -fhq, -fi, -fip, -fo, -fr, -ft, -fx, -i, -x, -xx, -v, -zg, -zs, -zk*, -zq, -q, -p*, -d/-u macros, -fti, -rod, -db, -ad* | - | coptions.c:627-845 |
| Not applicable / not defined for x86 | -xs, -mt, -xd (axp only), -si (axp only), -zps (axp only), -vcap (386 axp) | - | gml/options.gml (option/target lines) |

### What each optimisation switch lets the back end assume

| Switch | BE assumption / capability | BE cite |
|---|---|---|
| -oa `RELAX_ALIAS` | globals/statics not aliased by pointer stores: memory operands kept in regs across stores; statics (not `FE_GLOBAL\|FE_VISIBLE`) survive calls; scheduler may reorder FP memory | dataflo.c:189-195; conflict.c:92, 165; redefby.c:90-93, 208-210; scinfo.c:88; i87sched.c:621 |
| -ol `LOOP_OPTIMIZATION` | loop invariant hoisting, loop transforms, CSE with loop info, variable splitting | generate.c:160-170, 726-728 |
| -ol+ `LOOP_UNROLLING` | heuristic unrolling when no `#pragma unroll`; not at all if `OptSize>0`; known-iteration loops unrolled to divisor, `complete` if fully unrolled | unroll.c:301-322; loopopts.c:2392 |
| `#pragma unroll(n)` | per block `unroll_count`; n!=0 bypasses the `-ol+` and size test | cpragma.c:1433-1459; cstmt.c:156; cgen.c:1629-1632; bldins.c:382, 470-477; unroll.c:301-302 |
| FEINF_UNROLL_COUNT | declared in cgaux.h:63 but no reader or writer in cg/ or cc/ (grep); the C FE uses `BEUnrollCount` instead | cgaux.h:63; cgfuntab.h:56 |
| -on `FP_UNSTABLE_OPTIMIZATION` | `x/c` -> `x*(1/c)` for any constant c (else only exact powers of 2); CSE of FP divides | treefold.c:907-912; cse.c:758-759 |
| -or `INS_SCHEDULING` | final instruction scheduler runs | generate.c:287-290 |
| -om `I_MATH_INLINE` | FP math functions inline (x87 transcendental code); loop invariant code motion treats FP ops as not dangerous | i87opt.c:373; i87reg.c:698; i87exp.c:890, 901; loopopts.c:720-722 |
| -ob `BRANCH_PREDICTION` | block layout for predicted branches; skipped if `OptSize>50` | object.c:810-816 |
| -oh `SUPER_OPTIMAL` | gates extra work in register allocation and scoreboard (sites seen as one-line greps, not read in context) | regalloc.c:532, 539; scinfo.c:161, 213, 247 |
| -ok `FLOW_REG_SAVES` | gates `flowsave.c` (saves placed by flow analysis; one-line grep only) | flowsave.c:307 |
| -oz `NULL_DEREF_OK` | address of object can be null: disables `&obj == 0` folding and null-propagation | treefold.c:1437-1445; nullprop.c:494 |
| -oc `NO_CALL_RET_TRANSFORM` | disables call+ret -> jmp tail call | optpush.c:52 |
| -od `NO_OPTIMIZATION` | skips the whole optimizer pipeline | generate.c:152, 219, 236, 282 |
| -os / -ot (`OptForSize`) | size (100) or time (0) tested at these sites (only unroll.c:305 and object.c:814 read in context; others seen as one-line greps) | unroll.c:305; cse.c:625; inssched.c:261; encode.c:64; optcom.c:245; x86ver.c:109, 177, 230; x86ldstr.c:362, 431; x86mul.c:45 |
| -oe=n / `-oi` / `inline` | FE-level inlining: body copied at call if node count <= n, depth < 3; `#pragma off(inline)` disables per region; FE clears `FUNC_OK_TO_INLINE` for naked, varargs, and functions whose pragma bytes use autos; also cleared at cstring.c:327, cstmt.c:887, csym.c:931 (contexts not read) | cstmt.c:186-191, 1473-1479; cgen.c:1709-1726, 1746-1758; cpragx86.c:422; cstring.c:327 |
| `inline_depth`, `inline_recursion` pragmas | not in the C FE; depth is constant `MAX_INLINE_DEPTH 3` | cops.h:84; cgen.c:1716, 1752 |
| `#pragma intrinsic(f)` | `SYM_INTRINSIC`: call replaced by inline byte sequence (`InlineInfo`) with exact `parms`/`returns`/`save`; tables picked by -fpi/-os/-mf/big-data; math functions also become `OPR_MATHFUNC` | cpragma.c:1322-1352; cfeinfo.c:217-287, 347-399; cexpr.c:2286-2295 |
| `#pragma aux ... = bytes` | opaque code with exact register effects; `FLOATING_FIXUP_BYTE` fixups; cannot be inlined if it uses autos | cpragx86.c:352-578; cpragma.c:360-380 |
| -ox | = -ob -om -oi -ol -oe=20 -or -s | coptions.c:447-457 |
| Not in the table | no switch or pragma sets `CGSW_GEN_FORTRAN_ALIASING` (the strongest no-alias assumption, used in redefby.c:86, 204; tree.c:1434; bldcall.c:635; inssched.c:364; breakrtn.c:86; loopopts.c:560, 593; generate.c:397) from the C FE: grep of cc/ shows no writer | bld/cc grep: no hit for FORTRAN_ALIASING |

## 4. Pragmas and aux information

### Built-in calling conventions

`aux_info` fields: `cclass`, `cclass_target`, `code`, `parms`, `returns`, `streturn`, `save`, `objname`, `use`, `flags` (callinfo.h:44-54). Builtins start as copies of `WatcallInfo` (caux.c:PragmaAuxInit:77-83).

| Convention (keyword, magic word) | cclass | cclass_target | parms | returns/streturn | save (preserved) | objname | Cite |
|---|---|---|---|---|---|---|---|
| watcall (`__watcall`, default) | NONE | NONE (+FAR_CALL in big code models) | eax ebx ecx edx (ax bx cx dx 16-bit) | empty | FULL | NULL -> `TS_CODE_MANGLE` | callinfo.c:SetAuxWatcallInfo:436-449, 61-68; cmdlnx86.c:995,1008 |
| `__cdecl` | CALLER_POPS | NO_STRUCT_REG_RETURNS, ROUTINE_RETURN, SPECIAL_STRUCT_RETURN (16-bit adds LOAD_DS_ON_CALL, NO_FLOAT_REG_RETURNS) | stack | streturn ax/eax | FULL minus ABCD,ES (16-bit) or eax ecx edx; minus FLTS | `_*` | callinfo.c:131-167 |
| `__pascal` | REVERSE_PARMS | NO_FLOAT_REG_RETURNS, NO_STRUCT_REG_RETURNS, SPECIAL_STRUCT_RETURN | stack | empty | FULL minus regs, FLTS | `^` | callinfo.c:172-202 |
| `__fortran` | copy of watcall | - | regs | - | - | `^` | callinfo.c:126; caux.c:82 |
| `__stdcall` | NONE | SPECIAL_STRUCT_RETURN | stack | streturn ax (16-bit) / empty | FULL minus ABCD,ES or eax ecx edx; minus FLTS | `_*#` (or `_*` with -zz) | callinfo.c:208-244 |
| `__fastcall` | NONE | SPECIAL_STRUCT_RETURN (16-bit adds PARMS_PREFER_REGS) | ecx edx (386) / ax dx bx (16-bit) | empty | as above | `@*#` / `@*` | callinfo.c:84-91, 250-284 |
| `_Optlink` | CALLER_POPS | PARMS_STACK_RESERVE, NO_STRUCT_REG_RETURNS, SPECIAL_STRUCT_RETURN | eax ecx edx FLTS (386) | empty | as above | `*` | callinfo.c:97-100, 289-324 |
| `__syscall`, `__system` | CALLER_POPS | NO_STRUCT_REG_RETURNS, SPECIAL_STRUCT_RETURN | stack | empty | as above | `*` | callinfo.c:329-358; fe_misc/h/auxinfo.h:37,41 |
| `__far16` (386) | copy of cdecl/pascal | + `FECALL_X86_FAR16_CALL`; parms stack; cdecl clears EBX save | stack | - | - | copy | callinfo.c:368-379; cfeinfo.c:455-463 |
| vararg funcs | `CALLER_POPS\|HAS_VARARGS`; parms `DefaultVarParms` when no inline code; `__stdcall`/`__fastcall` varargs use cdecl name pattern | - | - | - | - | cfeinfo.c:517-519, 1207-1211, 772-780 |

Back-end queries, per symbol: `FEINF_CALL_CLASS` (generic cclass + ABORTS/NORETURN/DLL_EXPORT/MAKE_CALL_INLINE/HAS_VARARGS) cfeinfo.c:491-526, 1135; `FEINF_CALL_CLASS_TARGET` (x86 bits) cfeinfo.c:528-599, 1137; `FEINF_PARM_REGS` 1195-1213; `FEINF_RETURN_REG` 1189-1191; `FEINF_STRETURN_REG` 1215-1217; `FEINF_SAVE_REGS` 1174-1188; `FEINF_CALL_BYTES` 1192-1194; `FEExtName(EXTN_PATTERN)` = `aux->objname` (cfeinfo.c:751-795, 805-812).

### Pragmas

| Pragma | Sets | Back end told | Cite |
|---|---|---|---|
| `#pragma aux sym ...` (also `linkage`) | builds `aux_entry` + `aux_info` for `sym`; alias form `aux (alias) sym`, `aux (sym, alias)` | FindInfo(sym) returns the info; all FEINF_* above | cpragma.c:1786-1787; cpragx86.c:PragAux:936-1012; cpragma.c:GetPragmaAuxAliasInfo:278-304 |
| aux: merge rule | start from alias copy; `near` clears FAR_CALL; `routine` clears CALLER_POPS; `caller` (value struct) clears ROUTINE_RETURN; ORs cclass/cclass_target; `returns` taken only if SPECIAL_RETURN; `streturn` only if SPECIAL_STRUCT_RETURN; identical to alias -> shares alias (use count) | - | cpragx86.c:PragmaAuxEnd:151-206; cpragma.c:PragmaAuxEnding:523-561 |
| aux: `"objname"` | `objname` pattern (`*` = name) | `FEExtName` pattern | cpragma.c:PragObjNameInfo:564-571; cpragx86.c:957, 184-185 |
| aux: `= bytes / "asm"` | `info->code` byte_seq with FP/seg/offset fixup escapes (`FLOATING_FIXUP_BYTE`); `parms`/`modify` still apply; uses autos -> `FECALL_X86_NEEDS_BP_CHAIN` | `FEINF_CALL_BYTES`: BE expands inline; funcs with autos in pragma lose `FUNC_OK_TO_INLINE`, sym gets `SYM_USED_IN_PRAGMA` (-> `FE_MEMORY\|FE_ADDR_TAKEN\|FE_VOLATILE`) | cpragx86.c:968-970, 626-743, 352-578, 346-350, 417-423; cinfo.c:231-233; x86auxa.h:34 |
| aux: `far` / `near` | `FECALL_X86_FAR_CALL` set / cleared | far call | cpragx86.c:971-977 |
| aux: `loadds` | `FECALL_X86_LOAD_DS_ON_ENTRY` | callee loads DS | cpragx86.c:978-980 |
| aux: `rdosdev` | `FECALL_X86_LOAD_RDOSDEV_ON_ENTRY` | - | cpragx86.c:981-983 |
| aux: `export` | `FECALL_GEN_DLL_EXPORT` | export | cpragx86.c:984-986 |
| aux: `frame` | `FECALL_X86_GENERATE_STACK_FRAME` | frame | cpragx86.c:999-1001 |
| aux: `aborts` | `FECALL_GEN_ABORTS` | never returns; dead code after call (`FunctionAborts`) | cpragx86.c:993-995; cfeinfo.c:483-485; cexpr.c:2336 |
| aux: `parm [regs] [regs]..` | `parms` = list of reg sets (max `MAXIMUM_PARMSETS` 32) | `FEINF_PARM_REGS` | cpragx86.c:826-828, GetParmInfo:793-833; cpragma.c:PragManyRegSets:665-685 |
| aux: `parm caller` / `routine` | `FECALL_GEN_CALLER_POPS` set / cleared | stack cleanup | cpragx86.c:810-816 |
| aux: `parm reverse` | `FECALL_GEN_REVERSE_PARMS` | FE reverses parm decls (`ParmReverse`); `ParmsToBeReversed` | cpragx86.c:817-819; cgen.c:1237-1239 |
| aux: `parm nomemory` | `FECALL_GEN_NO_MEMORY_READ` | BE: callee reads no memory | cpragx86.c:820-822 |
| aux: `parm loadds` | `FECALL_X86_LOAD_DS_ON_CALL` | caller loads DS | cpragx86.c:823-825 |
| aux: `value [regs]` | `returns`, `FECALL_X86_SPECIAL_RETURN` | `FEINF_RETURN_REG` | cpragx86.c:894-897 |
| aux: `value no8087` | clears FLTS from returns, `FECALL_X86_NO_8087_RETURNS` | float result not in ST0 | cpragx86.c:890-893 |
| aux: `value struct float` | `FECALL_X86_NO_FLOAT_REG_RETURNS` | floats returned in memory | cpragx86.c:851-853 |
| aux: `value struct struct` | `FECALL_X86_NO_STRUCT_REG_RETURNS` | structs returned in memory | cpragx86.c:854-856 |
| aux: `value struct routine` / `caller` | `FECALL_X86_ROUTINE_RETURN` set / cleared | who allocates struct return | cpragx86.c:857-863 |
| aux: `value struct [regs]` | `streturn`, `FECALL_X86_SPECIAL_STRUCT_RETURN` | `FEINF_STRETURN_REG` | cpragx86.c:864-867 |
| aux: `modify [regs]` | `save` = regs turned OFF from FULL (modified). Unless `exact` or -r, floating/seg regs from default Watcall save are restored | `FEINF_SAVE_REGS` | cpragx86.c:996-998, GetSaveInfo:908-934, 193-204 |
| aux: `modify exact` | `FECALL_X86_MODIFY_EXACT` | only listed regs modified | cpragx86.c:921-923 |
| aux: `modify nomemory` | `FECALL_GEN_NO_MEMORY_CHANGED` | callee changes no memory | cpragx86.c:924-926 |
| `far16 sym` alias | `AUX_FLAG_FAR16` in `aux_info.flags` | `FindInfo` returns `Far16CdeclInfo`/`Far16PascalInfo` | cpragx86.c:GetPragmaAuxAlias:208-223; auxflags.h:32-34; cfeinfo.c:455-463 |
| aux: `interrupt`, `saveregs`, `caller/routine` at top level | not parsed by `PragAux` (only via `__interrupt`, `__saveregs` keywords) | - | cpragx86.c:967-1005 |
| `_asm` / `__asm` statement | inline function `F.n`: copy of WatcallInfo, `save=AsmRegsSaved`, `code`=bytes, `use=1`; call statement emitted | BE expands byte code inline | asmstmt.c:97-157; cpragma.c:CreateAuxInlineFunc:360-380; cpragx86.c:pragmaAuxInfoInit:57-68 |
| `#pragma intrinsic(f)` / `function(f)` | sets / clears `SYM_INTRINSIC` | `InfoLookup` replaces call info by inline table entry (`InlineInfo`: code, parms, returns, save) for x87, size, flat, bigdata variants | cpragma.c:1322-1352, 1814-1817; cfeinfo.c:217-287, 347-399 |
| `#pragma on/off/pop (name)` | `TOGGLE(name)`; names: check_stack, unreferenced, inline, reuse_duplicate_strings | check_stack -> `SYM_CHECK_STACK` -> `FEStackChk`; inline -> `FUNC_OK_TO_INLINE`; unreferenced -> FE warn; reuse_duplicate_strings -> string merge | cpragma.c:693-745, 1780-1785; h/togdef.h:34-37; cdecl1.c:122; cinfo.c:1002; cstmt.c:186; cdecl2.c:411; cstring.c:297 |
| `#pragma pack( n \| push[,n] \| pop \| )` | `PackAmount` (stack); `()` restores `GblPackAmount` | struct layout, `BEDefType` align | cpragma.c:844-919; cgen.c:1994-1999 |
| `#pragma enum int\|minimum\|original\|pop` | `make_enums_an_int` (stack) | enum storage size | cpragma.c:1281-1320 |
| `#pragma alloc_text(seg, fn,..)` | `sym.seginfo` = text segment; `multiple_code_segments=true` | `FESegID` returns that segment; BE `BEDefSeg` with `GIVEN_NAME` | cpragma.c:957-1013; cinfo.c:886-888, 677-680 |
| `#pragma code_seg(seg[,class])` | `DefCodeSegment`; `multiple_code_segments` | default text seg for later funcs | cpragma.c:1354-1394; cdecl1.c:187-189 |
| `#pragma data_seg(seg[,class])` | `DefDataSegment` via `AddSegName(SEGTYPE_DATA)` | var `segid`; `BEDefSeg(INIT\|GLOBAL)` | cpragma.c:1396-1431; cdecl2.c:559-567; cinfo.c:663-665 |
| `#pragma unroll(n)` | `UnrollCount` (0..255) | per-statement `unroll_count` -> `BEUnrollCount` | cpragma.c:1433-1459; cstmt.c:156; cgen.c:1629-1632 |
| `#pragma library[(names)]` | `AddLibraryName` (user priority) or `pragma_library=true` | `FEINF_NEXT_LIBRARY` / `FEINF_LIBRARY_NAME` -> default CRT lib also added | cpragma.c:747-820; cfeinfo.c:601-662 |
| `#pragma comment(lib, "x")` | same as library | same | cpragma.c:822-842 |
| `#pragma extref(sym\|"name")` | `ExtrefInfo` list | `FEINF_NEXT_IMPORT`(_S) forces external reference | cpragma.c:1609-1680; cfeinfo.c:990-1077 |
| `#pragma alias(a, b)` | `AliasHead` | `FEINF_NEXT_ALIAS`, `FEINF_ALIAS_*` -> linker alias record | cpragma.c:1682-1751; cfeinfo.c:664-711 |
| `#pragma warning # lvl`, `enable_message`, `disable_message`, `message`, `once`, `read_only_file`, `read_only_directory`, `include_alias`, `STDC` | FE diagnostics, include handling; STDC parsed and ignored | none | cpragma.c:1160-1206, 1208-1234, 1252-1279, 1567-1580, 1461-1509, 1511-1565, 1582-1607 |
| Not in this C front end | `segment`, `inline_depth`, `inline_recursion`, `initialize`, `init_seg`, `dump_object_model` (grep of c/ and h/: no hit; inline depth is fixed `MAX_INLINE_DEPTH 3`, cops.h:84). Unknown pragmas are skipped | - | cpragma.c:1834-1836; h/cops.h:84 |

### Keywords and attributes

| Attribute | FE flag | Back end told | Cite |
|---|---|---|---|
| `__cdecl __pascal __fortran __stdcall __syscall __fastcall __optlink __watcall` | `FLAG_CDECL..FLAG_WATCALL` in `sym->mods` | `FindInfo` -> `GetLangInfo(mods)` -> builtin aux_info (section 8) | cdecl2.c:907-918; flag values h/ctypes.h:94-101; cfeinfo.c:290-312, 465-467 |
| `__declspec(cdecl\|pascal\|fortran\|stdcall\|syscall\|fastcall\|optlink)` | language flag in `decl_mod` | same | ctype.c:520-537, 582-588 |
| `__declspec(dllimport)` / `overridable` | `DECLSPEC_DLLIMPORT` | `FE_DLLIMPORT` if import or -re; 16-bit sets `FLAG_FAR` | ctype.c:544-547; cinfo.c:243-247, 154-156 |
| `__declspec(dllexport)` | `DECLSPEC_DLLEXPORT` + `FLAG_EXPORT` | `FE_DLLEXPORT`, `FECALL_GEN_DLL_EXPORT` | ctype.c:548-549; cdecl2.c:609-611, 642-643; cinfo.c:248-252; cfeinfo.c:511-513 |
| `__declspec(thread)` | `DECLSPEC_THREAD` | `FE_THREAD_DATA`; var in `TS_SEG_TLS` segment | ctype.c:550-551; cinfo.c:253-255; cdecl2.c:405-408 |
| `__declspec(naked)` | `attribs.naked` | `FE_NAKED`; not inlined | ctype.c:552-557; cinfo.c:257-259; cstmt.c:188 |
| `__declspec(aborts)` / `__aborts` | `FLAG_ABORTS` | `FECALL_GEN_ABORTS`; `FunctionAborts` | ctype.c:558-560; cfeinfo.c:477-478, 505-507 |
| `__declspec(noreturn)` / `_Noreturn` | `FLAG_NORETURN` | `FECALL_GEN_NORETURN`; `FunctionAborts` | ctype.c:561-564, 458-459; cfeinfo.c:479-480, 508-510 |
| `__declspec(farss)` | `FLAG_FARSS` | `FECALL_X86_FARSS`; `FLOATING_SS` for that function | ctype.c:565-566; cfeinfo.c:551-553; cgen.c:1189-1195 |
| `__interrupt` | `FLAG_INTERRUPT` (= NEAR+FAR), implies `FLAG_FARSS` | `FAR_CALL` + `FECALL_X86_INTERRUPT`; no obsolete-decl warning | cdecl2.c:906, 196-198, 1419; ctypes.h:91; cfeinfo.c:557-561 |
| `__loadds` | `FLAG_LOADDS` | `FECALL_X86_LOAD_DS_ON_ENTRY` | cdecl2.c:921; cfeinfo.c:565-575 |
| `__saveregs` | `FLAG_SAVEREGS` | `FEINF_SAVE_REGS` adds `HW_SEGS` to save set | cdecl2.c:922; cfeinfo.c:1179-1181 |
| `__export` | `FLAG_EXPORT` | `FECALL_GEN_DLL_EXPORT`; 16-bit forces FAR | cdecl2.c:919-920; cfeinfo.c:511-513; cinfo.c:157-158 |
| `__near` / `__far` / `__huge` | `FLAG_NEAR/FAR/HUGE` (386: `__huge` = far, `__far16` = `FLAG_FAR16`) | pointer type (`TY_NEAR_POINTER`/`TY_LONG_POINTER`/`TY_HUGE_POINTER`); function: `FAR_CALL` set/cleared; var seg placement | cdecl2.c:895-905; cgen.c:2035-2058; cfeinfo.c:557-564; cinfo.c:161-185 |
| `__far16` / `_Far16` (386) | `FLAG_FAR16` | `Far16*Info`, args widened to 16-bit | cdecl2.c:902-903; cfeinfo.c:455-463; cinfo.c:987-993 |
| `__based(...)`, `__segment`, `__segname("x")`, `__self` | `FLAG_BASED`, `based_kind`, `segid` (segname via `AddSegName(SEGTYPE_BASED)`; `seg:` reg pegs) | `PTRCLS_BASED`; `PushSymSeg`; `FEINF_PEGGED_REGISTER`; segment def `INIT\|PRIVATE\|GLOBAL` | cdecl2.c:968-1045; cmath.c:513-514; cgen.c:358; cinfo.c:419-447, 666-668; cfeinfo.c:1169-1170 |
| `inline` / `__inline` | `FLAG_INLINE` | FE marks `FUNC_OK_TO_INLINE` even without -oe; `FECALL_GEN_MAKE_CALL_INLINE` when body known and depth < 3 | ctype.c:454-457; cstmt.c:186-191; cgen.c:IsInLineFunc:1709-1726; cfeinfo.c:514-516 |
| `volatile` | `FLAG_VOLATILE` | `FE_MEMORY\|FE_VOLATILE`; `const` -> `FE_CONSTANT` (const data to `SEG_CONST2` unless -re) | cinfo.c:237-241, 286-299 |
| `_Packed` | `packed` flag; struct parsed with `PackAmount=1` | layout | ctype.c:461-464, 1188-1190 |

### Call-class bits

| Bit | Source in FE | BE assumption | BE cite |
|---|---|---|---|
| `FECALL_GEN_ABORTS` | `#pragma aux ... aborts`, `__declspec(aborts)` | call never returns: `CALL_ABORTS`, which implies `CALL_WRITES_NO_MEMORY`; no return address/far-ret slot; `GenNoReturn` after call; code after call is dead | cpragx86.c:993-995; ctype.c:558-560; cfeinfo.c:505-507; x86reg.c:88-90; x86call.c:199-211; x86enc2.c:388, 412-415; x86proc.c:91-92, 914-915 |
| `FECALL_GEN_NORETURN` | `_Noreturn`, `__declspec(noreturn)` | as above via `CALL_NORETURN` (write-no-memory, `GenNoReturn`); differs from ABORTS: frame still built | ctype.c:458-459, 561-564; cfeinfo.c:508-510; x86reg.c:91-93; x86call.c:199-214; x86enc2.c:412-415 |
| `FECALL_GEN_NO_MEMORY_READ` | `#pragma aux ... parm nomemory` | `CALL_READS_NO_MEMORY`: call does not read memory, so stores before it need not be kept, scheduler may move loads/stores across it | cpragx86.c:820-822; x86reg.c:113-115; x86call.c:206-208; inssched.c:440; loadstor.c:99-100; varusage.c:377-378; savcode.h:173-174 |
| `FECALL_GEN_NO_MEMORY_CHANGED` | `#pragma aux ... modify nomemory` | `CALL_WRITES_NO_MEMORY`: memory values stay valid across the call (CSE, loop invariants, register-held globals) | cpragx86.c:924-926; x86reg.c:110-112; x86call.c:199-205; redefby.c:273; scblock.c:191; loopopts.c:547; savcode.h:165 |
| `FECALL_GEN_MAKE_CALL_INLINE` | FE: body known, `FUNC_OK_TO_INLINE`, depth < `MAX_INLINE_DEPTH` (3) | BE switches to `BGStartInline` instead of `BGInitCall` | cfeinfo.c:514-515; cgen.c:1709-1726; tree.c:2323-2328 |
| `FECALL_GEN_PARMS_BY_ADDRESS` | never set by the C FE (no hit in bld/cc for this bit) | BE would pass each parm by address (temp) | tree.c:2336-2345 |
| `FECALL_GEN_SETJMP_KLUGE` | never set by the C FE; read only by axp and mips `*reg.c` | - | axpreg.c:102; mpsreg.c:119 |
| `FECALL_GEN_HAS_VARARGS` / `CALLER_POPS` | `VarFunc` | varargs: caller pops, `DefaultVarParms` | cfeinfo.c:517-519, 1207-1211; x86reg.c:85-87 |
| `FECALL_X86_MODIFY_EXACT` | `modify exact` | `ROUTINE_MODIFY_EXACT`: exactly the listed regs change; nothing more is assumed clobbered | cpragx86.c:921-923; x86reg.c:107-109 |
| `FEINF_SAVE_REGS` set | `modify [regs]`; builtin conventions | `state->modify = FULL - save`; regs in `save` stay live across the call, so the allocator keeps values there; bigger `save` = fewer spills | cfeinfo.c:1174-1188; x86reg.c:63-68; cpragx86.c:193-204 |
| inline asm `save = AsmRegsSaved` | `_asm` and `= bytes` | only listed regs (eax ebx ecx edx esi edi, es 16-bit) are modified | cpragma.c:372; cpragx86.c:57-68 |
| `FEINF_PARM_REGS`, `RETURN_REG`, `STRETURN_REG` | `parm [..]`, `value [..]` | custom register convention; no stack traffic | cfeinfo.c:1195-1217 |
| `FE_CONSTANT` | `const` object | value is OK across calls (`CST_OK_ACROSS_CALLS`) | cinfo.c:239-241; conflict.c:91-93 |
| `FE_VOLATILE` + `FE_MEMORY` | `volatile`, `SYM_USED_IN_PRAGMA`, `SYM_TRY_VOLATILE` | no caching in regs | cinfo.c:231-238 |
| `FE_UNIQUE` | -ou / -za | function address identity kept | cinfo.c:225-229 |

## 5. Debug stream

### Levels

| flag | sets | cite |
|---|---|---|
| none / -d0 | nothing | cc/c/coptions.c:551-552 |
| -d1 | DBG_NUMBERS only | cc/c/coptions.c:549 |
| -d1+ | NUMBERS, TYPES, LOCALS, `debug_info_some`. Optimiser stays on. | cc/c/coptions.c:545-546 |
| -d2 | NUMBERS, TYPES, LOCALS, NO_OPTIMIZATION, oi off | cc/c/coptions.c:535,541-542 |
| -d2~ | as -d2 plus `no_debug_type_names` (no typedef names) | cc/c/coptions.c:532,535 |
| -d3 | as -d2 plus `dump_types_with_names` | cc/c/coptions.c:521-522,528-529 |
| -d9 | only `use_full_codegen_od` | cc/c/coptions.c:517-518 |
| -hw | no format bit; BE falls to Watcom format (else branch) | cc/c/cmdlnx86.c:520-521; cg/c/dbsyms.c:514-517 |
| -hc | CGSW_GEN_DBG_CV | cc/c/cmdlnx86.c:517-518 |
| -hd (default) | CGSW_GEN_DBG_DF | cc/c/cmdlnx86.c:530-532 |
| -hda / -hdg | DF + PREDEF, creates `__DFABBREV` sym (extern / global) | cc/c/cmdlnx86.c:522-528 |
| -db | `emit_browser_info`: separate DWARF browse pass (section 6) | cc/c/coptions.c:625; cc/c/ccmain.c:1101-1102 |

Format bits are cleared and reset in `SetDebugInfoFormat` (cc/c/cmdlnx86.c:515). RISC copy: cc/c/cmdlnrsc.c:191-210.

### DB calls

| call | carries | emitted when | cite |
|---|---|---|---|
| DBSrcFile | full path of source file, once per FNAME (`index_db` cache) | first tree node of a file, any level with cues | cc/c/cgen.c:1619-1623 |
| DBSrcCue | (BE file index, line, col=1 always) | each statement tree whose line/file changed; called at every level (BE filters) | cc/c/cgen.c:1615-1625 |
| DBModSym(func) | function symbol, TY_DEFAULT | function definition start, LOCALS, not inline | cc/c/cgen.c:1222-1224 |
| DBModSym(global) | every non-function, non-temp, non-typedef GlobalSym incl. extern (BE drops FE_IMPORT) | `EmitSyms` at module start, LOCALS | cc/c/cgen.c:1842-1851; cg/c/dbsyms.c:525-549 |
| DBLocalSym | auto / param / static-local symbol, TY_DEFAULT | see section 4 | cc/c/cgen.c:1134,1159 |
| DBBegBlock | no args | inline function body start (LOCALS) | cc/c/cgen.c:1225-1226 |
| DBBegBlock | no args | every OPR_NEWBLOCK (compound stmt, C99 `for` decl scope); no level check in cc | cc/c/cgen.c:1290-1291; cc/c/cstmt.c:528,720 |
| DBEndBlock | no args | every OPR_ENDBLOCK | cc/c/cgen.c:1294-1295; cc/c/cstmt.c:982 |
| DBEndBlock | no args | end of inlined function, LOCALS | cc/c/cgen.c:302-304 |
| DBScope | "struct", "union", "enum" scope handles | `InitDBType`, TYPES | cc/c/cdebug.c:51-53 |
| DBScalar | "char","signed char","unsigned char","short","unsigned short","int","unsigned int","long","unsigned long","__int64","unsigned __int64","_Bool" with cg_type | `InitDBType`, TYPES; always, eagerly | cc/c/cdebug.c:54-75 |
| DBScalar | "void" TY_DEFAULT, "float", "double", "long double" | lazy in `DBType` | cc/c/cdebug.c:244-255 |
| DBBegName / DBEndName | name + scope; wraps a type to name it | named struct/union (scope struct/union), typedef (scope NIL), named enum (scope enum) | cc/c/cdebug.c:276,283,306,312,405 |
| DBBegName("") + DBForward | anonymous name and forward ref | recursion hit on a type in progress (`DBG_FWD_TYPE`) | cc/c/cdebug.c:225-232 |
| DBTypeDef | typedef name + type | CV only, typedef reached via `DBType` | cc/c/cdebug.c:313-314 |
| DBBegName+DBEndName for SC_TYPEDEF sym | typedef name wraps `DBType(aliased type)` | TYPES, every global typedef sym in `EmitSyms`; skipped if aliased type is itself TYP_TYPEDEF; ignores `no_debug_type_names` | cc/c/cgen.c:1030-1033,1842 |
| DBIntArrayCG | array cg_type, hi = `dimension-1` (0 when dimension 0), element type | array type reached | cc/c/cdebug.c:256-262 |
| DBPtr | pointer cg_type (`PtrType`: near/far/huge/default), pointee type | non-based pointer | cc/c/cdebug.c:264-268; cc/c/cgen.c:2035-2056 |
| DBBasedPtr + DBLocInit/DBLocConst/DBLocOp(MK_FP, POINTS)/DBLocSym/DBLocFini | based pointer: loc = `[sym] points u16, 0, mk_fp`; sym NULL gives `0, mk_fp` | `__based(sym)` or `__based(void)` pointer | cc/c/cdebug.c:172-212 |
| DBBegNameStruct, DBStructForward, DBAddField, DBAddBitField, DBEndStruct | tag name ("" if anon), cg_type of struct, is_struct flag; field offset+name+type; bitfield offset+start+width+name+int type | struct/union reached | cc/c/cdebug.c:332-390 |
| DBBegEnum, DBAddConst64, DBEndEnum | underlying cg_type; name+64-bit value per enumerator | enum reached | cc/c/cdebug.c:393-408 |
| DBBegProc, DBAddParm, DBEndProc | TY_CODE_PTR, return type, param types (stop at `...`) | function type reached | cc/c/cdebug.c:287-299 |

Not called by cc at all (verified by grep over `cc/c`, `cc/h`, excluding `cgstub.c`): DBLineNum, DBLocalType, DBFtnType, DBDereference, DBIndCharBlock, DBLocCharBlock, DBCharBlock, DBCharBlockNamed, DBNested, DBArray, DBBegArray, DBDimCon, DBDimVar, DBEndArray, DBFtnArray, DBIntArray, DBSubRange, DBBegStruct, DBAddStField, DBAddMethod, DBAddNestedType, DBAddConst, DBAddMethParms, DBGenSym, DBGenStMem, DBAddLocField, DBAddInheritance, DBAddBaseInfo, DBAddVFuncInfo, DBLocTemp, DBObject. (`cc/c/cgstub.c:74-95` is a stub list, not callers.)

### FE callbacks the BE uses for debug

| callback | returns | cite |
|---|---|---|
| FEDbgType(sym) | `DBType(sym->sym_type)`; this is how locals/globals/params get types, lazily, at the moment the BE emits the symbol | cc/c/cdebug.c:410-415; cg/c/dfsyms.c:976,1006 |
| FEDbgRetType(sym) | `DBType(return type)` if function else DBG_NIL_TYPE | cc/c/cdebug.c:417-428 |
| FEAuxInfo FEINF_DBG_PREDEF_SYM | `SymDFAbbr` (-hda/-hdg) | cc/c/cfeinfo.c:1108 |
| FEAuxInfo FEINF_DBG_DWARF_PRODUCER | `DWARF_PRODUCER_ID` | cc/c/cfeinfo.c:1172 |

No other debug-specific query was found in cfeinfo.c (grep for dbg/debug: only the two above and a SYM_TEMP test at 435 that is for aux lookup).

### Eager vs lazy, and who gets locals

| what | eager or lazy | level | cite |
|---|---|---|---|
| base scalars (12 names) | eager, module start | TYPES (-d1+, -d2, -d3) | cc/c/cgen.c:1934-1935; cc/c/cdebug.c:47-77 |
| `EmitDBType` walk of all types: struct/union/enum with a tag, typedef types | eager, but only if `dump_types_with_names`, so **-d3 only** ("unused types"); typedef also skipped under -d2~ | -d3 | cc/c/cdebug.c:96-127 |
| global typedef symbols | eager (by name) in `EmitSyms` | TYPES | cc/c/cgen.c:1030-1033 |
| every other type (struct used by a variable, pointer, array, function) | lazy, on first `FEDbgType` from BE, cached in `typ->u1.debug_type` | TYPES | cc/c/cdebug.c:225-233,327 |
| global variable | `DBModSym` for all | LOCALS | cc/c/cgen.c:1842-1851 |
| parameter | `DBLocalSym` if NO_OPTIMIZATION (-d2/-d3); else only 386 && `!register_conventions` && -d1+ | LOCALS | cc/c/cgen.c:1153-1160 |
| auto local | `DBLocalSym` if NO_OPTIMIZATION; at -d1+ only if addr-taken, struct, union, array, complex (`emit_extra_info`) | LOCALS | cc/c/cgen.c:1084-1134 |
| static local | `DBLocalSym` once (first emit, `SYM_EMITTED`), any LOCALS level | LOCALS | cc/c/cgen.c:1089-1096,1130-1134 |
| temps (`SYM_TEMP`) | never | | cc/c/cgen.c:1133 |
| local typedefs, local struct/union/enum tags | no call; only reached lazily if a debugged symbol uses them; unused ones never (except -d3 walk, which walks the global type hash lists, see `WalkTypeList` cc/c/ctype.c:161-173; whether local tags are in those lists not checked) | | cc/c/cgen.c:1099-1100 (autos skip SC_TYPEDEF) |

`DBG_NUMBERS` gates line numbers in BE at cg/c/object.c:86. -d1 (no TYPES): BE `DBSrcCue` keeps only `fno == 0 && col == 1` and calls DBLineNum, so lines from a file whose BE index is not 0 are dropped (cg/c/dbsyms.c:431-435). Index 0 is the first file passed to DBSrcFile (cg/c/dbsyms.c:98-113). With TYPES set a full cue is added (cg/c/dbsyms.c:436-438).

### How each type kind is described

| C type | described as | cite |
|---|---|---|
| char, short, int, long, __int64, unsigned, _Bool | prebuilt DBScalar handles via `DBIntegralType` | cc/c/cdebug.c:129-170 |
| plain `char` | its own "char" scalar, signed or unsigned per option | cc/c/cdebug.c:54-59 |
| float, double, long double, void | DBScalar on demand | cc/c/cdebug.c:244-255 |
| pointer | DBPtr(cg_type, pointee); near/far/huge from `PtrType` | cc/c/cdebug.c:263-270 |
| based pointer | DBBasedPtr with loc program; CS/SS/segment-label base collapses to plain DBPtr | cc/c/cdebug.c:172-212 |
| array | DBIntArrayCG, one dimension per array node (multi-dim = nested), hi = count-1 | cc/c/cdebug.c:256-262; count: `TypeSize` of array = `dimension`, cc/c/ctype.c:1638-1640 |
| struct / union | DBBegNameStruct, fields in list order with byte offsets; anonymous member struct/union flattened into parent at `bias+offset`; trailing flexible array member gets the real element-array type | cc/c/cdebug.c:332-390 |
| bitfield | DBAddBitField(offset, start bit, width, name, integral type of declared field type) | cc/c/cdebug.c:348-354 |
| enum | DBBegEnum(cg_type of underlying) + DBAddConst64 per enumerator in `thread` order | cc/c/cdebug.c:393-408 |
| function | DBBegProc(TY_CODE_PTR, ret) + DBAddParm per param up to `...` | cc/c/cdebug.c:287-299 |
| typedef | DBBegName(name)/DBEndName around aliased type (+DBTypeDef in CV); name omitted under -d2~ | cc/c/cdebug.c:300-318 |
| recursive struct/typedef | DBBegName first, `DBG_FWD_TYPE` marker, DBForward on re-entry | cc/c/cdebug.c:225-232,273-285 |
| const / volatile | **dropped** (see gaps) | cc/c/cdebug.c:301-302 |
| function locals in nested blocks | DBBegBlock/DBEndBlock pairs around OPR_NEWBLOCK/ENDBLOCK; the block's syms are declared right after DBBegBlock by `CDoAutoDecl` | cc/c/cgen.c:1290-1295; cc/c/cstmt.c:515-531 |
| register / stack location of a local | FE sends only the symbol (`DBLocalSym(sym, TY_DEFAULT)`); BE builds `DBLocInit; DBLocSym(sym)` itself. How the BE resolves that to a register or frame offset was not traced. | cg/c/dbsyms.c:595-613 |

### dwarf.c (browse info, -db)

Separate path: uses the DW* library directly (`DWDeclFile`, `DWStruct`, ...), not DB*. Runs once before `DoCompile` (cc/c/ccmain.c:1101-1103), only if `emit_browser_info`.

| item | carries | cite |
|---|---|---|
| fundamentals | bool, char, uchar, wchar_t, short, int, long, __int64, float, double, long double, void with size | cc/c/dwarf.c:321-380,409-412 |
| pointer, typedef (with decl line), enum, array (`DWSimpleArray`), function type (params, ellipsis) | | cc/c/dwarf.c:150-270,384-407 |
| struct/union | fields with decl position, bitfields, DECLARATION flag if size 0 | cc/c/dwarf.c:96-187 |
| functions | near/far/far16 call type, static flag, params, return type, decl position; locals | cc/c/dwarf.c:429-522 |
| variables | type, decl position, global flag; no location (`dummyLoc`) | cc/c/dwarf.c:524-545 |
| lexical blocks | DWBeginLexicalBlock per OPR_NEWBLOCK | cc/c/dwarf.c:591-597 |
| references | DWReference for every PUSHSYM/PUSHADDR/FUNCNAME | cc/c/dwarf.c:598-611 |
| const/volatile modifier helper | `#if 0`, unused | cc/c/dwarf.c:272-307 |

### Not described (gaps)

| gap | evidence |
|---|---|
| const / volatile / restrict / unaligned on any type. `TF2_DUMMY_TYPEDEF` is tested in `DBType` but no code in `cc/` sets it; pointer `decl_flags` const/volatile never reach `PtrType` output. | cc/c/cdebug.c:301-302; cc/h/ctypes.h:234; `PtrType` reads near/far/huge only, cc/c/cgen.c:2044-2050 |
| `wchar_t`, `_Imaginary`, `_Complex` (TYP_WCHAR, TYP_FIMAGINARY..LDIMAGINARY, TYP_FCOMPLEX..LDCOMPLEX) fall to `B_Int` | cc/c/cdebug.c:242,323-325,133-168 (no case for them); list in cc/h/cdatatyp.h |
| array `a[]` and `a[1]` both give hi=0 | cc/c/cdebug.c:256-262 |
| variadic `...` and K&R (no prototype) not recorded; calling convention, far/near function type not recorded (always TY_CODE_PTR) | cc/c/cdebug.c:288,291-296 |
| function parameter names and locations inside function *types* (only types) | cc/c/cdebug.c:295 |
| based pointers on CS, SS or segment label degrade to plain pointer; `based_kind` ignored | cc/c/cdebug.c:188-199 |
| column (always 1); no statement-begin vs expression granularity, no `is_stmt` | cc/c/cgen.c:1625 |
| line cues of included files at -d1 | cg/c/dbsyms.c:431-435 |
| function inlining: `Saved_CurFunc` is declared and tested but never assigned, so the "not inlining" guards on cues and locals are always true | cc/c/cgen.c:78,1131,1615 (only three hits in `cc/`) |
| inlined-function locals at -d1+ / non-debug-opt: block is opened, no per-call-site info (no DW_TAG_inlined_subroutine) | cc/c/cgen.c:1222-1226 |
| variables declared in a block at -d1+ that are not addr-taken/aggregate (optimiser may register them) | cc/c/cgen.c:1100,1102,1113,1127-1128 |
| local typedefs and local struct/enum tags as scoped entities (DBLocalType never called) | grep, section 2 |
| global function declarations (prototype-only, extern) | DBModSym only for definitions: cc/c/cgen.c:1222-1224, and non-function syms at 1842-1851 |
| unnamed enum/struct/union get no DBBegName, but named enum returns `DBEndEnum` result and the name is wrapped separately and its return dropped | cc/c/cdebug.c:403-407 |
| bitfield storage unit/container type beyond integral type of declared type; signedness of `int` bitfield comes from `field_type` only | cc/c/cdebug.c:350-354 |
| macros, preprocessor defines, `#line` names | no call exists in cc; not searched beyond DB* inventory |
| C++ members/methods/inheritance (DBAdd{Method,StField,Inheritance,...}) | unused by C front end |
| enum tag for enum inside `typedef enum {..} T;` : tag name empty so only typedef wraps it | cc/c/cdebug.c:404 |
| `restrict` qualifier, `_Alignas`, attribute info | no path in cdebug.c |

### Lossy or dropped facts (verified)

| fact | what is lost or available | cite |
|---|---|---|
| const/volatile on pointee or pointer | DBPtr gets only cg_type from `PtrType`; qualifiers never passed | cc/c/cdebug.c:264,268; cc/c/cgen.c:2035-2056 |
| qualified non-pointer types | dummy-typedef branch returns the bare type; nothing sets the flag | cc/c/cdebug.c:301-302; cc/h/ctypes.h:234 |
| block scopes | available: one DBBegBlock/DBEndBlock pair per compound stmt, locals declared inside | cc/c/cgen.c:1290-1295 |
| block scopes at optimised levels | -d1+ declares only addr-taken/aggregate locals, so scopes may be empty | cc/c/cgen.c:1100-1134 |
| locations | FE passes the symbol only; no register/stack info from the FE | cc/c/cgen.c:1134,1159; cg/c/dbsyms.c:610 |
| based pointer base | CS/SS/segment label collapse to plain DBPtr | cc/c/cdebug.c:188-199 |
| line column | always 1 | cc/c/cgen.c:1625 |
| -d1 include-file lines | dropped by BE (only BE file index 0 kept) | cg/c/dbsyms.c:431-435 |
| array bound | count-1; unknown size and size 1 both give 0 | cc/c/cdebug.c:256-262 |
| wchar_t, imaginary, complex | described as int | cc/c/cdebug.c:242,323-325 |
| function type | fixed TY_CODE_PTR; `...` ends param list | cc/c/cdebug.c:288-296 |

## 6. Mapping onto llrm

How it was read: `wccq` (Open Watcom's front end linked to `toolchain/owshim/cgshim.c`) writes the stream; `crates/frontends/llrm-c/src/hir.rs` parses it; `translate.rs` makes HIR; the driver prints MIR to `--dump DIR/01-globalopt.ll`. `recorded` passes a fixed flag set (`compile.rs:recorded`, `-mm -3 -fpi87 -fp3 -fld -j -zp1 -ei -ecc -s -zl -zq`, plus `-d2` for `-g`). The probes ran `wccq` with that line and `QBOPT_CG_STREAM`, and `llrm-c --dump` on the same file.

Status: **kept** reaches MIR as a fact; **dropped** the stream carries it and `llrm-c` discards it; **never asked for** Open Watcom's front end holds it and no channel carries it (or the shim never queries it); **refused** `llrm-c` stops the compile.

### Fact by fact

| Fact | Open Watcom emits | cgshim.c | Lands in HIR / MIR | Status | Probe |
|---|---|---|---|---|---|
| volatile access | `CGVolatile` before `O_POINTS`, and `FE_VOLATILE` on the symbol (`cgen.c:PushSym:588`, `cinfo.c:FESymAttr:232-238`) | records both (`cgshim.c:CGVolatile`, `sym`:234) | `IndirectPlace.volatile` (`translate.rs:1291`) → `load volatile` | kept | volatile |
| const object | `FE_CONSTANT` and a read-only segment (`cinfo.c:237-241`) | `attr=` in `SYM` | `hir.rs:FE_CONSTANT`; `translate.rs:205` → `constant` global | kept | const |
| const pointee (`const T *p`) | nothing: the qualifier stays in the type (`ctype.c`, see §2 "const / volatile") | nothing to record | no `readonly` on the parameter or the load | never asked for | const |
| restrict | discarded by Open Watcom (`FLAG_RESTRICT` parsed, unused); our `cgen.c.patch` adds `CGAttr n 3` | records `CGAttr` (`cgshim.c:609`) | `Promise{unaliased}` (`translate.rs:849`) → `noalias` | kept (ours, beyond Open Watcom) | restrict |
| restrict + const pointee | same | same | `Promise.readonly` is always `false` (`translate.rs:851`) | never asked for | restrict |
| noreturn, `aborts` | `FECALL_GEN_NORETURN 0x4`, `FECALL_GEN_ABORTS 0x2` in call class (`cfeinfo.c:getCallClass:505-510`) | `CALLCONV class=` (`cgshim.c:268`) | `hir.rs` reads only `HAS_VARARGS`, `REVERSE_PARMS`, `CALLER_POPS` (`hir.rs:176-181`). HIR has no noreturn field (`llrm-hir/src/model.rs`); MIR has the `noreturn` attribute and `A/noreturn.rs:terminal_sites:103` reads it. The call is followed by `br` | dropped | noreturn |
| no memory read / write | `FECALL_GEN_NO_MEMORY_READ 0x100`, `NO_MEMORY_CHANGED 0x200`, only from `#pragma aux nomemory` | `class=0x380` | `hir.rs` ignores it. HIR has `Instruction.pure` (`model.rs:471`) and `H/mir.rs` drops it; MIR `memory(none)` / `readnone` on a call is read by `M/memory.rs:at:155-177`. The two `sq` calls stay | dropped | nomemory |
| `#pragma aux` parameter and return registers | `FEINF_PARM_REGS`, `FEINF_RETURN_REG` | `parms=[3:0,c0:0] ret=3:0` | `translate.rs:cleanup:450` | refused | pragma aux |
| `#pragma aux modify [regs]` | `FEINF_SAVE_REGS` | never queried (`cgshim.c` asks only AUX_LOOKUP, PARM_REGS, RETURN_REG, CALL_CLASS, CALL_CLASS_TARGET, CALL_BYTES, SOURCE_NAME) | nothing | never asked for | modify |
| inline byte code | `FEINF_CALL_BYTES` | `CODE y bytes= fix=` (`cgshim.c:278`) | call of `llrm.ia16.code.<hex>` (`translate.rs:1594`) | kept | inline code |
| bit field | `CGBitMask(addr,start,width,type)` (`cgen.c:DotOperator:667`) | records | `translate.rs:1295` refuses; debug `DBBitField` keeps offset, name, type, drops start and width (`hir.rs:413`) | refused | bit fields |
| switch | `CGSelCase` per value, `CGSelOther`, `CGSelect`; never `CGSelRange` | records all | `switch` terminator (`translate.rs:939`) | kept; ranges never sent | switch, run |
| varargs | call class `HAS_VARARGS`; no call for `va_start` on x86-16 (`hdr/watcom/stdarg.mh:93-100` is pointer arithmetic) | `class=` | `i16 @_sum(i16, ...)` and `llvm.va_start` | kept | varargs |
| struct copy | `CGLVAssign(dst, src, refno)`, size from `BEDefType` | `TYPE T25 size=20`, `CGLVAssign` | expanded to word loads and stores, no bulk-copy op | kept, as scalars | struct copy |
| far pointer | `TY_LONG_POINTER` | type name | `ptr addrspace(1)` | kept | far |
| based pointer | lowered by the front end to `O_CONVERT(off, seg)` with the symbol `.DS` (`cgen.c:EmitNodes:1510`) | `SYM name=".DS" attr=0x1042` | `translate.rs:1313` refuses; `DBBasedPtr` refused (`cgshim.c:972`) | refused | based |
| interrupt | target class `INTERRUPT 0x8` | `target=0x20071e` | `call_target & INTERRUPT` (`hir.rs:170`) → `x86_intrcc` | kept | interrupt |
| unaligned | `CGAttr(n, CG_SYM_UNALIGNED)` = 2 (`cgen.c:PushSym:585`) | `CGAttr n 2` | `translate.rs:1290` evaluates the inner node; no `align 1` on the load | dropped | unaligned |
| static initialisers | `DGInteger`, `DGBytes`, `DGFEPtr`, `DGBackPtr`, zero fill | all recorded | `DataObject` bytes and relocations | kept | init |
| address taken (`&x`) | never: `FE_ADDR_TAKEN` only for variables a pragma names (`cinfo.c:FESymAttr:231-233`) | `attr=0x0` for `x` in `g(&x)` | derived by our passes | never asked for | address |
| constant folding, sizeof, promotions | folded before the call; operands typed by the result | values | constants in MIR | kept | folding |
| signed overflow | `TY_INTEGER` vs `TY_UNSIGNED` | type name | `nsw` on signed ops (`translate.rs` header) | kept | folding |
| dead code (`if (0)`) | the dead statements are still emitted | recorded | dead block `b3` stays until a pass removes it | kept | folding |
| inline body | the back end asks `FEGenProc`, the front end then replays the callee (`cinfo.c:FEGenProc:267-275`) | not implemented, so never asked; `static sq` is emitted as a function and called | our inliner decides | never asked for | inline |
| relax alias `-oa`, loop flags `-ol`, time vs size `-ot`/`-os` | `cg_switches`, `size` in `BEInit` (`cgen.c:DoCompile:1924`) | `INIT sw= size=` | `hir.rs:386` reads only `target` | dropped | flags |
| `-d2` types, names, locals | §5 | `DB*` records | `Debug` types (`debug.rs`) | partly kept | debug |
| debug: qualifiers, enum constants, block scopes, typedef names, columns | see §5 gaps | `DBConst`, `DBBegBlock`, `DBEndBlock`, `DBTypeDef` are in `IGNORED` (`hir.rs:57-69`) | nothing | dropped | debug |

### Probes

Each probe: the C, the stream (lines for `LastParm`, `CGTemp` omitted), and the MIR as raised. The `recorded` flag set is fixed, so `restrict` needs `-za99` and was spelled `__restrict`.

#### volatile

```c
volatile int v;
int f(void){ return v + v; }
```

Recorded stream:

```
SYM y1 name="v" base="v" pattern="_*" attr=0x826 seg=11
b1 BENewBack y1
- DGLabel b1
- DGUBytes 2
SYM y2 name="f" base="f" pattern="_*" attr=0x7 seg=1
CALLCONV y2 class=0x80 target=0x716 parms=[] ret=0:0
- CGProcDecl y2 TY_INTEGER
n5 CGFEName y1 TY_INTEGER
n6 CGVolatile n5
n7 CGUnary O_POINTS n6 TY_INTEGER
n8 CGFEName y1 TY_INTEGER
n9 CGVolatile n8
n10 CGUnary O_POINTS n9 TY_INTEGER
n11 CGBinary O_PLUS n7 n10 TY_INTEGER
n12 CGUnary O_CONVERT n11 TY_INTEGER
n13 CGTempName t4 TY_INTEGER
n14 CGAssign n13 n12 TY_INTEGER
- CGDone n14
n15 CGTempName t4 TY_INTEGER
n16 CGUnary O_POINTS n15 TY_INTEGER
- CGReturn n16 TY_INTEGER
```

MIR as raised (`01-globalopt.ll`; `alloca` and `!tbaa` removed):

```
@_v = global [2 x i8] zeroinitializer
define i16 @_f() addrspace(1) {
b1:
  %1 = load volatile i16, ptr @_v
  %2 = load volatile i16, ptr @_v
  %3 = add nsw i16 %1, %2
  store i16 %3, ptr %0
  %4 = load i16, ptr %0
  ret i16 %4
}
```

#### const

```c
const int k = 7;
int g(const int *p);
int f(const int *p, int *q){ *q = 1; return *p + k + g(p); }
```

Recorded stream:

```
SYM y1 name="k" base="k" pattern="_*" attr=0x16 seg=3
b1 BENewBack y1
- DGLabel b1
- DGInteger 7 TY_INTEGER
SYM y2 name="f" base="f" pattern="_*" attr=0x7 seg=1
CALLCONV y2 class=0x80 target=0x716 parms=[] ret=0:0
- CGProcDecl y2 TY_INTEGER
SYM y3 name="p" base="p" pattern="_*" attr=0x0 seg=2
- CGParmDecl y3 TY_POINTER
SYM y4 name="q" base="q" pattern="_*" attr=0x0 seg=2
- CGParmDecl y4 TY_POINTER
n5 CGFEName y4 TY_POINTER
n6 CGUnary O_POINTS n5 TY_POINTER
n7 CGInteger 1 TY_INTEGER
n8 CGAssign n6 n7 TY_INTEGER
- CGDone n8
n9 CGFEName y3 TY_POINTER
n10 CGUnary O_POINTS n9 TY_POINTER
n11 CGUnary O_POINTS n10 TY_INTEGER
n12 CGFEName y1 TY_INTEGER
n13 CGUnary O_POINTS n12 TY_INTEGER
n14 CGBinary O_PLUS n11 n13 TY_INTEGER
SYM y5 name="g" base="g" pattern="_*" attr=0xf seg=1
CALLCONV y5 class=0x80 target=0x716 parms=[] ret=0:0
n15 CGFEName y5 TY_CODE_PTR
c16 CGInitCall n15 TY_INTEGER y5
n17 CGFEName y3 TY_POINTER
n18 CGUnary O_POINTS n17 TY_POINTER
- CGAddParm c16 n18 TY_POINTER
n19 CGCall c16
```

MIR as raised (`01-globalopt.ll`; `alloca` and `!tbaa` removed):

```
@_k = constant [2 x i8] c"\07\00"
define i16 @_f(ptr %0, ptr %1) addrspace(1) {
b1:
  store ptr %0, ptr %2
  store ptr %1, ptr %3
  %5 = load ptr, ptr %3
  store i16 1, ptr %5
  %6 = load ptr, ptr %2
  %7 = load i16, ptr %6
  %8 = load i16, ptr @_k
  %9 = add nsw i16 %7, %8
  %10 = load ptr, ptr %2
  %11 = call addrspace(1) i16 @_g(ptr %10)
  %12 = add nsw i16 %9, %11
  store i16 %12, ptr %4
  %13 = load i16, ptr %4
  ret i16 %13
}
...
```

#### restrict (`-za99`, `__restrict`)

```c
void add(int *__restrict d, const int *__restrict a, int n){ int i; for(i=0;i<n;i++) d[i]+=a[i]; }
```

Recorded stream:

```
CALLCONV y1 class=0x80 target=0x716 parms=[] ret=0:0
- CGParmDecl y2 TY_POINTER
- CGParmDecl y3 TY_POINTER
- CGParmDecl y4 TY_INTEGER
n15 CGAttr n14 3
n23 CGAttr n22 3
```

MIR as raised (`01-globalopt.ll`; `alloca` and `!tbaa` removed):

```
define void @_add(ptr noalias %0, ptr noalias %1, i16 %2) addrspace(1) {
b1:
  store ptr %0, ptr %3
...
```

#### noreturn, `#pragma aux aborts`

```c
__declspec(noreturn) void die(int c);
#pragma aux quit aborts
void quit(void);
int f(int x){ if(x) die(1); quit(); return 0; }
```

Recorded stream:

```
SYM y1 name="f" base="f" pattern="_*" attr=0x7 seg=1
CALLCONV y1 class=0x80 target=0x716 parms=[] ret=0:0
SYM y2 name="x" base="x" pattern="_*" attr=0x0 seg=2
SYM y3 name="die" base="die" pattern="_*" attr=0xf seg=1
CALLCONV y3 class=0x84 target=0x716 parms=[] ret=0:0
c10 CGInitCall n9 TY_INTEGER y3
SYM y4 name="quit" base="quit" pattern="_*" attr=0xf seg=1
CALLCONV y4 class=0x82 target=0x716 parms=[] ret=0:0
c14 CGInitCall n13 TY_INTEGER y4
```

MIR as raised (`01-globalopt.ll`; `alloca` and `!tbaa` removed):

```
define i16 @_f(i16 %0) addrspace(1) {
b1:
  store i16 %0, ptr %1
  %3 = load i16, ptr %1
  %4 = icmp ne i16 %3, 0
  %5 = zext i1 %4 to i16
  %6 = icmp ne i16 %5, 0
  br i1 %6, label %b3, label %b2
b2:
  %7 = call addrspace(1) i16 @_quit()
  store i16 0, ptr %2
  %8 = load i16, ptr %2
  ret i16 %8
b3:
  %9 = call addrspace(1) i16 @_die(i16 1)
  br label %b2
}
declare i16 @_quit() addrspace(1)
declare i16 @_die(i16) addrspace(1)
```

#### `#pragma aux ... nomemory`

```c
#pragma aux sq parm nomemory modify nomemory
int sq(int x);
int f(int a, int *p){ int r = sq(a); *p = 1; return r + sq(a); }
```

Recorded stream:

```
SYM y1 name="f" base="f" pattern="_*" attr=0x7 seg=1
CALLCONV y1 class=0x80 target=0x716 parms=[] ret=0:0
SYM y2 name="a" base="a" pattern="_*" attr=0x0 seg=2
SYM y3 name="p" base="p" pattern="_*" attr=0x0 seg=2
SYM y4 name="r" base="r" pattern="_*" attr=0x0 seg=2
SYM y5 name="sq" base="sq" pattern="_*" attr=0xf seg=1
CALLCONV y5 class=0x380 target=0x716 parms=[] ret=0:0
```

MIR as raised (`01-globalopt.ll`; `alloca` and `!tbaa` removed):

```
define i16 @_f(i16 %0, ptr %1) addrspace(1) {
b1:
  store i16 %0, ptr %2
  store ptr %1, ptr %3
  %6 = load i16, ptr %2
  %7 = call addrspace(1) i16 @_sq(i16 %6)
  store i16 %7, ptr %5
  %8 = load ptr, ptr %3
  store i16 1, ptr %8
  %9 = load i16, ptr %5
  %10 = load i16, ptr %2
  %11 = call addrspace(1) i16 @_sq(i16 %10)
  %12 = add nsw i16 %9, %11
  store i16 %12, ptr %4
  %13 = load i16, ptr %4
  ret i16 %13
...
```

#### `#pragma aux` registers

```c
#pragma aux myfn "*_x" parm [ax] [dx] value [ax] modify [bx cx] 
int myfn(int a,int b);
#pragma aux getds = "mov ax, ds" value [ax];
int getds(void);
int f(int a,int b){ return myfn(a,b)+getds(); }
```

Recorded stream:

```
SYM y1 name="f" base="f" pattern="_*" attr=0x7 seg=1
CALLCONV y1 class=0x80 target=0x716 parms=[] ret=0:0
SYM y2 name="a" base="a" pattern="_*" attr=0x0 seg=2
SYM y3 name="b" base="b" pattern="_*" attr=0x0 seg=2
SYM y4 name="myfn" base="myfn" pattern="*_x" attr=0xf seg=1
CALLCONV y4 class=0x80 target=0x717 parms=[3:0,c0:0] ret=3:0
n4 CGFEName y4 TY_CODE_PTR
SYM y5 name="getds" base="getds" pattern="_*" attr=0xf seg=1
CALLCONV y5 class=0x80 target=0x717 parms=[] ret=3:0
CODE y5 bytes=8cd8 fix=-
n12 CGFEName y5 TY_CODE_PTR
```

`llrm-c` refuses: `_f: myfn_x has a register calling convention`

#### `#pragma aux modify`

```c
#pragma aux clob modify [bx cx dx]
void clob(void);
int f(int a){ clob(); return a; }
```

Recorded stream:

```
SYM y1 name="f" base="f" pattern="_*" attr=0x7 seg=1
CALLCONV y1 class=0x80 target=0x716 parms=[] ret=0:0
SYM y2 name="a" base="a" pattern="_*" attr=0x0 seg=2
SYM y3 name="clob" base="clob" pattern="_*" attr=0xf seg=1
CALLCONV y3 class=0x80 target=0x716 parms=[] ret=0:0
```

MIR as raised (`01-globalopt.ll`; `alloca` and `!tbaa` removed):

```
define i16 @_f(i16 %0) addrspace(1) {
b1:
  store i16 %0, ptr %1
  %3 = call addrspace(1) i16 @_clob()
  %4 = load i16, ptr %1
  store i16 %4, ptr %2
  %5 = load i16, ptr %2
  ret i16 %5
}
declare i16 @_clob() addrspace(1)
```

#### inline byte code

```c
#pragma aux getds = "mov ax, ds" value [ax];
int getds(void);
int f(void){ return getds(); }
```

Recorded stream:

```
SYM y1 name="f" base="f" pattern="_*" attr=0x7 seg=1
CALLCONV y1 class=0x80 target=0x716 parms=[] ret=0:0
SYM y2 name="getds" base="getds" pattern="_*" attr=0xf seg=1
CALLCONV y2 class=0x80 target=0x717 parms=[] ret=3:0
CODE y2 bytes=8cd8 fix=-
n4 CGFEName y2 TY_CODE_PTR
```

MIR as raised (`01-globalopt.ll`; `alloca` and `!tbaa` removed):

```
define i16 @_f() addrspace(1) {
b1:
  %1 = call i32 @llrm.ia16.code.8cd8()
  %2 = trunc i32 %1 to i16
  store i16 %2, ptr %0
  %3 = load i16, ptr %0
  ret i16 %3
}
declare i32 @llrm.ia16.code.8cd8() nounwind
```

#### bit fields

```c
struct S { unsigned a:3; int b:5; unsigned c:8; };
struct S s;
int f(void){ s.a = 5; s.b = -3; return s.a + s.b + s.c; }
```

Recorded stream:

```
TYPE T25 size=2 align=1
SYM y1 name="s" base="s" pattern="_*" attr=0x6 seg=11
b1 BENewBack y1
- DGLabel b1
- DGUBytes 2
SYM y2 name="f" base="f" pattern="_*" attr=0x7 seg=1
CALLCONV y2 class=0x80 target=0x716 parms=[] ret=0:0
- CGProcDecl y2 TY_INTEGER
n5 CGFEName y1 T25
n6 CGInteger 0 TY_UNSIGNED
n7 CGBinary O_PLUS n5 n6 TY_POINTER
n8 CGBitMask n7 0 3 TY_UNSIGNED
n9 CGInteger 5 TY_INTEGER
n10 CGAssign n8 n9 TY_UNSIGNED
- CGDone n10
n11 CGFEName y1 T25
```

`llrm-c` refuses: `_f: CGBitMask n7 0 3 TY_UNSIGNED`

#### switch

```c
int f(int x){ switch(x){ case 1: return 10; case 2: return 20; case 3: case 4: return 30; case 100: return 40; default: return 0; } }
```

Recorded stream:

```
SYM y1 name="f" base="f" pattern="_*" attr=0x7 seg=1
CALLCONV y1 class=0x80 target=0x716 parms=[] ret=0:0
- CGProcDecl y1 TY_INTEGER
SYM y2 name="x" base="x" pattern="_*" attr=0x0 seg=2
- CGParmDecl y2 TY_INTEGER
n10 CGFEName y2 TY_INTEGER
n11 CGUnary O_POINTS n10 TY_INTEGER
s12 CGSelInit
- CGSelCase s12 l4 1
- CGSelCase s12 l6 2
- CGSelCase s12 l7 3
- CGSelCase s12 l7 4
```

MIR as raised (`01-globalopt.ll`; `alloca` and `!tbaa` removed):

```
define i16 @_f(i16 %0) addrspace(1) {
b1:
  store i16 %0, ptr %1
  %3 = load i16, ptr %1
  switch i16 %3, label %b2 [
    i16 1, label %b3
    i16 2, label %b4
    i16 3, label %b5
    i16 4, label %b5
    i16 100, label %b6
  ]
b2:
  store i16 0, ptr %2
  br label %b7
...
```

#### switch with a run of cases

```c
int f(unsigned char c){ switch(c){ case 1: case 2: case 3: case 4: case 5: return 1; case 'a': return 2; default: return 0;} }
```

Recorded stream:

```
SYM y1 name="f" base="f" pattern="_*" attr=0x7 seg=1
SYM y2 name="c" base="c" pattern="_*" attr=0x0 seg=2
s10 CGSelInit
- CGSelCase s10 l4 1
- CGSelCase s10 l4 2
- CGSelCase s10 l4 3
- CGSelCase s10 l4 4
- CGSelCase s10 l4 5
- CGSelCase s10 l6 97
- CGSelOther s10 l7
- CGSelect s10 n9
```

`llrm-c` refuses: ``

#### varargs

```c
typedef char *va_list;
#define va_start(ap,v) (ap = (va_list)&v + sizeof(v))
#define va_arg(ap,t) (*(t *)((ap += sizeof(t)) - sizeof(t)))
int sum(int n, ...){ va_list ap; int s=0; va_start(ap,n); while(n--) s+=va_arg(ap,int); return s; }
int call(void){ return sum(2, 3, 4); }
```

Recorded stream:

```
SYM y1 name="sum" base="sum" pattern="_*" attr=0x10007 seg=1
CALLCONV y1 class=0xa0 target=0x716 parms=[] ret=0:0
SYM y2 name="n" base="n" pattern="_*" attr=0x0 seg=2
- CGParmDecl y2 TY_INTEGER
SYM y3 name="ap" base="ap" pattern="_*" attr=0x0 seg=2
SYM y4 name="s" base="s" pattern="_*" attr=0x0 seg=2
SYM y5 name="call" base="call" pattern="_*" attr=0x7 seg=1
CALLCONV y5 class=0x80 target=0x716 parms=[] ret=0:0
- CGAddParm c39 n40 TY_INTEGER
- CGAddParm c39 n41 TY_INTEGER
- CGAddParm c39 n42 TY_INTEGER
```

MIR as raised (`01-globalopt.ll`; `alloca` and `!tbaa` removed):

```
define i16 @_sum(i16 %0, ...) addrspace(1) {
b1:
  call void @llvm.va_start.p0(ptr %4)
  %5 = load ptr, ptr %4
  store i16 0, ptr %3
  %6 = getelementptr i8, ptr %5, i16 -2
  %7 = getelementptr inbounds i8, ptr %6, i16 2
  store ptr %7, ptr %2
  br label %b2
b2:
  %8 = getelementptr i8, ptr %5, i16 -2
  %9 = load i16, ptr %8
...
```

#### struct copy

```c
struct B { int a[10]; };
struct B x, y;
struct B get(void){ return y; }
void f(void){ x = y; x = get(); }
```

Recorded stream:

```
TYPE T25 size=20 align=1
- CGProcDecl y3 T25
- CGProcDecl y4 TY_INTEGER
n17 CGLVAssign n14 n16 T25
n21 CGCall c20
n23 CGLVAssign n18 n22 T25
```

MIR as raised (`01-globalopt.ll`; `alloca` and `!tbaa` removed):

```
@_x = global [20 x i8] zeroinitializer
@_y = global [20 x i8] zeroinitializer
define ptr addrspace(1) @_get(ptr addrspace(1) %0) addrspace(1) {
b1:
  %2 = load i32, ptr @_y
  store i32 %2, ptr %1
  %3 = getelementptr inbounds i8, ptr @_y, i16 4
  %4 = getelementptr inbounds i8, ptr %1, i16 4
  %5 = load i32, ptr %3
  store i32 %5, ptr %4
...
```

#### far and near pointers

```c
int __far *fp;
int __near *np;
int f(void){ return *fp + *np; }
```

Recorded stream:

```
SYM y1 name="fp" base="fp" pattern="_*" attr=0x6 seg=11
b1 BENewBack y1
- DGLabel b1
- DGUBytes 4
SYM y2 name="np" base="np" pattern="_*" attr=0x6 seg=11
b2 BENewBack y2
- DGLabel b2
- DGUBytes 2
SYM y3 name="f" base="f" pattern="_*" attr=0x7 seg=1
CALLCONV y3 class=0x80 target=0x716 parms=[] ret=0:0
- CGProcDecl y3 TY_INTEGER
n6 CGFEName y1 TY_LONG_POINTER
n7 CGUnary O_POINTS n6 TY_LONG_POINTER
n8 CGUnary O_POINTS n7 TY_INTEGER
n9 CGFEName y2 TY_NEAR_POINTER
n10 CGUnary O_POINTS n9 TY_NEAR_POINTER
n11 CGUnary O_POINTS n10 TY_INTEGER
n12 CGBinary O_PLUS n8 n11 TY_INTEGER
n13 CGUnary O_CONVERT n12 TY_INTEGER
n14 CGTempName t5 TY_INTEGER
n15 CGAssign n14 n13 TY_INTEGER
- CGDone n15
n16 CGTempName t5 TY_INTEGER
n17 CGUnary O_POINTS n16 TY_INTEGER
- CGReturn n17 TY_INTEGER
```

MIR as raised (`01-globalopt.ll`; `alloca` and `!tbaa` removed):

```
@_fp = global [4 x i8] zeroinitializer
@_np = global [2 x i8] zeroinitializer
define i16 @_f() addrspace(1) {
b1:
  %1 = load ptr addrspace(1), ptr @_fp
  %2 = load i16, ptr addrspace(1) %1
  %3 = load ptr, ptr @_np
  %4 = load i16, ptr %3
...
```

#### `__based` pointer

```c
int __based(__segname("_DATA")) *bp;
int f(void){ return *bp; }
```

Recorded stream:

```
SYM y1 name="bp" base="bp" pattern="_*" attr=0x6 seg=11
b1 BENewBack y1
- DGLabel b1
- DGUBytes 2
SYM y2 name="f" base="f" pattern="_*" attr=0x7 seg=1
CALLCONV y2 class=0x80 target=0x716 parms=[] ret=0:0
- CGProcDecl y2 TY_INTEGER
SYM y3 name=".DS" base=".DS" pattern="_*" attr=0x1042 seg=4
n5 CGFEName y3 TY_UINT_2
n6 CGFEName y1 TY_NEAR_POINTER
n7 CGUnary O_POINTS n6 TY_NEAR_POINTER
n8 CGBinary O_CONVERT n7 n5 TY_LONG_POINTER
n9 CGUnary O_POINTS n8 TY_INTEGER
n10 CGUnary O_CONVERT n9 TY_INTEGER
n11 CGTempName t4 TY_INTEGER
n12 CGAssign n11 n10 TY_INTEGER
- CGDone n12
n13 CGTempName t4 TY_INTEGER
n14 CGUnary O_POINTS n13 TY_INTEGER
- CGReturn n14 TY_INTEGER
```

`llrm-c` refuses: `_f: .DS is neither defined nor imported`

#### interrupt

```c
volatile int ticks;
void __interrupt isr(void){ ticks++; }
```

Recorded stream:

```
SYM y1 name="ticks" base="ticks" pattern="_*" attr=0x826 seg=11
b1 BENewBack y1
- DGLabel b1
- DGUBytes 2
SYM y2 name="isr" base="isr" pattern="_*" attr=0x7 seg=1
CALLCONV y2 class=0x80 target=0x20071e parms=[] ret=0:0
- CGProcDecl y2 TY_INTEGER
n5 CGFEName y1 TY_INTEGER
n6 CGInteger 1 TY_INTEGER
n7 CGVolatile n5
n8 CGPostGets O_PLUS n7 n6 TY_INTEGER
- CGDone n8
- CGReturn n0 TY_INTEGER
```

MIR as raised (`01-globalopt.ll`; `alloca` and `!tbaa` removed):

```
@_ticks = global [2 x i8] zeroinitializer
define x86_intrcc void @_isr(ptr %0) addrspace(1) {
b1:
  %2 = load volatile i16, ptr @_ticks
  %3 = add nsw i16 %2, 1
  store volatile i16 %3, ptr @_ticks
  ret void
}
```

#### `__unaligned`

```c
int f(int __unaligned *p){ return *p; }
```

Recorded stream:

```
SYM y1 name="f" base="f" pattern="_*" attr=0x7 seg=1
CALLCONV y1 class=0x80 target=0x716 parms=[] ret=0:0
- CGProcDecl y1 TY_INTEGER
SYM y2 name="p" base="p" pattern="_*" attr=0x0 seg=2
- CGParmDecl y2 TY_POINTER
n4 CGFEName y2 TY_POINTER
n5 CGUnary O_POINTS n4 TY_POINTER
n6 CGAttr n5 2
n7 CGUnary O_POINTS n6 TY_INTEGER
n8 CGUnary O_CONVERT n7 TY_INTEGER
n9 CGTempName t3 TY_INTEGER
n10 CGAssign n9 n8 TY_INTEGER
- CGDone n10
n11 CGTempName t3 TY_INTEGER
n12 CGUnary O_POINTS n11 TY_INTEGER
- CGReturn n12 TY_INTEGER
```

MIR as raised (`01-globalopt.ll`; `alloca` and `!tbaa` removed):

```
define i16 @_f(ptr %0) addrspace(1) {
b1:
  store ptr %0, ptr %1
  %3 = load ptr, ptr %1
  %4 = load i16, ptr %3
  store i16 %4, ptr %2
  %5 = load i16, ptr %2
  ret i16 %5
}
```

#### static initialisers

```c
int a[4] = {1,2,3};
char s[] = "hi";
char *p = "hi";
struct T { char c; int i; } t = {1, 2};
double d = 1.5;
int *q = &a[1];
static int z;
```

Recorded stream:

```
TYPE T25 size=8 align=2
SYM y1 name="z" base="z" pattern="_*" attr=0x42 seg=11
b1 BENewBack y1
- DGLabel b1
- DGUBytes 2
TYPE T26 size=3 align=1
TYPE T27 size=3 align=1
SYM y2 name="a" base="a" pattern="_*" attr=0x6 seg=4
b2 BENewBack y2
- DGLabel b2
- DGInteger 1 TY_INTEGER
- DGInteger 2 TY_INTEGER
- DGInteger 3 TY_INTEGER
- DGIBytes 2 0
SYM y3 name="s" base="s" pattern="_*" attr=0x6 seg=4
b3 BENewBack y3
- DGLabel b3
- DGBytes 3 686900
SYM y4 name="p" base="p" pattern="_*" attr=0x6 seg=4
b4 BENewBack y4
- DGLabel b4
b5 BENewBack y0
- DGLabel b5
- DGBytes 3 686900
- DGBackPtr b5 2 0 TY_POINTER
SYM y5 name="t" base="t" pattern="_*" attr=0x6 seg=4
b6 BENewBack y5
- DGLabel b6
- DGInteger 1 TY_UINT_1
- DGInteger 2 TY_INTEGER
SYM y6 name="d" base="d" pattern="_*" attr=0x6 seg=4
b7 BENewBack y6
- DGLabel b7
- DGBytes 8 000000000000f83f
SYM y7 name="q" base="q" pattern="_*" attr=0x6 seg=4
b8 BENewBack y7
- DGLabel b8
- DGFEPtr y2 TY_POINTER 2
```

MIR as raised (`01-globalopt.ll`; `alloca` and `!tbaa` removed):

```
@L_b5 = private constant [3 x i8] c"hi\00"
@_a = global [8 x i8] c"\01\00\02\00\03\00\00\00"
@_s = global [3 x i8] c"hi\00"
@_p = global ptr @L_b5
@_t = global [3 x i8] c"\01\02\00"
@_d = global [8 x i8] c"\00\00\00\00\00\00\F8?"
@_q = global ptr getelementptr (i8, ptr @_a, i16 2)
@_z = internal constant [2 x i8] zeroinitializer
```

#### folding and dead code

```c
int f(int x){ int y = 3*4+1; if (0) return 99; return x*(sizeof(long)+2) + y; }
```

Recorded stream:

```
SYM y1 name="f" base="f" pattern="_*" attr=0x7 seg=1
CALLCONV y1 class=0x80 target=0x716 parms=[] ret=0:0
- CGProcDecl y1 TY_INTEGER
SYM y2 name="x" base="x" pattern="_*" attr=0x0 seg=2
- CGParmDecl y2 TY_INTEGER
SYM y3 name="y" base="y" pattern="_*" attr=0x0 seg=2
- CGAutoDecl y3 TY_INTEGER
n6 CGFEName y3 TY_INTEGER
n7 CGInteger 13 TY_INTEGER
n8 CGAssign n6 n7 TY_INTEGER
- CGDone n8
- CGControl O_GOTO n0 l4
n9 CGInteger 99 TY_INTEGER
n10 CGTempName t3 TY_INTEGER
n11 CGAssign n10 n9 TY_INTEGER
- CGDone n11
- CGControl O_LABEL n0 l4
n12 CGFEName y2 TY_INTEGER
n13 CGUnary O_POINTS n12 TY_INTEGER
n14 CGInteger 6 TY_UNSIGNED
n15 CGBinary O_TIMES n13 n14 TY_UNSIGNED
n16 CGFEName y3 TY_INTEGER
n17 CGUnary O_POINTS n16 TY_INTEGER
n18 CGBinary O_PLUS n15 n17 TY_UNSIGNED
n19 CGUnary O_CONVERT n18 TY_INTEGER
n20 CGTempName t3 TY_INTEGER
n21 CGAssign n20 n19 TY_INTEGER
- CGDone n21
- CGControl O_LABEL n0 l5
n22 CGTempName t3 TY_INTEGER
n23 CGUnary O_POINTS n22 TY_INTEGER
- CGReturn n23 TY_INTEGER
```

MIR as raised (`01-globalopt.ll`; `alloca` and `!tbaa` removed):

```
define i16 @_f(i16 %0) addrspace(1) {
b1:
  store i16 %0, ptr %1
  store i16 13, ptr %3
  br label %b2
b2:
  %4 = load i16, ptr %1
  %5 = mul i16 %4, 6
  %6 = load i16, ptr %3
  %7 = add i16 %5, %6
  store i16 %7, ptr %2
  br label %b4
b3:
  store i16 99, ptr %2
  br label %b2
b4:
  %8 = load i16, ptr %2
  ret i16 %8
}
```

#### inline functions

```c
static int sq(int x){ return x*x; }
int f(int a){ return sq(a)+sq(a); }
__inline int g(int x){ return x+1; }
int h(int a){ return g(a); }
```

Recorded stream:

```
SYM y1 name="sq" base="sq" pattern="_*" attr=0x43 seg=1
- CGProcDecl y1 TY_INTEGER
SYM y2 name="x" base="x" pattern="_*" attr=0x0 seg=2
SYM y3 name="f" base="f" pattern="_*" attr=0x7 seg=1
- CGProcDecl y3 TY_INTEGER
SYM y4 name="a" base="a" pattern="_*" attr=0x0 seg=2
SYM y5 name="h" base="h" pattern="_*" attr=0x7 seg=1
- CGProcDecl y5 TY_INTEGER
SYM y6 name="a" base="a" pattern="_*" attr=0x0 seg=2
SYM y7 name="g" base="g" pattern="_*" attr=0x43 seg=1
```

MIR as raised (`01-globalopt.ll`; `alloca` and `!tbaa` removed):

```
define internal i16 @_sq(i16 %0) {
b1:
  store i16 %0, ptr %1
  %3 = load i16, ptr %1
  %4 = load i16, ptr %1
  %5 = mul nsw i16 %3, %4
  store i16 %5, ptr %2
  %6 = load i16, ptr %2
...
```

#### options

`-oa -ol+ -ot -ou` on `addr.c` change the first record from `INIT sw=0x808000 target=0xec size=50` to `INIT sw=0xa68000 target=0xec size=0`: `RELAX_ALIAS` and `LOOP_OPTIMIZATION` set, time rather than size. `hir.rs:386` keeps only `target`.

#### address taken

```c
int g(int *p);
int f(void){ int x = 1; int y = 2; g(&x); return x + y; }
```

```
SYM y2 name="x" base="x" pattern="_*" attr=0x0 seg=2
SYM y3 name="y" base="y" pattern="_*" attr=0x0 seg=2
```

`x` has its address taken and `y` has not; the attributes are equal.

#### debug (`-d2`)

```c
typedef struct P { int x; unsigned f:3; const char *n; } P;
enum E { A, B=5 };
P g(P *p, enum E e, volatile int *v){ return *p; }
```

```
d16 DBStruct "P" struct 6
- DBField d16 0 "x" d6
- DBBitField d16 2 0 3 "f" d7
d17 DBPtr TY_POINTER d1
- DBField d16 4 "n" d17
d48 DBEnum TY_INTEGER
- DBConst d48 "A" 0
- DBConst d48 "B" 5
d50 DBPtr TY_POINTER d6
```

`const char *` is `DBPtr` to `char` (`d1`) and `volatile int *` is `DBPtr` to `int` (`d6`): no qualifier reaches the stream.

### Source against stream

| Source says | Stream shows |
|---|---|
| `FE_ADDR_TAKEN` is set only for pragma-used variables (`cinfo.c:231-233`) | `&x` leaves `attr=0x0` |
| `CGSelRange` is never called (`cgen.c:DoSwitch`) | a run of five cases is five `CGSelCase` |
| `CG_SYM_UNALIGNED` goes out through `CGAttr` | `CGAttr n 2` |
| `DGFloat` is not used | floats arrive as `DGBytes` |
| `-d2` sets `CGSW_GEN_NO_OPTIMIZATION` (`coptions.c`) | `sw=0x6988000` has bit `0x4000000` |
| `x86` `va_start` is a macro | no `O_VA_START`, no `CGVarargsBasePtr`; the shim refuses that call for AXP and MIPS only (`cgshim.c:617`) |
| the stream's `modify` list is a call-site fact | absent: the shim never asks (`cgshim.c:261-278`) |

No difference contradicted the source.

### Findings filed

Each is a fact the stream carries that `llrm-c` drops or refuses. None is fixed here; the reproducer is the probe above.

| # | Finding | Reproducer |
|---|---|---|
| 1 | `noreturn` and `aborts` are in the call class and ignored; the call falls through to `br` | noreturn |
| 2 | `nomemory` is in the call class and ignored | nomemory |
| 3 | `CGAttr n 2` (`__unaligned`) is ignored; the load has no `align 1` | unaligned |
| 4 | `INIT sw` (`-oa`, `-ol`, `-ot`) is ignored | options |
| 5 | `CGBitMask` is refused: bit fields do not compile on the rich route | bit fields |
| 6 | `#pragma aux` register parameters are refused | pragma aux |
| 7 | `__based` is refused (`.DS` has no definition) | based |
| 8 | `DBBitField` drops start and width | debug |
| 9 | Quick BASIC: `IndirectPlace.published` is not read by the rich route; a BYREF polling loop hangs (§7) | `poll.bas` in §7 |
| 10 | Nib: `&mut` and `&` parameters carry no `Promise` (§7) | `ref.nib` in §7 |
| 11 | Nib: the range-loop increment has no `nowrap` (survey, §7) | any `for i in a..b` with variable bounds |
| 12 | Quick BASIC: every dynamic array shares one `allocation` tag (`MIRH:1274`) | two far arrays in one loop |

## 7. What the other frontends know and do not emit

Rust paths in this section: `MODEL` = `crates/ir/llrm-hir/src/model.rs`, `MIRH` = `crates/ir/llrm-hir/src/mir.rs`, `SEM` = `crates/frontends/qbfront/src/semantic.rs`, `NIB` = `crates/frontends/llrm-nib/src`, `AN` = `crates/opt/llrm-analysis/src`, `TR` = `crates/opt/llrm-transforms/src`, `MIR` = `crates/ir/llrm-mir/src`. Both frontends default to the rich route (`crates/backend/llrm-core/src/driver/mod.rs:77,100`).

Two claims were checked by running. QuickBASIC drops `published` (below). A Nib `&mut`/`&` pair carries no promises: in `03-hir.json` of

```
struct Pt:
    mut x: i16
    y: i16

fn bump(p: &mut Pt, q: &Pt, n: i16) -> void:
    for i in 0..n:
        p.x += q.y
```

`bump` has `promises: None`.

### Quick BASIC drops `published`: a hang

```basic
DECLARE SUB WaitFor (x AS INTEGER)
DIM k AS INTEGER
WaitFor k
SUB WaitFor (x AS INTEGER)
  DO WHILE x = 0
  LOOP
END SUB
```

HIR: the load in `WAITFOR` has `"published":true,"volatile":false`. Default route:

```
    mov bx, word ptr [bp+6]
    cmp word ptr [bx], 0
    sete al
L1_3:
    or al, al
    jne L1_3
```

The load is outside the loop; the loop never reads `x` again. `--legacy` reloads: `L1_2: cmp word ptr [bx], 0` / `je L1_2`. `MIRH` never reads `published` (finding 9 below).

### HIR and MIR: carriers and who reads them

Prefixes here: H = `crates/ir/llrm-hir/src`, M = `crates/ir/llrm-mir/src`, A = `crates/opt/llrm-analysis/src`, T = `crates/opt/llrm-transforms/src`, B = `crates/backend/llrm-core/src`. The live pipeline is `B/driver/mod.rs:optimized:99-101` into `T/pipeline.rs:pipeline:165-195`; `M/transforms/*` is called only from tests.

#### HIR fields that state a fact

| Field | Line | Meaning |
|---|---|---|
| `Type.signed` | 168 | signedness; picks sext/zext, signed/unsigned predicates |
| `Type.kind/width/evaluation` | 166,167,169 | representation, float format |
| `Type.element/rank/bounds` | 170-172 | array element, dimension bounds (index range) |
| `Type.address` | 173 | near/far/huge/code/segment pointer class |
| `Place.storage` | 225 | local/parameter/static/module/common/external |
| `Place.extent` | 228 | bytes of the place, overrides type width |
| `Place.address` | 229 | placement class |
| `Place.volatile` | 230 | ordered access |
| `IndirectPlace.volatile` | 299 | ordered access |
| `IndirectPlace.published` | 303 | another agent may write pointee; ordered like volatile, one read may end a loop |
| `IndirectPlace.inbounds` | 305 | access stays inside one object |
| `IndirectPlace.origin` | 310 | value holding object's first-byte offset; object ends in its segment |
| `IndirectPlace.allocation` | 313 | descriptor place owning the far allocation; disjoint from every place |
| `DescriptorPlace` | 322-324 | no volatile/inbounds field |
| `Instruction.pure` | 471 | call is pure |
| `Instruction.nowrap` | 474 | signed result fits its width |
| `Instruction.inbounds` | 476 | PTR_OFFSET result stays in its object |
| `Instruction.line` | 478 | source line |
| `Instruction.callee` | 470 | direct callee name |
| `Asm.memory/clobbers/inputs/outputs` | 461,459,455,457 | inline asm effects |
| `Terminator.kind = Unreachable` | 494 | control stops |
| `Block.cold` | 517 | frontend expects it never to run |
| `CallAbi.cleanup/distance/float_return/order/callee` | 530-533,529 | per-call ABI |
| `CallAbi.promises` / `ArgumentPromise.bytes` | 534 / 543 | callee writes first `bytes` before reading, reads none, keeps none |
| `Callable.by_value/segmented/arrays/defined/symbol` | 553-558 | callee signature facts |
| `ProcedureAbi.cleanup/distance/parameter_bytes/float_return/variadic` | 563-568 | function ABI |
| `Function.linkage` | 617 | internal/external |
| `Function.promises` / `Promise.{bytes,unaliased,readonly}` | 618 / 697-700 | `dereferenceable(bytes)`, `noalias`, `readonly` on a pointer parameter |
| `Function.error_handler/_local/external_entries` | 614-616 | ON ERROR handling, RESUME entries |
| `Function.symbol` | 620 | link name |
| `DataObject.readonly` | 750 | constant data |
| `DataObject.linkage` | 752 (`DataLinkage` 206-212) | internal/external/exported/private |
| `DataObject.address/addressed/segment/align` | 755,758,760,761 | placement; `addressed=false`: only a named reference reaches it |
| `DataRelocation.code` | 742 | target is code |
| `AliasClass.name/parent/types` | 786-788 | TBAA class tree |
| `Module.debug` (`Debug*` structs) | 801 (628-691) | -g types, functions, variables, globals |
| `Module.line_numbers` / statement table | 804 / 813-826 | BASIC lines, RESUME targets |
| `RuntimePromises.calling_back/writers/nounwind/reads_arguments` | 863,865,867,870 | runtime routines: run program code, cells written, raise no error, only read pointer args |
| `Program.zeroed_locals/frames` | 912,913 | frame starts zeroed |
| `Program.entries/preserved/constant_segment` | 917,920,923 | outside entry points, preserved regs, constants segment |
| `Program.array_order/float_mode/float_semantics/target` | 907-909,906 | language modes |

No HIR field exists for: noreturn, nonnull, value range, per-access alignment, trip count, EH cleanup, per-call attributes beyond `promises`/`pure`.

#### MIR carriers the parser accepts

| Carrier | Accepted values | Cite |
|---|---|---|
| Flag attributes (function, param, return, call-site) | alwaysinline builtin cold convergent dead_on_unwind hot immarg inreg minsize mustprogress naked nest noalias nobuiltin nocallback nocapture nofree noinline nomerge nonnull norecurse noreturn nosync noundef nounwind optnone optsize readnone readonly returned returns_twice signext speculatable willreturn writable writeonly zeroext | `opcode.rs:FLAG_ATTRIBUTES:277-315`; parse `parse.rs:attributes:564-567`; print `print.rs:attributes:136` |
| Int attributes | align alignstack dereferenceable dereferenceable_or_null | `opcode.rs:INT_ATTRIBUTES:317`; `parse.rs:568-576`; `print.rs:137-138` |
| Type attributes | byref byval elementtype inalloca sret | `opcode.rs:TYPE_ATTRIBUTES:319`; `parse.rs:577-583` |
| `memory(loc: access,...)` | per-location access | `opcode.rs:Attribute::Memory:255`; `parse.rs:595-614` |
| `range(iN lo, hi)` | half-open range | `opcode.rs:Attribute::Range:257`; `parse.rs:584-594` |
| `initializes((lo,hi),...)` | bytes written before read | `opcode.rs:Attribute::Initializes:261`; `parse.rs:615-632`; verifier `verify.rs:function:93-106` |
| String attributes `"k"="v"` | any | `opcode.rs:Attribute::Str:262`; `parse.rs:559-563` |
| Attribute slots | `Function.attrs/parameter_attrs/return_attrs`; `CallInfo.attrs/argument_attrs/return_attrs` | `module.rs:96-98`; `opcode.rs:336-338`; call site `parse.rs:1447,1451` |
| Instruction flags | nuw nsw exact disjoint nneg samesign inbounds nusw, fast-math (reassoc nnan ninf nsz arcp contract afn, `fast`) | `opcode.rs:Flags:196-223`; allowed per opcode `parse.rs:1229-1244,1308`; GEP `parse.rs:gep_flags:1167-1178`; print `print.rs:347` |
| Load/store/alloca | `align`, `volatile`; alloca `address_space` | `opcode.rs:378-380`; `parse.rs:1401,1409` |
| Global variable | `constant`, `align`, `initializer`, `address_space`, `unnamed_addr` | `module.rs:297,299,313,312`; `parse.rs:418-447` |
| Linkage | internal private weak weak_odr linkonce linkonce_odr common extern_weak available_externally | `module.rs:LINKAGE:274-284` |
| Function | `personality`, `calling_convention` (ccc, fastcc, coldcc, x86_stdcallcc, x86_fastcallcc, x86_intrcc, 1000=BASIC), tail/musttail/notail | `module.rs:99,101`; `opcode.rs:CONVENTIONS:343,BASIC:346,X86_INTR:352,Tail:322-328` |
| Terminators | ret br switch invoke resume unreachable | `opcode.rs:Opcode:366-372` |
| EH | landingpad cleanup/catch/filter | `opcode.rs:388,Clause:355` |
| Instruction metadata | any `!kind !N`; kind is an unchecked string | `parse.rs:attachments:969-980`; print `print.rs:448-450`; `module.rs:Instruction.metadata:51` |
| Named metadata | any `!name = !{...}` | `parse.rs:named_metadata:887-906` |
| Metadata kinds/names actually used in code | `tbaa`, `dbg`, `var`; `llrm.named`, `llrm.writes`, `llrm.dbg.types/functions/globals` | `M/alias.rs:32`; `H/mir.rs:DEBUG_LINE:250`; `M/debuginfo.rs:VARIABLE:94,TYPES:90-92`; `H/mir.rs:95,118` |
| Intrinsic declarations get attributes from a table | nocallback nofree nosync nounwind willreturn speculatable, `memory(...)`, param nocapture/writeonly/immarg | `intrinsics.rs:PURE:132,PORT_ATTRS:137,attributes:330-339` |
| Refused as outside subset | undef fp128 ppc_fp128 half bfloat blockaddress indirectbr dso_local triple | `parse.rs:OUTSIDE_SUBSET:33` |
| Verifier checks | only `initializes` legality among attributes; no check of flags, other attributes or metadata | `verify.rs:93-106` |
| Lint (hir-mir binary only) | rejects every flag except GEP `inbounds` | `lint.rs:55-56`; caller `H/bin/hir-mir.rs:28` |

Not in the parser's vocabulary as named carriers: `!llvm.loop`, `!prof`, `!range`, `!nonnull`, `!noalias`, `!alias.scope`, `!invariant.load` are accepted as unchecked strings (`parse.rs:969-980`) and read by no code (grep over `crates/` for each string finds nothing).

#### HIR fact to MIR carrier

| HIR fact | MIR carrier | Cite |
|---|---|---|
| `Place.volatile`, `IndirectPlace.volatile` | `load/store volatile` | `untyped_place:1257,1263,1268,1278`; load `1212`; store `1456-1458` |
| `DescriptorPlace` | never volatile, no tbaa tag | `1284` |
| `IndirectPlace.published` | dropped | no read in `mir.rs`; only `codec.rs:157,915-926`. Legacy `B/hir/lower.rs:872-906` folds it into `volatile` |
| `IndirectPlace.inbounds` | GEP `inbounds` | `1278` -> `offset:1361-1368` |
| `IndirectPlace.origin` | dropped | only HIR check `verify.rs:420`; `A/ranges.rs:9-11` says "the old far origin has no counterpart" |
| `IndirectPlace.allocation` | `!tbaa` tag "allocation" | `1274-1275`; tags `Tags::new:181-194` |
| `Place`/array/projected access | `!tbaa` tag "place"; array GEP and projected offsets always `inbounds` | `1257,1263,1268`; `element:1322,1356-1357`; `base:1302` |
| indirect access proven inside a known `Place.extent` | `!tbaa` tag "place" | `1273,1276`; extent recorded `1482` |
| `AliasClass` | `!tbaa` type tree; tag only for classes with a parent | `class_tags:200-221`; applied `place:1248-1249`, `tagged:1291-1294` |
| `Type.bounds` | index minus lower bound, row stride multiply (no range attribute) | `element:1340-1354` |
| `Instruction.nowrap` | `nsw` on add/sub/mul and on neg (sub 0,x) | `1407`, `1511` |
| `Instruction.inbounds` (PTR_OFFSET) | GEP `inbounds` | `1546-1547` |
| `Type.signed` | sext/zext/fptosi/fptoui and signed vs unsigned icmp; byte call args get `signext`/`zeroext` | `1342,1428,1488,1542`; `extensions:1676-1686` |
| `Instruction.pure` | dropped | no read in `mir.rs`; only `codec.rs:173,967` |
| `Instruction.line` | `!dbg !{i32 line}` | `line_nodes:253-262`; `lined:1132-1139`; `emit_block:1111-1123` |
| `Instruction.asm`, `Op::Asm` | not lowered (`other => Err("HIR asm")`) | `1668` |
| `Terminator::Unreachable` | `unreachable` | `1805` |
| `Block.cold` | `cold` on every call in the block (call-site attr) | `emit_block:1125-1127`; `mark_cold:1014-1028` |
| `CallAbi.cleanup/distance` | calling convention number | `convention`; calls `1588,1637,1650` |
| `CallAbi.promises` | call-site arg attrs `nocapture writeonly initializes((0,bytes))` | `1613-1621` |
| `ProcedureAbi.variadic` | function type `variadic` | `657-658` |
| Callee raises no error (`RuntimePromises.nounwind`) | call instead of invoke; `nounwind` on runtime declaration | `1651`; `promised:104,111` |
| `Function.linkage` | `internal`/external linkage | `659-662` |
| `Function.promises.unaliased/readonly/bytes` | param `noalias`, `readonly`, `dereferenceable(bytes)` (only when bytes > 0) | `670-678` |
| `Function.error_handler*`, `Module.line_numbers`, statement table | invoke/landingpad handling, not attributes | `H/mir/handling.rs:452-468`; `mir.rs:373,387-391` |
| `DataObject.readonly` | `constant` global (when it has a typed layout) | `533` |
| `DataObject.linkage` | External (External/Exported), Internal, Private | `527-531` |
| `DataObject.align` | global `align` | `532-533` |
| `DataObject.address=Far` | `address_space` FAR | `535` |
| `DataObject.addressed=false` (external) | listed in `!llrm.named` of the runtime module | `runtime:56`, `promised:91-96` |
| `DataObject.segment`, `Program.constant_segment` | dropped from MIR; `constant_segment` read by the driver | no read in `mir.rs`; `B/driver/mod.rs:61` |
| `Program.entries` | `Program.exports.entries` (not a MIR attribute) | `B/driver/mod.rs:84`; `M/program.rs:Exports.entries:62` |
| `Program.preserved` | no MIR carrier; read by the legacy ABI `B/abi/qb.rs:922` | not in `mir.rs` |
| `RuntimePromises.calling_back/writers` | runtime decl `nocallback` + `!llrm.writes` node | `promised:103,110,112-118` |
| `RuntimePromises.reads_arguments` | runtime decl `memory(argmem: read)`, pointer params `nocapture` | `promised:120-137` |
| `Program.zeroed_locals` | `llvm.memset` of aggregate locals | `48`, `723-761`, `1163-1175` |
| `Module.debug` | `llrm.dbg.*` named metadata, `llvm.dbg.declare` with `!var` | `H/mir/debug.rs:14`; `M/debuginfo.rs:90-94` |
| `Callable.by_value/segmented/arrays/defined/symbol`, `Function.symbol` | dropped | not read in `mir.rs` |
| `Type.rank`, `Place.symbol` (beyond data lookup) | dropped / lookup only | `465,741,746,1309` |

#### Consumers

##### Attributes, flags, metadata: carrier, reader, use

| Carrier | Written by | Read by (file:function:line) | What it does |
|---|---|---|---|
| param `noalias` | `H/mir.rs:673` | `A/alias.rs:seeds:180-182` | makes the parameter a restrict root |
| | | `A/memory.rs:Provenance::intersects:319-321` | disjoint restrict roots prove no overlap (reaches GVN, DSE, promote, hoist, loopmotion via `regions::may_alias` `A/regions.rs:212-222`) |
| | | `M/alias.rs:object:78-81` | `Object::Unaliased`, distinct from every other object (llrm-mir stack only) |
| | | `M/memory.rs:invariant:221-226` | `noalias readonly` param memory is never written (`transforms/licm.rs:105`, `earlycse.rs:95,118,139`; llrm-mir stack only) |
| param `readonly` | `H/mir.rs:674`; `T/interprocedural.rs:426` | `M/memory.rs:through:186`, used by `A/alias.rs:_allowed:352-356` | limits what a call does through that argument |
| | | `M/memory.rs:invariant:226` | see above |
| param/fn `readnone`, `writeonly` | `T/interprocedural.rs:425,427` | `M/memory.rs:through:185,187` -> `A/alias.rs:_allowed:352-356`, `stated:126` | call effects |
| param `nocapture` | `H/mir.rs:1618`, `135`; `T/interprocedural.rs:419-420` | `A/alias.rs:_borrowed:381-385` | argument does not escape |
| | | `M/memory.rs:nocapture:84-87` -> `M/alias.rs:captured:102` | stack slot does not escape (llrm-mir stack only) |
| call-site `writeonly` | `H/mir.rs:1618` | `M/memory.rs:through:187`, `A/alias.rs:_allowed:354-355` | callee only writes through it |
| `initializes((0,bytes))` | `H/mir.rs:1618`; `T/interprocedural.rs:435-436` | `A/alias.rs:_fills:704-716`; `A/alias.rs:831-832` | call acts as a fill of those bytes for availability and dead stores |
| `memory(...)` (fn and call) | intrinsics `intrinsics.rs:337`; `H/mir.rs:132`; `T/ports.rs:24`; `T/interprocedural.rs:489`; `M/transforms/functionattrs.rs:46-48` | `M/memory.rs:at:155-177`, `stated:126`, `located:144`, `inaccessible:150` | call read/write footprint |
| | | `A/effects.rs:call_effects:38-43`, `unmodeled:46-52` | unmodeled reads/writes of a call |
| | | `A/memory.rs:unmodeled_write:906`; `A/memoryssa.rs:132,134` | which calls define a memory state (GVN, DSE, LICM, loopmotion, promote through `Accesses`) |
| | | `T/dead.rs:75`, `T/loopmotion.rs:93`, `T/fill.rs:455` via `M/memory.rs:only_value:115-122` | call removable when touches no memory and returns |
| | | `B/backend/isel.rs:2368`, `911` via `M/memory.rs:of:201` | selection ordering |
| | | `T/ports.rs:38` | skip a port call already narrowed |
| `willreturn` | intrinsics `intrinsics.rs:132,165,186,213,249`; `T/interprocedural.rs:411` | `M/memory.rs:returns:79-81`, `call_returns:230-233` | call may be deleted if unused |
| | | `A/interprocedural.rs:erasable:336-340`, `stated_pure:322-331` | dead pure-call removal |
| `nounwind` | `H/mir.rs:111`; runtime; `T/interprocedural.rs:411` | `A/effects.rs:exposes_memory:83`; `A/interprocedural.rs:339` | invoke may unwind to a handler; dead-call removal |
| | | `B/backend/ehprepare.rs:40,165` | which calls need a landing pad |
| `nocallback` | `H/mir.rs:110`; intrinsics | `A/globalsaa.rs:calls_back:98-103` | call cannot reach program code, so tracked globals stay private |
| `noreturn` | `B/backend/ehprepare.rs:204,222`, `bc/llrm-bc/src/runtime.rs:211`; not by `H/mir.rs` | `A/noreturn.rs:terminal_sites:103` | call ends the path; tail cut (`T/interprocedural.rs:303`), hoist (`T/hoist.rs:67`) |
| `cold` (call site) | `H/mir.rs:1018` | `A/noreturn.rs:cold:117` | marks cold blocks. `noreturn::cold` has no non-test caller (grep); `B/backend/isel.rs:cold:793-814` uses only `unreachable` |
| `naked` | `B/backend/ehprepare.rs:222` | `B/driver/mod.rs:framed:127` | no frame |
| `signext` | `H/mir.rs:1685` | `B/backend/isel.rs:2288` | movsx vs movzx of a byte argument |
| `zeroext` | `H/mir.rs:1685` | none | written, read by nothing |
| `byval`/`byref`/`inalloca` | parser only | `B/backend/isel.rs:100-101` | refuses the function |
| `noinline`, `optnone` | parser only | `M/transforms/inline.rs:57` | llrm-mir stack only; `T/inline.rs` reads no attribute |
| `align(N)` param | parser only | `M/valuetracking.rs:alignment:120-124` | `T/hoist.rs:164` trap check |
| global `align` | `H/mir.rs:532-533` | `M/valuetracking.rs:alignment:138` | same |
| `dereferenceable(N)` param | `H/mir.rs:676` | `M/valuetracking.rs:dereferenceable:233-235` | `T/hoist.rs:163`, `M/transforms/licm.rs:104`: load may move without faulting |
| load/store/alloca `align` | never set by lowering (`M/build.rs:190,194` write `None`) | `M/interpret.rs:434` (alloca) | nothing in the optimizer |
| global `constant` | `H/mir.rs:533`; `T/globalopt.rs:24` | `A/memory.rs:constant_bits:745` | load folds to initializer bytes |
| global/function linkage | `H/mir.rs:527-531,659-662` | `M/program.rs:Exports::exported:78-86`; `T/globaldce.rs:45,54-55`; `T/globalopt.rs:42`; `A/alias.rs:_summary:576`; `A/interprocedural.rs:_exact:279-280`; `T/interprocedural.rs:387`; `A/manager.rs:167` | what outside code may reach; which bodies are exact |
| `Exports.entries` | `B/driver/mod.rs:84` | `M/program.rs:81` | root set |
| `unnamed_addr` | parser | none (only `T/pipeline.rs:466` sets default) | |
| global `address_space` | `H/mir.rs:535` | `A/interprocedural.rs:cannot_fault:315` | near static vs far |
| `personality` | parser | `M/verify.rs:326,396`; `T/globaldce.rs:70`; `B/backend/ehprepare.rs:139` | invoke/landingpad legality |
| calling convention | `H/mir.rs` via `convention` | `B/backend/isel.rs:115,121,616`; `assemble.rs:78` | ABI |
| flag `inbounds` (GEP) | `H/mir.rs:1278,1322,1356,1546` | `A/memory.rs:835` -> `Addr.inbounds:612-613,658` | |
| | | `A/ranges.rs:exact_offsets:530` | offset from an object's start is exact |
| | | `A/induction.rs:_inbounds_trips:1185` | access bounds the trip count |
| | | `T/fill.rs:303` | fill stride may not wrap |
| | | `A/pointerfacts.rs:Offsets::relative:49` | constant offset chain (`A/interprocedural.rs:312`) |
| flag `nsw`/`nuw` | `H/mir.rs:1407,1511` | `A/induction.rs:_promised:914-916` | recurrence does not wrap, so trip count is exact |
| | | `M/interpret.rs:525-537,562,629` | poison on overflow (llrm-mir `instcombine.rs:105,141`; tests) |
| | | cleared: `T/algebraic.rs:528,532,553`; `T/indvars.rs:604-611`; `M/transforms/instcombine.rs:227` | |
| flags `exact disjoint nneg samesign` | parser only | `M/interpret.rs:546-581,635` | poison semantics; llrm-mir stack and tests only |
| flags `nusw`, `nuw` on GEP | parser only | none | |
| fast-math flag `nsz` | parser only | `A/floatfacts.rs:268` | zero sign insignificant in float fold (`T/floatfold.rs`) |
| other fast-math flags | parser only | none | |
| `volatile` load/store | `H/mir.rs:1257-1278` | `M/memory.rs:of:207`; `A/memory.rs:unmodeled_write:897`; `A/effects.rs:48`; `A/memoryssa.rs:127-129`; `A/avail.rs:74`; `T/promote.rs:147`; `A/memory.rs:745`; `T/unroll.rs:139`; `T/loopmotion.rs:91`; `A/interprocedural.rs:317`; `T/interprocedural.rs:397-398`; `M/alias.rs:99-100` | ordered, never moved/forwarded/deleted; function gets inaccessible-memory write |
| `!tbaa` | `H/mir.rs:1293` | `A/memory.rs:typed:881-888` -> `MemRef.typed:611,680` -> `A/regions.rs:typed_apart:150-155` (name inequality only; parents ignored) -> `may_alias:219,240` | GVN, DSE, promote, loopmotion, hoist by way of `Accesses`/`regions` |
| | | `T/promote.rs:100,315` | cell type class |
| | | `M/alias.rs:tag:32`, `typed_apart:127-132` (parent chain honoured) | llrm-mir LICM/earlycse only |
| `!dbg` | `H/mir.rs:1137` | `B/backend/isel.rs:395` | line table |
| passes | | `M/edit.rs:118` new instructions carry no metadata; `clone_instruction:274` keeps it | `!tbaa`/`!dbg` dropped on rebuilt instructions |
| `!var`, `llrm.dbg.*` | `M/debuginfo.rs` | `B/backend/isel.rs:381,1321`; `M/program.rs:114` (observed globals kept) | CodeView |
| `!llrm.named`, `!llrm.writes` | `H/mir.rs:95,118` | `A/globalsaa.rs:promised:126-131` | tracked private cells and their writers |
| `unreachable` | `H/mir.rs:1805` | `A/noreturn.rs:_ends_cold:136`; `B/backend/isel.rs:cold:801` | block placement |
| `range(...)` attr | none | none | accepted, written and read by nothing |
| `nonnull`, `dereferenceable_or_null`, `returned`, `noundef`, `speculatable`, `nofree`, `nosync`, `norecurse`, `mustprogress`, `hot`, `minsize`, `optsize`, `alwaysinline`, `immarg`, `sret`, `elementtype`, `writable`, `alignstack` | `speculatable nofree nosync immarg` by intrinsics table `intrinsics.rs:132-212` | none | grep of quoted names finds no reader |
| `alias.rs` `Object` logic, `functionattrs`, `inline`, `ipsccp`, `licm`, `earlycse` in `M/` | | | reachable only from `M/transforms_tests.rs` |

##### Written, read by nothing (live pipeline)

- `zeroext`, `range`, `nonnull`, `speculatable`, `nofree`, `nosync`, `unnamed_addr`, GEP `nusw`/`nuw`, non-`nsz` fast-math flags, load/store/alloca `align`.
- `cold` call attribute: only `noreturn::cold` reads it and that has no non-test caller.
- HIR fields with no MIR carrier at all: `published`, `origin`, `pure`, `Asm`, `DataObject.segment`, `Callable.by_value/segmented/arrays/defined/symbol`, `Function.symbol`; `Program.preserved` (legacy `B/abi/qb.rs:922` only).
- `exact disjoint nneg samesign`, `noinline`, `optnone`: read only by the llrm-mir stack (tests).

##### Facts passes re-derive

| Fact | Derived in | How |
|---|---|---|
| function memory effects, `nocapture`, readnone/readonly/writeonly per param, `initializes`, `willreturn`, `nounwind` | `T/interprocedural.rs:stamped:376-445` | from `alias::summaries` and body walk; called `T/interprocedural.rs:330` |
| the same, llrm-mir stack | `M/transforms/functionattrs.rs:36-62` | |
| noreturn body | `A/noreturn.rs:inferred:45-62`, `fixed:66-79` | greatest fixed point over direct calls; a stated `noreturn` is an input |
| pure call | `A/interprocedural.rs:stated_pure:322-331`, `erasable:336-340` | needs stated attributes, which `stamped` supplies |
| cannot fault | `A/interprocedural.rs:cannot_fault:306-318` | frame slot or near static global, non-volatile |
| nonnull pointer | `A/alias.rs:nonnull:131-139`, `nonnull_by_definition:144-` | from points-to object kind (Frame/Global/External/Named); used by `T/decide.rs:101-105` |
| pointer capture | `A/alias.rs` summaries (`captures`), `A/alias.rs:_borrowed:381` | no `nocapture` inference for locals outside `stamped` |
| object bounds for dereferenceable | `M/valuetracking.rs:dereferenceable:222-243` (alloca size, global size, param attr); `A/ranges.rs:inside_object:77-82` (object extent) | |
| stated alignment only | `M/valuetracking.rs:alignment:112-169` | no derived alignment |
| integer ranges | `A/ranges.rs` (branch-scoped, counted loops, `:1-11`); `M/valuetracking.rs:sign_bits:17-64` (used by `B/backend/isel/wide.rs:28` only) | no `range` attribute input |
| loop trip counts | `A/induction.rs` (uses `nsw/nuw` via `_promised:909-919`, inbounds via `:1175-1192`); `T/counting.rs` | no trip-count metadata input |
| constant loads | `A/memory.rs:constant_bits:740-752` | uses global `constant` |
| TBAA-like class | `A/memory.rs:typed:881` | reads only the leaf name |
| port call memory | `T/ports.rs:silent:31-59` | writes `memory(inaccessiblemem: readwrite)` on call |
| global privacy | `A/globalsaa.rs` | from escape analysis plus `!llrm.named` |

### What HIR carries and what MIR does with it

| HIR field (MODEL) | MIR carrier (MIRH) | Read by |
|---|---|---|
| `Function.promises[].unaliased` (:699) | param `noalias` (MIRH:673) | `AN/alias.rs:175-184` seeds a restrict root; `AN/memory.rs:320` separates two provenances only when BOTH carry restrict roots (a `noalias` vs a plain pointer proves nothing, TR/transform.rs:876-877). `MIR/memory.rs:219-226` `invariant` (noalias+readonly) feeds MIR's own licm/earlycse only. |
| `.readonly` (:700) | param `readonly` (MIRH:674) | `MIR/memory.rs:186` call effects; `invariant` above. |
| `.bytes` (:697) | param `dereferenceable(n)` (MIRH:676) | `MIR/valuetracking.rs:222-243` -> `TR/hoist.rs:156-165` `_may_fault` (speculating a load out of a loop). |
| `Instruction.nowrap` (:474) | `nsw` on add/sub/mul/neg (MIRH:1407,1511) | `AN/induction.rs:909-919` `_promised`: unit-step trip-count maximum for non-constant bounds. No `nuw` field exists. |
| `IndirectPlace.inbounds` (:305) | GEP `inbounds` on the place offset (MIRH:1278,1367) | `AN/pointerfacts.rs:49` (`relative` sees through only inbounds GEPs); `AN/memory.rs:835`. |
| `Instruction.inbounds` (:476, PTR_OFFSET) | GEP `inbounds` (MIRH:1546) | same |
| array `ArrayElement`/`ProjectedPlace` | GEP `inbounds` over the declared array type, always (MIRH:1356) | same |
| `IndirectPlace.allocation` (:313) | one shared `!tbaa` tag "allocation" if `Some` (MIRH:1274-1276); the place id is dropped | `AN/memory.rs:880-887` `typed`, `AN/regions.rs:148-153` `typed_apart` (different tag names are apart) |
| `IndirectPlace.origin` (:310) | none | `AN/memory.rs:26-28` says `origin` is "not carried" |
| `IndirectPlace.published` (:303) | none in the rich route (grep `published` in MIRH and MIRH/handling.rs: no hit). Old route: `volatile: *volatile \|\| *published` (crates/backend/llrm-core/src/hir/lower.rs:905) | - |
| `Instruction.pure` (:471) | none (grep `.pure` in llrm-hir/src: only codec/model) | - |
| `Block.cold` (:517) | `cold` attr on calls of the block (MIRH:1014-1026,1125) | `AN/noreturn.rs:112-125` `cold`; a block ending in `unreachable` is cold without it |
| `AliasClass` (:785) | `!tbaa` type nodes (MIRH:197-217) | `AN/regions.rs:148` |
| `Program.zeroed_locals` (:912) | zero store / memset per frame group (MIRH:48,1142-1167) | - |
| `DataObject.readonly/align/addressed` (:750-761) | `constant`, `align` (MIRH:532-533), named-cells (MIRH:53-58) | `MIR/valuetracking.rs:112-140` `alignment`; `TR/hoist.rs:165` |
| `RuntimePromises.nounwind` (:867) | `nounwind` on runtime decls; call vs `invoke` choice (MIRH:1651, handling.rs:452-465) | `TR/interprocedural.rs:368-400` |
| `RuntimePromises.reads_arguments` (:870) | `memory(argmem: read)` + param `nocapture` (MIRH:122-137) | `MIR/memory.rs:86-94`; `AN/alias.rs:379-385` |
| `ArgumentPromise` on a `CallAbi` (:541) | call-site `nocapture writeonly initializes((0,n))` (MIRH:1614-1620) | `AN/alias.rs:379`, `MIR/memory.rs:187,197` |

MIR attributes that exist but have no consumer found: `range` (`MIR/opcode.rs:257`; grep `Attribute::Range` in crates/opt and MIR/transforms: parse/print only), `nonnull` (attribute name at `MIR/opcode.rs:297`; `AN/alias.rs:131-153` derives non-null from the object kind and says "Incoming pointers remain nullable"), `noundef`. No `llvm.assume` intrinsic (grep `assume` in `MIR/intrinsics.rs`: none). Param `align` is read (`MIR/valuetracking.rs:121`) but no HIR field sets it.

### Quick BASIC

Emission path: `SEM` `json()` (:9496-9810) writes HIR JSON; `llrm-qb/src/driver.rs:decoded` (:247-265) adds `addressed=false` for runtime cells, `entries`, `promises.nounwind`.

| # | Fact | (a) Known | (b) Emitted today | (c) Not emitted: carrier, win |
|---|---|---|---|---|
| 1 | Differently named variables do not alias | Each module variable and each STATIC is its own zero-filled internal `DataObject` (SEM:2518-2530, 2633-2646); locals get distinct place offsets, grouped into one alloca only when bytes overlap (MIRH:690 `frame_groups`). memory-model.md "Logical storage objects" says the classification exists. | Yes, implicitly: distinct MIR globals/allocas. | Nothing needed. |
| 2 | Differently named arrays do not alias | Static arrays: own data object (SEM:2518). Dynamic far arrays: own allocation (crates/frontends/qbfront/src/semantic/shapes.rs:11-13 "B$DDIM puts a far array's data at offset 0 of its own segment"). | HIR `IndirectPlace.allocation = Some(descriptor place)` for far-array elements (SEM:10324-10330). MIRH:1274 maps any `Some` to one shared tag "allocation", so array A and array B share a tag. Only "allocation vs place" is separated. | Carrier: proposed, one `!tbaa` sibling type per allocation id in `Tags` (MIRH:175-194), consumed by `typed_apart` (`AN/regions.rs:148`). Win: medium. Loops copying one dynamic array into another (`a(i) = b(i)`) currently reload/store as may-alias; `gvn`/`dse`/`hoist` would treat them apart. |
| 3 | BYREF parameters | BYREF is the default; BYVAL, SEG, array params kept per callable (`Callable.by_value/segmented/arrays`, SEM json :9697-9740; signature tuple SEM:134-140). Passing the same variable twice is legal QB. | `dereferenceable(width)` for BYREF scalars/UDTs, `14+4` for array descriptors, none for BYVAL/SEG (SEM:575-590, json :9668-9676). `noalias:false`, `readonly:false` always (SEM:9674). | `noalias` must stay off: QB allows `CALL f(x, x)`. Carrier that would be valid: call-site `noalias`-style facts at calls whose arguments are provably distinct variables (proposed, call-site param attr on the `call`; `AN/alias.rs:379` reads call-site attrs for nocapture only). Win: small to medium; the optimizer infers `readonly`/`nocapture` by itself for defined procedures (`TR/interprocedural.rs:368-440`). |
| 4 | BYREF pointee may change asynchronously | Every BYREF scalar access is `published: true` (SEM:4698, 6099); abi.md "Ordinary BYREF": the VBDOS IN_KEYSTROKE loop hangs if the load is reused. | Yes in HIR (`IndirectPlace.published`). Rich route drops it (table 1 above): the BYREF load becomes a plain load. | **Possible miscompile in the default route**, not a missed optimisation. Carrier: `volatile` load (existing; MIRH:1278 passes `one.volatile`), set `volatile: published` there, or frontend sets `volatile`. Win: correctness. I did not build the IN_KEYSTROKE case to confirm the hang; what I checked is the absence of any read of `published` in the rich emitter. |
| 5 | SHARED | Module variables are visible to a procedure only if declared SHARED at module level or by the procedure's own SHARED statement (SEM:518-540, 2061-2070, 2199-2215). | Only as visibility: the procedure refers to the module data object (`Storage::Module`, symbol). No "not address-taken" flag. | Carrier: existing `GlobalVariable` linkage internal (emitted, SEM:2643) plus GlobalsAA capture analysis (`AN/globalsaa.rs`; `AN/memory.rs:17` "A global is `captured` unless GlobalsAA tracks it"). The only escape is passing it BYREF. Win: small; already derived. |
| 6 | STATIC | `STATIC` changes lifetime, not scope (SEM:2034-2041 comment, 2055-2060); own zero-filled internal object (SEM:2518-2530). | Yes: internal data object, `align` 2 for word elements (SEM:2515-2517). | none |
| 7 | COMMON | Not modelled. generated-parser.md:74 "nor model FAR/HUGE/COMMON"; legacy-removal-matrix.md:57 lists COMMON storage as remaining work; grep `Common` in qbfront/src finds no statement node. HIR has `Storage::Common` (MODEL:202) but qbfront never emits it (grep `"common"` in qbfront/src: none). | No | Unknown-to-frontend. Nothing to carry until parsed. |
| 8 | Pure functions / cannot modify globals | No notion in the language; DEF FN and FUNCTION may write SHARED/BYREF. abi.md "Audited file and string calls": "exact cleanup does not imply purity". | `pure:false` on every instruction (SEM:9539). | Carrier: function `memory(...)`/`readnone`/`nounwind`/`willreturn`, inferred by `TR/interprocedural.rs:368-440` (and `MIR/transforms/functionattrs.rs`, which skips call cycles, :30-33). Win: already obtained for defined procedures; recursion cycles and external `DECLARE`d procedures get nothing. Small. |
| 9 | FOR trip count | `end` and `step` are evaluated once into `$forEnd`/`$forStep` temporaries (SEM:4244-4257); test is per iteration, both directions, on the loaded step (SEM:4270-4318). The body may assign the counter (it is the user's variable, SEM:4225-4232), so the count is not an invariant of the loop. | Integer increment carries `nowrap` -> `nsw` (SEM:4342-4344; MIRH:1407). Bounds and step reach MIR as stores to temporaries. | Trip count itself: not emitted and not sound to state in general (counter writable). Derived after mem2reg/`promote` by `AN/induction.rs` when bounds are constant; `nsw` gives the unit-step maximum for variable bounds (`AN/induction.rs:808-816`). Win of an explicit count: small. SINGLE/DOUBLE counters get no flag (correct). |
| 10 | Integer overflow | Default: wraps (plain `add`, SEM:8147; `nowrap` only on FOR). `-fsanitize=signed-integer-overflow` emits a check and error 6 for INTEGER `+ - *` and LONG `+ -` (SEM:8140-8148, `overflow_checked` :8216-8237; cli.rs:107-110). FOR raises Overflow rather than wrapping (comment SEM:117) but no check is emitted for it. | `nowrap` on the FOR add only. | With checks on, the checked op could use `llvm.sadd.with.overflow` (intrinsic exists, `MIR/intrinsics.rs:168-173`) and a cold error block instead of xor/and/lt on the wrapped sum. Win: medium under `-fsanitize`, none by default. Without checks no `nsw` is valid (wrap is the compatible behaviour, docs/semantics/integer-overflow-policy.md). |
| 11 | Array bounds / OPTION BASE | Static bounds are constants in `Type.bounds` (SEM:10340-10350 `type_json`); `OPTION BASE` read once per module (SEM:1647-1662) and applied at :2337, :6001. Dynamic arrays: crates/frontends/qbfront/src/semantic/shapes.rs:1-12 proves constant counts/lower bounds where all allocations agree. | Bounds go in HIR `Type.bounds`; MIRH:1310-1356 linearises with `lower` subtracted and GEP `inbounds`. Dynamic: `origin` and `allocation` on the element pointer (SEM:10324-10336). Unchecked by default; `-fsanitize=bounds` routes every element through B$HARY (SEM:4713-4717). | `origin` not carried to MIR (see table 1). Carrier: existing `inbounds` GEP keeps "stays in object"; the origin (non-negative offset from the array start) would be proposed as `!range` on the offset value or an `nuw` on the scaled index. Win: small to medium for dynamic-array loops (hoisting descriptor loads needs the origin; `AN/pointerfacts.rs:49` only follows inbounds GEPs). Index range "0 <= i-lower < count" is not stated; the optimizer can only use it if a check exists. |
| 12 | Which calls raise errors | Runtime contracts carry `raises_error` (crates/frontends/llrm-qbruntime/src/lib.rs:253); QB checks are opt-in (`-fsanitize`). ON ERROR makes each raising call reachable from the handler. | `promises.nounwind` = contracts with `raises_error == false` (driver.rs:262). A call not in the list is an `invoke` to a pad only when the function or module has a handler (MIRH:1651, handling.rs:452-465). Error paths are `cold` blocks ending `unreachable` (SEM:8158-8165, 9257-9261; MIRH:1125). | Runtime `Control::Never` (B$ERR_xx etc.) is not stated `noreturn` (grep `noreturn` in MIRH: none); the `unreachable` terminator already cuts the path. Memory effects of contracts (`reads`/`writes: Memory`, qbruntime lib.rs:255) are not mapped to `memory(...)` (grep `Memory::` in MIRH: none); only `nocallback` + `!llrm.writes` for named cells are (MIRH:108-119). Win: medium for runtime-call-heavy loops (string ops): a call known to write only `Own` memory no longer clobbers all memory in `gvn`/`hoist`. `reads_arguments` is empty for QB (driver.rs:260 passes `RuntimePromises::of`, which sets it `Vec::new()`, MODEL:877-890). |
| 13 | String descriptors | Dynamic STRING is a 4-byte near descriptor owned by the runtime; expressions produce temporaries; `B$FLEN` may consume a temporary (memory-model.md "Procedure calls and frames"). | Opaque bytes + runtime calls (docs/frontends/qb/readme.md "Measured string boundaries": "Runtime calls remain visible and effectful"). | Carrier: `memory(argmem: read)` + `nocapture` on read-only routines (`B$FLEN`, `B$SCMP`) via `RuntimePromises.reads_arguments` (existing carrier, unset). Win: small to medium: `LEN`/compare in loops could be CSE'd/hoisted. Caveat from the doc: `B$FLEN` consumes temporaries, so it is not read-only for temporaries; not safe to state globally. |
| 14 | Uninitialised = zero | Locals start zeroed by `B$ENRA`; with own frames the frontend stores zero itself (SEM:1483-1485, `zero_locals` :1568-1596). Module/STATIC data is zero-filled (SEM:2524, 2640; "BASIC startup clears BC_DATA", docs/frontends/qb/readme.md). | `Program.zeroed_locals` defaults true (codec.rs:1391), so MIRH:1142-1167 zero-stores each frame group or memsets it; data objects get `zeroinitializer` (MIRH:data_initializer: all-zero bytes -> `ConstantKind::Zero`). | Carried. Win of the existing carrier: `mem2reg`/`promote` turn reads-before-write into `0`. |

### Nib

Emission path: `NIB/hir.rs` `json()` (:204-285), `function_json` (:287-405). Its `Instruction` has no `nowrap`, `inbounds`, `pure` or `cold` (hir.rs:75-85, 104-110); JSON writes `"pure":false` (hir.rs:329). Program JSON has no `zeroed_locals` key (hir.rs:284), so codec default `true` applies (codec.rs:1391).

| # | Fact | (a) Known | (b) Emitted today | (c) Not emitted: carrier, win |
|---|---|---|---|---|
| 1 | `&mut T` excludes aliases | Language: "While an exclusive borrow exists, no other borrow may access the same value. While shared borrows exist, the value may not be mutated or moved" (docs/frontends/nib/language-spec.md §8). Enforced per call: same owner twice with either mutable is an error (NIB/semantic/calls.rs:248-258); no owner written while a borrow binding is in scope (semantic/borrows.rs:1-4). Raw pointers "carry no ... aliasing guarantee" (spec §8). | Only for borrowed SLICE/VIEW params, and only on the 8-byte view descriptor: `Promise{unaliased:true, readonly:true, bytes: descriptor+4}` (semantic/mod.rs:1663, comment :1661-1662 "only reseating a binding writes one"). `&mut T`/`&T` for scalars and structs (`SignatureParameter::Borrowed`, mod.rs:876-880): no promise. Owned aggregate params (callee gets its own copy, calls.rs:238-243): no promise. | Carrier: existing param `noalias` (+`readonly` for `&T`, `dereferenceable(sizeof T)`) via `Promise`, for every borrowed and owned-aggregate param. Payload of a `&mut [T]`: `noalias` on the far data pointer is not expressible (the param points at the view, payload is behind a load); proposed: `!noalias`/scope metadata on the payload load, or passing the payload pointer as the parameter. Win: large for loops over `&mut [T]` plus a `&T` (`translate(points, delta)` reloads `delta.x` per iteration today): `AN/memory.rs:320` needs restrict roots on both pointers, so emit on all borrowed params at once. Drop it for functions with `unsafe` blocks that take raw pointers (spec §8 gives no guarantee there); I did not find a frontend flag that says so. |
| 2 | Borrows are non-owning, scoped | References cannot be stored, returned or outlive the local (docs/frontends/nib/readme.md; borrows.rs:1-4 `roots`). | - | Gives `nocapture` for borrowed params, except references kept in tuples/enums/generator items (references.rs:1-3, "`&T` inside a tuple, an enum payload or a generator's item is a far pointer"). Carrier: param `nocapture` (existing; `AN/alias.rs:379-385`). Win: small; `TR/interprocedural.rs:419-425` infers it for defined functions. |
| 3 | Nonnull references | No `null` in safe code (spec §3 "There is no `null`"); `0` is null only in `unsafe` raw pointers (spec §8). | No | Carrier: param `nonnull` (existing name, `MIR/opcode.rs:297`). Consumer only via `AN/alias.rs:136-153`, which derives it from object kind and ignores the attribute. Win: small (only matters if code compares a reference to null, which safe Nib cannot write). |
| 4 | Dereferenceable | Reference points at a whole `T`/view; size known (`descriptor::size(rank)+4` for slices, mod.rs:1663; struct width from `types.width`). | `bytes` only for slice descriptors (mod.rs:1663). | Carrier: `Promise.bytes` -> `dereferenceable(n)` (MIRH:676) for `&T`/`&mut T`/owned struct params. Consumer `TR/hoist.rs:156-165`. Win: medium: loads of struct fields through a reference can be hoisted out of loops that may run zero times. |
| 5 | Alignment | Structs at most 2-byte aligned on the 16-bit target (docs/frontends/nib/readme.md "Struct fields stay in source order, with at most two-byte alignment"); array descriptors are 16-bit words (docs/frontends/nib/readme.md). | No: `NIB/hir.rs:39-46` `DataObject` has no `align`; MIRH:1147 `alloca` has none; no param `align`. | Carrier: `DataObject.align` -> global `align` (MIRH:532); proposed: alloca align and param `align` from a HIR field (none exists). Consumer `MIR/valuetracking.rs:112-140` -> `Machine::load_may_trap` in `TR/hoist.rs:165`. Win: small (a word load at an odd offset only traps at offset FFFFh). |
| 6 | Ranges of fixed-width integers | `Type.width/signed` in HIR; `bool` is 0/1; `char` is a byte; enums have a tag (spec §3, §5). Exhaustive `match` has an `unreachable` default (semantic/matching.rs:66-68). | Widths become MIR `iN` (MIRH:value_type). No range. | Carrier: existing `range(iN lo, hi)` param/return attr (`MIR/opcode.rs:257`) and proposed `!range` on loads of `bool`/`char`/enum tag; **no consumer exists** (grep). Win: small today; medium once `AN/ranges.rs` reads it (zero-extend and mask elision on byte values, tag switches). |
| 7 | Array bounds | Fixed arrays: dims are constants, row-major, zero-based (docs/frontends/nib/readme.md "Fixed arrays have a zero lower bound"). Constant index checked at compile time (semantic/checks.rs:47-53). Runtime index: explicit unsigned compare + branch + `N$EBND` call + `unreachable`, unless `unsafe` or `--unchecked-bounds` (checks.rs:13-16, 54-62, 85-94). Views: dims loaded from the descriptor (checks.rs:13-23). | `Type.bounds` (hir.rs:12); element access `ArrayElement` (fixed arrays in place) or `IndirectPlace{inbounds:true}` (pointer form, semantic/mod.rs:1042, set whether or not a check ran). The indexing `ptr_offset` (indexing.rs:`indexed_pointer`) has no `inbounds`, so the GEP that adds the index is plain (MIRH:1546 reads `Instruction.inbounds`, Nib cannot set it). | Carrier: `Instruction.inbounds` on `ptr_offset` (HIR field exists, MODEL:476; Nib's HIR writer has no key); `nuw` on the `mul index, width` (no HIR field: proposed `nowrap`-style `nuw`). Win: medium for slice loops: `AN/pointerfacts.rs:49` follows only inbounds GEPs, so element addresses cannot be related to the slice base. The check itself is "the optimizer's to fold" (checks.rs:5-7); the facts that let it fold (loop range `i < len`) are in the `lt` compare. |
| 8 | Range loop trip count | `for i in a..b` evaluates both bounds once, immutable `i` of the bounds' type, exactly `b-a` iterations (docs/frontends/nib/readme.md "Its immutable induction variable"). Emitted as `$range_i` counter and `$range_limit_i` places (semantic/loops.rs:444-445), test `lt`/`below`, then `add i,1` (loops.rs:533). | Nothing on the add: `i < limit` holds there, so `i+1` cannot wrap in either signedness. | Carrier: `nowrap` -> `nsw` (HIR field exists, Nib writer lacks it) and proposed `nuw` for unsigned counters. Consumer `AN/induction.rs:909-919`. Win: large for variable bounds: without a flag the unit-step maximum `_unit_maximum` (`induction.rs:810-815`) has no `promised` and is not derived, so peeling/unrolling/loop-deletion and bounds-check folding lose the count. Constant bounds are already counted (:772-783). |
| 9 | Integer overflow | Two's complement, wraps to the width; division by zero and `min // -1` panic; shifts at or past the width panic; float-to-int out of range panics (spec §3, §Conversions). Shift/convert checks emitted (operators.rs:541, checks.rs:70-82); division relies on the #DE handler `N$EDIV` (runtime/start.asm:84), no compare emitted (division.rs). | No `nowrap` anywhere (wrap is the defined behaviour). | Nothing valid to add except the range-loop increment (row 8) and post-check index scaling (row 7). |
| 10 | No-return | There is no `!` type (grep "never/diverg/noreturn" in NIB and docs/frontends/nib/language-spec.md: none). The compiler knows `panic` never returns (checks.rs:84). | Each panic is a call to `N$E*` followed by an `unreachable` terminator (checks.rs:85-94). No `cold` field in Nib's `Block` (hir.rs:104-110). | Nothing needed: `AN/noreturn.rs:105-125` treats blocks ending in `unreachable` as cold. `noreturn` on the `N$E*` declarations (proposed: runtime promise, none in `RuntimePromises`) would additionally cut code after calls in functions compiled separately. Win: small. |
| 11 | Purity of `fn` | Language has none: `fn` may take `&mut`, call `unsafe` foreign code, print. No attribute in spec (grep `pure` in docs/frontends/nib/language-spec.md: none). Nib knows which `fn` take no `&mut` and no global writes only by analysis. | `pure:false` (hir.rs:329); no function-level attribute field in Nib's `Function` (hir.rs:135-152). | Carrier: function `memory(...)`/`readnone`/`nounwind`, inferred by `TR/interprocedural.rs:368-440`. Win: already obtained for defined functions. Small. |
| 12 | Known-initialised | Every binding needs an initialiser: `Statement::Bind { value: Expr }` is not optional (syntax.rs:468-474). Frames are not zeroed at runtime (freestanding start, runtime/start.asm:38 comment about stale memory). | `zeroed_locals` left at default `true`: MIRH:1142-1167 zero-stores every scalar frame group and memsets every aggregate before the real initialiser. | Carrier: `Program.zeroed_locals = false` (existing HIR field, codec.rs:360-362). Win: small to medium: one dead store per local, one `memset` call per struct/array local on every call; whether `dse` removes the memset when the literal overwrites it all I could not tell (grep `memset` in `TR/dse.rs`: no hit). |
| 13 | TBAA / type classes | Nib values are typed structs/scalars; no type punning in safe code. | No `alias_classes` (grep in NIB: none). All place accesses get the single "place" tag. | Carrier: `AliasClass` (MODEL:785) -> `!tbaa`. Win: small; most Nib memory is distinct objects, and pointer-based accesses are `&T` of one type. |
| 14 | Read-only data | Literals are read-only module objects (docs/frontends/nib/readme.md "String literals are read-only module objects", "Decimal floating literals are stored once in read-only module data"). | `DataObject.readonly` -> `constant` global (hir.rs:39-46, semantic/mod.rs:137,167,180; MIRH:533). | Carried. |

### Gaps ranked by expected win (from the survey)

| Rank | Gap | Frontend | HIR field | Where it pays |
|---|---|---|---|---|
| 1 | `nowrap` on the range-loop `add` | Nib | `Instruction.nowrap` (exists; Nib writer lacks it) | `AN/induction.rs` trip counts with variable bounds |
| 2 | `noalias`/`readonly`/`dereferenceable` for all borrowed and owned-aggregate params | Nib | `Promise` (exists) | `AN/memory.rs:320`, `TR/hoist.rs` |
| 3 | `published` dropped in the rich route | QB | `IndirectPlace.published` (exists) | correctness of BYREF polling loops |
| 4 | `inbounds`/`nuw` on indexing `ptr_offset` | Nib | `Instruction.inbounds` (exists); `nuw` proposed | `AN/pointerfacts.rs:49` |
| 5 | per-allocation `!tbaa` type | QB | `IndirectPlace.allocation` id (exists, dropped at MIRH:1274) | `AN/regions.rs:148` |
| 6 | runtime contract memory effects as `memory(...)`; `reads_arguments` | QB | `RuntimePromises` (exists) | call clobber scope in `gvn`/`hoist` |
| 7 | `zeroed_locals=false` | Nib | `Program.zeroed_locals` (exists) | entry stores |
| 8 | `with.overflow` for `-fsanitize` checks | QB | none (MIR intrinsic exists) | checked builds only |
| 9 | `range` on narrow loads | Nib | none; consumer missing | after `AN/ranges.rs` reads it |

Unclear, tried: (1) whether `dse` removes a full-overwrite `memset`: grep only. (2) Whether `TR/hoist.rs` consults `restrict` roots: read `AN/memory.rs:320` only; I did not trace every caller of `Provenance::intersects`. (3) QB BYREF `published` behaviour at run time: no build run (read-only task).


## 8. Proposal: one way to state a fact

Reviewed by Opus 5.5 (big-picture); its changes are in.

### The mechanism

| Step | What | Where |
|---|---|---|
| State | `facts.state(subject, fact)` on `llrm_hir::facts::Builder`, built as `debug::Builder` is (`crates/ir/llrm-hir/src/debug.rs`; C, Nib and Quick BASIC already hold one). The only way a frontend writes a fact. | every frontend |
| Declare | one table macro declares each `Fact`: variant and value type, allowed subject kinds, codec key, rewrite policy. Adding a fact is one row. | `llrm-hir` `facts.rs` |
| Store | `Module.facts`, a list of `(Subject, Fact, Provenance)`; `Provenance` is the frontend and source line, HIR only | `model.rs`, `codec.rs` |
| Check | the verifier checks subject kind, value type, and that every operand a fact names exists and dominates its subject | `verify.rs` |
| Lower | one `lower(subject, fact)` by exhaustive `match`: a fact with no arm does not compile | `mir.rs`, replacing the per-field code (`MIRH:673-678,1125,1278,1407`) |
| Read | `llrm_mir::facts`: one typed accessor per carrier. No pass parses an attribute or metadata by name, and no pass re-derives what a frontend stated (rule 7). Stated and derived facts come out of the same accessor. | `llrm-mir` (`M/memory.rs` and `M/valuetracking.rs` already read carriers there) |
| Rewrite | each fact has a policy in the table: kept on clone, intersected on merge (CSE, GVN), dropped on speculation or hoisting if it holds only on a path. Helpers `facts::merge(a, b)` and `facts::speculated(i)` apply it, so no pass learns a fact. Today `M/edit.rs:118` gives new instructions no metadata and `clone_instruction:274` keeps all. | `llrm-mir` |
| Prove | checked mode: the MIR interpreter traps on a violated `range`, `nonnull`, `noalias` or `readonly` under a flag, as it already does for `nsw` (`M/interpret.rs:525`). One end-to-end test per variant: an exhaustive `match` in a test names a fixture for each (source, HIR fact, MIR carrier, accessor, one optimisation). | tests |

Rules of the mechanism:

1. **A fact is droppable.** Removing it never changes what the program means; absence is the conservative case. Anything that changes meaning is IR proper, below.
2. **`Subject` exists only in HIR.** After `lower`, the fact is the carrier on the MIR entity. Subject kinds: `Function`, `Param(n)`, `Return`, `Instruction(id)` (a call, an access or arithmetic; the verifier checks the op fits), `Block`, `Place`, `Object`, `Program`. A `Place` fact fans out to every access of that place in `lower`, since places do not exist in MIR.
3. **A fact's value may name operands, subjects or a group id** (as `AliasClass` already names a group). This is how relations are stated (`Assume(cond)`, `Callees(set)`, scoped no-alias). The verifier checks the reference.
4. **Storage is a side list while HIR is write-once.** If HIR ever gets a rewriting pass, facts move onto the entity so ids need no remapping.
5. **A variant lands with its carrier, reader and test in one commit.** Rows below marked *later* are a roadmap, not schema.
6. **Frontends state what the language promises** and nothing about how a pass uses it.

### Not facts: IR proper

These change meaning, so they are fields the verifier enforces, not droppable facts.

| Item | Where it lives | Consequence |
|---|---|---|
| Ordered access (volatile, BYREF that another agent may write) | `volatile` on the access | `published` is retired; Quick BASIC sets `volatile`. This is the hang fix (defect 1). |
| Returns twice (`setjmp`) | a flag on the callee | changes the control-flow graph |
| Memory kind: port, fixed address, program | address space on the place | I/O ordering is meaning |
| Debug types and locations | `debug::Builder` | not a promise about the program |

### Debt: channels that are not this mechanism

| Existing channel | Why it is debt | Replaced by |
|---|---|---|
| `CGAttr n 3` for `restrict` (`toolchain/owshim/patches/cgen.c.patch`, `translate.rs:522,849`) | a private code for one fact from one frontend | one `FACT` record from the patched front end; `ow_facts` decodes it to `state(Param, NoAlias)` (rows 3, 4) |
| call-class decoding (`hir.rs:176-181`, `translate.rs:446`) | reads three bits, ignores the rest | the same `ow_facts` table, for every bit (rows 2, 5) |
| `IGNORED` (`hir.rs:57-69`) | silently drops records | the table lists each record it handles and each it ignores, with a reason; an unlisted record is an error |
| `Promise`, `Instruction.nowrap/inbounds/pure`, `Block.cold`, `IndirectPlace.published/inbounds`, `DataObject.readonly/align`, `Program.zeroed_locals`, `RuntimePromises` | each has its own field, codec entry and lowering | facts on the matching subject; each field goes when its last frontend has moved |
| `H/mir.rs` dropping `pure`, `origin`, `allocation` id | a fact stated and lost | `lower` has an arm for every fact |

The Open Watcom transport: the patched front end emits language terms (`restrict`, `const-pointee`, `static N`), never llrm schema names, so the patch knows nothing of HIR. The `FACT` record carries a version, and a stale build fails loudly. Facts Open Watcom already sends (call class, `FE_*`, `CGVolatile`) go through the same `ow_facts` table; `translate.rs` holds no per-fact code.

### Rows (roadmap)

Status: *now* the carrier and a reader exist; *reader* the carrier exists and nothing reads it; *carrier* neither exists. Rewrite: K kept on clone, I intersected on merge, S dropped if speculated.

| # | Fact | Subject | HIR call | MIR carrier | Read by | Status | Rewrite |
|---|---|---|---|---|---|---|---|
| 2 | Does not return | `Function`, `Instruction` | `NoReturn` | `noreturn` | `A/noreturn.rs:terminal_sites:103` | now | K I |
| 3 | Distinct object | `Param` | `NoAlias` | `noalias` | `A/alias.rs:seeds:180`, `A/memory.rs:319` | now | K |
| 4 | Not written, not kept, non-null, extent | `Param` | `ReadOnly`, `NoCapture`, `NonNull`, `Dereferenceable(n)` | `readonly`, `nocapture`, `nonnull`, `dereferenceable(n)` | `T/hoist.rs:156-165`; `nonnull` derived at `A/alias.rs:131` | now / reader | K |
| 5 | Memory effect | `Function`, `Instruction` | `Memory(None \| Read \| ArgRead \| ..)` | `memory(..)`, `readnone` | `M/memory.rs:at:155`, `A/effects.rs:call_effects:38` | now | K I |
| 6 | Raises no error; ends | `Function` | `NoUnwind`, `WillReturn` | `nounwind`, `willreturn` | `A/effects.rs:83`, `A/interprocedural.rs:336` | now | K I |
| 7 | No wrap | `Instruction` | `NoWrap{signed, unsigned}` | `nsw`, `nuw` | `A/induction.rs:_promised:914` | now | K S |
| 8 | Stays in its object | `Instruction` | `InBounds` | GEP `inbounds` | `A/pointerfacts.rs:49` | now | K S |
| 9 | Value range | `Instruction`, `Param`, `Return` | `Range{lo, hi}` | `range(iN lo, hi)` | none | reader | K I S |
| 10 | Holds after a check | `Instruction` (the branch) | `Assume(cond)` | assume or range | none | carrier | S |
| 11 | Alias class | `Instruction` | `AliasClass(id)` | `!tbaa` | `A/regions.rs:typed_apart:150` | now | K I |
| 12 | Immutable after init | `Object`, `Place` | `Immutable` | `constant`, `!invariant.load` | `A/memory.rs:constant_bits:745` | now | K |
| 13 | Known initialised | `Function` | `NoEntryZeroing` | no entry stores | `MIRH:1142-1167` | now | — |
| 14 | Alignment | `Place`, `Object`, `Instruction` | `Align(n)` | `align` | `M/valuetracking.rs:alignment:120` (parameters, globals only) | now / reader | K I |
| 15 | Inline, unroll hints | `Function`, `Block` | `Inline(Hint)`, `Unroll(n)` | `alwaysinline`, `noinline`; loop metadata | none | reader | K |
| 16 | Cold; likely | `Block` | `Cold`, `Weight(n)` | `cold`; `!prof` | `A/noreturn.rs:cold:117` (no non-test caller) | reader | K |
| 17 | Loop terminates | `Block` (header) | `MustProgress` | `mustprogress` | none | reader | K |
| 18 | Stack lifetime | `Place` | `Lifetime{start, end}` | lifetime markers | none | carrier | K |
| 19 | Callee set; non-recursive | `Instruction`, `Function` | `Callees(set)`, `NoRecurse` | `!callees`, `norecurse` | none | carrier | K |
| 20 | Unique ownership | `Param`, `Place` | `Owned` | `noalias` | as row 3 | now | K |
| 21 | Floating-point freedom | `Instruction` | `Reassoc`, `NoNaNs`, `NoInfs`, `NoSignedZeros`, `AllowReciprocal` | fast-math flags `reassoc nnan ninf nsz arcp` | `M/transforms/instcombine.rs` (`float_simplified`, `arcp`, `reassoc`), `A/floatfacts.rs` (`nsz`) | now | K |

Rows 2, 5 and 6 replace `Instruction.pure`, `CallAbi.promises` and `RuntimePromises`; rows 3, 4 and 20 replace `Promise`. Rows 18 and 19 are the ones whose carrier must be designed first; nothing lands for them until it is.

### What each frontend calls

`—` means the frontend cannot state it.

| # | C (from the recorded stream) | Quick BASIC | Nib | Cannot |
|---|---|---|---|---|
| 2 | call class `NORETURN`/`ABORTS` | — | panic routines `N$E*` | QB has no such routine; `ON ERROR` is an invoke |
| 3 | `restrict` | — | slice descriptors only. Not `&mut`: a callee may write a module `var` the caller lent (`bump(g)` compiles), so the language does not enforce exclusion | QB: the same variable may be passed twice (legal) |
| 4 | `const T *restrict` gives `ReadOnly` and `NoAlias`; `int a[static N]` gives `NonNull` and `Dereferenceable(N·size)` | BYREF: `Dereferenceable` | `&T`: `ReadOnly`, `NonNull`, `Dereferenceable`; `&mut`: the last two. Not `NoCapture`: a function may return its borrowed parameter | C: plain `const T *` promises nothing (a callee may cast `const` away) |
| 5 | call class `NO_MEMORY_*` | runtime contracts (`reads_arguments`); SUB/FUNCTION by the one inference pass | `fn` with no `&mut`, by the same pass | — |
| 6 | — | runtime contracts (`raises_error`) | — | C has no such notion |
| 7 | signed arithmetic, pointer arithmetic | `FOR` counter | range-loop counter | C/Nib unsigned, QB `+` (wrap is defined) |
| 8 | pointer arithmetic, array indexing | arrays | array indexing | — |
| 9 | `_Bool`, `unsigned char` loads | boolean −1/0; runtime result ranges | `bool`, `char`, enum tag | C enums (a variable may hold any value of its underlying type); full-width integers |
| 10 | `assert`, if the library is known | after a `-fsanitize` check | after an emitted bounds or divide check | C without `assert` |
| 11 | C type | one class per array allocation | struct type | — |
| 12 | `const` objects, string literals | statement table | literals, `let` bindings | — |
| 13 | locals (C does not zero) | — | every binding has an initialiser; measured: DSE already removes the entry zeroing (no change on two probes) | QB: the language zeroes |
| 14 | `__unaligned` (1), `BEDefType` alignment | data `align` 2 | structs ≤ 2 | — |
| 15 | `__inline`, `#pragma unroll`, `inline_depth` | — | — | QB, Nib: no syntax |
| 16 | `noreturn` paths | `ON ERROR` paths | panic paths | — |
| 17 | — | `FOR` | range loop | C: a loop may not terminate |
| 18 | block scope | — | scope | QB: locals live for the procedure |
| 19 | address-taken functions | — | — | QB, Nib: no function pointers surveyed |
| 20 | — | — | owned aggregates | C, QB |
| 21 | `-on` (in `INIT sw`) | alternate math | — | Nib |

The mechanism and the IR-proper items need no row: a frontend calls `state` for facts and sets the IR field for meaning. Rows that cannot be stated through the mechanism today:

- **Rows 10 and 18** have no MIR carrier (`assume`, lifetime markers). The HIR call is defined; the row waits for the carrier.
- **Rows 9, 15, 16, 17, 19 and 21** have carriers or none, and no reader that uses the stated fact. The fact is stated once; the reader is the work.
- **Rows 3 and 4 for C** need the patched front end to emit the `FACT` record (the `const` qualifier and `static N` are in its type; `restrict` is already sent as `CGAttr n 3`).

### Matrix: rich metadata by frontend

✔ reaches MIR or the back end. ◐ the frontend knows it and it is not emitted or is lost on the way. ✘ the language cannot state it. — not applicable. Open Watcom is the reference: what its C front end sends its own back end.

| Fact | Open Watcom C | `llrm-c` | Quick BASIC | Nib |
|---|---|---|---|---|
| Ordered access (volatile / `published`) | ✔ `CGVolatile`, `FE_VOLATILE` | ✔ `volatile` | ◐ `published` in HIR, dropped before MIR (hang) | ◐ not emitted for foreign memory |
| `noreturn` / `aborts` | ✔ call class | ◐ in the stream, dropped | ✘ | ◐ panics end in `unreachable`; callee not marked |
| No memory read / written (pure) | ✔ `#pragma aux nomemory` only | ◐ in the stream, dropped | ✔ runtime routines; ◐ user SUBs | ◐ `fn` without `&mut`, not emitted |
| Raises no error (`nounwind`) | — | — | ✔ runtime contracts | — |
| Distinct object (`noalias`) | ✘ (`restrict` parsed, discarded) | ✔ `restrict` via our patch | ✘ same variable may pass twice | ◐ `&mut` excludes aliases, not emitted |
| Read-only pointee (`readonly`) | ✘ type bit only | ◐ never asked for | ✘ | ◐ `&T`, not emitted |
| Whole object addressable (`dereferenceable`) | ✘ | ◐ bytes 0 | ✔ BYREF | ◐ slices only |
| Not kept (`nocapture`) | ✘ | ◐ passes derive it | ◐ passes derive it | ◐ borrows, not emitted |
| Non-null | ✘ | ◐ | ✘ | ◐ no `null` in safe code |
| No signed wrap (`nsw`) | ◐ signedness in type | ✔ | ✔ FOR counter only | ◐ range loop, not emitted |
| Pointer stays in object (`inbounds`) | ✘ | ✔ | ✔ arrays | ◐ array indexing only |
| Value range (`range`) | ✘ (source type only) | ◐ `_Bool`, `unsigned char`, enum | ◐ boolean −1/0 | ◐ `bool`, `char`, enum tag |
| Read-only data | ✔ `FE_CONSTANT`, ROM segment | ✔ `constant` | ✔ statement table | ✔ literals |
| Type-based alias classes | ✘ | ✔ by C type | ◐ one shared tag for all arrays | ✘ none |
| Known initialised (no entry zeroing) | — | ✔ | ✘ language zeroes | ◐ default zeroes anyway |
| Alignment | ✔ `BEDefType`, `CG_SYM_UNALIGNED` | ◐ `__unaligned` dropped | ✔ data `align` 2 | ✘ |
| Unroll / inline hints | ✔ unroll count | ◐ never asked for | — | — |
| Trip count facts (bounds evaluated once) | ✘ back end derives | ◐ | ◐ needs `nsw` on the add | ◐ needs `nsw` on the add |
| Cold paths | ✘ | ✘ | ✘ | ◐ `unreachable` blocks only |
| Returns twice (`setjmp`) | ✔ `SETJMP_KLUGE`, RISC scheduler only | ✘ | — | — |

### Order of work

1. Defects first, each with a fail-first test: Quick BASIC sets `volatile` where it set `published`; the `nowrap` and promise gaps in Nib.
2. The mechanism with the facts that already have a carrier and a reader (rows 2 to 8, 11 to 14, 20): `facts::Builder`, the table macro, `lower`, `llrm_mir::facts`, the merge and speculation helpers, checked mode. Move `Promise`, `nowrap`, `inbounds`, `pure`, `cold` onto it one at a time.
3. Replace `CGAttr n 3` with the `FACT` record and `ow_facts`; decode every call-class bit through it.
4. Readers for rows 9, 15, 16, 17, 21, then their variants. Carriers for rows 10, 18, 19 when a reader is ready.

### Status

Implemented in #119 (branch `alim/feat/hir-facts`), against `origin/main` at `fd3dcfd9`.

| Row | Fact | State | Measured |
|---|---|---|---|
| 3 | `NoAlias` | landed; C `restrict` via `CGFact v1`, Nib slice descriptors | 114 C, QB and Nib fixtures compile to the same assembly |
| 4 | `ReadOnly`, `NonNull`, `Dereferenceable` | landed; `Promise` removed; Nib `&T`, `&mut T` | Nib `&T` loop 17 → 15 instructions (no frame register saved) |
| 2 | `NoReturn` | landed; C call class | C `die`/`quit` probe 11 → 8 instructions |
| 5 | `Memory` | landed; C `nomemory` | no change: a loop with a `nomemory` call and a global trades one memory operand for another under two free registers |
| 7 | `NoSignedWrap`, `NoUnsignedWrap` | landed; C, Quick BASIC `FOR`, Nib range loops | no change on 36 Nib fixtures and two probes |
| 13 | `NoEntryZeroing` | not built | the entry zeroing of Nib locals is already removed by dead-store elimination |
| 8 | `InBounds` | landed on `Subject::Operand` (a place) and `Subject::Instruction` (`ptr_offset`); `inbounds` fields removed; C, Quick BASIC, Nib | 112 programs and 3 demos compile to identical assembly |
| 14 | `Align` | landed on `Subject::Object`; `DataObject.align` removed | same |
| (call) | `NoCapture`, `WriteOnly`, `Initializes` | landed on `Subject::Operand` of a call; `CallAbi.promises` removed; Quick BASIC fills | same |
| 9, 10 | `Range`, `Align` of an instruction, `NoAlias` of a result | landed as carriers: a pair on the wire, `range` attribute or `!range`, access `align`; readers by their owners | none yet: nothing states them |
| (rewrite) | merge and speculate policy | `Fact::merged`, `Facts::merged`, `Fact::survives_speculation`: one exhaustive `match` each, in `facts_rewrite.rs`; no pass merges across a promise (flags are in every value-numbering key) and every site on the compile route that moves an instruction was audited (listed in the module) | nothing calls them yet |
| 11, 12, 20 | `AliasClass`, `Immutable`, `Owned` | not facts: `alias_classes` is a module table each access refers to, as LLVM's TBAA nodes are; `zeroed_locals` is Quick BASIC's meaning, not a droppable promise | |
| 6 | `NoUnwind`, `WillReturn` | `RuntimePromises` stays a named table with one owner: Quick BASIC derives it from the runtime description (`nounwind`, writers, user-code entries); C's `reads_arguments` is the C library's alone | |
| 9, 10, 15 to 19, 21 | `Range`, `Assume`, hints, `Cold`, `MustProgress`, lifetimes, `Callees`, float flags | no variant: none has a program that shows a win, and a variant lands with its reader and a measured win | |

Checked mode covers `noalias` only. Operand dominance in the verifier waits for the first relational fact.
