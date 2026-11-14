#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include "xiso.h"
#ifdef _WIN32
#include <windows.h>
#include "windows_sparse_disc.inc"
static pthread_mutex_t completion_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t completion_event = PTHREAD_COND_INITIALIZER;
static int completed;
int crash_reporter_process(void) { return 0; }
#endif
void platform_log(const char *format, ...) { (void)format; }
static void *reader(void *unused) {
    (void)unused;
    for (int i = 0; i < 500; ++i) {
        uint8_t out[32] = {0};
        assert(gamepkg_path_read("/virtual/disc/maps/UI.MAP", 2, out, 30) == 8);
        assert(!memcmp(out, "23456789", 8));
    }
#ifdef _WIN32
    pthread_mutex_lock(&completion_lock);
    completed++;
    pthread_cond_signal(&completion_event);
    pthread_mutex_unlock(&completion_lock);
#endif
    return NULL;
}
int main(int argc, char **argv) {
#ifdef _WIN32
    int original_descriptor = -1;
    if (argc == 4 && !strcmp(argv[1], "--sparse"))
        return make_sparse_image(argv[2], argv[3]);
    if (argc == 4 && !strcmp(argv[1], "--inherited")) {
        char handle[32];
        original_descriptor = _open(argv[2], _O_RDONLY | _O_BINARY);
        assert(original_descriptor >= 0);
        snprintf(handle, sizeof(handle), "%llu",
                 (unsigned long long)(uintptr_t)_get_osfhandle(original_descriptor));
        assert(!_putenv_s("GAMEPKG_DISC_HANDLE", handle));
        argv[1] = "Z:/gamepkg-ungranted-source/absent.iso";
        argv[2] = argv[3];
        argc = 3;
    }
#endif
    assert(argc == 3);
    char error[512];
    int mounted = xiso_view_mount(argv[1], "/virtual/disc", error, sizeof(error));
#ifdef _WIN32
    if (original_descriptor >= 0) assert(!_close(original_descriptor));
#endif
    if (!strcmp(argv[2], "bad")) { assert(!mounted); return 0; }
    assert(mounted);
    assert(!xiso_view_mount(argv[1], "/different", error, sizeof(error)));
#ifdef _WIN32
    DWORD written = 0;
    assert(xiso_view_source_fd() >= 0);
    assert(!WriteFile((HANDLE)_get_osfhandle(xiso_view_source_fd()), "x", 1, &written, NULL));
    assert(GetLastError() == ERROR_ACCESS_DENIED);
#else
    assert((fcntl(xiso_view_source_fd(), F_GETFL) & O_ACCMODE) == O_RDONLY);
    assert(write(xiso_view_source_fd(), "x", 1) == -1 && errno == EBADF);
#endif
    gamepkg_path_info info;
    assert(gamepkg_path_stat("/virtual/disc/maps/UI.MAP", &info) == 0);
    assert(info.kind == GAMEPKG_PATH_FILE && info.size == 10);
    assert(info.flags & GAMEPKG_PATH_READ_ONLY);
    assert(gamepkg_path_stat("/virtual/disc/maps", &info) == 0);
    assert(info.kind == GAMEPKG_PATH_DIRECTORY);
    assert(xiso_view_lookup("/virtual/disc/maps/../ui.map") == -1);
    assert(xiso_view_lookup("/virtual/disc-other/maps/ui.map") == -2);
    assert(xiso_view_lookup("/virtual/disc/maps/not-there.map") == -1);
    uint8_t out[32] = {0};
    assert(gamepkg_path_read("/virtual/disc/maps/ui.map", 9, out, 32) == 1 && out[0] == '9');
    assert(gamepkg_path_read("/virtual/disc/maps/ui.map", 10, out, 32) == 0);
    assert(gamepkg_path_read("/virtual/disc/maps/ui.map", UINT64_MAX, out, 32) == 0);
    assert(gamepkg_path_read("/virtual/disc/maps/ui.map", 0, NULL, 1) == -1);
    uint32_t h, stale;
    assert(gamepkg_dir_open("/virtual/disc/maps", &h) == 1);
    stale = h;
    assert(gamepkg_dir_read(h, out, sizeof(out)) == 6 && !strcmp((char *)out, "ui.map"));
    assert(gamepkg_dir_read(h, out, sizeof(out)) == 0);
    gamepkg_dir_close(h);
    assert(gamepkg_dir_read(stale, out, sizeof(out)) == -1);
    assert(gamepkg_dir_open("/virtual/disc/maps", &h) == 1 && h != stale);
    gamepkg_dir_close(h);
    pthread_t threads[8];
    for (int i = 0; i < 8; ++i) assert(!pthread_create(&threads[i], NULL, reader, NULL));
#ifdef _WIN32
    /* The port's Windows threads are detached and intentionally have no join. */
    pthread_mutex_lock(&completion_lock);
    while (completed < 8) assert(!pthread_cond_wait(&completion_event, &completion_lock));
    pthread_mutex_unlock(&completion_lock);
#else
    for (int i = 0; i < 8; ++i) assert(!pthread_join(threads[i], NULL));
#endif
    puts("PASS native immutable disc view, bounds, directory tokens and eight concurrent readers");
    return 0;
}
