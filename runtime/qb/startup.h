/* What the portable startup asks of the target (runtime/qb/<os>/<mode>). */
#ifndef QB_STARTUP_H
#define QB_STARTUP_H

#include "qb.h"

/* Runs each initializer the linked modules registered. */
void qb_run_initializers(void);

/* The room the string space and the local heap share. */
void qb_dynamic_region(char **first, char **top);

#endif
