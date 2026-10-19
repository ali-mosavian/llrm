# Huge pointers

For contributors to isel and the loop passes. A huge pointer is a far pointer whose offset carries into its selector; MIR states it as a pointer in the target's huge address space and isel gives it its representation.

## Representation

MIR has `getelementptr` on a pointer of the huge space (`Spaces::huge`, from `datalayout.toml`). Nothing above isel knows selectors, registers or the stride. The stride is the platform's: `Machine::huge_shift` is 12 in real mode (64K is `1 << 12` paragraphs), a platform-stated value under protected addressing, and none on a flat target, where a huge pointer is refused ("a huge pointer on a machine that states no selector stride").

```
 MIR:   %q = getelementptr i16, ptr addrspace(huge) %p, i32 %i

 isel (carried):  total    = zext(offset(p)) + index*2 + constant   ; dword
                  offset'  = low16(total)
                  selector' = selector(p) + ((total >> 16) << huge_shift)
                  q = selector':offset'
```

Every huge pointer is made this way, so selector:offset is canonical within an object: comparison is lexicographic, and a difference is `((selector - selector') >> shift << 16) + (offset - offset')` (`pointer_difference`). Both temporary arithmetic and the packed result wrap at the target widths: `2000:fffe + 2` is `3000:0000`, and `2000:0004 - 8` is `1000:fffc`. A huge pointer is not an integer: casting it to one is refused, because its packed bits are not its address. Memory access through a huge pointer loads the selector into a segment register around the access; register allocation chooses every register.

Code: `carried`, `window` and `pointer_difference` in [`isel.rs`](../optimizations/backend/isel.md).

## Making the carry cheap

Built from a counter, each use pays the whole carry (`OperationCosts::carry`); stepped by a constant, the step pays a borrow spread to a mask, cut to the selector's stride and added to it (`carry_step`). So `lsr` walks a huge array with a huge pointer ([lsr](../optimizations/passes/lsr_pass.md)).

`window` ([`window.rs`](../optimizations/passes/window.md)) runs after `lsr`: a counted loop's huge recurrences are split into windows. Inside a window every recurrence is normalized (`llrm.ia16.window`) and the loop runs as many whole trips as the target's window holds (`Machine::huge_window`) on far pointers that never carry; an outer loop walks the windows. The split is priced: a window's setup against the carries it saves.

```
 for i in 0..n: a[i] = ...         for w in windows:
   (carry on every i)       ==>      p = normalize(base + w)
                                     for i in window: *p++ = ...   ; far, no carry
```

## BC objects

BC calls `B$HARY` for the element address of a huge (`/AH`) array and the raise keeps it a call ([bounds-checks](../reference/semantics/bounds-checks.md)); a program compiled from source takes the path above in C, BASIC and Nib alike. The runtime's selector stride is the library's `b$HugeShift`.
