#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static int allocation_countdown = -1;
static void *checked_calloc(size_t n, size_t size)
{
    if (allocation_countdown == 0) return NULL;
    if (allocation_countdown > 0) allocation_countdown--;
    return calloc(n, size);
}
#define calloc checked_calloc
#include "gamepkg_ram_cache.h"
#undef calloc
#ifdef _WIN32
int crash_reporter_process(void) { return 0; }
#endif

static pthread_mutex_t complete_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t complete_event = PTHREAD_COND_INITIALIZER;
static unsigned completed;
static void *writer(void *argument)
{
    unsigned index = (unsigned)(uintptr_t)argument, i;
    unsigned char bytes[257], readback[257];
    struct gp_ram_file *file;
    memset(bytes, (int)index + 1, sizeof(bytes));
    file = gp_ram_open("q:\\cache002.map", O_RDWR, 1, NULL);
    assert(file);
    for (i = 0; i < 200; i++)
    {
        assert(gp_ram_write(file, bytes, sizeof(bytes), 1, index * 65536u + 65400u) == sizeof(bytes));
        assert(gp_ram_read(file, readback, sizeof(readback), 1, index * 65536u + 65400u) == sizeof(readback));
        assert(!memcmp(bytes, readback, sizeof(bytes)));
    }
    gp_ram_close(file);
    pthread_mutex_lock(&complete_lock);
    completed++;
    pthread_cond_signal(&complete_event);
    pthread_mutex_unlock(&complete_lock);
    return NULL;
}

static int layout(void)
{
    static const uint64_t sizes[] = {291504128, 291504128, 36700160, 49283072, 49283072, 49283072};
    struct gp_ram_file *files[7], *extra;
    char path[32], byte;
    unsigned i;
    assert(gp_ram_configure(UINT64_C(1073741824)) == 0);
    for (i = 0; i < 6; i++)
    {
        sprintf(path, "q:\\cache%03u.map", i);
        files[i] = gp_ram_open(path, O_RDWR | O_CREAT, 1, NULL); assert(files[i]);
        assert(gp_ram_seek(files[i], (int64_t)sizes[i], SEEK_SET, NULL) == 0);
        assert(gp_ram_truncate(files[i]) == 0);
        assert(gp_ram_write(files[i], "x", 1, 1, sizes[i] - 1) == 1);
        assert(gp_ram_read(files[i], &byte, 1, 1, sizes[i] - 1) == 1 && byte == 'x');
    }
    files[6] = gp_ram_open("q:\\last_language.dat", O_RDWR | O_CREAT, 1, NULL); assert(files[6]);
    assert(gp_ram_write(files[6], "lang", 4, 1, 0) == 4);
    assert(gp_ram_logical == UINT64_C(767557636));
    assert(gp_ram_allocated == 7 * GP_RAM_PAGE);
    extra = gp_ram_open("q:\\cache006.map", O_RDWR | O_CREAT, 1, NULL); assert(extra);
    assert(gp_ram_seek(extra, 400 * 1024 * 1024, SEEK_SET, NULL) == 0);
    assert(gp_ram_truncate(extra) == -1 && errno == ENOSPC);
    gp_ram_close(extra); assert(gp_ram_unlink("q:\\cache006.map") == 0);
    for (i = 0; i < 7; i++) gp_ram_close(files[i]);
    for (i = 0; i < 6; i++)
    { sprintf(path, "q:\\cache%03u.map", i); assert(gp_ram_unlink(path) == 0); }
    assert(gp_ram_unlink("q:\\last_language.dat") == 0);
    assert(gp_ram_logical == 0 && gp_ram_allocated == 0);
    puts("PASS RAM cache full six-map layout within 1 GiB, sparse storage, aggregate budget and cleanup");
    return 0;
}

