/* Optional private SMAA optimization. Unsupported allocations retain the original path. */
#ifdef GAMEPKG_POST_EDGE_MASK
static struct {ID3D11Texture2D *tex;ID3D11DepthStencilView *view;ID3D11DepthStencilState *mark,*test;UINT w,h;int tried;} gp_edge_mask;
static int gp_edge_mask_requested=1;
#ifdef GAMEPKG_POST_TEST
static UINT gp_edge_mask_test_reference=1;
#endif
static void gp_edge_mask_free(void){if(gp_edge_mask.view)ID3D11DepthStencilView_Release(gp_edge_mask.view);if(gp_edge_mask.tex)ID3D11Texture2D_Release(gp_edge_mask.tex);gp_edge_mask.view=NULL;gp_edge_mask.tex=NULL;gp_edge_mask.w=gp_edge_mask.h=0;}
static int gp_edge_mask_size(UINT w,UINT h){
 if(!gp_edge_mask.tried){
  gp_edge_mask.tried=1;D3D11_DEPTH_STENCIL_DESC d={0};d.DepthWriteMask=D3D11_DEPTH_WRITE_MASK_ZERO;d.DepthFunc=D3D11_COMPARISON_ALWAYS;d.StencilEnable=TRUE;d.StencilReadMask=d.StencilWriteMask=0xff;
  d.FrontFace.StencilFunc=D3D11_COMPARISON_ALWAYS;d.FrontFace.StencilPassOp=D3D11_STENCIL_OP_REPLACE;d.FrontFace.StencilFailOp=d.FrontFace.StencilDepthFailOp=D3D11_STENCIL_OP_KEEP;d.BackFace=d.FrontFace;
  if(FAILED(ID3D11Device_CreateDepthStencilState(s_dev,&d,&gp_edge_mask.mark)))return 0;
  d.StencilWriteMask=0;d.FrontFace.StencilFunc=D3D11_COMPARISON_EQUAL;d.FrontFace.StencilPassOp=D3D11_STENCIL_OP_KEEP;d.BackFace=d.FrontFace;
  if(FAILED(ID3D11Device_CreateDepthStencilState(s_dev,&d,&gp_edge_mask.test))){ID3D11DepthStencilState_Release(gp_edge_mask.mark);gp_edge_mask.mark=NULL;return 0;}
 }
 if(!gp_edge_mask.mark||!gp_edge_mask.test)return 0;
 if(gp_edge_mask.w==w&&gp_edge_mask.h==h&&gp_edge_mask.view)return 1;
 gp_edge_mask_free();D3D11_TEXTURE2D_DESC d={0};d.Width=w;d.Height=h;d.MipLevels=d.ArraySize=d.SampleDesc.Count=1;d.Format=DXGI_FORMAT_D24_UNORM_S8_UINT;d.Usage=D3D11_USAGE_DEFAULT;d.BindFlags=D3D11_BIND_DEPTH_STENCIL;
 if(FAILED(ID3D11Device_CreateTexture2D(s_dev,&d,NULL,&gp_edge_mask.tex)))return 0;
 if(FAILED(ID3D11Device_CreateDepthStencilView(s_dev,(ID3D11Resource*)gp_edge_mask.tex,NULL,&gp_edge_mask.view))){gp_edge_mask_free();return 0;}
 gp_edge_mask.w=w;gp_edge_mask.h=h;return 1;
}
static ID3D11DepthStencilView *gp_edge_mask_bind(int shader,UINT w,UINT h){
 ID3D11DepthStencilView *view=NULL;ID3D11DepthStencilState *state=s_ds_off;UINT reference=1;
 if(gp_edge_mask_requested&&gp_edge_mask.view&&gp_edge_mask.w==w&&gp_edge_mask.h==h){
  if(shader==GP_POST_EDGE){view=gp_edge_mask.view;state=gp_edge_mask.mark;ID3D11DeviceContext_ClearDepthStencilView(s_ctx,view,D3D11_CLEAR_STENCIL,1,0);}
  else if(shader==GP_POST_WEIGHT){view=gp_edge_mask.view;state=gp_edge_mask.test;
#ifdef GAMEPKG_POST_TEST
   reference=gp_edge_mask_test_reference;
#endif
  }
 }
 ID3D11DeviceContext_OMSetDepthStencilState(s_ctx,state,reference);return view;
}
#endif
