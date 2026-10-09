/* Error state shared by the modules that end or resume a program. */
#ifndef QB_ERROR_H
#define QB_ERROR_H

#include "qb.h"

extern word b_errnum;
extern word b_inonerr;
void qb_no_resume(void);

#endif
