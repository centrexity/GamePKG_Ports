/* Maintainer original C oracle with bounded synthetic semaphores; never game code. */
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <assert.h>
#include <string.h>
typedef uint64_t HANDLE;typedef HANDLE* PHANDLE;typedef void* PXBOX_OBJECT_ATTRIBUTES;
typedef uint32_t ULONG;typedef uint8_t BOOLEAN;typedef int32_t BOOL;typedef int32_t LONG;typedef LONG* PLONG;typedef int32_t NTSTATUS;
#undef NULL
#define NULL 0
#undef __stdcall
#define __stdcall
#define TRUE 1
#define FALSE 0
#define XboxNotificationEvent 0
#define STATUS_INVALID_PARAMETER ((NTSTATUS)0xc000000d)
#define STATUS_INSUFFICIENT_RESOURCES ((NTSTATUS)0xc000009a)
#define STATUS_INVALID_HANDLE ((NTSTATUS)0xc0000008)
#define STATUS_SUCCESS 0
#define INVALID_HANDLE_VALUE UINT64_MAX
#define BRIDGE_HANDLE_TAG 0x48000000u
#define BRIDGE_HANDLE_MASK 0x00ffffffu
#define BRIDGE_HANDLE_MAX 16384
#define xbox_log(...) ((void)0)
#define fprintf(...) ((void)0)
#define GetLastError() 1
static HANDLE s_handle_table[BRIDGE_HANDLE_MAX];static int depth;
static void handle_lock(void){assert(depth==0);depth++;}static void handle_unlock(void){assert(depth==1);depth--;}
static struct {uint32_t live,count,maximum;} native_semaphores[128];
static uint32_t native_index(HANDLE h){return h>0x100000000ull && h<0x100000080ull ? (uint32_t)(h-0x100000000ull):0;}
static HANDLE CreateSemaphoreW(void*a,LONG initial,LONG maximum,void*b){(void)a;(void)b;if(initial<0||maximum<=0||initial>maximum)return 0;for(uint32_t i=1;i<128;i++)if(!native_semaphores[i].live){native_semaphores[i].live=1;native_semaphores[i].count=(uint32_t)initial;native_semaphores[i].maximum=(uint32_t)maximum;return 0x100000000ull+i;}return 0;}
static BOOL ReleaseSemaphore(HANDLE h,LONG permits,PLONG previous){uint32_t i=native_index(h);if(!i||!native_semaphores[i].live||permits<=0||(uint64_t)native_semaphores[i].count+(uint32_t)permits>native_semaphores[i].maximum)return 0;if(previous)*previous=(LONG)native_semaphores[i].count;native_semaphores[i].count+=(uint32_t)permits;return 1;}
static void native_close(HANDLE h){uint32_t i=native_index(h);assert(i&&native_semaphores[i].live);native_semaphores[i].live=0;}
static uint32_t guest[16],g_args[4],g_eax;
#define STACK_ARG(n) (g_args[n])
#define BRIDGE_MEM32(addr) (guest[(addr)/4])
static void* native_address(uint32_t addr){if(!addr)return NULL;if(addr<sizeof(guest))return (uint8_t*)guest+addr;return (void*)(uintptr_t)addr;}
#define XBOX_TO_NATIVE(addr) native_address(addr)
NTSTATUS __stdcall xbox_NtCreateSemaphore(
    PHANDLE SemaphoreHandle,
    PXBOX_OBJECT_ATTRIBUTES ObjectAttributes,
    LONG InitialCount,
    LONG MaximumCount)
{
    HANDLE hSemaphore;

    (void)ObjectAttributes;

    if (!SemaphoreHandle)
        return STATUS_INVALID_PARAMETER;

    hSemaphore = CreateSemaphoreW(NULL, InitialCount, MaximumCount, NULL);
    if (!hSemaphore) {
        xbox_log(XBOX_LOG_ERROR, XBOX_LOG_SYNC,
            "NtCreateSemaphore: CreateSemaphoreW failed (error %u)", GetLastError());
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    *SemaphoreHandle = hSemaphore;

    xbox_log(XBOX_LOG_DEBUG, XBOX_LOG_SYNC,
        "NtCreateSemaphore: handle=%p, initial=%d, max=%d",
        hSemaphore, InitialCount, MaximumCount);

    return STATUS_SUCCESS;
}

NTSTATUS __stdcall xbox_NtReleaseSemaphore(
    HANDLE SemaphoreHandle,
    LONG ReleaseCount,
    PLONG PreviousCount)
{
    if (!ReleaseSemaphore(SemaphoreHandle, ReleaseCount, PreviousCount)) {
        xbox_log(XBOX_LOG_ERROR, XBOX_LOG_SYNC,
            "NtReleaseSemaphore: failed for handle %p (error %u)",
            SemaphoreHandle, GetLastError());
        return STATUS_INVALID_HANDLE;
    }

    return STATUS_SUCCESS;
}

static uint32_t bridge_handle_token_ex(HANDLE h, int fresh)
{
    int i;
    if (!h || h == INVALID_HANDLE_VALUE) return 0;
    handle_lock();
    for (i = 1; !fresh && i < BRIDGE_HANDLE_MAX; i++)
        if (s_handle_table[i] == h) {
            handle_unlock();
            return BRIDGE_HANDLE_TAG | (uint32_t)i;
        }
    for (i = 1; i < BRIDGE_HANDLE_MAX; i++)
        if (s_handle_table[i] == NULL) {
            s_handle_table[i] = h;
            handle_unlock();
            return BRIDGE_HANDLE_TAG | (uint32_t)i;
        }
    handle_unlock();
    fprintf(stderr, "  [BRIDGE] handle table full\n");
    return 0;
}

static uint32_t bridge_handle_token(HANDLE h)
{
    return bridge_handle_token_ex(h, 0);
}

static HANDLE bridge_take_handle(uint32_t token)
{
    if ((token & 0xFF000000u) == BRIDGE_HANDLE_TAG) {
        uint32_t i = token & BRIDGE_HANDLE_MASK;
        if (i > 0 && i < BRIDGE_HANDLE_MAX) {
            HANDLE h;
            handle_lock();
            h = s_handle_table[i];
            s_handle_table[i] = NULL;
            handle_unlock();
            return h;
        }
    }
    return NULL;   /* untagged -> not a table handle, do not close */
}

static HANDLE bridge_untagged_handle(uint32_t token)
{
    if (token == 0xFFFFFFFFu || token == 0xFFFFFFFEu)
        return (HANDLE)(intptr_t)(int32_t)token;
    return (HANDLE)(uintptr_t)token;
}

static HANDLE bridge_resolve_handle(uint32_t token)
{
    if ((token & 0xFF000000u) == BRIDGE_HANDLE_TAG) {
        uint32_t i = token & BRIDGE_HANDLE_MASK;
        return (i > 0 && i < BRIDGE_HANDLE_MAX) ? s_handle_table[i] : NULL;
    }
    return bridge_untagged_handle(token);
}

static void bridge_write_handle(uint32_t handle_va, HANDLE h)
{
    if (handle_va)
        BRIDGE_MEM32(handle_va) = bridge_handle_token(h);
}

static void bridge_NtCreateSemaphore(void)
{
    uint32_t handle_va = STACK_ARG(0);
    HANDLE h = NULL;
    NTSTATUS st;

    st = xbox_NtCreateSemaphore(
        &h, XBOX_TO_NATIVE(STACK_ARG(1)),
        (LONG)STACK_ARG(2), (LONG)STACK_ARG(3));
    if (st >= 0 && handle_va)
        bridge_write_handle(handle_va, h);
    g_eax = (uint32_t)st;
}

static void bridge_NtReleaseSemaphore(void)
{
    uint32_t count_va = STACK_ARG(2);

    g_eax = (uint32_t)xbox_NtReleaseSemaphore(
        bridge_resolve_handle(STACK_ARG(0)), (LONG)STACK_ARG(1),
        count_va ? (PLONG)XBOX_TO_NATIVE(count_va) : NULL);
}

static void emit32(uint8_t*out,uint32_t*cursor,uint32_t v){for(uint32_t i=0;i<4;i++)out[*cursor+i]=(uint8_t)(v>>(i*8));*cursor+=4;}
uint32_t semaphore_reference(uint8_t*out){
 memset(s_handle_table,0,sizeof(s_handle_table));memset(native_semaphores,0,sizeof(native_semaphores));memset(guest,0,sizeof(guest));uint32_t cursor=0;
 const uint32_t maxima[]={1,2,4,0x7fffffffu};const uint32_t permits[]={0,1,2,0x7fffffffu,0x80000000u,0xffffffffu};
 for(uint32_t m=0;m<4;m++){uint32_t initials[]={0,1,maxima[m]/2,maxima[m]};for(uint32_t n=0;n<4;n++){
  guest[1]=0x33333333;guest[2]=0xabcdef01;g_args[0]=4;g_args[1]=0xffffffff;g_args[2]=initials[n];g_args[3]=maxima[m];bridge_NtCreateSemaphore();assert(g_eax==0&&guest[1]==0x48000001&&guest[2]==0xabcdef01);
  HANDLE h=bridge_resolve_handle(guest[1]);uint32_t i=native_index(h);emit32(out,&cursor,guest[1]);emit32(out,&cursor,native_semaphores[i].count);emit32(out,&cursor,native_semaphores[i].maximum);
  for(uint32_t p=0;p<6;p++){guest[3]=0x55555555;g_args[0]=guest[1];g_args[1]=permits[p];g_args[2]=12;bridge_NtReleaseSemaphore();assert(g_eax==0||g_eax==0xc0000008);emit32(out,&cursor,g_eax==0);emit32(out,&cursor,guest[3]);emit32(out,&cursor,native_semaphores[i].count);}
  native_close(bridge_take_handle(guest[1]));
 }}assert(cursor==1344);return cursor;
}
