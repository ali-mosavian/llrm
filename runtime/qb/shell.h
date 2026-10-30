/* What SHELL asks of the target: the memory a child program needs, and getting it back. */
#ifndef QB_SHELL_H
#define QB_SHELL_H

/* Frees the program's memory above what it holds; the program's arrays stay. */
void qb_shell_release(void);
/* Takes the memory back after the child. */
void qb_shell_reclaim(void);

#endif
