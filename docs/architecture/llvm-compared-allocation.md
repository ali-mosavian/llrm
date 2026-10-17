# Allocation and instruction layout against LLVM

TL;DR: compiling `d_faces` at `-O2`, llrm makes 3,765,923 allocations (638 MB); clang's `cc1` makes 62,881 (38 MB), 60x fewer. Peak live heap is 23 MB against 5.5 MB; peak resident set is 8 to 18 KB per IR instruction against 0.3 to 0.7 KB. 24% of llrm's L1d misses are allocation and copying.

## LLVM

An instruction is one allocation: the operands (`Use`) sit in the same block, before the object, so building an instruction costs one `operator new` whatever its operand count.

[`User.cpp#L137-L168`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/User.cpp#L137-L168)

```cpp
void *User::allocateFixedOperandUser(size_t Size, unsigned Us,
                                     unsigned DescBytes) {
  assert(Us < (1u << NumUserOperandsBits) && "Too many operands");

  static_assert(sizeof(DescriptorInfo) % sizeof(void *) == 0, "Required below");

  unsigned DescBytesToAllocate =
      DescBytes == 0 ? 0 : (DescBytes + sizeof(DescriptorInfo));
  assert(DescBytesToAllocate % sizeof(void *) == 0 &&
         "We need this to satisfy alignment constraints for Uses");

  size_t LeadingSize = DescBytesToAllocate + sizeof(Use) * Us;

  // Ensure we allocate at least one pointer's worth of space before the main
  // user allocation. We use this memory to pass information from the destructor
  // to the deletion operator, so it can recover the true allocation start.
  LeadingSize = std::max(LeadingSize, sizeof(void *));

  uint8_t *Storage = static_cast<uint8_t *>(::operator new(LeadingSize + Size));
  User *Obj = reinterpret_cast<User *>(Storage + LeadingSize);
  Use *Operands = reinterpret_cast<Use *>(Obj) - Us;
  Obj->NumUserOperands = Us;
  Obj->HasHungOffUses = false;
  Obj->HasDescriptor = DescBytes != 0;

  if (DescBytes != 0) {
    auto *DescInfo = reinterpret_cast<DescriptorInfo *>(Operands) - 1;
    DescInfo->SizeInBytes = DescBytes;
  }

  return Obj;
}
```

A terminator's successors are a range over its operands; nothing is allocated to ask for them ([`Instruction.h#L1029-L1035`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/Instruction.h#L1029-L1035), [`Instruction.cpp#L1462-L1472`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L1462-L1472)).

## llrm

An instruction owns two `Vec`s, and `successors` returns a fresh `Vec` per call, with a linear duplicate check.

[`module.rs#L70-L79`](https://github.com/ali-mosavian/llrm/blob/21f5c9fc8e67b5bf405cf9ecdaf65247a26c0450/crates/ir/llrm-mir/src/module.rs#L70-L79)

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Instruction {
    pub opcode: Opcode,
    /// The result's type; `void` when there is none.
    pub ty: TypeId,
    pub operands: Vec<Operand>,
    pub flags: Flags,
    pub result: Option<ValueId>,
    pub metadata: Vec<(String, MetadataId)>,
}
```

[`module.rs#L548-L565`](https://github.com/ali-mosavian/llrm/blob/21f5c9fc8e67b5bf405cf9ecdaf65247a26c0450/crates/ir/llrm-mir/src/module.rs#L548-L565)

```rust
    pub fn successors(
        &self,
        block: BlockId,
    ) -> crate::blocklist::BlockList {
        let operands = self.terminator(block).map(|one| self.instruction(one).operands.as_slice()).unwrap_or_default();
        if operands.len() > SCAN {
            return Self::listed_once(operands);
        }
        let mut out = crate::blocklist::BlockList::new();
        for operand in operands {
            if let Operand::Block(target) = operand
                && !out.contains(target)
            {
                out.push(*target);
            }
        }
        out
    }
```

## Divergence

The instruction itself is not where DHAT finds the allocations (`Instruction::clone` is 86,673 of 3.77 M); they are in the tables built beside the IR, one `Vec` per block, per value or per query, rebuilt after every pass that changes the body. `Function::successors` alone makes 258,958 allocations (6.9%), for `cfg::graph` (109 k), `DominatorTree::new` (34 k), `LoopInfo::new` (31 k) and `cfg::natural` (30 k). Allocation inside CFG, loop and dominator builders is 14% of all allocations, memory SSA 10.7% (126 MB, 20% of the bytes), alias facts 8.5% (73 MB), register allocation 11.1%.

## Measured

`d_faces.c` of qbq-llrm, `-O2 -m32 -march=i486 -fno-inline-functions`. llrm-c built `--profile dist` from main `21f5c9fc8` and, for DHAT, with the global allocator removed (`valgrind --tool=dhat`; 3.26 G instructions against 2.8 G with mimalloc). `cc1` is the driver's own command line under DHAT.

| | clang 24 `cc1` | llrm-c |
|---|---:|---:|
| allocations | 62,881 | 3,765,923 |
| bytes allocated | 38.4 MB | 638 MB |
| mean block | 611 B | 170 B |
| peak live (`t-gmax`) | 5.5 MB in 7,843 blocks | 23.3 MB in 106,021 blocks |
| blocks living under 1,000 instructions | 12% | 36% |
| blocks of 16 bytes or less | 9% | 36% |
| peak resident set (`/usr/bin/time`) | 63 MB | 85 MB |

Resident set above an empty input, per pre-optimization IR instruction (`tools/ladder/rss.py`, clang end to end against `llrm-c`):

| shape, N | IR instructions | clang | llrm-c |
|---|---:|---:|---:|
| mem 100 | 1,205 | 697 B | 17,890 B |
| mem 1600 | 19,205 | 302 B | 16,192 B |
| arith 1600 | 14,467 | 445 B | 7,836 B |
| diamond 1600 | 6,413 | 941 B | 15,108 B |

Allocations by owner in llrm-c (first matching frame of the DHAT stack, so each block counts once):

| owner | allocations | share | MB |
|---|---:|---:|---:|
| CFG, loop and dominator builders | 523,311 | 13.9% | 31 |
| register allocation | 419,531 | 11.1% | 72 |
| memory SSA | 403,671 | 10.7% | 126 |
| alias facts | 320,130 | 8.5% | 73 |
| peephole | 187,922 | 5.0% | 22 |
| `gvn` | 172,936 | 4.6% | 27 |
| `fold` and `consts` | 102,622 | 2.7% | 27 |
| instruction selection | 100,049 | 2.7% | 18 |

## Proposed change

LLVM does not make small allocations. It keeps the few elements inline ([`SmallVector`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/SmallVector.h#L1251), storage at [`#L1166`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/SmallVector.h#L1166)), puts an instruction's operands in the instruction's own block ([`User.cpp#L137`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/User.cpp#L137)), and gives per-compile tables a bump arena ([`BumpPtrAllocatorImpl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/Allocator.h#L68)). In llrm 2,518,817 allocations (67%) are 64 bytes or less, and 28% of all allocations are that small and gone within 1,000 instructions. The changes in that shape, largest first:

1. Inline operands. [`Instruction::operands: Vec<Operand>`](https://github.com/ali-mosavian/llrm/blob/21f5c9fc8e67b5bf405cf9ecdaf65247a26c0450/crates/ir/llrm-mir/src/module.rs#L75) becomes a small vector with 3 inline slots (`SmallVec<[Operand; 3]>`, LLVM's `SmallVector<Value *, 3>` is the same choice). `Instruction::clone` makes 86,673 allocations, and every instruction built makes one. Needs a `smallvec` dependency (not in `Cargo.lock`) or a local inline type. Saving: unmeasured; operand counts per instruction are not measured either.
2. Successor and predecessor lists without a heap block: `successors` as an iterator (258,958 allocations), `predecessors` (44,655) as an iterator or a 2-slot inline list. No dependency for the iterator. [`module.rs#L548`](https://github.com/ali-mosavian/llrm/blob/21f5c9fc8e67b5bf405cf9ecdaf65247a26c0450/crates/ir/llrm-mir/src/module.rs#L548).
3. A per-pass scratch arena for tables that die within the pass: `Vec<i64>` collects 143,858, `forest::forest` 82,268, `DominatorTree::new` 70,084, `alias::escapes` 56,507, `alias::point_values` 32,607: 385,324 allocations (10%). Either `bumpalo::Bump` reset after each pass (new dependency) or a `Scratch` struct held by `Analyses` whose vectors are cleared and reused, which is how LLVM's passes keep member `SmallVector`s. Saving: unmeasured.
4. Presize the 4-byte-id vectors: 515,831 allocations are one `RawVec::grow_one` shared by every `Vec` of `BlockId`, `ValueId`, `InstId` and `TypeId`, and `finish_grow` is 49.9 M self instructions. `Vec::with_capacity(known length)` where the length is known (`cfg::graph`, `LoopInfo::new`, `forest::forest`). Saving: unmeasured; bound 49.9 M plus the copies after a `realloc` (2.0 M of 3.9 M D1 misses).
5. Build the CFG tables once per body version (`cfg::graph` 108,796, `DominatorTree::new` 34,406, `LoopInfo::new` 30,876, `cfg::natural` 30,346; 523,311 in CFG builders, 13.9%): share the successor lists of `cfg::Shape`. Saving: unmeasured.
6. Memory-SSA and alias tables dense: `memoryssa::structure_of` 36,340 blocks, 28.9 MB; see lazy memory facts in the proposals.

Dependencies are a decision for the owner; changes 2, 4, 5 and 6 need none.

Ranked list: [proposals](llvm-compared-proposals.md).

Open: what holds 8 to 18 KB per IR instruction, and whether `successors` as an iterator saves the 259 k allocations and their misses ([#1994](https://github.com/ali-mosavian/llrm/issues/1994)).
