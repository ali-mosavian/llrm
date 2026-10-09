/* Memory outside DGROUP, addressed by paragraph (the real-mode segment). */
#ifndef QB_FAR_H
#define QB_FAR_H

#include "qb.h"

typedef word __far *FarWords;

/* The segment DGROUP is in. */
word dgroup_segment(void);
FarWords far_words(word segment);
/* Zero `paras` paragraphs from `segment`. */
void far_clear(word segment, word paras);
/* Zero `bytes` from `segment`:`offset`. */
void far_clear_bytes(word segment, word offset, unsigned long bytes);
/* Copy `paras` paragraphs between segments that may overlap. */
void far_move(word from, word to, word paras);

#endif
