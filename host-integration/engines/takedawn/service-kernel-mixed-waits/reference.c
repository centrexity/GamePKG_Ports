/* Maintainer C witness; never a game unit or user requirement. */
#include <stdint.h>
#include <stddef.h>
#ifdef _WIN32
#include <windows.h>
#else
typedef uint32_t DWORD;
typedef int BOOL;
typedef struct {int type;int manual;int count;} mixed_object;
typedef mixed_object *HANDLE;
#define INFINITE UINT32_MAX
#define WAIT_TIMEOUT 258u
#define WAIT_FAILED UINT32_MAX
static DWORD WaitForMultipleObjectsEx(DWORD count,const HANDLE *objects,BOOL all,DWORD ms,BOOL alertable){
 (void)ms;(void)alertable;if(!count||count>64)return WAIT_FAILED;
 for(DWORD i=0;i<count;i++){if(!objects[i])return WAIT_FAILED;for(DWORD j=0;j<i;j++)if(objects[i]==objects[j])return WAIT_FAILED;}
 DWORD selected=WAIT_FAILED;for(DWORD i=0;i<count;i++){if(!objects[i]->count){if(all)return WAIT_TIMEOUT;}else if(selected==WAIT_FAILED)selected=i;}
 if(selected==WAIT_FAILED)return WAIT_TIMEOUT;
 if(all){for(DWORD i=0;i<count;i++)if(objects[i]->type==3||!objects[i]->manual)objects[i]->count--;return 0;}
 if(objects[selected]->type==3||!objects[selected]->manual){objects[selected]->count--;}
 return selected;
}
static DWORD WaitForSingleObjectEx(HANDLE object,DWORD ms,BOOL alertable){return WaitForMultipleObjectsEx(1,&object,0,ms,alertable);}
#endif
static int xbox_TimeWarpOn(void){return 0;}
static uint64_t xbox_GuestNowNs(void){return 0;}
static DWORD wait_many(DWORD count, const HANDLE *h, BOOL all, DWORD ms, BOOL alertable)
{
    uint64_t deadline;

    if (!xbox_TimeWarpOn() || ms == 0 || ms == INFINITE)
        return WaitForMultipleObjectsEx(count, h, all, ms, alertable);
    deadline = xbox_GuestNowNs() + (uint64_t)ms * 1000000u;
    for (;;) {
        DWORD r = WaitForMultipleObjectsEx(count, h, all, 1, alertable);
        if (r != WAIT_TIMEOUT || xbox_GuestNowNs() >= deadline)
            return r;
    }
}
static void mixed_u32(uint8_t*out,size_t*at,uint32_t value){for(unsigned i=0;i<4;i++)out[(*at)++]=(uint8_t)(value>>(8*i));}
size_t mixed_reference(uint8_t*out,size_t capacity){
 if(capacity<2560){return 0;}
 size_t at=0;
 for(unsigned code=0;code<128;code++){
  HANDLE objects[4];int manual=(code&16)==0;
#ifdef _WIN32
  objects[0]=CreateEventW(NULL,manual,code&1,NULL);objects[1]=CreateEventW(NULL,!manual,(code>>1)&1,NULL);
  objects[2]=CreateSemaphoreW(NULL,code&4?2:0,3,NULL);objects[3]=CreateSemaphoreW(NULL,(code>>3)&1,2,NULL);
#else
  mixed_object storage[4]={{1,manual,(int)(code&1)},{1,!manual,(int)((code>>1)&1)},{3,0,code&4?2:0},{3,0,(int)((code>>3)&1)}};
  for(unsigned i=0;i<4;i++)objects[i]=&storage[i];
#endif
  for(unsigned i=0;i<4;i++)if(!objects[i])return 0;
  HANDLE ordered[4];if(!(code&64)){ordered[0]=objects[0];ordered[1]=objects[2];ordered[2]=objects[1];ordered[3]=objects[3];}else{ordered[0]=objects[3];ordered[1]=objects[1];ordered[2]=objects[2];ordered[3]=objects[0];}
  DWORD result=wait_many(4,ordered,(code&32)!=0,0,0);
  mixed_u32(out,&at,result==WAIT_TIMEOUT?UINT32_MAX-1:result<4?result:UINT32_MAX-2);
  for(unsigned i=0;i<2;i++)mixed_u32(out,&at,WaitForSingleObjectEx(objects[i],0,0)==0);
  for(unsigned i=2;i<4;i++){unsigned count=0;while(WaitForSingleObjectEx(objects[i],0,0)==0){if(++count>3)return 0;}mixed_u32(out,&at,count);}
#ifdef _WIN32
  for(unsigned i=0;i<4;i++)CloseHandle(objects[i]);
#endif
 }
 return at;
}
