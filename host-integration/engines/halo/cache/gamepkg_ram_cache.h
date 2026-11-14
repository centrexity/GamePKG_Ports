/* SPDX-License-Identifier: Apache-2.0
 * GamePKG's process-local decompression cache. No backing file or driver.
 * The caller translates errors through errno and owns each open handle.
 * All contents and metadata are serialized by the same process-local lock.
 */
#ifndef GAMEPKG_RAM_CACHE_H
#define GAMEPKG_RAM_CACHE_H
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifndef O_ACCMODE
#define O_ACCMODE (O_WRONLY | O_RDWR)
#endif

#define GP_RAM_PAGE 65536u
#define GP_RAM_PAGES 8192u
#define GP_RAM_FILES 16u
#define GP_RAM_MAX ((uint64_t)GP_RAM_PAGE * GP_RAM_PAGES)

struct gp_ram_node {
    char name[32];
    uint64_t size, created, accessed, modified;
    unsigned references, read_only;
    unsigned char *pages[GP_RAM_PAGES];
};
struct gp_ram_file {
    struct gp_ram_node *node;
    uint64_t position;
    int access;
};
struct gp_ram_info {
    uint64_t size, created, accessed, modified;
    unsigned read_only;
};
static pthread_mutex_t gp_ram_lock = PTHREAD_MUTEX_INITIALIZER;
static struct gp_ram_node gp_ram_nodes[GP_RAM_FILES];
static uint64_t gp_ram_capacity, gp_ram_logical, gp_ram_allocated;

/* 1: supported Q file, 0: another drive, -1: rejected Q spelling.
 * Never fall back to disk for a rejected name in the cache namespace. */
static int gp_ram_path(const char *path, char name[32])
{
    unsigned i, n;
    const char *p;
    if (!path || !path[0] || (path[0] != 'q' && path[0] != 'Q') || path[1] != ':')
        return 0;
    p = path + 2;
    if (*p != '/' && *p != '\\') return -1;
    p++;
    n = (unsigned)strlen(p);
    if (n >= 32) return -1;
    for (i = 0; i <= n; i++)
        name[i] = p[i] >= 'A' && p[i] <= 'Z' ? (char)(p[i] + 'a' - 'A') : p[i];
    if (!strcmp(name, "last_language.dat")) return 1;
    if (n == 12 && !memcmp(name, "cache", 5) && !strcmp(name + 8, ".map") &&
        name[5] >= '0' && name[5] <= '9' && name[6] >= '0' && name[6] <= '9' &&
        name[7] >= '0' && name[7] <= '9') return 1;
    return -1;
}

static int gp_ram_configure(uint64_t bytes)
{
    int result = 0;
    pthread_mutex_lock(&gp_ram_lock);
    if (!bytes || bytes > UINT64_C(1073741824)) { errno = EINVAL; result = -1; }
    else if (gp_ram_capacity && gp_ram_capacity != bytes) { errno = EBUSY; result = -1; }
    else gp_ram_capacity = bytes;
    pthread_mutex_unlock(&gp_ram_lock);
    return result;
}

static struct gp_ram_node *gp_ram_find(const char *name)
{
    unsigned i;
    for (i = 0; i < GP_RAM_FILES; i++)
        if (gp_ram_nodes[i].name[0] && !strcmp(gp_ram_nodes[i].name, name))
            return &gp_ram_nodes[i];
    return NULL;
}

/* Lock held. Extending a sparse cache changes its size without allocating
 * all of its pages. Truncation discards bytes, including a partial tail. */
static int gp_ram_resize(struct gp_ram_node *node, uint64_t size)
{
    unsigned i, first;
    if (size > GP_RAM_MAX || size > gp_ram_capacity - (gp_ram_logical - node->size))
    { errno = ENOSPC; return -1; }
    if (size < node->size)
    {
        first = (unsigned)((size + GP_RAM_PAGE - 1) / GP_RAM_PAGE);
        for (i = first; i < GP_RAM_PAGES; i++)
            if (node->pages[i])
            { free(node->pages[i]); node->pages[i] = NULL; gp_ram_allocated -= GP_RAM_PAGE; }
        if (size % GP_RAM_PAGE && node->pages[size / GP_RAM_PAGE])
            memset(node->pages[size / GP_RAM_PAGE] + size % GP_RAM_PAGE, 0,
                GP_RAM_PAGE - (size_t)(size % GP_RAM_PAGE));
    }
    gp_ram_logical = gp_ram_logical - node->size + size;
    node->size = size;
    return 0;
}

