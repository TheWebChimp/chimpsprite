/* La función C de chimpsprite-thumbnails (src/ffi.rs) para la extensión de Quick Look.
 * Compilar la biblioteca estática:
 *   cargo rustc --release -p chimpsprite-thumbnails --crate-type staticlib
 * y enlazar libchimpsprite_thumbnails.a en la extensión (bridging header: este archivo). */
#ifndef CHIMP_THUMBNAIL_H
#define CHIMP_THUMBNAIL_H

#include <stddef.h>
#include <stdint.h>

#define CHIMP_THUMBNAIL_OK 0     /* *out y *out_len tienen el PNG */
#define CHIMP_THUMBNAIL_NONE 1   /* el archivo no trae miniatura */
#define CHIMP_THUMBNAIL_ERROR -1 /* puntero nulo o archivo roto */

/* La miniatura (PNG RGBA, lado mayor <= 256) de los bytes de un .chimp. Si devuelve
 * CHIMP_THUMBNAIL_OK, liberar el PNG con chimp_thumbnail_free(*out, *out_len). */
int32_t chimp_thumbnail_png(const uint8_t *data, size_t len, uint8_t **out, size_t *out_len);

void chimp_thumbnail_free(uint8_t *ptr, size_t len);

#endif
