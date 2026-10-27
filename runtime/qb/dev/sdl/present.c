/* The window: one streaming texture, drawn from the lines of the framebuffer that the program has touched. */
#include <SDL.h>
#include <stdatomic.h>
#include <string.h>

#include "qbsdl.h"
#include "video.h"

enum {
    CURSOR_ROWS = 2,
    BLINK_FRAMES = 30,
    ASPECT_NUMERATOR = 3,
    ASPECT_DENOMINATOR = 4,
    START_WIDTH = 960
};

static SDL_Window *window;
static SDL_Renderer *renderer;
static SDL_Texture *texture;
static uint32_t argb[HOST_MAX_WIDTH * HOST_MAX_HEIGHT];
static unsigned frame_width, frame_height, serial = ~0u, shown;
static unsigned blink;
static int output_width, output_height;
static int cursor_drawn, cursor_row, cursor_column;

int sdl_present_open(void)
{
    SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY, "nearest");
    window = SDL_CreateWindow("QuickBASIC", SDL_WINDOWPOS_UNDEFINED, SDL_WINDOWPOS_UNDEFINED, START_WIDTH,
                              START_WIDTH * ASPECT_NUMERATOR / ASPECT_DENOMINATOR, SDL_WINDOW_RESIZABLE);
    if (!window)
        return -1;
    renderer = SDL_CreateRenderer(window, -1, SDL_RENDERER_ACCELERATED | SDL_RENDERER_PRESENTVSYNC);
    if (!renderer)
        renderer = SDL_CreateRenderer(window, -1, SDL_RENDERER_SOFTWARE);
    return renderer ? 0 : -1;
}

void sdl_present_close(void)
{
    if (texture)
        SDL_DestroyTexture(texture);
    if (renderer)
        SDL_DestroyRenderer(renderer);
    if (window)
        SDL_DestroyWindow(window);
    texture = 0;
    renderer = 0;
    window = 0;
}

const uint32_t *sdl_present_argb(unsigned *width, unsigned *height)
{
    if (!shown)
        return 0;
    *width = frame_width;
    *height = frame_height;
    return argb;
}

/* A new mode: a texture of its size, shown at 4:3 as the adapter does. */
static int start_frame(void)
{
    unsigned y;

    if (texture)
        SDL_DestroyTexture(texture);
    frame_width = host_width();
    frame_height = host_height();
    texture = SDL_CreateTexture(renderer, SDL_PIXELFORMAT_ARGB8888, SDL_TEXTUREACCESS_STREAMING, (int)frame_width,
                                (int)frame_height);
    if (!texture)
        return -1;
    for (y = 0; y < frame_height; y++)
        atomic_store_explicit(&host_dirty[y], 1, memory_order_release);
    cursor_drawn = 0;
    return 0;
}

/* The program writes pixels without synchronisation and flags the line afterwards, so a read here can be of a line that is
   being written again; whoever wrote last flagged it, and the next frame converts it whole. The race is the protocol's, so
   ThreadSanitizer is told to leave this reader alone (#4669, docs/working/qb-host.md). */
#if defined(__clang__)
#define NO_TSAN __attribute__((no_sanitize("thread")))
#else
#define NO_TSAN __attribute__((no_sanitize_thread))
#endif

NO_TSAN static void convert_line(unsigned y, const uint8_t rgb[256][3])
{
    const uint8_t *from = host_pixels() + (size_t)y * frame_width;
    uint32_t *to = argb + (size_t)y * frame_width;
    unsigned x;

    for (x = 0; x < frame_width; x++) {
        const uint8_t *color = rgb[from[x]];

        to[x] = 0xFF000000u | (uint32_t)color[0] << 16 | (uint32_t)color[1] << 8 | color[2];
    }
}

/* The text cursor: the bottom rows of its cell, in the colour of the cell's character, blinking. */
static void draw_cursor(const uint8_t rgb[256][3], unsigned row, unsigned column, int on)
{
    unsigned line, x;

    for (line = HOST_CELL_HEIGHT - CURSOR_ROWS; line < HOST_CELL_HEIGHT; line++) {
        unsigned y = row * HOST_CELL_HEIGHT + line;

        convert_line(y, rgb);
        if (on)
            for (x = 0; x < HOST_CELL_WIDTH; x++)
                argb[(size_t)y * frame_width + column * HOST_CELL_WIDTH + x] = 0xFFAAAAAAu;
    }
}

int sdl_present_frame(void)
{
    uint8_t rgb[256][3];
    unsigned y, copied = 0, row, column;
    int visible, on, texels, width, height, resized;
    SdlRect to;
    void *pixels;
    int pitch;

    if (!renderer)
        return 0;
    host_state_lock();
    if (host_mode_serial() != serial) {
        serial = host_mode_serial();
        if (start_frame()) {
            host_state_unlock();
            return 0;
        }
    }
    host_colors(rgb);
    for (y = 0; y < frame_height; y++)
        if (atomic_exchange_explicit(&host_dirty[y], 0, memory_order_acquire)) {
            convert_line(y, rgb);
            copied++;
        }
    host_cursor(&row, &column, &visible);
    blink++;
    on = host_mode() == HOST_MODE_TEXT && visible && (blink / BLINK_FRAMES & 1) == 0;
    if (cursor_drawn != on || (on && (row != (unsigned)cursor_row || column != (unsigned)cursor_column)) || copied) {
        if (cursor_drawn)
            draw_cursor(rgb, (unsigned)cursor_row, (unsigned)cursor_column, 0);
        if (on)
            draw_cursor(rgb, row, column, 1);
        copied += cursor_drawn != on || on;
        cursor_drawn = on;
        cursor_row = (int)row;
        cursor_column = (int)column;
    }
    host_state_unlock();
    SDL_GetRendererOutputSize(renderer, &width, &height);
    resized = width != output_width || height != output_height;
    output_width = width;
    output_height = height;
    if (!copied && shown && !resized)
        return 0;
    texels = SDL_LockTexture(texture, 0, &pixels, &pitch);
    if (texels == 0) {
        for (y = 0; y < frame_height; y++)
            memcpy((uint8_t *)pixels + (size_t)y * (unsigned)pitch, argb + (size_t)y * frame_width, frame_width * 4);
        SDL_UnlockTexture(texture);
    }
    SDL_RenderClear(renderer);
    to = sdl_fit(width, height, (int)frame_width);
    SDL_RenderCopy(renderer, texture, 0, &(SDL_Rect){to.x, to.y, to.w, to.h});
    SDL_RenderPresent(renderer);
    shown = 1;
    return 1;
}
