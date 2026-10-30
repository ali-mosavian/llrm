> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopclone.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopclone.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopclone.rs sha256:4d0bdd1ad4f6b0937b4eb98b617b787f04eccd1f9f598dcf3ef5c34f59406910 -->

# loopclone

Clones loop iterations as CFG, for the passes that peel a loop: `peeled`
copies whole iterations ahead of it.

## How it works
This is the mechanism for peeling; it makes no profitability decision. It
returns each copy's map from an original block to its clone, or `None`
having changed nothing when the loop is not one it can clone. It needs
- a header that is not the entry, one latch that ends in a jump to it, and
  one outside predecessor that falls into the header (a preheader);
- no block but the header entered from outside the loop;
- only `br` and `switch` as multi-way terminators in the body;
- loop live-outs in LCSSA: no instruction outside the loop reads a value the
  loop defines, and no phi outside takes one from outside the loop.

`peeled(function, loop, count)` clones the loop's blocks `count` times, in
order, and puts them between the preheader and the loop:

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart LR
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  pre["<b>pre</b>"]:::mid
  h["<b>H</b>"]:::mid
  body["<b>body</b>"]:::mid
  ex["<b>exit</b>"]:::mid
  pre --> h
  h --> body
  body --> h
  h --> ex
```

*Before*

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart LR
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  pre["<b>pre</b>"]:::mid
  h1["<b>H1</b>"]:::mid
  b1["<b>body1</b>"]:::mid
  h2["<b>H2</b>"]:::mid
  b2["<b>body2</b>"]:::mid
  h["<b>H</b>"]:::mid
  body["<b>body</b>"]:::mid
  ex["<b>exit</b>"]:::mid
  pre --> h1
  h1 --> b1
  b1 --> h2
  h2 --> b2
  b2 --> h
  h --> body
  body --> h
  h1 --> ex
  h2 --> ex
  h --> ex
```

*After peeled(.., 2)*

The first copy's header phis take the preheader's arm, each later copy's
the previous copy's latch value, and the residual loop's header the last
copy's latch value. Every exit phi gains an arm for each copy's exit edge.
The residual loop stays so the result is right whatever the trip count;
the caller proves it unreachable where the count is exact.

## Example
`peeled(.., 1)` on a loop whose body is a diamond with an early exit.
Before:

```mir
define i16 @f(i16 %seed, i16 %n, i16 %limit) {
b0:
  br label %b1

b1:
  %carried = phi i16 [ %seed, %b0 ], [ %stepped, %b5 ]
  %go = icmp ult i16 %carried, %n
  br i1 %go, label %b2, label %b6
; ... b2 to b5: the diamond, leaving from b3 to b6 or going on to b5
b6:
  %answer = phi i16 [ %carried, %b1 ], [ %left, %b3 ]
  ret i16 %answer
}
```

After:

```mir
define i16 @f(i16 %seed, i16 %n, i16 %limit) {
b0:
  br label %0

b1:
  %carried = phi i16 [ %stepped, %b5 ], [ %13, %11 ]
  ; ...
b6:
  %answer = phi i16 [ %carried, %b1 ], [ %left, %b3 ], [ %1, %0 ], [ %7, %6 ]
  ret i16 %answer

0:
  %1 = phi i16 [ %seed, %b0 ]
  %2 = icmp ult i16 %1, %n
  br i1 %2, label %3, label %b6
; ... 3 to 9: the copy of b2 to b4
11:
  %12 = phi i16 [ %7, %6 ], [ %10, %9 ]
  %13 = add i16 %12, 1
  br label %b1
}
```

`b0` enters the copy `0`, whose last block `11` enters the residual loop at
`b1`; `b6` gains an arm for each of the copy's two exits.

From the fixture of unit test
`test_peeling_clones_diamond_and_early_exit_phis` in `loopclone_tests.rs`,
peeled once where the test peels twice.

## Runs
Called by [`crate::loopunrollpass`](loopunrollpass.md) (`peeled_any_latch`); not a pass.

## Needs
LCSSA ([`crate::lcssa`](lcssa.md)) and a simplified loop ([`crate::loopsimplify`](loopsimplify.md)).

## See also
LLVM's `peelLoop` (`LoopPeel.cpp`) and `UnrollLoop`.
