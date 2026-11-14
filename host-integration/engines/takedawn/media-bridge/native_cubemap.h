/* Native D3D11 cube resources. Uses the runtime's existing NV2A face layout,
 * decoder and render-target ownership rules, also used by its Vulkan backend.
 * Six faces in +X,-X,+Y,-Y,+Z,-Z order; each guest face includes fmt_levels
 * and NV2A's 128-byte alignment, even when fewer mip levels are sampled. */
#define GP_CUBE_CACHE 8
struct gp_cube_entry {
 uint32_t offset,color,width,pitch,palette,pal_len,fmt_levels,levels,side,mask,gen;
 uint64_t used,hash,face_seq[6];
 ID3D11Texture2D *tex;
 ID3D11ShaderResourceView *srv;
};
static struct gp_cube_entry gp_cubes[GP_CUBE_CACHE];
static ID3D11Texture2D *gp_cube_black_tex;
static ID3D11ShaderResourceView *gp_cube_black_srv;
static uint64_t gp_cube_clock,gp_cube_copies,gp_cube_uploads,gp_cube_binds;
#include "private_cube_profile.h"
static void gp_cube_free(struct gp_cube_entry *e) {
 s_sc_init=0;
 if(e->srv)ID3D11ShaderResourceView_Release(e->srv);
 if(e->tex)ID3D11Texture2D_Release(e->tex);
 memset(e,0,sizeof *e);
}
static int gp_cube_resource(UINT side,UINT levels,ID3D11Texture2D **tex,ID3D11ShaderResourceView **srv) {
 D3D11_TEXTURE2D_DESC d={0};D3D11_SHADER_RESOURCE_VIEW_DESC view={0};
 d.Width=d.Height=side;d.MipLevels=levels;d.ArraySize=6;
 d.Format=DXGI_FORMAT_B8G8R8A8_UNORM;d.SampleDesc.Count=1;
 d.Usage=D3D11_USAGE_DEFAULT;d.BindFlags=D3D11_BIND_SHADER_RESOURCE;
 d.MiscFlags=D3D11_RESOURCE_MISC_TEXTURECUBE;
 view.Format=d.Format;view.ViewDimension=D3D11_SRV_DIMENSION_TEXTURECUBE;
 view.TextureCube.MipLevels=levels;
 GP_CUBE_DIAG_ALLOC();
 if(FAILED(ID3D11Device_CreateTexture2D(s_dev,&d,NULL,tex)))return 0;
 if(FAILED(ID3D11Device_CreateShaderResourceView(s_dev,(ID3D11Resource*)*tex,&view,srv))){ID3D11Texture2D_Release(*tex);*tex=NULL;return 0;}
 return 1;
}
static ID3D11ShaderResourceView *gp_cube_black(void) {
 if(!gp_cube_black_srv&&gp_cube_resource(1,1,&gp_cube_black_tex,&gp_cube_black_srv)){
  const uint32_t black=0xff000000u;
  for(unsigned f=0;f<6;f++)ID3D11DeviceContext_UpdateSubresource(s_ctx,(ID3D11Resource*)gp_cube_black_tex,f,NULL,&black,4,0);
 }
 /* A null view samples zero; never return a mismatched Texture2D resource. */
 return gp_cube_black_srv;
}
static RenderTarget *gp_cube_face_rt(uint32_t addr,uint32_t side) {
 /* Only formats already supported by the native render-to-texture path. */
 for(unsigned i=0;i<RT_MAX;i++){
  GP_CUBE_DIAG_SCAN();
  RenderTarget *rt=&s_rt[i];
  if(!rt->tex||((rt->addr^addr)&0x07ffffffu)||rt->w<side||rt->h<side)continue;
  GP_CUBE_DIAG_OWN();
  if(rt_drop_if_stale(rt,"cube bind"))continue;
  return rt;
 }
 return NULL;
}
static int gp_cube_key(const struct gp_cube_entry *e,const Stage *t,unsigned levels,unsigned side,unsigned mask) {
 return e->tex&&e->offset==t->offset&&e->color==t->color&&e->width==t->width&&
 e->pitch==t->pitch&&e->palette==t->palette&&e->pal_len==t->pal_len&&
 e->fmt_levels==t->fmt_levels&&e->levels==levels&&e->side==side&&e->mask==mask;
}
static ID3D11ShaderResourceView *gp_cube_bind(const Stage *t) {
 GP_CUBE_DIAG_BEGIN();
 const uint8_t *mem=(const uint8_t*)xbox_GetMemoryOffset();
 const uint32_t flip=nv2a_pb_gpu_state()->flips;
 RenderTarget *faces[6]={0};unsigned mask=0,w=t->width,side=w,levels=t->levels;
 if(!t->valid||!t->cube||!w||w!=t->height||!levels||levels>15||
   (uint64_t)w*w*6>TEX_MAX_TEXELS)GP_CUBE_DIAG_RETURN(gp_cube_black());
 uint32_t stride=nv2a_tex_face_stride(t->color,w,w,t->pitch,t->fmt_levels);
 uint32_t bytes=nv2a_tex_extent(t->color,w,w,t->pitch,levels);
 if(!stride||!bytes||bytes>stride||(uint64_t)t->offset+5ull*stride+bytes>0x100000000ull)GP_CUBE_DIAG_RETURN(gp_cube_black());
 int rtt=!d3d8_format_dxt_block_bytes(t->color)&&nv2a_tex_texel_bytes(nv2a_tex_linear_twin(t->color))==4;
 if(rtt)for(unsigned f=0;f<6;f++){
  faces[f]=gp_cube_face_rt(t->offset+f*stride,w);
  if(faces[f]){unsigned size=w*faces[f]->hw/faces[f]->w;if(!mask)side=size;
   /* All faces of one cube share an extent. Fail explicitly rather than
    * copying outside a source if mismatched target scales ever occur. */
   if(!size||size!=side||w*faces[f]->hh/faces[f]->h!=side)GP_CUBE_DIAG_RETURN(gp_cube_black());
   mask|=1u<<f;}
 }
 if(mask)levels=1;
 if(!side||side>D3D11_MAX_DIM)GP_CUBE_DIAG_RETURN(gp_cube_black());
 GP_CUBE_DIAG_MARK(1);
 struct gp_cube_entry *e=NULL,*victim=&gp_cubes[0];
 for(unsigned i=0;i<GP_CUBE_CACHE;i++){
  struct gp_cube_entry *c=&gp_cubes[i];
  if(gp_cube_key(c,t,levels,side,mask)){e=c;break;}
  if(!c->tex||(victim->tex&&c->used<victim->used))victim=c;
 }
 int fresh=!e;
 if(!e){e=victim;gp_cube_free(e);e->offset=t->offset;e->color=t->color;e->width=w;e->pitch=t->pitch;
  e->palette=t->palette;e->pal_len=t->pal_len;e->fmt_levels=t->fmt_levels;
  e->levels=levels;e->side=side;e->mask=mask;
  if(!gp_cube_resource(side,levels,&e->tex,&e->srv))GP_CUBE_DIAG_RETURN(gp_cube_black());
  for(unsigned f=0;f<6;f++)e->face_seq[f]=UINT64_MAX;
 }
 GP_CUBE_DIAG_MARK(2);
 if(mask!=63&&(fresh||e->gen!=flip)){
  uint64_t hash=NV2A_TEX_HASH_SEED;
  /* Hash all sampled levels of every memory-backed face. Face spacing
   * uses the full format count, not CONTROL0's sampled level count. */
  for(unsigned f=0;f<6;f++)if(!faces[f]){
#ifdef GAMEPKG_CUBE_HASH_FACE_ZERO_NEGATIVE
   if(f)continue; /* Must fail a late face/mip mutation. */
#endif
   uint32_t checked_bytes=bytes;
#ifdef GAMEPKG_CUBE_HASH_BASE_LEVEL_NEGATIVE
   checked_bytes=nv2a_tex_extent(t->color,w,w,t->pitch,1); /* Must fail a later mip. */
#endif
   hash=hash*31u+tex_hash(mem+t->offset+f*stride,checked_bytes);
  }
  if(t->color==NV2A_TEX_P8)hash^=tex_hash(mem+t->palette,t->pal_len*4u)*31u;
  if(fresh||hash!=e->hash){
   for(unsigned f=0;f<6;f++)if(!faces[f]){
    Stage face=*t;face.offset+=f*stride;face.levels=levels;
    if(!decode_texture(&face)){gp_cube_free(e);GP_CUBE_DIAG_RETURN(gp_cube_black());}
    const uint32_t *src=s_texels;uint32_t *up=NULL;
    if(side!=w){
     up=(uint32_t*)malloc((size_t)side*side*4);
     if(!up){gp_cube_free(e);GP_CUBE_DIAG_RETURN(gp_cube_black());}
     /* Match the runtime's existing Vulkan mixed drawn/memory cube path. */
     for(unsigned y=0;y<side;y++)for(unsigned x=0;x<side;x++)up[(size_t)y*side+x]=src[(size_t)(y*w/side)*w+x*w/side];
     src=up;
    }
    for(unsigned l=0;l<levels;l++){
     unsigned lw=nv2a_tex_level_dim(side,l),sub=l+f*levels;
#ifdef GAMEPKG_CUBE_WRONG_FACE_NEGATIVE
     sub=l; /* Must fail the GPU six-face tests. */
#endif
     ID3D11DeviceContext_UpdateSubresource(s_ctx,(ID3D11Resource*)e->tex,sub,NULL,src,lw*4,0);
     src+=(size_t)lw*lw;
    }
    free(up);gp_cube_uploads++;
   }
   e->hash=hash;
  }
  e->gen=flip;
 }
 GP_CUBE_DIAG_MARK(3);
 for(unsigned f=0;f<6;f++)if(faces[f]){
  RenderTarget *rt=faces[f];uint64_t seq=rt->last_draw>rt->last_clear?rt->last_draw:rt->last_clear;
  if(e->face_seq[f]==seq)continue;
  D3D11_BOX box={0,0,0,side,side,1};
  ID3D11DeviceContext_CopySubresourceRegion(s_ctx,(ID3D11Resource*)e->tex,f*levels,0,0,0,(ID3D11Resource*)rt->tex,0,&box);
  e->face_seq[f]=seq;gp_cube_copies++;
 }
 GP_CUBE_DIAG_MARK(4);
 e->used=++gp_cube_clock;gp_cube_binds++;
 if(gp_cube_binds==1)fprintf(stderr,LOGP "native cube sampling: six faces, rendered-face GPU copies, mip chains\n");
 GP_CUBE_DIAG_RETURN(e->srv);
}
