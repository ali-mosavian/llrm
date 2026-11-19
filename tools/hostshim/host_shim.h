/* Force-included in every bench program on the host: the text screen is a buffer in the shim, not 0xB8000. */
extern unsigned char host_text_screen[];
