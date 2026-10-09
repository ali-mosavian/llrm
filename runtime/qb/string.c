/* QB dynamic strings: stable four-byte descriptors, movable near payloads. */

#include "types.h"

typedef struct {
    word length;
    word data;
} string;

typedef struct {
    word owner;
    word length;
} allocation;

#define HEAP_BYTES 8192
#define TEMPORARIES 32

static byte heap[HEAP_BYTES];
static word heap_used;
static string null_string;
static string temporary[TEMPORARIES];
static byte temporary_used[TEMPORARIES];

static word qb_string_address_of(const void *pointer) {
    return (word)pointer;
}

static string *qb_string_descriptor(word address) {
    return (string *)address;
}

static word qb_string_descriptor_address(const string *value) {
    return qb_string_address_of(value);
}

static word qb_string_temporary_index(const string *value) {
    word address = qb_string_descriptor_address(value);
    word first = qb_string_descriptor_address(&temporary[0]);
    word bytes = TEMPORARIES * sizeof(string);

    if (address < first || address >= first + bytes || ((address - first) % sizeof(string)) != 0) {
        return TEMPORARIES;
    }
    return (address - first) / sizeof(string);
}

static string *qb_string_temporary_descriptor(void) {
    word index;

    for (index = 0; index < TEMPORARIES; ++index) {
        if (temporary_used[index] == 0) {
            temporary_used[index] = 1;
            temporary[index].length = 0;
            temporary[index].data = 0;
            return &temporary[index];
        }
    }
    return (string *)0;
}

static void qb_string_release_temporary(string *value) {
    word index = qb_string_temporary_index(value);

    if (index != TEMPORARIES) {
        temporary_used[index] = 0;
        value->length = 0;
        value->data = 0;
    }
}

static void qb_string_copy_bytes(byte *destination, const byte *source, word length) {
    while (length != 0) {
        *destination++ = *source++;
        --length;
    }
}

static void qb_string_compact(void) {
    byte *read = heap;
    byte *write = heap;
    byte *end = heap + heap_used;

    while (read != end) {
        allocation *old = (allocation *)read;
        word bytes = old->length;
        byte *payload = read + sizeof(allocation);
        string *owner = qb_string_descriptor(old->owner);

        if (owner->data == qb_string_address_of(payload)) {
            allocation *next = (allocation *)write;
            byte *next_payload = write + sizeof(allocation);
            if (write != read) {
                qb_string_copy_bytes(next_payload, payload, bytes);
            }
            next->owner = old->owner;
            next->length = bytes;
            owner->data = qb_string_address_of(next_payload);
            write = next_payload + bytes;
        }
        read = payload + bytes;
    }
    heap_used = (word)(write - heap);
}

static byte *qb_string_allocate(word owner, word length) {
    word header = sizeof(allocation);
    allocation *record;
    byte *payload;

    qb_string_compact();
    if (length > HEAP_BYTES - header || heap_used > HEAP_BYTES - header - length) {
        return (byte *)0;
    }

    record = (allocation *)(heap + heap_used);
    payload = (byte *)(record + 1);
    record->owner = owner;
    record->length = length;
    heap_used += header + length;
    return payload;
}

void __near qb_string_delete(word destination_address) {
    string *destination = qb_string_descriptor(destination_address);

    destination->length = 0;
    destination->data = 0;
    qb_string_release_temporary(destination);
}

void __near qb_string_assign(word source_address, word destination_address) {
    string *source = qb_string_descriptor(source_address);
    string *destination = qb_string_descriptor(destination_address);
    word length;
    byte *payload;
    allocation *record;

    if (source == destination) {
        return;
    }

    length = source->length;
    qb_string_delete(destination_address);
    if (qb_string_temporary_index(source) != TEMPORARIES) {
        destination->length = length;
        destination->data = source->data;
        if (length != 0) {
            record = ((allocation *)source->data) - 1;
            record->owner = destination_address;
        }
        qb_string_release_temporary(source);
        return;
    }
    if (length == 0) {
        return;
    }

    payload = qb_string_allocate(destination_address, length);
    if (payload == (byte *)0) {
        return;
    }

    qb_string_copy_bytes(payload, (const byte *)source->data, length);
    destination->length = length;
    destination->data = qb_string_address_of(payload);
}

word __near qb_string_space(word length) {
    string *result;
    byte *payload;
    word index;

    if (length == 0) {
        return qb_string_descriptor_address(&null_string);
    }
    result = qb_string_temporary_descriptor();
    if (result == (string *)0) {
        return qb_string_descriptor_address(&null_string);
    }
    payload = qb_string_allocate(qb_string_descriptor_address(result), length);
    if (payload == (byte *)0) {
        qb_string_release_temporary(result);
        return qb_string_descriptor_address(&null_string);
    }
    for (index = 0; index < length; ++index) {
        payload[index] = ' ';
    }
    result->length = length;
    result->data = qb_string_address_of(payload);
    return qb_string_descriptor_address(result);
}
