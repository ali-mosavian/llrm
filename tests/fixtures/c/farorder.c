/* Borland orders far pointers by their offset words alone; == and !=
   compare all 32 bits (bcc -S). */
int below(char far *a, char far *b) { return a < b; }
int same(char far *a, char far *b) { return a == b; }
