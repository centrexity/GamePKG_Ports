/* Private coarse CPU timing; compiled away in normal builds. */
#ifdef GAMEPKG_PIXEL_TIMING
static uint64_t gp_pixel_ns[6],gp_pixel_visits[6],gp_pixel_calls;
struct gp_pixel_scope {uint64_t last;unsigned stage;};
static struct gp_pixel_scope gp_pixel_begin(void){struct gp_pixel_scope scope={xbox_HostNowNs(),0};gp_pixel_calls++;return scope;}
static void gp_pixel_mark(struct gp_pixel_scope*scope,unsigned next){uint64_t now=xbox_HostNowNs();gp_pixel_ns[scope->stage]+=now-scope->last;gp_pixel_visits[scope->stage]++;scope->last=now;scope->stage=next;}
static void gp_pixel_end(struct gp_pixel_scope*scope){gp_pixel_mark(scope,scope->stage);}
static void gp_pixel_profile_frame(unsigned flip){
 static FILE*file;static int checked;
 if(!checked){checked=1;const char*path=getenv("GAMEPKG_PIXEL_TIMING_PATH");if(path&&*path)file=fopen(path,"wb");if(file){fputs("flip,pixel_calls",file);const char*names[]={"decode","render_targets","textures","samplers","upload_bind","shader"};for(unsigned i=0;i<6;i++)fprintf(file,",%s_ms,%s_visits",names[i],names[i]);fputc('\n',file);}}
 if(file){fprintf(file,"%u,%llu",flip,(unsigned long long)gp_pixel_calls);for(unsigned i=0;i<6;i++)fprintf(file,",%.6f,%llu",(double)gp_pixel_ns[i]/1e6,(unsigned long long)gp_pixel_visits[i]);fputc('\n',file);if(flip%60==0)fflush(file);}
 memset(gp_pixel_ns,0,sizeof gp_pixel_ns);memset(gp_pixel_visits,0,sizeof gp_pixel_visits);gp_pixel_calls=0;
}
#define GP_PIXEL_DIAG_BEGIN() struct gp_pixel_scope gp_pixel_local=gp_pixel_begin()
#define GP_PIXEL_DIAG_MARK(stage) gp_pixel_mark(&gp_pixel_local,stage)
#define GP_PIXEL_DIAG_FINISH() gp_pixel_end(&gp_pixel_local)
#define GP_PIXEL_DIAG_RETURN(value) do{ID3D11PixelShader *gp_pixel_result=(value);gp_pixel_end(&gp_pixel_local);return gp_pixel_result;}while(0)
#else
#define GP_PIXEL_DIAG_BEGIN() ((void)0)
#define GP_PIXEL_DIAG_MARK(stage) ((void)0)
#define GP_PIXEL_DIAG_FINISH() ((void)0)
#define GP_PIXEL_DIAG_RETURN(value) return (value)
#endif
