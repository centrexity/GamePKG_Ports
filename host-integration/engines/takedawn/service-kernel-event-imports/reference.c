/* Maintainer original C oracle with bounded synthetic events; never game code. */
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
static struct {uint32_t live,manual,signaled;} native_events[127];
static HANDLE CreateEventW(void*a,BOOL manual,BOOL signaled,void*b){(void)a;(void)b;for(uint32_t i=1;i<=126;i++)if(!native_events[i].live){native_events[i].live=1;native_events[i].manual=manual!=0;native_events[i].signaled=signaled!=0;return i;}return 0;}
static BOOL SetEvent(HANDLE h){if(h==0||h>126||!native_events[h].live)return 0;native_events[h].signaled=1;return 1;}
static void native_close(HANDLE h){assert(h>0&&h<=126&&native_events[h].live);native_events[h].live=0;}
static uint32_t guest[16],g_args[4],g_eax;
#define STACK_ARG(n) (g_args[n])
#define BRIDGE_MEM32(addr) (guest[(addr)/4])
static void* native_address(uint32_t addr){if(!addr)return NULL;if(addr<sizeof(guest))return (uint8_t*)guest+addr;return (void*)(uintptr_t)addr;}
#define XBOX_TO_NATIVE(addr) native_address(addr)
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

NTSTATUS __stdcall xbox_NtCreateEvent(
    PHANDLE EventHandle,
    PXBOX_OBJECT_ATTRIBUTES ObjectAttributes,
    ULONG EventType,
    BOOLEAN InitialState)
{
    HANDLE hEvent;
    BOOL bManualReset;

    (void)ObjectAttributes;

    if (!EventHandle)
        return STATUS_INVALID_PARAMETER;

    /* XboxNotificationEvent = manual-reset, XboxSynchronizationEvent = auto-reset */
    bManualReset = (EventType == XboxNotificationEvent) ? TRUE : FALSE;

    hEvent = CreateEventW(NULL, bManualReset, InitialState, NULL);
    if (!hEvent) {
        xbox_log(XBOX_LOG_ERROR, XBOX_LOG_SYNC,
            "NtCreateEvent: CreateEventW failed (error %u)", GetLastError());
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    *EventHandle = hEvent;

    xbox_log(XBOX_LOG_DEBUG, XBOX_LOG_SYNC,
        "NtCreateEvent: handle=%p, type=%s, initial=%d",
        hEvent, bManualReset ? "notification" : "synchronization", InitialState);

    return STATUS_SUCCESS;
}

NTSTATUS __stdcall xbox_NtSetEvent(HANDLE EventHandle, PLONG PreviousState)
{
    if (PreviousState)
        *PreviousState = 0; /* We don't track previous state */

    if (!SetEvent(EventHandle)) {
        xbox_log(XBOX_LOG_ERROR, XBOX_LOG_SYNC,
            "NtSetEvent: SetEvent failed for handle %p (error %u)",
            EventHandle, GetLastError());
        return STATUS_INVALID_HANDLE;
    }

    return STATUS_SUCCESS;
}

static void bridge_NtCreateEvent(void)
{
    uint32_t handle_ptr = STACK_ARG(0);
    uint32_t obj_attr_ptr = STACK_ARG(1);
    uint32_t event_type = STACK_ARG(2);
    uint32_t initial_state = STACK_ARG(3);

    /* Use local HANDLE to avoid 8-byte write to 4-byte Xbox memory slot.
     * On x64, HANDLE is 8 bytes but Xbox expects 4-byte handles. */
    HANDLE local_handle = NULL;
    NTSTATUS status = xbox_NtCreateEvent(
        &local_handle,
        XBOX_TO_NATIVE(obj_attr_ptr),
        event_type, initial_state);

    if (handle_ptr) {
        bridge_write_handle(handle_ptr, local_handle);
    }

    fprintf(stderr, "  [BRIDGE] NtCreateEvent: handle_ptr=0x%08X type=%u init=%u → status=0x%08X handle=0x%08X\n",
            handle_ptr, event_type, initial_state, (uint32_t)status,
            (uint32_t)(uintptr_t)local_handle);

    g_eax = (uint32_t)status;
}

static void bridge_NtSetEvent(void)
{
    HANDLE   handle = bridge_resolve_handle(STACK_ARG(0));
    uint32_t prev   = STACK_ARG(1);
    g_eax = (uint32_t)xbox_NtSetEvent(handle, XBOX_TO_NATIVE(prev));
}

static void emit32(uint8_t*out,uint32_t*cursor,uint32_t v){for(uint32_t i=0;i<4;i++)out[*cursor+i]=(uint8_t)(v>>(i*8));*cursor+=4;}
uint32_t event_reference(uint8_t*out){
 memset(s_handle_table,0,sizeof(s_handle_table));memset(native_events,0,sizeof(native_events));memset(guest,0,sizeof(guest));uint32_t cursor=0;
 const uint32_t types[]={0,1,2,0xffffffffu};const uint32_t initials[]={0,1,0x100,0x101,0xffffff00u,0xffffffffu};
 for(uint32_t t=0;t<4;t++)for(uint32_t s=0;s<6;s++){
  guest[1]=0x33333333;guest[2]=0xabcdef01;g_args[0]=4;g_args[1]=0xffffffff;g_args[2]=types[t];g_args[3]=initials[s];bridge_NtCreateEvent();assert(g_eax==0&&guest[1]==0x48000001&&guest[2]==0xabcdef01);
  HANDLE h=bridge_resolve_handle(guest[1]);uint32_t manual=native_events[h].manual,signal=native_events[h].signaled;
  guest[3]=0x55555555;g_args[0]=guest[1];g_args[1]=12;bridge_NtSetEvent();assert(g_eax==0&&guest[3]==0);
  uint32_t values[]={0,guest[1],manual,signal,guest[3],g_eax,native_events[h].signaled};for(uint32_t i=0;i<7;i++)emit32(out,&cursor,values[i]);native_close(bridge_take_handle(guest[1]));
 }
 const uint32_t invalid[]={0,0xffffffffu,0xfffffffeu,0xdead0001,0xbeef0010,0x48000000,0x48004000,0x48003fff,0x49000001};
 for(uint32_t i=0;i<9;i++){guest[3]=0x55555555;g_args[0]=invalid[i];g_args[1]=12;bridge_NtSetEvent();assert(g_eax==0xc0000008&&guest[3]==0);emit32(out,&cursor,g_eax);emit32(out,&cursor,guest[3]);}
 g_args[0]=0;g_args[1]=0xffffffff;g_args[2]=1;g_args[3]=0x100;bridge_NtCreateEvent();assert(g_eax==0);emit32(out,&cursor,g_eax);
 g_args[0]=4;g_args[1]=0xffffffff;g_args[2]=0;g_args[3]=1;bridge_NtCreateEvent();assert(g_eax==0&&guest[1]==0x48000001);emit32(out,&cursor,g_eax);emit32(out,&cursor,guest[1]);native_close(bridge_take_handle(guest[1]));
 for(uint32_t i=0;i<126;i++){guest[1]=0x33333333;g_args[0]=4;g_args[1]=0xffffffff;g_args[2]=0;g_args[3]=0;bridge_NtCreateEvent();assert(g_eax==(i<125?0:0xc000009a)&&guest[1]==(i<125?0x48000001+i:0));emit32(out,&cursor,g_eax);emit32(out,&cursor,guest[1]);}
 assert(cursor==1764);return cursor;
}