int main(int argc, char **argv)
{
    struct gp_ram_file *a, *b;
    struct gp_ram_info info;
    pthread_t threads[8];
    uint64_t position, stamp = 123456789;
    unsigned i;
    int existed = -1;
    char name[32];
    unsigned char data[3 * GP_RAM_PAGE], bytes[3 * GP_RAM_PAGE];
    static const char *bad[] = {"q:cache000.map", "q:\\..\\cache000.map", "q:\\cache000.map:stream",
        "q:\\cache000.map.", "q:\\cache000.map ", "q:\\cache000.map/x", "q:\\cache-01.map",
        "q:\\\\cache000.map", "q:\\saved\\profile.sav", "q:\\", "q:\\cache000.MAP/../cache001.map"};
    if (argc == 2 && !strcmp(argv[1], "--layout")) return layout();
    assert(argc == 1);
    assert(gp_ram_path("z:\\savegame.bin", name) == 0);
    assert(gp_ram_path("Q:/CACHE000.MAP", name) == 1 && !strcmp(name, "cache000.map"));
    assert(gp_ram_path("q:\\last_language.dat", name) == 1);
    for (i = 0; i < sizeof(bad)/sizeof(bad[0]); i++)
        assert(gp_ram_path(bad[i], name) == -1);
    assert(gp_ram_configure(8 * 1024 * 1024) == 0);
    assert(gp_ram_configure(8 * 1024 * 1024) == 0);
    assert(gp_ram_configure(16 * 1024 * 1024) == -1 && errno == EBUSY);
    assert(!gp_ram_open("q:\\cache000.map", O_RDONLY, 1, NULL) && errno == ENOENT);
    a = gp_ram_open("q:\\cache000.map", O_RDWR | O_CREAT | O_EXCL, 1, &existed);
    assert(a && !existed);
    assert(!gp_ram_open("Q:/CACHE000.MAP", O_RDWR | O_CREAT | O_EXCL, 1, NULL) && errno == EEXIST);
    b = gp_ram_open("Q:/CACHE000.MAP", O_RDONLY, 1, &existed);
    assert(b && existed);
    assert(gp_ram_seek(a, 4 * 1024 * 1024, SEEK_SET, &position) == 0);
    assert(gp_ram_truncate(a) == 0 && gp_ram_allocated == 0);
    assert(gp_ram_read(b, bytes, sizeof(bytes), 0, 0) == sizeof(bytes));
    for (i = 0; i < sizeof(bytes); i++) assert(bytes[i] == 0);
    for (i = 0; i < sizeof(data); i++) data[i] = (unsigned char)(i * 31u + 17u);
    assert(gp_ram_write(a, data, sizeof(data), 1, GP_RAM_PAGE - 37) == sizeof(data));
    assert(gp_ram_read(b, bytes, sizeof(bytes), 1, GP_RAM_PAGE - 37) == sizeof(bytes));
    assert(!memcmp(data, bytes, sizeof(data)));
    assert(gp_ram_seek(b, 0, SEEK_CUR, &position) == 0 && position == sizeof(bytes));
    assert(gp_ram_write(b, data, 1, 1, 0) == -1 && errno == EBADF);
    assert(gp_ram_read(b, bytes, 1, 1, UINT64_MAX) == 0);
    assert(gp_ram_seek(a, INT64_MIN, SEEK_CUR, NULL) == -1 && errno == EINVAL);
    assert(gp_ram_seek(a, -1, SEEK_SET, NULL) == -1 && errno == EINVAL);
    assert(gp_ram_seek(a, 1, 99, NULL) == -1 && errno == EINVAL);
    assert(gp_ram_seek(a, INT64_MAX, SEEK_SET, NULL) == 0);
    assert(gp_ram_seek(a, 1, SEEK_CUR, NULL) == -1 && errno == EINVAL);
    assert(gp_ram_truncate(a) == -1 && errno == ENOSPC);
    gp_ram_fstat(a, &info); assert(info.size == 4 * 1024 * 1024);
    assert(gp_ram_write(a, data, 1, 1, UINT64_MAX) == -1 && errno == ENOSPC);
    assert(gp_ram_read(a, NULL, 1, 1, 0) == -1 && errno == EINVAL);
    assert(gp_ram_write(a, NULL, 1, 1, 0) == -1 && errno == EINVAL);
    assert(gp_ram_unlink("q:\\cache000.map") == -1 && errno == EBUSY);
    assert(gp_ram_seek(a, GP_RAM_PAGE + 3, SEEK_SET, NULL) == 0);
    assert(gp_ram_truncate(a) == 0);
    assert(gp_ram_seek(a, 3 * GP_RAM_PAGE, SEEK_SET, NULL) == 0);
    assert(gp_ram_truncate(a) == 0);
    memset(bytes, 1, sizeof(bytes));
    assert(gp_ram_read(b, bytes, GP_RAM_PAGE, 1, GP_RAM_PAGE + 3) == GP_RAM_PAGE);
    for (i = 0; i < GP_RAM_PAGE; i++) assert(bytes[i] == 0);
    gp_ram_times(a, &stamp, &stamp, &stamp);
    assert(gp_ram_stat("q:\\cache000.map", &info) == 0);
    assert(info.created == stamp && info.accessed == stamp && info.modified == stamp);
    assert(gp_ram_readonly("q:\\cache000.map", 1) == 0);
    assert(gp_ram_write(a, data, 1, 1, 0) == -1 && errno == EACCES);
    assert(!gp_ram_open("q:\\cache000.map", O_RDWR, 1, NULL) && errno == EACCES);
    assert(gp_ram_readonly("q:\\cache000.map", 0) == 0);
    gp_ram_close(b); gp_ram_close(a);
    assert(gp_ram_unlink("q:\\cache000.map") == 0 && gp_ram_allocated == 0 && gp_ram_logical == 0);
    assert(gp_ram_stat("q:\\cache000.map", &info) == -1 && errno == ENOENT);
    /* Failure on the second page must roll back the first page and size. */
    a = gp_ram_open("q:\\cache001.map", O_RDWR | O_CREAT, 1, NULL); assert(a);
    allocation_countdown = 1;
    assert(gp_ram_write(a, data, sizeof(data), 1, 0) == -1 && errno == ENOMEM);
    assert(gp_ram_allocated == 0 && gp_ram_logical == 0);
    allocation_countdown = -1;
    assert(gp_ram_write(a, data, sizeof(data), 1, 0) == sizeof(data));
    gp_ram_close(a); assert(gp_ram_unlink("q:\\cache001.map") == 0);
    a = gp_ram_open("q:\\cache002.map", O_RDWR | O_CREAT, 1, NULL); assert(a);
    for (i = 0; i < 8; i++) assert(!pthread_create(&threads[i], NULL, writer, (void *)(uintptr_t)i));
    pthread_mutex_lock(&complete_lock);
    while (completed != 8) assert(!pthread_cond_wait(&complete_event, &complete_lock));
    pthread_mutex_unlock(&complete_lock);
#ifndef _WIN32
    for (i = 0; i < 8; i++) assert(!pthread_join(threads[i], NULL));
#endif
    gp_ram_close(a); assert(gp_ram_unlink("q:\\cache002.map") == 0);
    assert(gp_ram_allocated == 0 && gp_ram_logical == 0);
    a = gp_ram_open("q:\\cache003.map", O_RDWR | O_CREAT, 1, NULL); assert(a);
    b = gp_ram_open("q:\\cache004.map", O_RDWR | O_CREAT, 1, NULL); assert(b);
    assert(gp_ram_seek(a, 6 * 1024 * 1024, SEEK_SET, NULL) == 0 && gp_ram_truncate(a) == 0);
    assert(gp_ram_seek(b, 3 * 1024 * 1024, SEEK_SET, NULL) == 0);
    assert(gp_ram_truncate(b) == -1 && errno == ENOSPC);
    gp_ram_fstat(b, &info); assert(info.size == 0);
    assert(gp_ram_logical == 6 * 1024 * 1024 && gp_ram_allocated == 0);
    gp_ram_close(a); gp_ram_close(b);
    assert(gp_ram_unlink("q:\\cache003.map") == 0 && gp_ram_unlink("q:\\cache004.map") == 0);
    puts("PASS RAM cache sparse data, boundaries, permissions, timestamps, allocation rollback, eight concurrent writers and cleanup");
    return 0;
}
