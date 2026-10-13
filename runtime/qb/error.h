/* Error state shared by the modules that end or resume a program. */
#ifndef QB_ERROR_H
#define QB_ERROR_H

#include "qb.h"

extern unsigned b_errnum;
extern unsigned b_inonerr;
void qb_no_resume(void);
/* END: the files are closed and the program leaves. */
void qb_end(void);
/* The message of an error number. */
/* The text of error `n` copied to `out` (QB_ERROR_TEXT_MAX + 1 bytes), and its length: the table is far, not in DGROUP. */
enum { QB_ERROR_TEXT_MAX = 39 };
unsigned qb_error_text(unsigned n, char *out);

#endif
