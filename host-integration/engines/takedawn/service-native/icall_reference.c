/* Maintainer-only inert console adapter. Exact pinned logger bodies call this
 * instead of an actual console. Only numeric original fields are collected;
 * it is never linked into a game or the trusted GamePKG runtime. */
#include <stdarg.h>
#include <stdint.h>
static int contains(const char *text, const char *needle) {
    for (; *text; ++text) {
        const char *left=text, *right=needle;
        while (*right && *left==*right) { ++left; ++right; }
        if (!*right) return 1;
    }
    return 0;
}
extern void gamepkg_reference_noncode(uint32_t, uint64_t, uint64_t);
extern void gamepkg_reference_missing(uint32_t, uint64_t);
void *gamepkg_native_stderr(void) { return (void *)0; }
int gamepkg_native_fflush(void *stream) { (void)stream; return 0; }
int gamepkg_native_fprintf(void *stream, const char *format, ...) {
    (void)stream;
    va_list args;va_start(args,format);
    if (contains(format,"is not code -- skipped")) {
        uint32_t target=va_arg(args,unsigned int);
        uint64_t hits=va_arg(args,unsigned long long);
        uint64_t total=va_arg(args,unsigned long long);
        gamepkg_reference_noncode(target,hits,total);
    } else if (contains(format,"Failed to resolve VA")) {
        uint32_t target=va_arg(args,unsigned int);
        uint64_t total=va_arg(args,unsigned long long);
        gamepkg_reference_missing(target,total);
    }
    va_end(args);return 0;
}
