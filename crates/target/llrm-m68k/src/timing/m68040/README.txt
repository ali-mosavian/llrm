MC68040 USER'S MANUAL, SECTION 10 INSTRUCTION TIMING TABLES
Source: /home/alim/scratch/fetch-m68000prm/MC68040UM.pdf, printed pages 10-1 to 10-37.

FILES (title, printed page, data rows)
t10-2.csv: Table 10-2. Number of Memory Accesses | page 10-3 | rows 17
t10-3.csv: Table 10-3. CINV Timing | page 10-8 | rows 3
t10-4.csv: Table 10-4. CPUSH Best and Worst Case Timing | page 10-8 | rows 2
t10.1.csv: Table 10.1. Instruction Timing Index | page 10-1, 10-2 | rows 47
t10.4.csv: 10.4 MOVE INSTRUCTION TIMING | page 10-9, 10-10 | rows 15
t10.5.csv: 10.5 MISCELLANEOUS INTEGER UNIT INSTRUCTION TIMINGS | page 10-11, 10-12 | rows 75
t10.6-1.csv: 10.6 INTEGER UNIT INSTRUCTION TIMINGS: ADD, AND, EOR, OR, SUB, TST | ADDA | ADDI, ANDI, EORI, ORI, SUBI | page 10-13 | rows 17
t10.6-10.csv: 10.6 INTEGER UNIT INSTRUCTION TIMINGS: MOVE to CCR | MOVE from SR [a] | MOVE to SR [b] | page 10-22 | rows 17
t10.6-11.csv: 10.6 INTEGER UNIT INSTRUCTION TIMINGS: MOVEA.L [a] | MOVEM <list>,<ea> [b,c] | MOVEM.L <ea>,<list> [b,c] | page 10-23 | rows 17
t10.6-12.csv: 10.6 INTEGER UNIT INSTRUCTION TIMINGS: MOVES <ea>,An* | MOVES <ea>,Dn* | MOVES Rn,<ea>* | page 10-24 | rows 17
t10.6-13.csv: 10.6 INTEGER UNIT INSTRUCTION TIMINGS: MULS.W/L* | MULU.W/L * | NBCD | page 10-25 | rows 17
t10.6-14.csv: 10.6 INTEGER UNIT INSTRUCTION TIMINGS: NEG, NEGX, NOT | PEA | ROL, ROR | page 10-26 | rows 17
t10.6-15.csv: 10.6 INTEGER UNIT INSTRUCTION TIMINGS: ROXL, ROXR | Scc | SUBA | page 10-27 | rows 17
t10.6-16.csv: 10.6 INTEGER UNIT INSTRUCTION TIMINGS: TAS * | page 10-28 | rows 17
t10.6-2.csv: 10.6 INTEGER UNIT INSTRUCTION TIMINGS: ADDQ, SUBQ | ASL | ASR, LSL, LSR | page 10-14 | rows 17
t10.6-3.csv: 10.6 INTEGER UNIT INSTRUCTION TIMINGS: BCHG, BCLR, BSET [a] | BFCHG, BFCLR, BFSET [b,c] | BFEXTS, BFEXTU [b,d] | page 10-15 | rows 17
t10.6-4.csv: 10.6 INTEGER UNIT INSTRUCTION TIMINGS: BFFFO [a,b] | BFINS [a,c] | BFTST [a] | page 10-16 | rows 17
t10.6-5.csv: 10.6 INTEGER UNIT INSTRUCTION TIMINGS: BTST | CAS [b] | CHK [c,d] (<ea>, Dn) | page 10-17 | rows 17
t10.6-6.csv: 10.6 INTEGER UNIT INSTRUCTION TIMINGS: CHK2* (<ea>, Rn) | CLR | CMP | page 10-18 | rows 17
t10.6-7.csv: 10.6 INTEGER UNIT INSTRUCTION TIMINGS: CMPA.L | CMPI | CMP2* | page 10-19 | rows 17
t10.6-8.csv: 10.6 INTEGER UNIT INSTRUCTION TIMINGS: DIVS.W, DIVU.W* | DIVS.L, DIVU.L, DIVSL.L, DIVUL.L* | JMP | page 10-20 | rows 17
t10.6-9.csv: 10.6 INTEGER UNIT INSTRUCTION TIMINGS: JSR | LEA | MOVE from CCR | page 10-21 | rows 17
t10.7.1.csv: 10.7.1 Miscellaneous Integer Unit Support Timings | page 10-29 | rows 4
t10.7.2-1.csv: 10.7.2 FABS, FADD, FCMP, FDIV, FMOVE, FMUL, FNEG, FSQRT, FSUB, FTST <ea>,FPn* | page 10-30 | rows 17
t10.7.2-2.csv: 10.7.2 FMOVE FPn,<ea> * | page 10-31 | rows 17
t10.7.2-3.csv: 10.7.2 FMOVE/FMOVEM to/from Control Register; FMOVEM <list>,<ea> and <ea>,<list>; FScc | page 10-32 | rows 17
t10.7.2-4.csv: 10.7.2 FSAVE <ea>* | page 10-33 | rows 18
t10.7.2-5.csv: 10.7.2 FRESTORE <ea>* | page 10-34 | rows 18
t10.7.3.csv: 10.7.3 Timings in the Floating-Point Unit | page 10-35, 10-36, 10-37 | rows 91

