Nib frontend stage dumps
========================

00-input.nib       exact source presented to the frontend
01-tokens.txt      lexer output with source positions
02-syntax.txt      indentation-aware syntax tree
03-hir.json        verified, source-neutral common HIR
mir/               the MIR after each pipeline pass, listing.asm and cost:
                   what -S and -o compile
