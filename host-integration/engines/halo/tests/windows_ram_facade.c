/* Maintainer fixture: actual XDK-facing file and handle code, synthetic data only. */
#include "platform.h"
#include "port_config.h"
#include <assert.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
const char *config_string(const char *name) { (void)name; return ""; }
BOOL platform_offer_game_data(const char *directory) { (void)directory; return FALSE; }
BOOL platform_is_contiguous(const void *address) { (void)address; return FALSE; }
void memory_watch_prepare_write(void *address, unsigned long bytes) { (void)address; (void)bytes; }
int crash_reporter_process(void) { return 0; }
static int completed;
static void CALLBACK completion(DWORD error, DWORD bytes, LPOVERLAPPED request)
{
    assert(error == ERROR_SUCCESS && bytes == 10 && request->InternalHigh == 10);
    completed++;
}
int main(void)
{
    HANDLE a, b;
    DWORD bytes, high;
    OVERLAPPED at;
    FILETIME stamp = {1234, 5678}, creation;
    WIN32_FILE_ATTRIBUTE_DATA info;
    char data[32] = {0};
    assert(_putenv_s("GAMEPKG_RAM_CACHE_BYTES", "1048576") == 0);
    a = CreateFileA("q:\\cache000.map", GENERIC_READ | GENERIC_WRITE, 0, NULL,
        OPEN_ALWAYS, FILE_FLAG_OVERLAPPED, NULL);
    assert(a != INVALID_HANDLE_VALUE && GetLastError() == ERROR_SUCCESS);
    b = CreateFileA("Q:/CACHE000.MAP", GENERIC_READ, 0, NULL, OPEN_EXISTING, 0, NULL);
    assert(b != INVALID_HANDLE_VALUE);
    assert(WriteFile(a, "original cache", 14, &bytes, NULL) && bytes == 14);
    assert(ReadFile(b, data, 32, &bytes, NULL) && bytes == 14 && !memcmp(data, "original cache", 14));
    assert(!WriteFile(b, "x", 1, &bytes, NULL) && bytes == 0);
    memset(&at, 0, sizeof(at)); at.Offset = 65534;
    assert(WriteFile(a, "cross-page", 10, &bytes, &at) && bytes == 10 && at.InternalHigh == 10);
    memset(data, 0, sizeof(data));
    assert(ReadFile(b, data, 10, &bytes, &at) && bytes == 10 && !memcmp(data, "cross-page", 10));
    memset(&at, 0, sizeof(at)); at.Offset = 65534;
    assert(WriteFileEx(a, "cross-page", 10, &at, completion));
    assert(completed == 0);
    SleepEx(0, TRUE); assert(completed == 1);
    memset(data, 0, sizeof(data));
    assert(ReadFileEx(b, data, 10, &at, completion));
    assert(completed == 1);
    SleepEx(0, TRUE); assert(completed == 2 && !memcmp(data, "cross-page", 10));
    assert(SetFilePointer(a, 65536, NULL, FILE_BEGIN) == 65536 && SetEndOfFile(a));
    assert(SetFilePointer(a, 65544, NULL, FILE_BEGIN) == 65544 && SetEndOfFile(a));
    at.Offset = 65536; memset(data, 1, sizeof(data));
    assert(ReadFile(b, data, 8, &bytes, &at) && bytes == 8);
    for (unsigned i = 0; i < 8; i++) assert(data[i] == 0);
    assert(GetFileSize(a, &high) == 65544 && high == 0);
    assert(SetFileTime(a, &stamp, NULL, NULL));
    assert(GetFileTime(b, &creation, NULL, NULL));
    assert(creation.dwLowDateTime == stamp.dwLowDateTime && creation.dwHighDateTime == stamp.dwHighDateTime);
    assert(GetFileAttributesExA("q:\\cache000.map", GetFileExInfoStandard, &info));
    assert(info.nFileSizeLow == 65544 && info.ftCreationTime.dwLowDateTime == 1234);
    assert(SetFileAttributesA("q:\\cache000.map", FILE_ATTRIBUTE_READONLY));
    assert(GetFileAttributesA("q:\\cache000.map") == FILE_ATTRIBUTE_READONLY);
    assert(!WriteFile(a, "x", 1, &bytes, NULL));
    assert(SetFileAttributesA("q:\\cache000.map", FILE_ATTRIBUTE_NORMAL));
    assert(!CopyFileA("q:\\cache000.map", "z:\\must-not-copy.map", FALSE));
    assert(!MoveFileA("q:\\cache000.map", "z:\\must-not-move.map"));
    assert(!DeleteFileA("q:\\cache000.map"));
    assert(CloseHandle(b) && CloseHandle(a));
    assert(DeleteFileA("q:\\cache000.map"));
    assert(GetFileAttributesA("q:\\cache000.map") == (DWORD)-1);
    assert(CreateFileA("q:\\..\\cache000.map", GENERIC_READ | GENERIC_WRITE, 0, NULL, CREATE_ALWAYS, 0, NULL) == INVALID_HANDLE_VALUE);
    puts("PASS actual Windows engine file facade: memory-only cache, handles, positioned I/O, truncation, dates, attributes, permissions and copy/move denial");
    return 0;
}
