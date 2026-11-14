#ifdef GAMEPKG_POST_TEST
#include <float.h>
static int gp_cube_changed;
static unsigned gp_cube_changed_level;
static uint32_t gp_cube_pattern(unsigned face,unsigned level,unsigned x,unsigned y) {
 if(gp_cube_changed&&face==5&&level==gp_cube_changed_level&&x==0&&y==0)return 0xff123456u;
 return 0xff000000u|((face*37+level*13+x*11+y*7)&255u)<<16|
 ((face*19+level*31+x*3+y*17)&255u)<<8|((face*41+level*7+x*23+y*5)&255u);
}
static ID3D11PixelShader *gp_cube_test_ps;
static ID3D11VertexShader *gp_cube_test_vs;
static ID3D11Buffer *gp_cube_test_cb;
static ID3D11SamplerState *gp_cube_test_sampler;
static int gp_cube_test_init(void) {
 const char *vs="float4 main(uint id:SV_VertexID):SV_Position{float2 uv=float2((id<<1)&2,id&2);return float4(uv*float2(2,-2)+float2(-1,1),0,1);}";
 const char *ps="TextureCube tex:register(t0);SamplerState samp:register(s0);cbuffer P:register(b0){uint width;uint face;uint level;uint pad;}float4 main(float4 p:SV_Position):SV_Target{float u=2*p.x/width-1,v=2*p.y/width-1;float3 d=face==0?float3(1,-v,-u):face==1?float3(-1,-v,u):face==2?float3(u,1,v):face==3?float3(u,-1,-v):face==4?float3(u,-v,1):float3(-u,-v,-1);return tex.SampleLevel(samp,d,level);}";
 ID3DBlob *code=NULL;HRESULT hr;
 if(!compile("cube-test-vs",vs,"vs_5_0",&code))return 0;
 hr=ID3D11Device_CreateVertexShader(s_dev,ID3D10Blob_GetBufferPointer(code),ID3D10Blob_GetBufferSize(code),NULL,&gp_cube_test_vs);ID3D10Blob_Release(code);if(FAILED(hr))return 0;
 if(!compile("cube-test-ps",ps,"ps_5_0",&code))return 0;
 hr=ID3D11Device_CreatePixelShader(s_dev,ID3D10Blob_GetBufferPointer(code),ID3D10Blob_GetBufferSize(code),NULL,&gp_cube_test_ps);ID3D10Blob_Release(code);if(FAILED(hr))return 0;
 D3D11_BUFFER_DESC bd={0};bd.ByteWidth=16;bd.Usage=D3D11_USAGE_DEFAULT;bd.BindFlags=D3D11_BIND_CONSTANT_BUFFER;
 if(FAILED(ID3D11Device_CreateBuffer(s_dev,&bd,NULL,&gp_cube_test_cb)))return 0;
 D3D11_SAMPLER_DESC sd={0};sd.Filter=D3D11_FILTER_MIN_MAG_MIP_POINT;sd.AddressU=sd.AddressV=sd.AddressW=D3D11_TEXTURE_ADDRESS_CLAMP;sd.MaxLOD=FLT_MAX;sd.MaxAnisotropy=1;sd.ComparisonFunc=D3D11_COMPARISON_NEVER;
 return SUCCEEDED(ID3D11Device_CreateSamplerState(s_dev,&sd,&gp_cube_test_sampler));
}
static int gp_cube_test_sample(ID3D11ShaderResourceView *srv,unsigned side,unsigned levels,unsigned base,unsigned mask,const uint32_t colors[6],uint64_t *checks) {
 D3D11_SHADER_RESOURCE_VIEW_DESC view;ID3D11ShaderResourceView_GetDesc(srv,&view);
 if(view.ViewDimension!=D3D11_SRV_DIMENSION_TEXTURECUBE||view.TextureCube.MipLevels!=levels)return 0;
 for(unsigned l=0;l<levels;l++){
  unsigned w=nv2a_tex_level_dim(side,l);RenderTarget out={0};
  if(!gp_post_texture(w,w,DXGI_FORMAT_B8G8R8A8_UNORM,NULL,0,&out))return 0;
  uint32_t *pixels=malloc((size_t)w*w*4);if(!pixels)return 0;
  for(unsigned f=0;f<6;f++){
   unsigned args[4]={w,f,l,0};ID3D11DeviceContext_UpdateSubresource(s_ctx,(ID3D11Resource*)gp_cube_test_cb,0,NULL,args,0,0);
   ID3D11DeviceContext_IASetInputLayout(s_ctx,NULL);ID3D11DeviceContext_IASetPrimitiveTopology(s_ctx,D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
   ID3D11DeviceContext_VSSetShader(s_ctx,gp_cube_test_vs,NULL,0);ID3D11DeviceContext_GSSetShader(s_ctx,NULL,NULL,0);
   ID3D11DeviceContext_PSSetShader(s_ctx,gp_cube_test_ps,NULL,0);ID3D11DeviceContext_PSSetConstantBuffers(s_ctx,0,1,&gp_cube_test_cb);
   ID3D11DeviceContext_PSSetSamplers(s_ctx,0,1,&gp_cube_test_sampler);ID3D11DeviceContext_PSSetShaderResources(s_ctx,0,1,&srv);
   ID3D11DeviceContext_RSSetState(s_ctx,s_rs[0][0]);D3D11_VIEWPORT vp={0,0,(float)w,(float)w,0,1};ID3D11DeviceContext_RSSetViewports(s_ctx,1,&vp);D3D11_RECT rect={0,0,(LONG)w,(LONG)w};ID3D11DeviceContext_RSSetScissorRects(s_ctx,1,&rect);
   ID3D11DeviceContext_OMSetDepthStencilState(s_ctx,s_ds_off,0);ID3D11DeviceContext_OMSetBlendState(s_ctx,NULL,NULL,0xffffffffu);
   ID3D11DeviceContext_OMSetRenderTargets(s_ctx,1,&out.rtv,NULL);ID3D11DeviceContext_Draw(s_ctx,3,0);
   ID3D11ShaderResourceView *none=NULL;ID3D11DeviceContext_PSSetShaderResources(s_ctx,0,1,&none);
   if(!gp_post_read(&out,pixels))return 0;
   unsigned bw=nv2a_tex_level_dim(base,l);
   for(unsigned y=0;y<w;y++)for(unsigned x=0;x<w;x++){
    uint32_t expected=mask&(1u<<f)?colors[f]:gp_cube_pattern(f,l,x*bw/w,y*bw/w);
    if(pixels[(size_t)y*w+x]!=expected){fprintf(stderr,"[cube-test] FAIL face%u mip%u %u,%u got%08x expected%08x\n",f,l,x,y,pixels[(size_t)y*w+x],expected);return 0;}
    ++*checks;
   }
  }
  free(pixels);ID3D11RenderTargetView_Release(out.rtv);ID3D11ShaderResourceView_Release(out.srv);ID3D11Texture2D_Release(out.tex);
 }
 s_sc_init=0;return 1;
}
int gamepkg_cube_unit_test(void) {
#define CU_CHECK(x) do{if(!(x)){fprintf(stderr,"[cube-test] FAIL line %d: %s\n",__LINE__,#x);return 100;}}while(0)
 CU_CHECK(init()&&gp_cube_test_init());uint64_t checks=0;
 uint8_t *mem=NULL;for(uintptr_t p=0x16000000u;p<0x19000000u&&!mem;p+=0x1000000u)mem=VirtualAlloc((void*)p,0x100000,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);
 CU_CHECK(mem&&xbox_GetMemoryOffset()==0);Stage t={0};t.offset=(uint32_t)(uintptr_t)mem;t.color=0x06;t.cube=1;t.dims=2;t.mode=t.raw_mode=3;t.on=t.valid=1;
 uint32_t cpu[256],colors[6]={0};
 /* Every source face has distinctive XY and mip colours. Format levels
  * remain packed even when sampled levels are clamped to one or two. */
 for(unsigned n=0;n<5;n++){
  unsigned w=1u<<n;t.width=t.height=w;t.fmt_levels=n+1;
  uint32_t stride=nv2a_tex_face_stride(t.color,w,w,0,t.fmt_levels);CU_CHECK(stride*6u<=0x100000u);
  memset(mem,0xcd,stride*6u);
  for(unsigned f=0;f<6;f++)for(unsigned l=0;l<t.fmt_levels;l++){
   unsigned d=nv2a_tex_level_dim(w,l);for(unsigned y=0;y<d;y++)for(unsigned x=0;x<d;x++)cpu[y*d+x]=gp_cube_pattern(f,l,x,y);
   xbox_swizzle_rect(mem+f*stride+nv2a_tex_level_offset(t.color,w,w,l),cpu,d,d,4);
  }
  for(unsigned levels=1;levels<=t.fmt_levels;levels++){
   t.levels=levels;ID3D11ShaderResourceView *srv=gp_cube_bind(&t);CU_CHECK(srv&&srv!=gp_cube_black_srv);
   CU_CHECK(gp_cube_test_sample(srv,w,levels,w,0,colors,&checks));
   uint64_t uploads=gp_cube_uploads;CU_CHECK(gp_cube_bind(&t)==srv&&gp_cube_uploads==uploads);
  }
  /* A late face's smallest mip is outside face zero and level zero.
   * Revalidation must inspect it, not reuse a partially checked snapshot. */
  uint32_t offset=5u*stride+nv2a_tex_level_offset(t.color,w,w,n);
  uint32_t changed=0xff123456u;memcpy(mem+offset,&changed,4);
  gp_cube_changed=1;gp_cube_changed_level=n;
  for(unsigned i=0;i<GP_CUBE_CACHE;i++)gp_cubes[i].gen=nv2a_pb_gpu_state()->flips-1u;
  ID3D11ShaderResourceView *updated=gp_cube_bind(&t);
  CU_CHECK(updated&&gp_cube_test_sample(updated,w,t.levels,w,0,colors,&checks));
  gp_cube_changed=0;
 }
 /* GPU-rendered faces at native 2x plus memory-backed faces at guest size. */
 t.width=t.height=8;t.fmt_levels=4;t.levels=3;
 uint32_t stride=nv2a_tex_face_stride(t.color,8,8,0,t.fmt_levels);
 for(unsigned f=0;f<6;f++)for(unsigned l=0;l<4;l++){
  unsigned d=nv2a_tex_level_dim(8,l);for(unsigned y=0;y<d;y++)for(unsigned x=0;x<d;x++)cpu[y*d+x]=gp_cube_pattern(f,l,x,y);
  xbox_swizzle_rect(mem+f*stride+nv2a_tex_level_offset(t.color,8,8,l),cpu,d,d,4);
 }
 for(unsigned i=0;i<RT_MAX;i++)if(s_rt[i].tex)rt_release(&s_rt[i]);s_cur=s_drawn=NULL;
 for(unsigned f=0;f<6;f+=2){
  RenderTarget *rt=&s_rt[f];CU_CHECK(gp_post_texture(16,16,DXGI_FORMAT_B8G8R8A8_UNORM,NULL,0,rt));
  rt->w=rt->h=8;rt->addr=rt->offset=t.offset+f*stride;
  float c[4]={(float)(f+1)/8,.25f,.75f,1};colors[f]=0xff000000u|((uint32_t)floorf(c[0]*255+.5f))<<16|64u<<8|191u;
  ID3D11DeviceContext_ClearRenderTargetView(s_ctx,rt->rtv,c);rt->last_clear=++s_seq;
 }
 ID3D11ShaderResourceView *srv=gp_cube_bind(&t);CU_CHECK(srv&&gp_cube_test_sample(srv,16,1,8,21,colors,&checks));
 uint64_t copies=gp_cube_copies;CU_CHECK(gp_cube_bind(&t)==srv&&gp_cube_copies==copies);
 float changed[4]={1,0,0,1};ID3D11DeviceContext_ClearRenderTargetView(s_ctx,s_rt[2].rtv,changed);s_rt[2].last_clear=++s_seq;colors[2]=0xffff0000u;
 CU_CHECK(gp_cube_bind(&t)==srv&&gp_cube_copies==copies+1);CU_CHECK(gp_cube_test_sample(srv,16,1,8,21,colors,&checks));
 /* Retiring one rendered surface must change that face back to guest data. */
 rt_release(&s_rt[2]);srv=gp_cube_bind(&t);CU_CHECK(srv&&gp_cube_test_sample(srv,16,1,8,17,colors,&checks));
 for(unsigned i=0;i<RT_MAX;i++)if(s_rt[i].tex)rt_release(&s_rt[i]);s_cur=s_drawn=NULL;
 srv=gp_cube_bind(&t);CU_CHECK(srv&&gp_cube_test_sample(srv,8,3,8,0,colors,&checks));
 /* Cache keys distinguish palette, pitch, full face spacing and level count. */
 struct gp_cube_entry e={0};e.tex=(ID3D11Texture2D*)(uintptr_t)1;e.offset=t.offset;e.color=t.color;e.width=8;e.pitch=t.pitch;e.palette=t.palette;e.pal_len=t.pal_len;e.fmt_levels=4;e.levels=3;e.side=8;
 CU_CHECK(gp_cube_key(&e,&t,3,8,0));t.fmt_levels++;CU_CHECK(!gp_cube_key(&e,&t,3,8,0));t.fmt_levels--;t.pitch++;CU_CHECK(!gp_cube_key(&e,&t,3,8,0));t.pitch--;t.palette++;CU_CHECK(!gp_cube_key(&e,&t,3,8,0));t.palette--;t.pal_len++;CU_CHECK(!gp_cube_key(&e,&t,3,8,0));t.pal_len--;
 t.height=7;CU_CHECK(gp_cube_bind(&t)==gp_cube_black());t.height=8;
 for(unsigned i=0;i<GP_CUBE_CACHE;i++)gp_cube_free(&gp_cubes[i]);
 VirtualFree(mem,0,MEM_RELEASE);s_sc_init=0;
 CU_CHECK(SUCCEEDED(ID3D11Device_GetDeviceRemovedReason(s_dev)));
 fprintf(stderr,"[cube-test] PASS %llu exact hardware-sampled face/mip/orientation texels, memory/drawn/mixed 2x, unchanged/changed RT generations, retirement and complete cache keys\n",(unsigned long long)checks);return 0;
#undef CU_CHECK
}
#endif
