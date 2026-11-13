/*
 * MIT License — Copyright (c) 2026 LovelessCodes
 *
 * webOS ships glibc 2.12, which predates getauxval(). Rust's standard
 * library and ring call it for HWCAP and stack-size detection, so an ARM
 * build fails to link without this fallback. Read the auxiliary vector the
 * same way glibc does, from /proc/self/auxv, and keep the result cached.
 */
#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <unistd.h>

struct auxiliary_entry {
    unsigned long key;
    unsigned long value;
};

#define MAX_ENTRIES 128

static struct auxiliary_entry entries[MAX_ENTRIES];
static size_t entry_count;
static int load_error;
static int loaded;

static void load_auxiliary_vector(void)
{
    int fd = open("/proc/self/auxv", O_RDONLY | O_CLOEXEC);
    if (fd < 0) {
        load_error = errno;
        loaded = 1;
        return;
    }

    unsigned char *bytes = (unsigned char *)entries;
    size_t total = sizeof(entries);
    size_t offset = 0;
    while (offset < total) {
        ssize_t count = read(fd, bytes + offset, total - offset);
        if (count < 0) {
            if (errno == EINTR) {
                continue;
            }
            load_error = errno;
            break;
        }
        if (count == 0) {
            break;
        }
        offset += (size_t)count;
    }
    close(fd);
    entry_count = offset / sizeof(entries[0]);
    loaded = 1;
}

unsigned long getauxval(unsigned long key)
{
    if (!loaded) {
        load_auxiliary_vector();
    }
    if (load_error != 0) {
        errno = load_error;
        return 0;
    }
    for (size_t index = 0; index < entry_count; index++) {
        if (entries[index].key == key) {
            return entries[index].value;
        }
        if (entries[index].key == 0) {
            break;
        }
    }
    errno = ENOENT;
    return 0;
}