FILE FORMAT
Row 1: #title,<title>,page,<printed page>,notes,<footnote text>. Footnote texts of one table are joined with spaces.
Row 2: column header. Rows 3 on: data. Python csv quoting, UTF-8. Blank cell = blank on the page.

NAMES
t10.1, t10-2, t10-3, t10-4 keep the printed table numbers (Table 10.1 is printed with a dot).
Tables with no printed number are named by their printed heading: t10.4 (10.4 MOVE), t10.5 (10.5), t10.7.1 (10.7.1), t10.7.3 (10.7.3, pages 10-35 to 10-37).
Where one heading holds several tables, they are numbered in page order: t10.6-1 to t10.6-16 (one per page of 10.6), t10.7.2-1 to t10.7.2-5 (one per printed table on pages 10-30 to 10-34).
t10.4 is one table: its four column blocks (pages 10-9 and 10-10) share the 15 source rows and are placed side by side.

CELL CONVENTIONS
Superscript footnote letters are shown in brackets after the text: 'BSET [a]', '1/2 [a]', 'CHK [c,d]'. Printed asterisks are kept: 'CMP2*', '3/4 *'.
Expressions are normalized to 'nL + m' (for example '1L + 6'). The text layer prints the same expression with varying spacing; the page image shows the same value.
'(d16,An)' is written '(d 16,An)' to match the row labels. Dashes printed as em dash '—' and en dash '–' are kept.
Two-level headers are flattened to '<group> <ea> Calculate' and '<group> Execute'.
Merged label cells appear in their first row only. 10.7.3 'FMOVE,/FABS,/FNEG' is one merged cell, written 'FMOVE,\nFABS,\nFNEG' on its first row. Table 10-4 'CPUSHP/CPUSHA' is one cell containing both names.
Table 10-4: the printed 'Execution Time' super-header is dropped; the columns are headed Best Case and Worst Case.
TAS (10.6, page 10-28): the page prints three column pairs, but only the first is filled. The two empty pairs are omitted.
10.7.1 (page 10-29): merged two-line cells are kept as two lines, for example 'Taken' / 'Not Taken'.
The 10.7.3 FDIV first row has a blank Size cell on the page; it is left blank.
Printed typo kept: 'MOEA.W' in the MOVEA.L note (page 10-23).
Page furniture (headers, footers, 'Freescale Semiconductor, Inc.') is removed.

READABILITY
Unreadable cells: none.
All cells come from the text layer, parsed by line range and checked for column count (every row matches its header).
Compared visually with page images: 10-4 (prose for definitions), 10-8 (Tables 10-3 and 10-4), 10-17 (10.6 BTST/CAS/CHK), 10-9 (10.4 blocks 1 and 2 only), 10-19 (CMPA/CMPI/CMP2), 10-23 (MOVEA/MOVEM), 10-28 (TAS), 10-29 (10.7.1), 10-32 (FMOVE/FMOVEM/FScc), 10-35 to 10-37 (10.7.3).
Not compared with page images: 10-10 (10.4 blocks 3 and 4), 10-11 and 10-12 (10.5), the other 10.6 pages, 10-30, 10-31, 10-33, 10-34 (10.7.2), and Tables 10.1 and 10-2.

NOT TRANSCRIBED
Figures 10-1, 10-2, 10-3 and the Section 10.2 examples are pipeline diagrams, not tables.

DEFINITIONS
definitions.txt holds the page text that defines the columns, the lead/base time, the stall rule, the 'L' and 'N L' notation, the pipeline overlap statements, and the assumptions (suppositions 1 to 4).
