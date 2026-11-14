/* Private CPU timing and work counters. No hook, file, clock or counter in
 * normal builds. Stages are disjoint inside each actual cube bind. */
#ifdef GAMEPKG_CUBE_TIMING
static uint64_t gp_cp_ns[5],gp_cp_binds,gp_cp_scan,gp_cp_alloc,gp_cp_checks;
struct gp_cp_scope {uint64_t last;unsigned stage;};
static struct gp_cp_scope gp_cp_begin(void){struct gp_cp_scope s={xbox_HostNowNs(),0};gp_cp_binds++;return s;}
static void gp_cp_mark(struct gp_cp_scope*s,unsigned next){uint64_t now=xbox_HostNowNs();gp_cp_ns[s->stage]+=now-s->last;s->last=now;s->stage=next;}
static void gp_cp_frame(unsigned flip){
 static FILE *file;static int checked;static uint64_t last_copies,last_uploads;
 if(!checked){checked=1;const char *p=getenv("GAMEPKG_CUBE_TIMING_PATH");if(p&&*p)file=fopen(p,"wb");if(file)fputs("flip,binds,lookup_ms,resource_ms,memory_ms,copy_ms,finish_ms,rt_scan_entries,ownership_checks,allocations,gpu_copies,memory_uploads\n",file);}
 if(file){fprintf(file,"%u,%llu",flip,(unsigned long long)gp_cp_binds);for(unsigned i=0;i<5;i++)fprintf(file,",%.6f",(double)gp_cp_ns[i]/1e6);fprintf(file,",%llu,%llu,%llu,%llu,%llu\n",(unsigned long long)gp_cp_scan,(unsigned long long)gp_cp_checks,(unsigned long long)gp_cp_alloc,(unsigned long long)(gp_cube_copies-last_copies),(unsigned long long)(gp_cube_uploads-last_uploads));if(flip%60==0)fflush(file);}
 memset(gp_cp_ns,0,sizeof gp_cp_ns);gp_cp_binds=gp_cp_scan=gp_cp_alloc=gp_cp_checks=0;last_copies=gp_cube_copies;last_uploads=gp_cube_uploads;
}
#define GP_CUBE_DIAG_BEGIN() struct gp_cp_scope gp_cp_local=gp_cp_begin()
#define GP_CUBE_DIAG_MARK(s) gp_cp_mark(&gp_cp_local,s)
#define GP_CUBE_DIAG_RETURN(value) do{ID3D11ShaderResourceView *gp_cp_result=(value);gp_cp_mark(&gp_cp_local,gp_cp_local.stage);return gp_cp_result;}while(0)
#define GP_CUBE_DIAG_SCAN() (++gp_cp_scan)
#define GP_CUBE_DIAG_OWN() (++gp_cp_checks)
#define GP_CUBE_DIAG_ALLOC() (++gp_cp_alloc)
#else
#define GP_CUBE_DIAG_BEGIN() ((void)0)
#define GP_CUBE_DIAG_MARK(s) ((void)0)
#define GP_CUBE_DIAG_RETURN(value) return(value)
#define GP_CUBE_DIAG_SCAN() ((void)0)
#define GP_CUBE_DIAG_OWN() ((void)0)
#define GP_CUBE_DIAG_ALLOC() ((void)0)
#endif
