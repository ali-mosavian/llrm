/* QB file-channel state, separate from the DOS operations that use it. */

#include "types.h"

#define QB_FILE_CHANNELS 256
#define QB_INVALID_HANDLE 0xffff

typedef struct {
    word handle;
    byte mode;
    byte open;
} file_channel;

static file_channel file_channels[QB_FILE_CHANNELS];

static file_channel *qb_file_channel(word channel) {
    if (channel == 0 || channel >= QB_FILE_CHANNELS) {
        return (file_channel *)0;
    }
    return &file_channels[channel];
}

word __near qb_file_attach(word channel, word handle, word mode) {
    file_channel *entry = qb_file_channel(channel);

    if (entry == (file_channel *)0 || handle == QB_INVALID_HANDLE) {
        return 0;
    }
    entry->handle = handle;
    entry->mode = (byte)mode;
    entry->open = 1;
    return 1;
}

word __near qb_file_handle(word channel) {
    file_channel *entry = qb_file_channel(channel);

    if (entry == (file_channel *)0 || entry->open == 0) {
        return QB_INVALID_HANDLE;
    }
    return entry->handle;
}

word __near qb_file_detach(word channel) {
    file_channel *entry = qb_file_channel(channel);
    word handle;

    if (entry == (file_channel *)0 || entry->open == 0) {
        return QB_INVALID_HANDLE;
    }
    handle = entry->handle;
    entry->handle = QB_INVALID_HANDLE;
    entry->mode = 0;
    entry->open = 0;
    return handle;
}