static struct gp_ram_file *gp_ram_open(const char *path, int flags, uint64_t now, int *existed)
{
    char name[32];
    struct gp_ram_file *file = NULL;
    struct gp_ram_node *node;
    unsigned i;
    int access = flags & O_ACCMODE;
    if (gp_ram_path(path, name) != 1) { errno = EINVAL; return NULL; }
    pthread_mutex_lock(&gp_ram_lock);
    if (!gp_ram_capacity) { errno = ENODEV; goto done; }
    node = gp_ram_find(name);
    if (existed) *existed = node != NULL;
    if (node && (flags & O_CREAT) && (flags & O_EXCL)) { errno = EEXIST; goto done; }
    if (!node && !(flags & O_CREAT)) { errno = ENOENT; goto done; }
    if (node && node->read_only && (access != O_RDONLY || (flags & O_TRUNC)))
    { errno = EACCES; goto done; }
    if ((flags & O_TRUNC) && access == O_RDONLY) { errno = EACCES; goto done; }
    if (access != O_RDONLY && access != O_WRONLY && access != O_RDWR)
    { errno = EINVAL; goto done; }
    file = (struct gp_ram_file *)calloc(1, sizeof(*file));
    if (!file) { errno = ENOMEM; goto done; }
    if (!node)
    {
        for (i = 0; i < GP_RAM_FILES && gp_ram_nodes[i].name[0]; i++) {}
        if (i == GP_RAM_FILES) { free(file); file = NULL; errno = ENOSPC; goto done; }
        node = &gp_ram_nodes[i];
        memcpy(node->name, name, strlen(name) + 1);
        node->created = node->accessed = node->modified = now;
    }
    if (flags & O_TRUNC) gp_ram_resize(node, 0);
    file->node = node;
    file->access = access;
    node->references++;
done:
    pthread_mutex_unlock(&gp_ram_lock);
    return file;
}

static void gp_ram_close(struct gp_ram_file *file)
{
    if (!file) return;
    pthread_mutex_lock(&gp_ram_lock);
    file->node->references--;
    pthread_mutex_unlock(&gp_ram_lock);
    free(file);
}

static int64_t gp_ram_read(struct gp_ram_file *file, void *buffer, uint32_t bytes,
    int positioned, uint64_t offset)
{
    uint32_t total = 0, count, in_page;
    uint64_t size;
    unsigned page;
    pthread_mutex_lock(&gp_ram_lock);
    if (file->access == O_WRONLY) { errno = EBADF; goto failed; }
    if (bytes && !buffer) { errno = EINVAL; goto failed; }
    if (!positioned) offset = file->position;
    size = file->node->size;
    if (offset >= size) bytes = 0;
    else if ((uint64_t)bytes > size - offset) bytes = (uint32_t)(size - offset);
    while (total < bytes)
    {
        page = (unsigned)(offset / GP_RAM_PAGE);
        in_page = (uint32_t)(offset % GP_RAM_PAGE);
        count = GP_RAM_PAGE - in_page;
        if (count > bytes - total) count = bytes - total;
        if (file->node->pages[page])
            memcpy((char *)buffer + total, file->node->pages[page] + in_page, count);
        else memset((char *)buffer + total, 0, count);
        offset += count; total += count;
    }
    if (!positioned) file->position = offset;
    pthread_mutex_unlock(&gp_ram_lock);
    return total;
failed:
    pthread_mutex_unlock(&gp_ram_lock);
    return -1;
}

static int64_t gp_ram_write(struct gp_ram_file *file, const void *buffer, uint32_t bytes,
    int positioned, uint64_t offset)
{
    struct gp_ram_node *node = file->node;
    unsigned char fresh[GP_RAM_PAGES] = {0};
    unsigned first, last, i, missing = 0;
    uint32_t total = 0, count, in_page;
    uint64_t end;
    pthread_mutex_lock(&gp_ram_lock);
    if (file->access == O_RDONLY) { errno = EBADF; goto failed; }
    if (node->read_only) { errno = EACCES; goto failed; }
    if (bytes && !buffer) { errno = EINVAL; goto failed; }
    if (!positioned) offset = file->position;
    if (!bytes) { pthread_mutex_unlock(&gp_ram_lock); return 0; }
    if (offset > GP_RAM_MAX || bytes > GP_RAM_MAX - offset) { errno = ENOSPC; goto failed; }
    end = offset + bytes;
    if (end > node->size && end > gp_ram_capacity - (gp_ram_logical - node->size))
    { errno = ENOSPC; goto failed; }
    first = (unsigned)(offset / GP_RAM_PAGE); last = (unsigned)((end - 1) / GP_RAM_PAGE);
    for (i = first; i <= last; i++) if (!node->pages[i]) missing++;
    if ((uint64_t)missing * GP_RAM_PAGE > gp_ram_capacity - gp_ram_allocated)
    { errno = ENOSPC; goto failed; }
    for (i = first; i <= last; i++)
        if (!node->pages[i])
        {
            node->pages[i] = (unsigned char *)calloc(1, GP_RAM_PAGE);
            if (!node->pages[i])
            {
                unsigned j;
                for (j = first; j < i; j++)
                    if (fresh[j]) { free(node->pages[j]); node->pages[j] = NULL; }
                errno = ENOMEM; goto failed;
            }
            fresh[i] = 1;
        }
    gp_ram_allocated += (uint64_t)missing * GP_RAM_PAGE;
    if (end > node->size) gp_ram_resize(node, end);
    while (total < bytes)
    {
        i = (unsigned)(offset / GP_RAM_PAGE); in_page = (uint32_t)(offset % GP_RAM_PAGE);
        count = GP_RAM_PAGE - in_page;
        if (count > bytes - total) count = bytes - total;
        memcpy(node->pages[i] + in_page, (const char *)buffer + total, count);
        total += count; offset += count;
    }
    if (!positioned) file->position = offset;
    pthread_mutex_unlock(&gp_ram_lock);
    return total;
failed:
    pthread_mutex_unlock(&gp_ram_lock);
    return -1;
}

