/* Maintainer original-source NT oracle. Never linked into game objects. */
#include "../service-kernel-semaphores/reference.c"
uint32_t semaphore_import_reference(uint8_t*out){
 memset(s_handle_table,0,sizeof(s_handle_table));memset(native_semaphores,0,sizeof(native_semaphores));memset(guest,0,sizeof(guest));uint32_t cursor=0;
 /* KIC4 fixture also owns the shadow mutex: 126 native permits remain. */
 native_semaphores[127].live=1;
 const uint32_t maxima[]={1,2,4,0x7fffffffu};const uint32_t permits[]={0,1,2,0x7fffffffu,0x80000000u,0xffffffffu};
 for(uint32_t m=0;m<4;m++){uint32_t initials[]={0,1,maxima[m]/2,maxima[m]};for(uint32_t n=0;n<4;n++){
  guest[1]=0x33333333;guest[2]=0xabcdef01;g_args[0]=4;g_args[1]=0xffffffff;g_args[2]=initials[n];g_args[3]=maxima[m];bridge_NtCreateSemaphore();assert(g_eax==0&&guest[1]==0x48000001&&guest[2]==0xabcdef01);
  HANDLE h=bridge_resolve_handle(guest[1]);uint32_t i=native_index(h);emit32(out,&cursor,g_eax);emit32(out,&cursor,guest[1]);emit32(out,&cursor,native_semaphores[i].count);emit32(out,&cursor,native_semaphores[i].maximum);
  for(uint32_t p=0;p<6;p++){guest[3]=0x55555555;g_args[0]=guest[1];g_args[1]=permits[p];g_args[2]=12;bridge_NtReleaseSemaphore();emit32(out,&cursor,g_eax);emit32(out,&cursor,guest[3]);emit32(out,&cursor,native_semaphores[i].count);}
  native_close(bridge_take_handle(guest[1]));
 }}
 const uint32_t invalid_counts[][2]={{0,0},{1,0},{2,1},{0xffffffffu,1},{0,0x80000000u},{1,0xffffffffu},{0x80000000u,0x7fffffffu},{0x7fffffffu,0}};
 for(uint32_t i=0;i<8;i++){guest[1]=0x33333333;guest[2]=0xabcdef01;g_args[0]=4;g_args[1]=0xffffffff;g_args[2]=invalid_counts[i][0];g_args[3]=invalid_counts[i][1];bridge_NtCreateSemaphore();assert(g_eax==0xc000009a&&guest[1]==0x33333333);emit32(out,&cursor,g_eax);emit32(out,&cursor,guest[1]);emit32(out,&cursor,guest[2]);}
 const uint32_t invalid[]={0,0xffffffffu,0xfffffffeu,0xdead0001,0xbeef0010,0x48000000,0x48004000,0x48003fff,0x49000001};
 for(uint32_t i=0;i<9;i++){guest[3]=0x55555555;g_args[0]=invalid[i];g_args[1]=1;g_args[2]=12;bridge_NtReleaseSemaphore();assert(g_eax==0xc0000008&&guest[3]==0x55555555);emit32(out,&cursor,g_eax);emit32(out,&cursor,guest[3]);}
 g_args[0]=0;g_args[1]=0xffffffff;g_args[2]=0;g_args[3]=4;bridge_NtCreateSemaphore();assert(g_eax==0);emit32(out,&cursor,g_eax);
 g_args[0]=4;g_args[1]=0xffffffff;g_args[2]=1;g_args[3]=4;bridge_NtCreateSemaphore();assert(g_eax==0&&guest[1]==0x48000001);emit32(out,&cursor,g_eax);emit32(out,&cursor,guest[1]);native_close(bridge_take_handle(guest[1]));
 for(uint32_t i=0;i<126;i++){guest[1]=0x33333333;guest[2]=0xabcdef01;g_args[0]=4;g_args[1]=0xffffffff;g_args[2]=0;g_args[3]=1;bridge_NtCreateSemaphore();assert(g_eax==(i<125?0:0xc000009a)&&guest[1]==(i<125?0x48000001+i:0x33333333));emit32(out,&cursor,g_eax);emit32(out,&cursor,guest[1]);emit32(out,&cursor,guest[2]);}
 assert(cursor==3100);return cursor;
}
