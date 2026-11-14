#ifndef GAMEPKG_WINDOWS_DISC_STREAM_H
#define GAMEPKG_WINDOWS_DISC_STREAM_H
/* Host-owned native startup/SDL transport. No CRT FILE object crosses DLLs. */
#include <stddef.h>
#include <stdint.h>
struct SDL_RWops;
#ifdef __cplusplus
extern "C" {
#endif
int gpw_load_xbe(const char *, void **, size_t *);
unsigned char *gpw_pcache_xbe(const char *, size_t *);
/* A named image may only be the launcher's canonical virtual entry. */
int gpw_game_data_enter(const char *, const char *, int);
int64_t gpw_file_size(const char *);
struct SDL_RWops *gpw_SDL_RWFromFile(const char *, const char *);
#ifdef __cplusplus
}
#endif
#endif
