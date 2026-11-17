/* Maintainer original bridge table, synthetic scalar HANDLEs, never game code. */
#include <stdint.h>
#include <stdio.h>
#include <assert.h>
typedef uint64_t HANDLE;
#undef NULL
#define NULL 0
#define INVALID_HANDLE_VALUE UINT64_MAX
#define BRIDGE_HANDLE_TAG 0x48000000u
#define BRIDGE_HANDLE_MASK 0x00ffffffu
#define BRIDGE_HANDLE_MAX 16384
#define fprintf(...) ((void)0)
static HANDLE s_handle_table[BRIDGE_HANDLE_MAX];
static int depth;
static void handle_lock(void){assert(depth==0);depth++;}
static void handle_unlock(void){assert(depth==1);depth--;}
static HANDLE bridge_untagged_handle(uint32_t token){assert(!"raw native handle fallback forbidden in fixture");return token;}
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
static HANDLE bridge_resolve_handle(uint32_t token)
{
    if ((token & 0xFF000000u) == BRIDGE_HANDLE_TAG) {
        uint32_t i = token & BRIDGE_HANDLE_MASK;
        return (i > 0 && i < BRIDGE_HANDLE_MAX) ? s_handle_table[i] : NULL;
    }
    return bridge_untagged_handle(token);
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
static void put64(uint8_t*p,uint64_t v){for(int i=0;i<8;i++)p[i]=(uint8_t)(v>>(8*i));}
uint32_t handle_reference(uint32_t index,const uint64_t*args,uint8_t*out){
 if(index==0){for(uint32_t i=0;i<16383;i++)assert(bridge_handle_token_ex(UINT64_C(0x100000000)+i,0)==(0x48000001u+i));}
 uint64_t result=args[0]==0?bridge_handle_token_ex(args[1],(int)args[2]):args[0]==1?bridge_resolve_handle((uint32_t)args[1]):bridge_take_handle((uint32_t)args[1]);
 put64(out,result);put64(out+8,bridge_resolve_handle(0x48000001u+index%16383));uint32_t size=16;
 if(index==511){for(uint32_t i=1;i<16384;i++){put64(out+size,s_handle_table[i]);size+=8;}}
 return size;
}
