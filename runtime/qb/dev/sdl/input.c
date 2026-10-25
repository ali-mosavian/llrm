/* SDL's events as the keyboard of a PC: bytes (the character, or a control code), and for the function and cursor
   keys a zero and the scan code. The SDL thread writes the queue and the worker reads it; one writer, one reader. */
#include <SDL.h>
#include <stdatomic.h>
#include <string.h>

#include "qbsdl.h"

enum { QUEUE = 256, EXTENDED = 0x100 };

/* An entry is the byte, or EXTENDED | scan. */
static uint16_t queue[QUEUE];
static atomic_uint head, tail;
static atomic_int interrupted;
static int pending_scan = -1;

/* CP437's characters 0x80 to 0xFF as Unicode, to type them. */
static const uint16_t cp437_high[128] = {
    0x00C7, 0x00FC, 0x00E9, 0x00E2, 0x00E4, 0x00E0, 0x00E5, 0x00E7, 0x00EA, 0x00EB, 0x00E8, 0x00EF, 0x00EE, 0x00EC,
    0x00C4, 0x00C5, 0x00C9, 0x00E6, 0x00C6, 0x00F4, 0x00F6, 0x00F2, 0x00FB, 0x00F9, 0x00FF, 0x00D6, 0x00DC, 0x00A2,
    0x00A3, 0x00A5, 0x20A7, 0x0192, 0x00E1, 0x00ED, 0x00F3, 0x00FA, 0x00F1, 0x00D1, 0x00AA, 0x00BA, 0x00BF, 0x2310,
    0x00AC, 0x00BD, 0x00BC, 0x00A1, 0x00AB, 0x00BB, 0x2591, 0x2592, 0x2593, 0x2502, 0x2524, 0x2561, 0x2562, 0x2556,
    0x2555, 0x2563, 0x2551, 0x2557, 0x255D, 0x255C, 0x255B, 0x2510, 0x2514, 0x2534, 0x252C, 0x251C, 0x2500, 0x253C,
    0x255E, 0x255F, 0x255A, 0x2554, 0x2569, 0x2566, 0x2560, 0x2550, 0x256C, 0x2567, 0x2568, 0x2564, 0x2565, 0x2559,
    0x2558, 0x2552, 0x2553, 0x256B, 0x256A, 0x2518, 0x250C, 0x2588, 0x2584, 0x258C, 0x2590, 0x2580, 0x03B1, 0x00DF,
    0x0393, 0x03C0, 0x03A3, 0x03C3, 0x00B5, 0x03C4, 0x03A6, 0x0398, 0x03A9, 0x03B4, 0x221E, 0x03C6, 0x03B5, 0x2229,
    0x2261, 0x00B1, 0x2265, 0x2264, 0x2320, 0x2321, 0x00F7, 0x2248, 0x00B0, 0x2219, 0x00B7, 0x221A, 0x207F, 0x00B2,
    0x25A0, 0x00A0};

void sdl_input_open(void)
{
    SDL_StartTextInput();
}

void sdl_key_push(unsigned ascii, unsigned scan)
{
    unsigned at = atomic_load_explicit(&tail, memory_order_relaxed);

    if (at - atomic_load_explicit(&head, memory_order_acquire) >= QUEUE)
        return;
    queue[at % QUEUE] = (uint16_t)(ascii ? ascii : EXTENDED | scan);
    atomic_store_explicit(&tail, at + 1, memory_order_release);
}

int sdl_key_waiting(void)
{
    return atomic_load_explicit(&head, memory_order_relaxed) != atomic_load_explicit(&tail, memory_order_acquire);
}

int sdl_key_take(void)
{
    unsigned at;
    int key;

    if (pending_scan >= 0) {
        key = pending_scan;
        pending_scan = -1;
        return key;
    }
    if (!sdl_key_waiting())
        return -1;
    at = atomic_load_explicit(&head, memory_order_relaxed);
    key = queue[at % QUEUE];
    atomic_store_explicit(&head, at + 1, memory_order_release);
    if (key & EXTENDED) {
        pending_scan = key & 0xFF;
        return 0;
    }
    return key;
}

