/* Private asynchronous command-span timing. Never blocks or flushes. */
#ifdef GAMEPKG_GPU_TIMING
#define GP_GPU_SLOTS 8
struct gp_gpu_slot { ID3D11Query *disjoint,*stamp[4]; unsigned live, main_end, flip; };
static struct gp_gpu_slot gp_gpu_slots[GP_GPU_SLOTS];
static unsigned gp_gpu_head,gp_gpu_pending,gp_gpu_active,gp_gpu_started,gp_gpu_disabled,gp_gpu_drops;
static FILE *gp_gpu_file;
static void gp_gpu_row(unsigned flip,int valid,double main_ms,double post_ms){
 static int checked;if(!checked){checked=1;const char*p=getenv("GAMEPKG_GPU_TIMING_PATH");if(p&&*p)gp_gpu_file=fopen(p,"wb");if(gp_gpu_file)fputs("flip,valid,main_span_ms,post_span_ms,dropped_frames\n",gp_gpu_file);}
 if(gp_gpu_file){fprintf(gp_gpu_file,"%u,%d,%.6f,%.6f,%u\n",flip,valid,main_ms,post_ms,gp_gpu_drops);if(flip%60==0)fflush(gp_gpu_file);}
}
static void gp_gpu_poll(void){
 while(gp_gpu_pending){struct gp_gpu_slot *r=&gp_gpu_slots[gp_gpu_head];D3D11_QUERY_DATA_TIMESTAMP_DISJOINT d={0};
  HRESULT hr=ID3D11DeviceContext_GetData(s_ctx,(ID3D11Asynchronous*)r->disjoint,&d,sizeof d,D3D11_ASYNC_GETDATA_DONOTFLUSH);if(hr==S_FALSE)break;
  uint64_t t[4]={0};int ok=SUCCEEDED(hr)&&!d.Disjoint&&d.Frequency;
  if(ok)for(unsigned n=0;n<4;n++){hr=ID3D11DeviceContext_GetData(s_ctx,(ID3D11Asynchronous*)r->stamp[n],&t[n],sizeof t[n],D3D11_ASYNC_GETDATA_DONOTFLUSH);if(hr==S_FALSE)return;if(FAILED(hr))ok=0;}
  ok=ok&&t[1]>=t[0]&&t[3]>=t[2];double unit=ok?1000.0/(double)d.Frequency:0;
  gp_gpu_row(r->flip,ok,ok?(t[1]-t[0])*unit:0,ok?(t[3]-t[2])*unit:0);gp_gpu_head=(gp_gpu_head+1)%GP_GPU_SLOTS;gp_gpu_pending--;
 }
}
static int gp_gpu_create(struct gp_gpu_slot *r){
 if(r->live)return 1;D3D11_QUERY_DESC q={D3D11_QUERY_TIMESTAMP_DISJOINT,0};
 if(FAILED(ID3D11Device_CreateQuery(s_dev,&q,&r->disjoint)))goto fail;
 q.Query=D3D11_QUERY_TIMESTAMP;
 for(unsigned n=0;n<4;n++)if(FAILED(ID3D11Device_CreateQuery(s_dev,&q,&r->stamp[n])))goto fail;
 r->live=1;return 1;
 fail:for(unsigned n=0;n<4;n++)if(r->stamp[n]){ID3D11Query_Release(r->stamp[n]);r->stamp[n]=NULL;}if(r->disjoint){ID3D11Query_Release(r->disjoint);r->disjoint=NULL;}gp_gpu_disabled=1;return 0;
}
static void gp_gpu_begin(void){
 if(gp_gpu_started||gp_gpu_disabled)return;gp_gpu_started=1;gp_gpu_poll();
 if(gp_gpu_pending==GP_GPU_SLOTS){gp_gpu_drops++;return;}
 struct gp_gpu_slot *r=&gp_gpu_slots[(gp_gpu_head+gp_gpu_pending)%GP_GPU_SLOTS];if(!gp_gpu_create(r))return;
 r->main_end=0;gp_gpu_active=1;ID3D11DeviceContext_Begin(s_ctx,(ID3D11Asynchronous*)r->disjoint);ID3D11DeviceContext_End(s_ctx,(ID3D11Asynchronous*)r->stamp[0]);
}
static void gp_gpu_main_end(void){
 if(!gp_gpu_active)return;struct gp_gpu_slot *r=&gp_gpu_slots[(gp_gpu_head+gp_gpu_pending)%GP_GPU_SLOTS];
 if(!r->main_end){ID3D11DeviceContext_End(s_ctx,(ID3D11Asynchronous*)r->stamp[1]);r->main_end=1;}
}
static void gp_gpu_post_begin(void){gp_gpu_begin();gp_gpu_main_end();if(gp_gpu_active){struct gp_gpu_slot*r=&gp_gpu_slots[(gp_gpu_head+gp_gpu_pending)%GP_GPU_SLOTS];ID3D11DeviceContext_End(s_ctx,(ID3D11Asynchronous*)r->stamp[2]);}}
static void gp_gpu_finish(unsigned flip){
 if(gp_gpu_active){struct gp_gpu_slot*r=&gp_gpu_slots[(gp_gpu_head+gp_gpu_pending)%GP_GPU_SLOTS];r->flip=flip;ID3D11DeviceContext_End(s_ctx,(ID3D11Asynchronous*)r->stamp[3]);ID3D11DeviceContext_End(s_ctx,(ID3D11Asynchronous*)r->disjoint);gp_gpu_pending++;}
 gp_gpu_active=gp_gpu_started=0;gp_gpu_poll();
}
#define GP_GPU_BEGIN() gp_gpu_begin()
#define GP_GPU_MAIN_END() gp_gpu_main_end()
#define GP_GPU_POST_BEGIN() gp_gpu_post_begin()
#define GP_GPU_FINISH(flip) gp_gpu_finish(flip)
#else
#define GP_GPU_BEGIN() ((void)0)
#define GP_GPU_MAIN_END() ((void)0)
#define GP_GPU_POST_BEGIN() ((void)0)
#define GP_GPU_FINISH(flip) ((void)0)
#endif
