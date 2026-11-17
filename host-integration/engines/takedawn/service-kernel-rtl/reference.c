/* Maintainer original-source witness. Never compiled into game objects. */
#include <stdint.h>
#include <stddef.h>
#include <assert.h>
#include <string.h>
#ifndef _WIN32
#include <strings.h>
#define _strnicmp strncasecmp
#endif
typedef char* PCHAR;typedef uint8_t BOOLEAN;typedef struct {uint16_t Length,MaximumLength;char*Buffer;} XBOX_ANSI_STRING,*PXBOX_ANSI_STRING;
#undef __stdcall
#define __stdcall
#define TRUE 1
#define FALSE 0
static uint8_t guest[262144];static uint32_t g_args[3],g_eax;
#define STACK_ARG(n) (g_args[n])
#define BRIDGE_MEM16(a) (*(uint16_t*)(guest+(a)))
#define BRIDGE_MEM32(a) (*(uint32_t*)(guest+(a)))
#define XBOX_TO_NATIVE(a) ((a)?(void*)(guest+(a)):NULL)
BOOLEAN __stdcall xbox_RtlEqualString(
    PXBOX_ANSI_STRING String1,
    PXBOX_ANSI_STRING String2,
    BOOLEAN CaseInSensitive)
{
    if (String1->Length != String2->Length)
        return FALSE;

    if (CaseInSensitive)
        return _strnicmp(String1->Buffer, String2->Buffer, String1->Length) == 0;
    else
        return strncmp(String1->Buffer, String2->Buffer, String1->Length) == 0;
}
static void bridge_RtlCompareMemoryUlong(void)
{
    uint32_t base_va = STACK_ARG(0);
    uint32_t length  = STACK_ARG(1);
    uint32_t pattern = STACK_ARG(2);
    uint32_t i;

    if (!base_va) {
        g_eax = 0;
        return;
    }
    length &= ~3u;                      /* whole ULONGs only */
    for (i = 0; i < length; i += 4) {
        if (BRIDGE_MEM32(base_va + i) != pattern)
            break;
    }
    g_eax = i;
}
static void bridge_RtlEqualString(void)
{
    uint32_t s1_va  = STACK_ARG(0);
    uint32_t s2_va  = STACK_ARG(1);
    uint32_t nocase = STACK_ARG(2);
    XBOX_ANSI_STRING a, b;

    if (!s1_va || !s2_va) {
        g_eax = 0;
        return;
    }
    a.Length        = BRIDGE_MEM16(s1_va + 0);
    a.MaximumLength = BRIDGE_MEM16(s1_va + 2);
    a.Buffer        = (PCHAR)XBOX_TO_NATIVE(BRIDGE_MEM32(s1_va + 4));
    b.Length        = BRIDGE_MEM16(s2_va + 0);
    b.MaximumLength = BRIDGE_MEM16(s2_va + 2);
    b.Buffer        = (PCHAR)XBOX_TO_NATIVE(BRIDGE_MEM32(s2_va + 4));

    if (!a.Buffer || !b.Buffer) {
        g_eax = 0;
        return;
    }
    g_eax = xbox_RtlEqualString(&a, &b, (BOOLEAN)nocase) ? 1 : 0;
}
static void bridge_RtlInitAnsiString(void)
{
    uint32_t dest_va = STACK_ARG(0);
    uint32_t src_va  = STACK_ARG(1);

    if (!dest_va) {
        g_eax = 0;
        return;
    }
    if (src_va) {
        const char *src = (const char *)XBOX_TO_NATIVE(src_va);
        size_t len = strlen(src);
        if (len > 0xFFFE) {
            len = 0xFFFE;
        }
        BRIDGE_MEM16(dest_va + 0) = (uint16_t)len;
        BRIDGE_MEM16(dest_va + 2) = (uint16_t)(len + 1);
        BRIDGE_MEM32(dest_va + 4) = src_va;
    } else {
        BRIDGE_MEM16(dest_va + 0) = 0;
        BRIDGE_MEM16(dest_va + 2) = 0;
        BRIDGE_MEM32(dest_va + 4) = 0;
    }
    g_eax = 0;
}
static void bridge_RtlInitUnicodeString(void)
{
    uint32_t dest_va = STACK_ARG(0);
    uint32_t src_va  = STACK_ARG(1);

    if (!dest_va) {
        g_eax = 0;
        return;
    }
    if (src_va) {
        const uint16_t *wp = (const uint16_t *)XBOX_TO_NATIVE(src_va);
        size_t bytes = 0;

        while (wp[bytes / 2])
            bytes += sizeof(uint16_t);
        if (bytes > 0xFFFE)
            bytes = 0xFFFE;
        BRIDGE_MEM16(dest_va + 0) = (uint16_t)bytes;
        BRIDGE_MEM16(dest_va + 2) = (uint16_t)(bytes + sizeof(uint16_t));
        BRIDGE_MEM32(dest_va + 4) = src_va;
    } else {
        BRIDGE_MEM16(dest_va + 0) = 0;
        BRIDGE_MEM16(dest_va + 2) = 0;
        BRIDGE_MEM32(dest_va + 4) = 0;
    }
    g_eax = 0;
}
static void bridge_RtlNtStatusToDosError(void)
{
    uint32_t status = STACK_ARG(0);

    /* Simple mapping of common status codes */
    switch (status) {
    case 0x00000000: g_eax = 0; break;          /* STATUS_SUCCESS → ERROR_SUCCESS */
    case 0xC0000034: g_eax = 2; break;          /* STATUS_OBJECT_NAME_NOT_FOUND → ERROR_FILE_NOT_FOUND */
    case 0xC000003A: g_eax = 3; break;          /* STATUS_OBJECT_PATH_NOT_FOUND → ERROR_PATH_NOT_FOUND */
    case 0xC0000022: g_eax = 5; break;          /* STATUS_ACCESS_DENIED → ERROR_ACCESS_DENIED */
    case 0xC0000008: g_eax = 6; break;          /* STATUS_INVALID_HANDLE → ERROR_INVALID_HANDLE */
    case 0xC0000017: g_eax = 8; break;          /* STATUS_NO_MEMORY → ERROR_NOT_ENOUGH_MEMORY */
    case 0xC000000D: g_eax = 87; break;         /* STATUS_INVALID_PARAMETER → ERROR_INVALID_PARAMETER */
    /* The informational and warning codes, which are not failures and must
     * not fall through to the generic answer.
     *
     * 317 is ERROR_MR_MID_NOT_FOUND -- "there is no message text for this
     * number" -- and as a default for a status nobody has mapped yet it is
     * honest. As an answer to "is this request still in flight?" it is not:
     * a caller comparing against ERROR_IO_PENDING gets "no" and takes the
     * branch for a request that never started.
     *
     * Shin Megami Tensei: Nine does exactly that. Its resource loader issues
     * a read, sees the call fail, asks for the error, and marks the object as
     * loading only when the answer is ERROR_IO_PENDING. With 317 the object
     * stayed idle, the poll that finishes the load returned "not started" on
     * every frame, and the title sat in its first boot state forever with
     * everything else working. */
    case 0x00000103: g_eax = 997; break;        /* STATUS_PENDING → ERROR_IO_PENDING */
    case 0x00000102: g_eax = 1460; break;       /* STATUS_TIMEOUT → ERROR_TIMEOUT */
    case 0x00000104: g_eax = 0; break;          /* STATUS_REPARSE → ERROR_SUCCESS */
    case 0x80000005: g_eax = 234; break;        /* STATUS_BUFFER_OVERFLOW → ERROR_MORE_DATA */
    case 0x80000006: g_eax = 18; break;         /* STATUS_NO_MORE_FILES → ERROR_NO_MORE_FILES */
    case 0xC0000011: g_eax = 38; break;         /* STATUS_END_OF_FILE → ERROR_HANDLE_EOF */
    case 0xC0000023: g_eax = 122; break;        /* STATUS_BUFFER_TOO_SMALL → ERROR_INSUFFICIENT_BUFFER */
    case 0xC0000035: g_eax = 183; break;        /* STATUS_OBJECT_NAME_COLLISION → ERROR_ALREADY_EXISTS */
    case 0xC00000BB: g_eax = 50; break;         /* STATUS_NOT_SUPPORTED → ERROR_NOT_SUPPORTED */

    default:         g_eax = 317; break;         /* ERROR_MR_MID_NOT_FOUND (generic) */
    }
}
static void emit(uint8_t*out,uint32_t*at,uint32_t v){for(unsigned n=0;n<4;n++)out[(*at)++]=(uint8_t)(v>>(n*8));}
uint32_t rtl_reference(uint8_t*out){
 uint32_t at=0;const uint32_t lengths[]={0,1,2,3,4,5,7,8,9,15,16,31,32,63,64,255,256,257,511};
 for(uint32_t n=0;n<19;n++)for(uint32_t mode=0;mode<4;mode++){
  const uint32_t pattern=0x91abcdef;for(uint32_t i=0;i<512;i+=4)BRIDGE_MEM32(0x14001+i)=pattern;
  if(mode==1)BRIDGE_MEM32(0x14001)=pattern^1;
  if(mode==2&&lengths[n]>=4)BRIDGE_MEM32(0x14001+(lengths[n]&~3u)-4)=pattern^1;
  g_args[0]=mode==3?0:0x14001;g_args[1]=lengths[n];g_args[2]=pattern;bridge_RtlCompareMemoryUlong();emit(out,&at,g_eax);
 }
 const uint32_t sizes[]={0,1,2,3,7,31,255,256,257,512};const uint32_t flags[]={0,1,0x100,0x101,0xffffffff};
 for(uint32_t n=0;n<10;n++)for(uint32_t f=0;f<5;f++)for(uint32_t mode=0;mode<5;mode++){
  uint32_t len=sizes[n];for(uint32_t i=0;i<len;i++){guest[0x14000+i]=(uint8_t)(65+i%26);guest[0x16000+i]=guest[0x14000+i];}
  if(len&&mode==1)for(uint32_t i=0;i<len;i++)guest[0x16000+i]+=32;
  if(len&&mode==2)guest[0x16000+len-1]^=0x80;
  if(mode==3&&len){guest[0x14000+len/2]=0;guest[0x16000+len/2]=0;if(len-1>len/2)guest[0x16000+len-1]^=0x80;}
  if(mode==4&&len){guest[0x14000]=0xc0;guest[0x16000]=0xe0;}
  for(uint32_t i=0;i<2;i++){BRIDGE_MEM16(0x13000+i*16)=(uint16_t)len;BRIDGE_MEM16(0x13002+i*16)=0;BRIDGE_MEM32(0x13004+i*16)=0x14000+i*0x2000;}
  g_args[0]=0x13000;g_args[1]=0x13010;g_args[2]=flags[f];bridge_RtlEqualString();emit(out,&at,g_eax);
 }
 const uint32_t sizes2[]={0,1,2,255,256,257,32767,65533,65534,65536};
 for(uint32_t wide=0;wide<2;wide++)for(uint32_t n=0;n<10;n++)for(uint32_t mode=0;mode<3;mode++){
  uint32_t len=sizes2[n];for(uint32_t i=0;i<len;i++){if(wide)BRIDGE_MEM16(0x18000+i*2)=(uint16_t)(65+i%26);else guest[0x18000+i]=(uint8_t)(65+i%26);}
  if(wide)BRIDGE_MEM16(0x18000+len*2)=0;else guest[0x18000+len]=0;
  memset(guest+0x12ffc,0xcc,16);g_args[0]=mode==2?0:0x13000;g_args[1]=mode==1?0:0x18000;
  if(wide)bridge_RtlInitUnicodeString();else bridge_RtlInitAnsiString();emit(out,&at,g_eax);
  memcpy(out+at,guest+0x12ffc,16);at+=16;
 }
 const uint32_t statuses[]={0,0x104,0xc0000034,0xc000003a,0xc0000022,0xc0000008,0xc0000017,0xc000000d,0x103,0x102,0x80000005,0x80000006,0xc0000011,0xc0000023,0xc0000035,0xc00000bb,0xffffffff,0xc000009a,0xc0000001,1,0x80000000,0x7fffffff,0x101,0x105,0xc0000020,0xc0000033,0xc0000012,0x80000007};
 for(uint32_t i=0;i<92;i++){g_args[0]=i<28?statuses[i]:0x91234567u*(i-27);bridge_RtlNtStatusToDosError();emit(out,&at,g_eax);}
 assert(at==2872);return at;
}