int sdl_key_peek_scan(void)
{
    unsigned at;

    if (pending_scan >= 0 || !sdl_key_waiting())
        return -1;
    at = atomic_load_explicit(&head, memory_order_relaxed);
    return queue[at % QUEUE] & EXTENDED ? queue[at % QUEUE] & 0xFF : -1;
}

int sdl_interrupted(void)
{
    return atomic_load(&interrupted);
}

void sdl_interrupt(void)
{
    atomic_store(&interrupted, 1);
}

/* The scan codes of the keys that are not text. */
static int scan_of(SDL_Keycode key)
{
    switch (key) {
    case SDLK_F1: case SDLK_F2: case SDLK_F3: case SDLK_F4: case SDLK_F5:
    case SDLK_F6: case SDLK_F7: case SDLK_F8: case SDLK_F9: case SDLK_F10:
        return 0x3B + (int)(key - SDLK_F1);
    case SDLK_F11:
        return 0x85;
    case SDLK_F12:
        return 0x86;
    case SDLK_UP: return 0x48;
    case SDLK_DOWN: return 0x50;
    case SDLK_LEFT: return 0x4B;
    case SDLK_RIGHT: return 0x4D;
    case SDLK_HOME: return 0x47;
    case SDLK_END: return 0x4F;
    case SDLK_PAGEUP: return 0x49;
    case SDLK_PAGEDOWN: return 0x51;
    case SDLK_INSERT: return 0x52;
    case SDLK_DELETE: return 0x53;
    default: return 0;
    }
}

static void key_down(const SDL_KeyboardEvent *event)
{
    SDL_Keycode key = event->keysym.sym;
    int scan = scan_of(key);

    if (scan) {
        sdl_key_push(0, (unsigned)scan);
    } else if (key == SDLK_RETURN || key == SDLK_KP_ENTER) {
        sdl_key_push('\r', 0);
    } else if (key == SDLK_BACKSPACE) {
        sdl_key_push(8, 0);
    } else if (key == SDLK_TAB) {
        sdl_key_push(9, 0);
    } else if (key == SDLK_ESCAPE) {
        sdl_key_push(27, 0);
    } else if ((event->keysym.mod & KMOD_CTRL) && key >= 'a' && key <= 'z') {
        sdl_key_push((unsigned)(key - 'a' + 1), 0);
    }
}

/* One character of typed text (UTF-8) as the code page 437 byte, or none. */
static void text(const char *utf8)
{
    unsigned code = 0, more = 0;
    const unsigned char *at = (const unsigned char *)utf8;

    for (; *at; at++) {
        if (more) {
            code = code << 6 | (*at & 0x3F);
            if (--more)
                continue;
        } else if (*at < 0x80) {
            code = *at;
        } else if ((*at & 0xE0) == 0xC0) {
            code = *at & 0x1F, more = 1;
            continue;
        } else if ((*at & 0xF0) == 0xE0) {
            code = *at & 0x0F, more = 2;
            continue;
        } else {
            continue;
        }
        if (code >= 0x20 && code < 0x7F) {
            sdl_key_push(code, 0);
        } else {
            unsigned index;

            for (index = 0; index < 128; index++)
                if (cp437_high[index] == code)
                    sdl_key_push(0x80 + index, 0);
        }
    }
}

void sdl_input_pump(void)
{
    SDL_Event event;

    while (SDL_PollEvent(&event)) {
        if (event.type == SDL_QUIT)
            sdl_interrupt();
        else if (event.type == SDL_KEYDOWN)
            key_down(&event.key);
        else if (event.type == SDL_TEXTINPUT)
            text(event.text.text);
    }
}
