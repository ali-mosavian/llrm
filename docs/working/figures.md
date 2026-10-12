# Figures and the docs site

For contributors who add a diagram to a document or build the site. Figures are
written as short text specs in a `fig` fence; the site build renders each one to an
inline SVG that follows the reader's light or dark setting. Nothing is rendered in
the repository.

## Writing a figure

A figure is a fenced block whose language is `fig`, in a Markdown file or in a
Rust `//!` header (the [pass reference](../optimizations/readme.md) is built from
those headers). The first line names the kind. Lines starting with `#` are comments.

```text
kind: graph | stack | lanes
title: caption under the figure, also the accessible title
```

A quoted string takes `\n` for a line break. A spec that does not parse fails
the build with the line of the figure that is wrong.

### graph

Boxes and arrows, laid out in layers. Back edges become arcs around the drawing.

| Line | Meaning |
|---|---|
| `dir: down` or `dir: right` | Direction of flow, default `down` |
| `node ID "label" [fill=a..e] [shape=box\|round\|note]` | A node |
| `edge A -> B "label"` | An edge; `..>` is dashed, `==>` is bold; the label is optional |
| `group NAME "caption": ID ID ...` | A labelled container around the nodes |

The fills `a` to `e` are a fixed categorical palette; without `fill` a node is neutral.

```fig
kind: graph
dir: down
title: From source to object file
node qb "QuickBASIC\nllrm-qb" fill=a
node nib "Nib\nllrm-nib" fill=a
node c "C\nllrm-c" fill=a
node hir "HIR\nllrm-hir"
node mir "MIR\nllrm-mir"
node opt "Optimizer\nllrm-transforms" fill=b
node sel "Instruction selection\nLIR over virtual values"
node ph "Machine phases\nregister allocation, peephole"
node obj "Object writer\nOMF, ELF, COFF, Mach-O" fill=c
edge qb -> hir
edge nib -> hir
edge c -> hir
edge hir -> mir "mir::emit"
edge mir -> opt
edge opt -> sel "lowering boundary"
edge sel -> ph
edge ph -> obj
group front "Source front ends": qb nib c
group back "Backend": sel ph obj
```

### stack

A memory, stack or record layout, top to bottom as written.

| Line | Meaning |
|---|---|
| `cell "label" [off="bp+4"] [size="2"] [note="text"] [fill=a..e]` | A cell; the offset is on the left, the size inside the cell at its right, the note on the right |
| `gap "..."` | A cell that stands for omitted cells |
| `arrow "label" up\|down` | A growth direction |
| `bracket "label" from=N to=M` | Spans cells N to M, counting `cell` and `gap` lines from 1 |
| `scale: yes` | Cell heights follow `size` (8 pixels a unit, at most 160) |

```fig
kind: stack
title: Frame of a QuickBASIC procedure (far call, Pascal order)
arrow "higher addresses" up
cell "formal 1" off="bp+8" size="2"
cell "formal 2" off="bp+6" size="2" note="last formal: nearest the return address"
cell "return address" off="bp+2" size="4" note="far: offset, then segment" fill=a
cell "runtime header" off="bp" size="10" note="saved BP, frame chain, SI, DI (QB 4.5)" fill=b
cell "locals and spills" note="SP is at the bottom"
gap "..."
arrow "stack grows down" down
bracket "pushed by the caller" from=1 to=3
bracket "B$ENRA" from=4 to=5
```

### lanes

A timeline of lanes over numbered columns, or a call and return sequence.

| Line | Meaning |
|---|---|
| `lane ID "label"` | A lane, top to bottom in the order written |
| `tick N "label"` | Column N, with its label under the axis |
| `span LANE FROM TO "label" [kind=shared\|mut\|owner\|info]` | A bar over columns FROM to TO |
| `mark LANE N "label" [kind=error\|ok]` | A point in column N |
| `msg FROMLANE TOLANE N "label" [kind=call\|ret]` | An arrow between two lanes in column N; `ret` is dashed |

```fig
kind: lanes
title: A change while a shared borrow is still used
lane v "v (owner)"
lane r "r = &v[0]"
tick 1 "let mut v"
tick 2 "let r = &v[0]"
tick 3 "v.push(3)"
tick 4 "print(r)"
span v 1 4 "owner" kind=owner
span r 2 4 "shared borrow, live to its last use" kind=shared
mark v 3 "refused: r is used later" kind=error
```

## Building the site

```sh
python3 tools/docsbuild/build.py --out site
```

writes the site to `site/`; open `site/index.html`. `--check` builds to a temporary
directory and fails on a figure that does not parse, a link that does not resolve
in the site, or a Markdown construct the converter does not know. Links to files
outside `docs/` become links to the repository on GitHub.

The converter's tests run with `uv run --project tools pytest tools/docsbuild`.

## Publishing

The `docs` workflow builds the site on every change to the documents, the build
tools or the crate sources, and keeps it as an artifact. The deploy job runs on
`main` only when the repository variable `DOCS_PAGES` is `true`; otherwise it
is skipped and the run summary says so. To publish, set Pages source to GitHub
Actions (Settings, Pages) and add the variable `DOCS_PAGES` with the value `true`
(Settings, Secrets and variables, Actions, Variables).
