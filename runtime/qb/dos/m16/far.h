/* Memory outside DGROUP, addressed by paragraph (the real-mode segment). */
#ifndef QB_FAR_H
#define QB_FAR_H

#include "qb.h"

typedef u16 __far *FarWords;

/* The segment DGROUP is in. */
u16 dgroup_segment(void);
FarWords far_words(u16 segment);
/* Zero `paras` paragraphs from `segment`. */
void far_clear(u16 segment, u16 paras);
/* Zero `bytes` from `segment`:`offset`. */
void far_clear_bytes(u16 segment, u16 offset, unsigned long bytes);
/* Copy `paras` paragraphs between segments that may overlap. */
void far_move(u16 from, u16 to, u16 paras);

#endif
