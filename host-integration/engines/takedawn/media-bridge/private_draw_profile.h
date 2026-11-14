/* Private coarse CPU timing; compiled away in normal builds. */
#ifdef GAMEPKG_DRAW_TIMING
static uint64_t gp_draw_ns[6],gp_draw_visits[6],gp_draw_calls;
struct gp_draw_scope {uint64_t last;unsigned stage;};
static struct gp_draw_scope gp_draw_begin(void){struct gp_draw_scope scope={xbox_HostNowNs(),0};gp_draw_calls++;return scope;}
static void gp_draw_mark(struct gp_draw_scope*scope,unsigned next){uint64_t now=xbox_HostNowNs();gp_draw_ns[scope->stage]+=now-scope->last;gp_draw_visits[scope->stage]++;scope->last=now;scope->stage=next;}
static void gp_draw_end(struct gp_draw_scope*scope){gp_draw_mark(scope,scope->stage);}
static void gp_draw_profile_frame(unsigned flip){
 static FILE*file;static int checked;
 if(!checked){checked=1;const char*path=getenv("GAMEPKG_DRAW_TIMING_PATH");if(path&&*path)file=fopen(path,"wb");if(file){fputs("flip,draw_calls",file);const char*names[]={"setup","streams","constants","bindings","pixels","submit"};for(unsigned i=0;i<6;i++)fprintf(file,",%s_ms,%s_visits",names[i],names[i]);fputc('\n',file);}}
 if(file){fprintf(file,"%u,%llu",flip,(unsigned long long)gp_draw_calls);for(unsigned i=0;i<6;i++)fprintf(file,",%.6f,%llu",(double)gp_draw_ns[i]/1e6,(unsigned long long)gp_draw_visits[i]);fputc('\n',file);if(flip%60==0)fflush(file);}
 memset(gp_draw_ns,0,sizeof gp_draw_ns);memset(gp_draw_visits,0,sizeof gp_draw_visits);gp_draw_calls=0;
}
#define GP_DRAW_DIAG_BEGIN() struct gp_draw_scope gp_draw_local=gp_draw_begin()
#define GP_DRAW_DIAG_MARK(stage) gp_draw_mark(&gp_draw_local,stage)
#define GP_DRAW_DIAG_FINISH() gp_draw_end(&gp_draw_local)
#define GP_DRAW_DIAG_RETURN() do{gp_draw_end(&gp_draw_local);return;}while(0)
#else
#define GP_DRAW_DIAG_BEGIN() ((void)0)
#define GP_DRAW_DIAG_MARK(stage) ((void)0)
#define GP_DRAW_DIAG_FINISH() ((void)0)
#define GP_DRAW_DIAG_RETURN() return
#endif
