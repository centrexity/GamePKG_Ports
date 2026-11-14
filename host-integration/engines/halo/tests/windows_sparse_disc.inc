#ifdef _WIN32
#include <windows.h>
#include <winioctl.h>
static int make_sparse_image(const char *small_path,const char *large_path) {
    HANDLE input=CreateFileA(small_path,GENERIC_READ,FILE_SHARE_READ,NULL,OPEN_EXISTING,FILE_ATTRIBUTE_NORMAL,NULL);
    assert(input!=INVALID_HANDLE_VALUE);
    HANDLE output=CreateFileA(large_path,GENERIC_READ|GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
    assert(output!=INVALID_HANDLE_VALUE);DWORD done;
    assert(DeviceIoControl(output,FSCTL_SET_SPARSE,NULL,0,NULL,0,&done,NULL));
    unsigned char bytes[131072];DWORD count;
    assert(ReadFile(input,bytes,sizeof bytes,&count,NULL));assert(count>44*2048&&count<sizeof bytes);
    assert(WriteFile(output,bytes,count,&done,NULL)&&done==count);
    LARGE_INTEGER at;at.QuadPart=42*2048+4;assert(SetFilePointerEx(output,at,NULL,FILE_BEGIN));
    unsigned sector=1200000;unsigned char encoded[4]={(unsigned char)sector,(unsigned char)(sector>>8),(unsigned char)(sector>>16),(unsigned char)(sector>>24)};
    assert(WriteFile(output,encoded,4,&done,NULL)&&done==4);
    at.QuadPart=(LONGLONG)sector*2048;assert(SetFilePointerEx(output,at,NULL,FILE_BEGIN));
    assert(WriteFile(output,"0123456789",10,&done,NULL)&&done==10);
    assert(CloseHandle(input)&&CloseHandle(output));puts("PASS created sparse synthetic ISO beyond2GiB, no retail content");return 0;
}
#endif