static int gp_ram_seek(struct gp_ram_file *file, int64_t delta, int whence, uint64_t *position)
{
    uint64_t base, magnitude, next;
    pthread_mutex_lock(&gp_ram_lock);
    if (whence != SEEK_SET && whence != SEEK_CUR && whence != SEEK_END)
    { errno = EINVAL; goto failed; }
    base = whence == SEEK_SET ? 0 : whence == SEEK_CUR ? file->position : file->node->size;
    magnitude = delta < 0 ? (uint64_t)(-(delta + 1)) + 1 : (uint64_t)delta;
    if ((delta < 0 && magnitude > base) || (delta >= 0 && magnitude > INT64_MAX - base))
    { errno = EINVAL; goto failed; }
    next = delta < 0 ? base - magnitude : base + magnitude;
    file->position = next;
    if (position) *position = next;
    pthread_mutex_unlock(&gp_ram_lock);
    return 0;
failed:
    pthread_mutex_unlock(&gp_ram_lock);
    return -1;
}

static int gp_ram_truncate(struct gp_ram_file *file)
{
    int result;
    pthread_mutex_lock(&gp_ram_lock);
    if (file->access == O_RDONLY || file->node->read_only) { errno = EACCES; result = -1; }
    else result = gp_ram_resize(file->node, file->position);
    pthread_mutex_unlock(&gp_ram_lock);
    return result;
}

static void gp_ram_information(struct gp_ram_node *node, struct gp_ram_info *info)
{
    info->size = node->size; info->created = node->created;
    info->accessed = node->accessed; info->modified = node->modified;
    info->read_only = node->read_only;
}

static void gp_ram_fstat(struct gp_ram_file *file, struct gp_ram_info *info)
{
    pthread_mutex_lock(&gp_ram_lock);
    gp_ram_information(file->node, info);
    pthread_mutex_unlock(&gp_ram_lock);
}

static int gp_ram_stat(const char *path, struct gp_ram_info *info)
{
    char name[32];
    struct gp_ram_node *node;
    int result = -1;
    if (gp_ram_path(path, name) != 1) { errno = EINVAL; return -1; }
    pthread_mutex_lock(&gp_ram_lock);
    node = gp_ram_find(name);
    if (node) { gp_ram_information(node, info); result = 0; }
    else errno = ENOENT;
    pthread_mutex_unlock(&gp_ram_lock);
    return result;
}

static void gp_ram_times(struct gp_ram_file *file, const uint64_t *created,
    const uint64_t *accessed, const uint64_t *modified)
{
    pthread_mutex_lock(&gp_ram_lock);
    if (created) file->node->created = *created;
    if (accessed) file->node->accessed = *accessed;
    if (modified) file->node->modified = *modified;
    pthread_mutex_unlock(&gp_ram_lock);
}

static int gp_ram_readonly(const char *path, unsigned read_only)
{
    char name[32];
    struct gp_ram_node *node;
    int result = -1;
    if (gp_ram_path(path, name) != 1) { errno = EINVAL; return -1; }
    pthread_mutex_lock(&gp_ram_lock);
    node = gp_ram_find(name);
    if (node) { node->read_only = read_only != 0; result = 0; }
    else errno = ENOENT;
    pthread_mutex_unlock(&gp_ram_lock);
    return result;
}

static int gp_ram_unlink(const char *path)
{
    char name[32];
    struct gp_ram_node *node;
    int result = -1;
    if (gp_ram_path(path, name) != 1) { errno = EINVAL; return -1; }
    pthread_mutex_lock(&gp_ram_lock);
    node = gp_ram_find(name);
    if (!node) errno = ENOENT;
    else if (node->references) errno = EBUSY;
    else if (node->read_only) errno = EACCES;
    else { gp_ram_resize(node, 0); memset(node, 0, sizeof(*node)); result = 0; }
    pthread_mutex_unlock(&gp_ram_lock);
    return result;
}
#endif
