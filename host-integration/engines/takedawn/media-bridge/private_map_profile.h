/* Private diagnostics only; normal player builds do not include these hooks. */
#ifdef GAMEPKG_MAP_TIMING
static struct {uint64_t map_ns,unmap_ns,maps,unmaps,failed;} gp_map_stats[6];
static unsigned gp_map_class(ID3D11Resource *resource) {
 if(resource==(ID3D11Resource*)s_cb_consts)return 0;
 if(resource==(ID3D11Resource*)s_cb_vp)return 1;
 if(resource==(ID3D11Resource*)s_cb_ps)return 2;
 if(resource==(ID3D11Resource*)s_vb)return 3;
 if(resource==(ID3D11Resource*)s_ib)return 4;
 return 5;
}
static HRESULT gp_profile_map(ID3D11DeviceContext *context,ID3D11Resource *resource,
 UINT subresource,D3D11_MAP type,UINT flags,D3D11_MAPPED_SUBRESOURCE *mapped) {
 unsigned kind=gp_map_class(resource);uint64_t begin=xbox_HostNowNs();
 HRESULT result=ID3D11DeviceContext_Map(context,resource,subresource,type,flags,mapped);
 gp_map_stats[kind].map_ns+=xbox_HostNowNs()-begin;gp_map_stats[kind].maps++;
 if(FAILED(result))gp_map_stats[kind].failed++;
 return result;
}
static void gp_profile_unmap(ID3D11DeviceContext *context,ID3D11Resource *resource,UINT subresource) {
 unsigned kind=gp_map_class(resource);uint64_t begin=xbox_HostNowNs();
 ID3D11DeviceContext_Unmap(context,resource,subresource);
 gp_map_stats[kind].unmap_ns+=xbox_HostNowNs()-begin;gp_map_stats[kind].unmaps++;
}
static void gp_map_profile_frame(unsigned flip) {
 static FILE *file;static int checked;
 if(!checked){checked=1;const char *path=getenv("GAMEPKG_MAP_TIMING_PATH");
  if(path&&*path)file=fopen(path,"wb");
  if(file){
   fputs("flip",file);
   const char *names[]={"constants","viewport","pixels","vertices","indices","other"};
   for(unsigned i=0;i<6;i++)fprintf(file,",%s_map_ms,%s_unmap_ms,%s_maps,%s_unmaps,%s_failed",names[i],names[i],names[i],names[i],names[i]);
   fputc('\n',file);
   uint64_t best=UINT64_MAX;
   for(unsigned i=0;i<1000;i++){uint64_t a=xbox_HostNowNs(),b=xbox_HostNowNs();if(b-a<best)best=b-a;}
   fprintf(stderr,"[map-profile] clock-pair minimum %llu ns; diagnostic timing includes wrapper overhead\n",(unsigned long long)best);
  }
 }
 if(file){fprintf(file,"%u",flip);for(unsigned i=0;i<6;i++)fprintf(file,",%.6f,%.6f,%llu,%llu,%llu",(double)gp_map_stats[i].map_ns/1e6,(double)gp_map_stats[i].unmap_ns/1e6,(unsigned long long)gp_map_stats[i].maps,(unsigned long long)gp_map_stats[i].unmaps,(unsigned long long)gp_map_stats[i].failed);fputc('\n',file);if(flip%60==0)fflush(file);}
 memset(gp_map_stats,0,sizeof gp_map_stats);
}
#undef ID3D11DeviceContext_Map
#undef ID3D11DeviceContext_Unmap
#define ID3D11DeviceContext_Map gp_profile_map
#define ID3D11DeviceContext_Unmap gp_profile_unmap
#endif
