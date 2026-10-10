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
const char *qb_error_text(unsigned n);

#endif
