# Open Watcom C front end: what it gathers and emits

Source: Open Watcom v2 commit `703e1ae2f`, the commit `toolchain/owshim/build.sh` builds. Paths are relative to `bld/`; `cc/c/` is the front end and `cg/` the code generator. Every cite is `file:function:line`, read from that tree. A row marked *not traced* was read only as far as it says.

Aim: the facts Open Watcom's front end gives its back end, so that every llrm frontend can state the same facts in HIR (rule 7).

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

