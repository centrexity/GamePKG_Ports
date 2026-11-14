/**
 * A D3D11 backend for the pushbuffer executor (struct nv2a_pb_backend).
 *
 * The executor (src/kernel/nv2a_pb_exec.c) decodes the title's pushbuffer into
 * the state in nv2a_pb_state.h and calls the hooks of struct nv2a_pb_backend;
 * this file answers them with D3D11 instead of the CPU rasteriser:
 *
 *   on_clear  -> ClearRenderTargetView on the surface at color_offset
 *   on_draw   -> the batch's vertex program, translated to HLSL by d3d8_vsh.c
 *                and compiled with D3DCompile (Wine's d3dcompiler_47 under
 *                Proton), or a pass-through for pre-transformed batches
 *   on_flip   -> copy the surface the walker picked for this flip
 *                (nv2a_pb_present_state) into the swap chain, Present
 *
 * Opt-in: RECOMP_PB_BACKEND=d3d11 (see nv2a_pb_d3d11_register_from_env).
 *
 * Depth and stencil: a D32_FLOAT_S8X24_UINT buffer per zeta offset, cleared
 * by on_zclear.
 *
 * Not yet: the clear rectangle (whole surface), 16-bit surfaces, guest readback of
 * the rendered surface (the title never sees D3D11's pixels).
 *
 * Everything runs on the NV2A ack thread: the device, the window and its
 * message pump are created and used there only.
 *
 * Off Windows this file compiles to nothing.
 */
#ifdef _WIN32

#define COBJMACROS
#include <windows.h>
#ifdef GAMEPKG_PROFILE_GENERATE
void __llvm_profile_set_filename(const char *);
int __llvm_profile_write_file(void);
#endif
#include "recomp_env.h"
#include "recomp_exit_hook.h"
#include <d3d11.h>
#include <d3d11_1.h>   /* ID3D11DeviceContext1::ClearView, for a partial clear */
#include <dxgi.h>
#include <dxgi1_3.h>   /* IDXGISwapChain2: the frame-latency waitable */
#include "latency_trace.h"
#include <d3dcompiler.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "d3d8_vsh.h"
#include "d3d8_combiners.h"
#include "d3d8_swizzle.h"
#include "nv2a_pb_state.h"
#include "nv2a_backend_common.h"
#include "nv2a_vsh_cpu.h"      /* the batch trace runs the CPU interpreter */
#include "xbox_memory_layout.h"   /* xbox_GetMemoryOffset */
#include "video/video_player.h"   /* xbox_FramebufferWindowTitleText */
#include "video/window_title.h"

void xbox_PacingPresent(uint64_t when_ns);   /* src/kernel/kernel_pacing.c */
void xbox_VblankDisplayRefresh(uint64_t when_ns, uint64_t period_ns);
int  xbox_VblankDisplayLockOn(void);
uint64_t xbox_HostNowNs(void);

#define LOGP "[d3d11] "
#if defined(GAMEPKG_VERIFY_CONSTANT_DIRTY) || defined(GAMEPKG_VERIFY_RC_KEY_CACHE) || defined(GAMEPKG_VERIFY_VS_CACHE) || defined(GAMEPKG_D3D11_VERIFY_OUTPUT_CACHE)
static struct {uint64_t constants,raw,vertex,output;} gp_verify_counts;
#endif
#define D3D11_WND_CLASS "RecompNv2aD3D11"   /* RegisterClassA, then CreateWindowW as L"" D3D11_WND_CLASS */

typedef HRESULT (WINAPI *compile_fn)(LPCVOID, SIZE_T, LPCSTR,
                                     const D3D_SHADER_MACRO *, ID3DInclude *,
                                     LPCSTR, LPCSTR, UINT, UINT,
                                     ID3DBlob **, ID3DBlob **);

/* ---- device ------------------------------------------------------------ */

static int                     s_failed;        /* init failed: draw nothing */
static HWND                    s_hwnd;
/* This window is not fb_present.c's, which keeps the title bar's name (and,
 * with RECOMP_TRACE=title, FPS and draws); ours comes from the same place.
 * One clock for the creation title and the per-flip refresh, so the window
 * opens with the game's name and the first flip has nothing to rewrite. */
static struct xbox_title_clock s_title_clock;
/* Present's sync interval. Stock D3D11 presents without waiting for the
 * display (0): the guest's vblank clock paces the frames. RECOMP_PRESENT_VSYNC
 * is the SDL window's knob (default on there); here only a value that turns
 * it on changes anything, so an unset key keeps the stock behaviour. */
static UINT                    s_sync_interval;
/* RECOMP_PRESENT_VSYNC=fifo: two flip-model buffers, Present(1) and the
 * frame-latency waitable (SetMaximumFrameLatency, RECOMP_PRESENT_LATENCY,
 * 1 by default -- the BO-74 latency measurement's recommendation), waited on
 * before each present: the swap chain queues at most that many frames ahead
 * of the display and, when full, holds the executor -- the display's
 * refresh back-pressures the title, the documented low-latency flip-model
 * pacing (IDXGISwapChain2::SetMaximumFrameLatency) that DXVK honours (under
 * Proton also DXVK_CONFIG="dxgi.maxFrameLatency = 1"). s_swap_flags is what
 * ResizeBuffers must keep. */
static UINT                    s_swap_flags;
static HANDLE                  s_latency_wait;
static ID3D11Device           *s_dev;
static ID3D11DeviceContext    *s_ctx;
static ID3D11DeviceContext1   *s_ctx1;   /* NULL below D3D 11.1: partial clears clear it all */
static IDXGISwapChain         *s_swap;
static ID3D11Texture2D        *s_back;
static UINT                    s_back_w, s_back_h;
/* The swap chain size init() created, before any WM_SIZE resize. */
static UINT                    s_made_w, s_made_h;
/* Present (E1, nv2a_host_opts): the client size WM_SIZE last reported (0 =
 * none since init); the back buffer's view, made for the scaling blit only;
 * whether the frame being dumped is the present RT rather than the back
 * buffer (the blit path: the dump stays the frame, not the window). */
static volatile UINT           s_client_w, s_client_h;
static ID3D11RenderTargetView *s_back_rtv;
static int                     s_dump_rt;
static compile_fn              s_compile;

static ID3D11Buffer           *s_cb_consts;     /* b1: c[192]            */
static ID3D11Buffer           *s_cb_vp;         /* b2: viewport, fog, specular */
static ID3D11Buffer           *s_cb_ps;         /* PS b0: PsConsts       */

/* Per-batch CPU memos (RECOMP_D3D11_MEMO=0 turns them off for A/B): a
 * constant buffer is only rewritten when its bytes change, and the shader
 * and texture lookups reuse the last batch's answer when their inputs are
 * the same. Each one returns what the full path would, so the output does
 * not change. */
static int s_memo = -1;
static int memo_on(void)
{
    if (s_memo < 0) {
        const char *e = recomp_env(RENV_D3D11_MEMO);
        s_memo = !(e && *e == '0');
    }
    return s_memo;
}

/* The pipeline state last bound, so a batch binding the same again skips
 * the call. Only state this file alone sets and nothing unbinds behind it;
 * SRVs (which the runtime nulls on a render-target hazard), the vertex and
 * index buffers (new offsets every batch) and the output merger's targets
 * and depth state are always set. Releasing a shader or input layout bumps
 * s_state_gen, since a new object can come back at a released one's
 * address. All 0xFF is "nothing known". */
static uint32_t s_state_gen;
static struct {
    uint32_t                  gen;
    ID3D11RenderTargetView   *om_rtv;
    ID3D11DepthStencilView   *om_dsv;
    ID3D11DepthStencilState  *om_dss;
    UINT                     om_ref;
    ID3D11BlendState         *bs;
    float                     bf[4];    /* OMSetBlendState's blend factor */
    ID3D11RasterizerState    *rs;
    ID3D11InputLayout        *il;
    D3D11_PRIMITIVE_TOPOLOGY  topo;
    ID3D11VertexShader       *vs;
    ID3D11PixelShader        *ps;
    ID3D11Buffer             *vs_cb[3], *ps_cb;
    ID3D11SamplerState       *smp[4];
    ID3D11ShaderResourceView *ps_srv[4];
    D3D11_VIEWPORT            vp;
} s_sc;
static int s_sc_init;

static int sc_on(void)
{
    if (!memo_on())
        return 0;
    if (!s_sc_init || s_sc.gen != s_state_gen) {
        memset(&s_sc, 0xFF, sizeof s_sc);
        s_sc.gen = s_state_gen;
        s_sc_init = 1;
    }
    return 1;
}
static ID3D11Buffer           *s_vb;            /* ring of float4 streams */
static UINT                    s_vb_size, s_vb_pos;
static ID3D11Buffer           *s_ib;
static UINT                    s_ib_size, s_ib_pos;
/* Rasteriser states: [cull][front CCW], cull 0 none, 1 back, 2 front. */
static ID3D11RasterizerState  *s_rs[3][2];
static ID3D11DepthStencilState *s_ds_off;
static ID3D11PixelShader      *s_ps;
static ID3D11PixelShader      *gp_cube_basic;

/* The pixel shaders' constants (PS b0): struct nv2a_ps_consts, whose
 * layout PS_CB_DECL must match. */
typedef struct nv2a_ps_consts PsConsts;

/* PsConsts and the legacy path's NV2APSConstants are one shader ABI. */
#define PS_ABI_SAME(a, b) \
    _Static_assert(offsetof(PsConsts, a) == offsetof(NV2APSConstants, b) \
                   && sizeof(((PsConsts *)0)->a) \
                      == sizeof(((NV2APSConstants *)0)->b), #a " != " #b)
PS_ABI_SAME(c0, c0);
PS_ABI_SAME(c1, c1);
PS_ABI_SAME(fc0, final_c0);
PS_ABI_SAME(fc1, final_c1);
PS_ABI_SAME(fog_color, fog_color);
PS_ABI_SAME(alpha_ref, alpha_ref);
PS_ABI_SAME(alpha_func, alpha_func);
PS_ABI_SAME(alpha_test_enable, alpha_test_enable);
PS_ABI_SAME(fog_enable, fog_enable);
PS_ABI_SAME(alpha_only, alpha_only);
PS_ABI_SAME(tex_scale, tex_scale);
PS_ABI_SAME(tex_mode, tex_mode);
_Static_assert(sizeof(PsConsts) == sizeof(NV2APSConstants),
               "PsConsts and NV2APSConstants differ in size");
#undef PS_ABI_SAME

#define PS_CB_DECL \
    "cbuffer CombinerCB : register(b0) {\n" \
    "    float4 c0[8]; float4 c1[8]; float4 fc0; float4 fc1; float4 fog_color;\n" \
    "    float alpha_ref; uint alpha_func; uint alpha_test_enable; uint fog_enable;\n" \
    "    uint4 alpha_only; float4 tex_scale[4]; uint4 tex_mode;\n" \
    "};\n"

static uint64_t s_presents, s_clears, s_draws, s_draws_skipped;
/* Draws dropped because CULL_FACE was FRONT_AND_BACK (D3D11 has no such
 * mode); counted apart from s_draws_skipped, which means "could not draw". */
static uint64_t s_draws_culled;
static uint64_t s_draws_bconst;   /* drawn with a CONSTANT_* blend factor */
static uint64_t s_tex_uploads, s_tex_rehash, s_tex_changed;
static uint64_t s_tex_mip_uploads;   /* uploads with more than one level */
static uint64_t s_vb_native, s_vb_expanded;  /* vertex bytes copied / made */
static uint64_t s_draws_prog, s_draws_tex;   /* with a vertex program / a texture */

/* ---- render targets: one per guest surface ------------------------------ */

/* Burnout 3 draws 14 surfaces a frame in races and Crash mode (the 640x480
 * pair, a 640x480 and a 320x240 work target, two 160x120, seven 128x128 and
 * a 64x32). With 8 slots the LRU evicted every one of them every frame: a
 * new texture seeded from guest memory each time, and, since this backend
 * writes nothing back, an off-screen one evicted before it was sampled lost
 * what was drawn into it. xemu keeps its surfaces in an unbounded list; 32
 * slots hold any frame seen so far (as the Metal backend, 73ed271). */
#define RT_MAX 32
/* render.scale keeps targets inside D3D11's 2D limit at feature level 11
 * (nv2a_host_size lowers the factor for a larger surface). */
#define D3D11_MAX_DIM 16384u
typedef struct {
    uint32_t                offset;     /* color_offset, as the title set it */
    uint32_t                addr;       /* ... resolved to a guest address */
    UINT                    w, h;       /* guest size: the key, the mapping */
    UINT                    hw, hh;     /* host size: the texture (render.scale) */
    ID3D11Texture2D        *tex;
    ID3D11RenderTargetView *rtv;
    ID3D11ShaderResourceView *srv;      /* for render-to-texture, made lazily */
    uint64_t                last_use;
    uint64_t                last_draw, last_clear;  /* s_seq at the last one */
    uint32_t                pitch;      /* the guest pitch it was made with */
    struct nv2a_rt_own      own;        /* is the guest memory under it still its own */
} RenderTarget;
static ID3D11Texture2D *gp_post_shown_tex;
static int gp_post_fused;

typedef struct nv2a_stage Stage;
static RenderTarget s_rt[RT_MAX];
static RenderTarget *s_drawn;   /* what the last flip presented */
static RenderTarget *s_cur;     /* the target of the batch being drawn */
static uint64_t      s_tick;
static uint64_t      s_seq;              /* draw/clear order */
static uint64_t      s_rtt_binds;        /* textures served from a render target */

/* ---- shaders --------------------------------------------------------------- */

#define VS_MAX 128
typedef struct {
    uint32_t            hash;     /* 0 = pass-through (no program) */
    int                 used;
    int                 bad;      /* failed to compile: skip its batches */
    uint16_t            inputs;   /* ATTR slots the shader reads */
    ID3D11VertexShader *vs;
    ID3DBlob           *code;     /* for input layouts made later */
    struct {
        uint8_t            fmt[16];  /* DXGI_FORMAT per ATTR slot, 0 unread */
        ID3D11InputLayout *il;
    } lay[8];                     /* one per combination of stream formats */
    int                 lay_next;
} VsEntry;

static VsEntry s_vs[VS_MAX];

#define BLEND_MAX 32
typedef struct {
    uint32_t          key[5];
    ID3D11BlendState *bs;
} BlendEntry;
static BlendEntry s_blend[BLEND_MAX];
static int        s_blend_n;

/* ------------------------------------------------------------------------- */

/* The pad stand-in's key table (src/input/keyboard.c); declared here rather
 * than included so this file keeps no dependency on src/input's headers. */
extern void xbox_InputKeySet(int vk, int down);
extern void xbox_InputKeysClear(void);
extern void xbox_InputSetBackground(int on);    /* src/input/input_core.c */
#include "native_win32_keyboard.h"

#include "windows_fullscreen.h"
extern void gamepkg_SetFullscreen(int fullscreen);
static struct gamepkg_window_mode gamepkg_window_mode;
static LRESULT CALLBACK wnd_proc(HWND h, UINT m, WPARAM w, LPARAM l)
{
    int shortcut = gamepkg_native_fullscreen_key(&gamepkg_window_mode,h,m,w,l);
    if(shortcut) {
        /* A plain Enter may have been down before Alt joined the chord.
         * Consuming its release must not leave the legacy game key held. */
        gamepkg_native_win32_shortcut_release(VK_RETURN, l);
        if(shortcut==2) {
            int next=!nv2a_host_opts()->fullscreen;
            if(gamepkg_native_fullscreen(&gamepkg_window_mode,h,next))
                gamepkg_SetFullscreen(next);
            else fprintf(stderr,"[GamePKG] Native fullscreen change failed: %lu\n",GetLastError());
        }
        return 0;
    }
    switch (m) {
    /* Keys for RECOMP_KEYBOARD, as the GDI window (fb_present.c) records
     * them. This window is the only one a D3D11 run has, so without these
     * the keyboard is a port-0 pad that never presses anything. */
    case WM_KEYDOWN:
    case WM_SYSKEYDOWN:
        gamepkg_native_win32_key_down(w, l);
        if (recomp_env(RENV_KEY_TRACE)) {
            static unsigned n;
            if (n++ < 40) {
                fprintf(stderr, "  [KEY] down vk=0x%02X\n", (unsigned)w);
                fflush(stderr);
            }
        }
        /* System keys still go to Windows, or Alt+F4 stops closing us. */
        return m == WM_SYSKEYDOWN ? DefWindowProcW(h, m, w, l) : 0;
    case WM_KEYUP:
    case WM_SYSKEYUP:
        gamepkg_native_win32_key_up(w, l);
        return m == WM_SYSKEYUP ? DefWindowProcW(h, m, w, l) : 0;
    /* Alt-tabbing away with a key held would leave it held for ever, and a
     * pad in the hand would go on shaking: the motors stop until the window
     * is in front again (input_core.c). */
    case WM_KILLFOCUS:
        gamepkg_native_win32_focus(0);
        return 0;
    case WM_CHAR:
        gamepkg_win32_keyboard_message(m, w, l);
        return 0;
    case WM_UNICHAR:
        if(w==UNICODE_NOCHAR)return TRUE;
        gamepkg_win32_keyboard_message(m, w, l);
        return 0;
    case WM_SETFOCUS:
        gamepkg_native_win32_focus(1);
        break;
    }
    if (m == WM_CLOSE) {
        gamepkg_native_win32_focus(0);
        /* Closing the window ends the run, like the CPU path's window.
         * ExitProcess skips atexit, where the missing-file summary prints. */
        recomp_exit_hook_run("window");
#ifdef GAMEPKG_PROFILE_GENERATE
        __llvm_profile_set_filename("gamepkg-runtime.profraw");
        int gp_profile_result = __llvm_profile_write_file();
        fprintf(stderr, "[GP-PGO] training profile write status=%d\n", gp_profile_result);
#endif
        ExitProcess(0);
    }
    if(m==WM_DESTROY){gamepkg_native_win32_focus(0);gamepkg_win32_keyboard_shutdown();}
    if((m==WM_SIZE&&w==SIZE_MINIMIZED)||(m==WM_SHOWWINDOW&&!w))gamepkg_native_win32_focus(0);
    if (m == WM_SIZE && w != SIZE_MINIMIZED && LOWORD(l) && HIWORD(l)) {
        /* Recorded only: the flip (this thread, which pumps the window)
         * resizes the swap chain before its next present. */
        s_client_w = LOWORD(l);
        s_client_h = HIWORD(l);
    }
    return DefWindowProcW(h, m, w, l);
}

static void pump(void)
{
    MSG msg;
    while (PeekMessageW(&msg, NULL, 0, 0, PM_REMOVE)) {
        if(gamepkg_win32_keyboard_skip_altgr(&msg))continue;
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}

static HRESULT gamepkg_timed_create_pixel_shader(ID3D11Device *dev,
        const void *bytes, SIZE_T n, ID3D11ClassLinkage *linkage,
        ID3D11PixelShader **shader)
{
    uint64_t begin = xbox_HostNowNs();
    HRESULT hr = ID3D11Device_CreatePixelShader(dev, bytes, n, linkage, shader);
    fprintf(stderr, LOGP "shader create pixel at flip %u: %.3f ms, hr=0x%08lX\n",
            nv2a_pb_gpu_state()->flips,
            (double)(xbox_HostNowNs() - begin) / 1e6, (unsigned long)hr);
    return hr;
}

/* Exact generated source and target identify a bytecode result. Only this
 * player's local cache is used; no ROM-derived shaders are packaged. */
#include "nv2a_pcache.h"
#include <d3d11shader.h>
#define GP_CACHE_RECORD 0x43425844u
#define GP_CACHE_MEM 128
static void gp_cache_prepare_manifest(const char *dir);
static nv2a_pcache *gp_cache;
static int gp_cache_started;
static uint64_t gp_cache_hits, gp_cache_misses;
static struct { char *source, *target; ID3DBlob *blob; int unindexed; } gp_mem[GP_CACHE_MEM];
static unsigned gp_mem_next;
/* The upstream key list exposes its open-time snapshot. Once a newly
 * appended shader leaves our small blob memo, refresh that snapshot before
 * treating a miss as a compile. Reopen at most once per memo generation. */
static int gp_cache_refresh_needed;
static char gp_cache_path[1200], gp_cache_version[200];
static uint32_t gp_get32(const unsigned char *p) {
 return (uint32_t)p[0]|(uint32_t)p[1]<<8|(uint32_t)p[2]<<16|(uint32_t)p[3]<<24;
}
static void gp_put32(unsigned char *p,uint32_t x) {
 for(unsigned i=0;i<4;i++)p[i]=(unsigned char)(x>>(8*i));
}
static int gp_payload(const void *data,uint32_t length,const char *source,
 const char *target,const void **bytes,uint32_t *nbytes) {
 const unsigned char *p=data;
 if(!data||!source||!target||length<12)return 0;
 uint32_t ns=gp_get32(p),nt=gp_get32(p+4),nb=gp_get32(p+8);
 if((uint64_t)ns+nt+nb+12!=length||!ns||!nt||nb<32||
    ns!=strlen(source)||nt!=strlen(target))return 0;
 if(memcmp(p+12,source,ns)||memcmp(p+12+ns,target,nt)||
    memcmp(p+12+ns+nt,"DXBC",4))return 0;
 *bytes=p+12+ns+nt;*nbytes=nb;return 1;
}
static void gp_cache_finish(void) {
 if(gp_cache){nv2a_pcache_close(gp_cache);gp_cache=NULL;}
 for(unsigned i=0;i<GP_CACHE_MEM;i++){
  free(gp_mem[i].source);free(gp_mem[i].target);
  if(gp_mem[i].blob)ID3D10Blob_Release(gp_mem[i].blob);
  memset(&gp_mem[i],0,sizeof gp_mem[i]);
 }
}
static void gp_cache_start(void) {
 if(gp_cache_started)return;gp_cache_started=1;
 const char *cfg=recomp_env(RENV_RENDER_SHADER_CACHE);
 if(cfg&&(!*cfg||!strcmp(cfg,"none")||!strcmp(cfg,"off")||!strcmp(cfg,"0")))return;
 char dir[1024],path[1200],module[MAX_PATH],version[200],message[256];
 WIN32_FILE_ATTRIBUTE_DATA attr;
 HMODULE dll=GetModuleHandleA("d3dcompiler_47.dll");
 if(!dll||!GetModuleFileNameA(dll,module,MAX_PATH)||
    !GetFileAttributesExA(module,GetFileExInfoStandard,&attr))return;
 if(!nv2a_pcache_dir(cfg?cfg:"./shader-cache",nv2a_pcache_app(),dir,sizeof dir)||
    snprintf(path,sizeof path,"%s/gamepkg-d3d11-dxbc.bin",dir)>=(int)sizeof path)return;
 /* A large or foreign file is optional data, never a reason to fail play. */
 WIN32_FILE_ATTRIBUTE_DATA existing;
 if(GetFileAttributesExA(path,GetFileExInfoStandard,&existing)&&
    (existing.nFileSizeHigh||existing.nFileSizeLow>128u*1024u*1024u))return;
 snprintf(version,sizeof version,"gamepkg-dxbc-source-v1-o3-%08lx%08lx-%08lx%08lx",
  (unsigned long)attr.ftLastWriteTime.dwHighDateTime,(unsigned long)attr.ftLastWriteTime.dwLowDateTime,
  (unsigned long)attr.nFileSizeHigh,(unsigned long)attr.nFileSizeLow);
 gp_cache=nv2a_pcache_open(path,version,message,sizeof message);
 if(gp_cache){fprintf(stderr,LOGP "local shader bytecode cache: %s (%s)\n",message,path);atexit(gp_cache_finish);}
 if(gp_cache) {
  memcpy(gp_cache_path,path,strlen(path)+1);
  memcpy(gp_cache_version,version,strlen(version)+1);
  gp_cache_prepare_manifest(dir);
 }
}
static void gp_cache_refresh(void) {
 char message[256];
 gp_cache_refresh_needed=0;
 if(!gp_cache)return;
 /* No pointers into the key list escape a lookup. Blob references in the
  * memo own their bytes, so flushing/reopening cannot invalidate them. */
 nv2a_pcache_close(gp_cache);gp_cache=NULL;
 WIN32_FILE_ATTRIBUTE_DATA attributes;
 if(!GetFileAttributesExA(gp_cache_path,GetFileExInfoStandard,&attributes)||
    attributes.nFileSizeHigh||attributes.nFileSizeLow>128u*1024u*1024u)return;
 gp_cache=nv2a_pcache_open(gp_cache_path,gp_cache_version,message,sizeof message);
 if(gp_cache)for(unsigned i=0;i<GP_CACHE_MEM;i++)gp_mem[i].unindexed=0;
}
static void gp_remember(const char *source,const char *target,ID3DBlob *blob,int unindexed) {
 char *s=_strdup(source),*t=_strdup(target);if(!s||!t){free(s);free(t);return;}
 unsigned i=gp_mem_next++%GP_CACHE_MEM;
 if(gp_mem[i].unindexed)gp_cache_refresh_needed=1;
 free(gp_mem[i].source);free(gp_mem[i].target);
 if(gp_mem[i].blob)ID3D10Blob_Release(gp_mem[i].blob);
 gp_mem[i].source=s;gp_mem[i].target=t;gp_mem[i].blob=blob;gp_mem[i].unindexed=unindexed;ID3D10Blob_AddRef(blob);
}
static int gp_cache_load(const char *source,const char *target,ID3DBlob **out) {
 gp_cache_start();
 if(!gp_cache)return 0;
 for(unsigned i=0;i<GP_CACHE_MEM;i++)if(gp_mem[i].blob&&
  !strcmp(gp_mem[i].source,source)&&!strcmp(gp_mem[i].target,target)){
  *out=gp_mem[i].blob;ID3D10Blob_AddRef(*out);gp_cache_hits++;return 1;
 }
 int refreshed=0;
search_disk:
 for(size_t i=0;i<nv2a_pcache_count(gp_cache);i++){
  uint32_t type,len,nb;const void *data,*bytes;
  if(!nv2a_pcache_get(gp_cache,i,&type,&data,&len)||type!=GP_CACHE_RECORD||
     !gp_payload(data,len,source,target,&bytes,&nb))continue;
  ID3D11ShaderReflection *reflection=NULL;
  if(FAILED(D3DReflect(bytes,nb,&IID_ID3D11ShaderReflection,(void **)&reflection))||!reflection)continue;
  reflection->lpVtbl->Release(reflection);
  ID3DBlob *blob=NULL;if(FAILED(D3DCreateBlob(nb,&blob)))return 0;
  memcpy(ID3D10Blob_GetBufferPointer(blob),bytes,nb);*out=blob;
  gp_remember(source,target,blob,0);gp_cache_hits++;return 1;
 }
 if(!refreshed&&gp_cache_refresh_needed){
  refreshed=1;gp_cache_refresh();
  if(gp_cache)goto search_disk;
 }
 return 0;
}
static void gp_cache_store(const char *source,const char *target,ID3DBlob *blob) {
 if(!gp_cache)return;
 size_t ns=strlen(source),nt=strlen(target),nb=ID3D10Blob_GetBufferSize(blob);
 if(ns>NV2A_PCACHE_MAX_LEN||nt>NV2A_PCACHE_MAX_LEN||nb>NV2A_PCACHE_MAX_LEN||
    ns+nt+nb+12>NV2A_PCACHE_MAX_LEN)return;
 uint32_t len=(uint32_t)(ns+nt+nb+12);unsigned char *p=malloc(len);if(!p)return;
 gp_put32(p,(uint32_t)ns);gp_put32(p+4,(uint32_t)nt);gp_put32(p+8,(uint32_t)nb);
 memcpy(p+12,source,ns);memcpy(p+12+ns,target,nt);
 memcpy(p+12+ns+nt,ID3D10Blob_GetBufferPointer(blob),nb);
 int stored=nv2a_pcache_put(gp_cache,GP_CACHE_RECORD,p,len);free(p);
 gp_remember(source,target,blob,stored>=0);
}

static int compile(const char *what, const char *src, const char *target,
                   ID3DBlob **code)
{
    ID3DBlob *err = NULL;
    uint64_t gp_lookup_begin = xbox_HostNowNs();
    if (gp_cache_load(src, target, code)) {
        fprintf(stderr, LOGP "shader cache hit %s target %s at flip %u: %.3f ms\n",
                what, target, nv2a_pb_gpu_state()->flips,
                (double)(xbox_HostNowNs() - gp_lookup_begin) / 1e6);
        return 1;
    }
    gp_cache_misses++;
    uint64_t gp_compile_begin = xbox_HostNowNs();
    HRESULT hr = s_compile(src, strlen(src), what, NULL, NULL, "main", target,
                           D3DCOMPILE_OPTIMIZATION_LEVEL3, 0, code, &err);
    uint64_t gp_compile_ns = xbox_HostNowNs() - gp_compile_begin;
    fprintf(stderr, LOGP "shader compile %s target %s at flip %u: %.3f ms, hr=0x%08lX\n",
            what, target, nv2a_pb_gpu_state()->flips,
            (double)gp_compile_ns / 1e6, (unsigned long)hr);
    if (FAILED(hr)) {
        fprintf(stderr, LOGP "compile %s failed hr=0x%08lX\n%s\n", what,
                (unsigned long)hr,
                err ? (const char *)ID3D10Blob_GetBufferPointer(err) : "");
        if (recomp_env(RENV_D3D11_VERBOSE))
            fprintf(stderr, "--- source ---\n%s\n--- end ---\n", src);
        if (err) ID3D10Blob_Release(err);
        return 0;
    }
    if (err) ID3D10Blob_Release(err);
    gp_cache_store(src, target, *code);
    return 1;
}

/* Reuse the port's coverage manifest, resolving vertex bodies from its
 * read-only virtual ISO path. Prepare bytecode only: no live GPU state is
 * changed, and the renderer's shader-object slots remain available. */
/* Keep the legacy 2D variant and the original definition too: prewarming changes neither live state nor
 * the shader the draw asks for. Only local derived bytecode is retained. */
static int gp_prepare_legacy_texture_modes(const NV2ACombinerState *original,
                                          NV2ACombinerState *variant) {
 memcpy(variant,original,sizeof *variant);int changed=0;
 for(unsigned i=0;i<4;i++) {
  NV2ATextureMode mode=variant->tex_mode[i];
  if(mode!=NV2A_TEXMODE_NONE&&mode!=NV2A_TEXMODE_PASSTHRU&&
     mode!=NV2A_TEXMODE_CLIPPLANE&&mode!=NV2A_TEXMODE_2D) {
   variant->tex_mode[i]=NV2A_TEXMODE_2D;changed=1;
  }
 }
 return changed;
}
/* The live draw and shader prewarm use exactly the same resource types. */
static unsigned gp_native_texture_mode(unsigned mode,unsigned raw,int cube) {
 if(raw==5)return 5;
 if(mode==3&&cube)return 3;
 return mode&&mode!=4?1u:mode;
}
static void gp_prepare_native_texture_modes(const NV2ACombinerState *original,
                                           unsigned cube_mask,NV2ACombinerState *variant) {
 memcpy(variant,original,sizeof *variant);
 for(unsigned i=0;i<4;i++)variant->tex_mode[i]=(NV2ATextureMode)gp_native_texture_mode(
   (unsigned)original->tex_mode[i],(unsigned)original->tex_mode[i],(cube_mask>>i)&1u);
}
static int gp_prepare_pixel(const NV2ACombinerState *state,const char *name) {
 static char text[65536];ID3DBlob *blob=NULL;
 if(d3d8_combiners_generate_hlsl(state,text,sizeof text)<0)return 0;
 int ok=compile(name,text,"ps_5_0",&blob);
 if(blob)ID3D10Blob_Release(blob);
 return ok;
}
static void gp_cache_prepare_manifest(const char *dir) {
 char path[1200],version[200],legacy[200],extra_version[200],message[256];
 snprintf(legacy,sizeof legacy,"vk pipelines 1; PsoDisk 48; NV2ACombinerState %zu; vsh slots %u",
          sizeof(NV2ACombinerState),(unsigned)NV2A_VSH_SLOTS);
 snprintf(version,sizeof version,"gamepkg d3d11 prewarm keys 1; NV2ACombinerState %zu; vsh slots %u",sizeof(NV2ACombinerState),(unsigned)NV2A_VSH_SLOTS);
 snprintf(extra_version,sizeof extra_version,"gamepkg d3d11 coverage 1; NV2ACombinerState %zu",sizeof(NV2ACombinerState));
 if(snprintf(path,sizeof path,"%s/gamepkg-d3d11-prewarm-keys.bin",dir)>=(int)sizeof path)return;
 WIN32_FILE_ATTRIBUTE_DATA a;
 if(GetFileAttributesExA(path,GetFileExInfoStandard,&a)&&
    (a.nFileSizeHigh||a.nFileSizeLow>128u*1024u*1024u))return;
 nv2a_pcache *keys=nv2a_pcache_open(path,version,message,sizeof message);
 if(!keys)return;
 nv2a_pcache_manifest_stat stat,extra;
 nv2a_pcache_manifest_apply(keys,"vulkan",legacy,&stat);
 nv2a_pcache_manifest_apply(keys,"d3d11",extra_version,&extra);
 nv2a_pcache_close(keys);
 if(!stat.found&&!extra.found)return;
 keys=nv2a_pcache_open(path,version,message,sizeof message);if(!keys)return;
 uint64_t begin=xbox_HostNowNs();unsigned vertices=0,pixels=0,failed=0;
 static char text[65536];static NV2AVshProgram parsed;
 for(size_t i=0;i<nv2a_pcache_count(keys);i++){
  uint32_t type,n;const void *data;ID3DBlob *blob=NULL;int ok=0;
  if(!nv2a_pcache_get(keys,i,&type,&data,&n))continue;
  if(type==NV2A_PC_VS){
   if(!n||n%16||n/16>NV2A_VSH_SLOTS)continue;
   uint32_t slots[NV2A_VSH_SLOTS][4];memcpy(slots,data,n);
   unsigned count=n/16;int valid=1;
   for(unsigned k=0;k<count;k++)if(slots[k][0]||((slots[k][3]&1)!=0)!=(k==count-1)){valid=0;break;}
   if(!valid)continue;
   d3d8_vsh_parse(&slots[0][0],(int)count,&parsed);
   if(d3d8_vsh_generate_hlsl_ex(&parsed,NV2A_VSH_HLSL_SCREEN_SPACE,text,sizeof text)<=0)continue;
   ok=compile("gamepkg_prewarm_vertex",text,"vs_5_0",&blob);vertices++;
  }else if(type==NV2A_PC_FS&&n==sizeof(NV2ACombinerState)){
   NV2ACombinerState state;memcpy(&state,data,sizeof state);
   if(state.num_stages<0||state.num_stages>NV2A_MAX_COMBINER_STAGES)continue;
   pixels++;if(!gp_prepare_pixel(&state,"gamepkg_prewarm_pixel"))failed++;
   NV2ACombinerState variant;
   if(gp_prepare_legacy_texture_modes(&state,&variant)) {
    pixels++;if(!gp_prepare_pixel(&variant,"gamepkg_prewarm_legacy_pixel"))failed++;
   }
   unsigned cube_mask=0;
   for(unsigned i=0;i<4;i++)if(state.tex_mode[i]==NV2A_TEXMODE_CUBEMAP)cube_mask|=1u<<i;
   /* At most 15 additional typed combinations. Formats are live resource
    * state, not stored in the combiner coverage manifest. Include mixed
    * cube/legacy modes before gameplay rather than compiling on a draw. */
   for(unsigned mask=1;mask<16;mask++)if(!(mask&~cube_mask)) {
    gp_prepare_native_texture_modes(&state,mask,&variant);
    if(memcmp(&state,&variant,sizeof state)) {
     pixels++;if(!gp_prepare_pixel(&variant,"gamepkg_prewarm_native_pixel"))failed++;
    }
   }
   continue;
  }else continue;
  if(!ok)failed++;
  if(blob)ID3D10Blob_Release(blob);
 }
 nv2a_pcache_close(keys);
 fprintf(stderr,LOGP "prepared local shaders before gameplay: %u vertex, %u pixel, %u failures in %.3f ms; manifest vertex resolution %u/%u\n",
  vertices,pixels,failed,(double)(xbox_HostNowNs()-begin)/1e6,stat.vs_resolved,stat.vs_listed);
}

static ID3D11Buffer *make_buffer(UINT size, UINT bind)
{
    D3D11_BUFFER_DESC bd;
    ID3D11Buffer *b = NULL;
    memset(&bd, 0, sizeof bd);
    bd.ByteWidth = size;
    bd.Usage = D3D11_USAGE_DYNAMIC;
    bd.BindFlags = bind;
    bd.CPUAccessFlags = D3D11_CPU_ACCESS_WRITE;
    if (FAILED(ID3D11Device_CreateBuffer(s_dev, &bd, NULL, &b)))
        return NULL;
    return b;
}

/* The VS output struct d3d8_vsh.c emits. A pixel shader that declares the
 * same struct links with any of its vertex shaders. */
#define VS_OUT_DECL \
    "struct VS_OUT {\n" \
    "    float4 oPos : SV_POSITION;\n" \
    "    float4 oD0  : COLOR0;\n" \
    "    float4 oD1  : COLOR1;\n" \
    "    float4 oT0  : TEXCOORD0;\n" \
    "    float4 oT1  : TEXCOORD1;\n" \
    "    float4 oT2  : TEXCOORD2;\n" \
    "    float4 oT3  : TEXCOORD3;\n" \
    "    float  oFog : FOG;\n" \
    "    float  oPts : PSIZE;\n" \
    "    float4 oB0  : TEXCOORD4;\n" \
    "    float4 oB1  : TEXCOORD5;\n" \
    "};\n"

/* Batches without register combiners: the CPU path's MODULATE, oD0 * T0
 * when stage 0 is on, then the alpha test (D3DCMP numbering, on the 8-bit
 * values the way the CPU compares them). */
static const char s_ps_src[] =
    VS_OUT_DECL
    PS_CB_DECL
    "Texture2D tex0 : register(t0);\n"
    "SamplerState samp0 : register(s0);\n"
    "float4 main(VS_OUT i) : SV_Target {\n"
    "    float4 c = i.oD0;\n"
    "    if (tex_mode.x == 4u) {\n"
    "        c *= saturate(i.oT0);\n"
    "    } else if (tex_mode.x != 0u) {\n"
    "        float2 uv = i.oT0.xy;\n"
    "        if (tex_mode.x == 1u && i.oT0.w != 0.0 && i.oT0.w != 1.0) uv /= i.oT0.w;\n"
    "        c *= tex0.Sample(samp0, uv * tex_scale[0].xy);\n"
    "    }\n"
    "    if (alpha_test_enable != 0u) {\n"
    "        uint a = (uint)(saturate(c.a) * 255.0 + 0.5), r = (uint)(alpha_ref * 255.0 + 0.5);\n"
    "        bool ok = true;\n"
    "        if      (alpha_func == 1u) ok = false;\n"
    "        else if (alpha_func == 2u) ok = a <  r;\n"
    "        else if (alpha_func == 3u) ok = a == r;\n"
    "        else if (alpha_func == 4u) ok = a <= r;\n"
    "        else if (alpha_func == 5u) ok = a >  r;\n"
    "        else if (alpha_func == 6u) ok = a != r;\n"
    "        else if (alpha_func == 7u) ok = a >= r;\n"
    "        if (!ok) discard;\n"
    "    }\n"
    "    return c;\n"
    "}\n";

static const char gp_cube_basic_src[] =
    VS_OUT_DECL
    PS_CB_DECL
    "TextureCube tex0 : register(t0);\n"
    "SamplerState samp0 : register(s0);\n"
    "float4 main(VS_OUT i) : SV_Target {\n"
    "    float4 c = i.oD0;\n"
    "    if (tex_mode.x == 4u) {\n"
    "        c *= saturate(i.oT0);\n"
    "    } else if (tex_mode.x != 0u) {\n"
    "        float2 uv = i.oT0.xy;\n"
    "        if (tex_mode.x == 1u && i.oT0.w != 0.0 && i.oT0.w != 1.0) uv /= i.oT0.w;\n"
    "        c *= tex0.Sample(samp0, i.oT0.xyz);\n"
    "    }\n"
    "    if (alpha_test_enable != 0u) {\n"
    "        uint a = (uint)(saturate(c.a) * 255.0 + 0.5), r = (uint)(alpha_ref * 255.0 + 0.5);\n"
    "        bool ok = true;\n"
    "        if      (alpha_func == 1u) ok = false;\n"
    "        else if (alpha_func == 2u) ok = a <  r;\n"
    "        else if (alpha_func == 3u) ok = a == r;\n"
    "        else if (alpha_func == 4u) ok = a <= r;\n"
    "        else if (alpha_func == 5u) ok = a >  r;\n"
    "        else if (alpha_func == 6u) ok = a != r;\n"
    "        else if (alpha_func == 7u) ok = a >= r;\n"
    "        if (!ok) discard;\n"
    "    }\n"
    "    return c;\n"
    "}\n";

/* Batches without a vertex program: attribute 0 is already in surface pixels
 * (x, y, z, rhw); 3 and 4 are the diffuse and specular colours, 9..12 the
 * texture coordinates. */
static const char s_vs_passthrough_src[] =
    "cbuffer VSH_Viewport : register(b2) { float4 vp_scale; float4 vp_off; };\n"
    "struct VS_IN { float4 v0 : ATTR0; float4 v3 : ATTR3; float4 v4 : ATTR4;\n"
    "               float4 v9 : ATTR9; float4 v10 : ATTR10; float4 v11 : ATTR11;\n"
    "               float4 v12 : ATTR12; };\n"
    VS_OUT_DECL
    "VS_OUT main(VS_IN i) {\n"
    "    VS_OUT o = (VS_OUT)0;\n"
    "    o.oPos = float4(i.v0.xy * vp_scale.xy + vp_off.xy,\n"
    "                    saturate(i.v0.z * vp_scale.z), 1.0);\n"
    "    o.oD0 = i.v3; o.oD1 = i.v4; o.oFog = 1.0;\n"
    "    o.oT0 = i.v9; o.oT1 = i.v10; o.oT2 = i.v11; o.oT3 = i.v12;\n"
    "    return o;\n"
    "}\n";

static void create_black_texture(void);


/* RECOMP_VBLANK_SYNC=display: a thread that waits for each vertical blank of
 * the swap chain's output (IDXGIOutput::WaitForVBlank) and reports it to the
 * vblank timer (kernel_pacing.c), which then keeps the emulated vblank in
 * phase with the display. */
static DWORD WINAPI vblank_link_thread(LPVOID param)
{
    IDXGIOutput *out = (IDXGIOutput *)param;
    for (;;) {
        if (FAILED(IDXGIOutput_WaitForVBlank(out))) {
            Sleep(16);
            continue;
        }
        xbox_VblankDisplayRefresh(xbox_HostNowNs(), 0);
    }
    return 0;
}

static void vblank_link_start(void)
{
    IDXGIOutput *out = NULL;
    HANDLE t;
    if (!xbox_VblankDisplayLockOn() || !s_swap)
        return;
    if (FAILED(IDXGISwapChain_GetContainingOutput(s_swap, &out)) || !out) {
        fprintf(stderr, LOGP "vblank sync: no output to wait on; the vblank keeps"
                " its own 60 Hz timer\n");
        return;
    }
    t = CreateThread(NULL, 0, vblank_link_thread, out, 0, NULL);
    if (t) {
        SetThreadPriority(t, THREAD_PRIORITY_TIME_CRITICAL);
        CloseHandle(t);
        fprintf(stderr, LOGP "vblank sync: the emulated vblank follows the output's"
                " vertical blank (RECOMP_VBLANK_SYNC=display)\n");
    }
}

static uint64_t gamepkg_native_started_ms, gamepkg_native_quit_ms;
static int init(void)
{
    static int tried;
    HMODULE dll;
    DXGI_SWAP_CHAIN_DESC sd;
    D3D_FEATURE_LEVEL fl;
    HRESULT hr;
    WNDCLASSW wc;
    wchar_t title[192];
    RECT r = {0, 0, 640, 480};
    int full = nv2a_host_opts()->fullscreen;

    UINT bw = 640, bh = 480;

    if (tried)
        return !s_failed;
    tried = 1;
    s_failed = 1;
    /* Explicit --seconds only; no default watchdog or frame limit. */
    const char *quit_text=recomp_env(RENV_WINDOW_QUIT_AFTER);
    char *quit_end=NULL;
    if(quit_text && *quit_text) {
        unsigned long long seconds=strtoull(quit_text,&quit_end,10);
        if(quit_end && !*quit_end && seconds && seconds<=UINT64_MAX/1000)
            gamepkg_native_quit_ms=(uint64_t)seconds*1000;
    }
    gamepkg_native_started_ms=GetTickCount64();

    dll = LoadLibraryA("d3dcompiler_47.dll");
    if (dll)
        s_compile = (compile_fn)(void (*)(void))GetProcAddress(dll, "D3DCompile");
    if (!s_compile) {
        fprintf(stderr, LOGP "no D3DCompile in d3dcompiler_47.dll\n");
        return 0;
    }

    memset(&wc, 0, sizeof wc);
    wc.lpfnWndProc = wnd_proc;
    wc.hInstance = GetModuleHandleW(NULL);
    wc.hCursor = LoadCursorW(NULL, MAKEINTRESOURCEW(32512));
    wc.lpszClassName = L"" D3D11_WND_CLASS;
    RegisterClassW(&wc);
    if (!xbox_FramebufferWindowTitleText(title, 192, &s_title_clock))
        title[0] = 0;
    if (full) {
        /* present.fullscreen: borderless over the primary monitor (no HWND
         * yet to ask MonitorFromWindow), no exclusive mode. */
        bw = (UINT)GetSystemMetrics(SM_CXSCREEN);
        bh = (UINT)GetSystemMetrics(SM_CYSCREEN);
        if (!bw || !bh) {
            bw = 640;
            bh = 480;
            full = 0;
        }
    }
    DWORD visible = recomp_env_on(RENV_HEADLESS) ? 0 : WS_VISIBLE;
#ifdef GAMEPKG_NATIVE_TEST
    if(getenv("GAMEPKG_TEST_HIDDEN_WINDOW") && !strcmp(getenv("GAMEPKG_TEST_HIDDEN_WINDOW"),"1"))visible=0;
#endif
    if (full) {
        s_hwnd = CreateWindowW(L"" D3D11_WND_CLASS, title,
                               WS_POPUP | visible, 0, 0, (int)bw, (int)bh,
                               NULL, NULL, wc.hInstance, NULL);
    } else {
        AdjustWindowRect(&r, WS_OVERLAPPEDWINDOW, FALSE);
        s_hwnd = CreateWindowW(L"" D3D11_WND_CLASS, title,
                               WS_OVERLAPPEDWINDOW | visible,
                               CW_USEDEFAULT, CW_USEDEFAULT,
                               r.right - r.left, r.bottom - r.top,
                               NULL, NULL, wc.hInstance, NULL);
    }
    if (!s_hwnd) {
        fprintf(stderr, LOGP "CreateWindow failed (%lu)\n", GetLastError());
        return 0;
    }
    s_sync_interval = recomp_env_on(RENV_PRESENT_VSYNC) ? 1 : 0;
    {
        const char *v = recomp_env(RENV_PRESENT_VSYNC);
        if (v && !strcmp(v, "fifo")) {
            s_sync_interval = 1;
            s_swap_flags = DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT;
        }
    }
    if (recomp_env(RENV_PRESENT_VSYNC))
        fprintf(stderr, LOGP "present vsync %s (RECOMP_PRESENT_VSYNC=%s)\n",
                s_sync_interval ? "on" : "off", recomp_env(RENV_PRESENT_VSYNC));

    memset(&sd, 0, sizeof sd);
    sd.BufferDesc.Width = bw;
    sd.BufferDesc.Height = bh;
    sd.BufferDesc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    sd.SampleDesc.Count = 1;
    sd.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
    sd.BufferCount = 2;
    sd.OutputWindow = s_hwnd;
    sd.Windowed = TRUE;
    sd.SwapEffect = DXGI_SWAP_EFFECT_FLIP_DISCARD;
    sd.Flags = s_swap_flags;
    hr = D3D11CreateDeviceAndSwapChain(NULL, D3D_DRIVER_TYPE_HARDWARE, NULL, 0,
                                       NULL, 0, D3D11_SDK_VERSION, &sd,
                                       &s_swap, &s_dev, &fl, &s_ctx);
    if (FAILED(hr)) {
        fprintf(stderr, LOGP "D3D11CreateDeviceAndSwapChain hr=0x%08lX\n",
                (unsigned long)hr);
        return 0;
    }
    if (s_swap_flags) {
        IDXGISwapChain2 *sc2 = NULL;
        const char *e = recomp_env(RENV_PRESENT_LATENCY);
        UINT lat = e ? (UINT)atoi(e) : 1;
        if (lat < 1 || lat > 3)
            lat = 1;
        if (SUCCEEDED(IDXGISwapChain_QueryInterface(s_swap, &IID_IDXGISwapChain2,
                                                    (void **)&sc2))) {
            IDXGISwapChain2_SetMaximumFrameLatency(sc2, lat);
            s_latency_wait = IDXGISwapChain2_GetFrameLatencyWaitableObject(sc2);
            IDXGISwapChain2_Release(sc2);
        }
        fprintf(stderr, LOGP "present: FIFO, 2 buffers, max frame latency %u%s\n",
                lat, s_latency_wait ? "" : " (no waitable: Present blocks instead)");
    }
    vblank_link_start();
    IDXGISwapChain_GetBuffer(s_swap, 0, &IID_ID3D11Texture2D, (void **)&s_back);
    if (FAILED(ID3D11DeviceContext_QueryInterface(s_ctx, &IID_ID3D11DeviceContext1,
                                                  (void **)&s_ctx1)))
        s_ctx1 = NULL;
    s_back_w = s_made_w = bw;
    s_back_h = s_made_h = bh;

    s_cb_consts = make_buffer(NV2A_VSH_CONSTANTS * 16, D3D11_BIND_CONSTANT_BUFFER);
    s_cb_vp     = make_buffer(80, D3D11_BIND_CONSTANT_BUFFER);
    s_cb_ps     = make_buffer(sizeof(PsConsts), D3D11_BIND_CONSTANT_BUFFER);
    s_vb_size   = 8u << 20;
    s_vb        = make_buffer(s_vb_size, D3D11_BIND_VERTEX_BUFFER);
    s_ib_size   = NV_MAX_INDICES * 3u * 4u;  /* any accepted batch fits */
    s_ib        = make_buffer(s_ib_size, D3D11_BIND_INDEX_BUFFER);
    if (!s_cb_consts || !s_cb_vp || !s_cb_ps || !s_vb || !s_ib) {
        fprintf(stderr, LOGP "buffer creation failed\n");
        return 0;
    }
    {
        D3D11_RASTERIZER_DESC rd;
        memset(&rd, 0, sizeof rd);
        rd.FillMode = D3D11_FILL_SOLID;
        int c, f;
        /* The vertex programs hand D3D11 real clip-space positions (see the
         * oPos epilogue in d3d8_vsh.c: the title's divide by w is undone, w
         * kept), so the rasteriser clips triangles with vertices behind the
         * eye against the near plane instead of dropping them, as the CPU
         * path has to. Depth clipping stays on for that; NV2A depth runs
         * 0..zmax, which vp_scale.z maps onto D3D's 0..1. */
        rd.DepthClipEnable = TRUE;
        /* Culling as the CPU path does it (nv2a_pb_exec.c), from
         * SET_CULL_FACE_ENABLE / CULL_FACE / FRONT_FACE: rasterizer_state().
         * The vertex program's pixels map onto the target unflipped (y down
         * on both), so a triangle clockwise as displayed is clockwise to
         * D3D11 too. */
        for (c = 0; c < 3; c++)
            for (f = 0; f < 2; f++) {
                rd.CullMode = c == 0 ? D3D11_CULL_NONE
                            : c == 1 ? D3D11_CULL_BACK : D3D11_CULL_FRONT;
                rd.FrontCounterClockwise = f;
                ID3D11Device_CreateRasterizerState(s_dev, &rd, &s_rs[c][f]);
            }
    }
    {
        D3D11_DEPTH_STENCIL_DESC dd;
        memset(&dd, 0, sizeof dd);
        dd.DepthEnable = FALSE;
        dd.DepthWriteMask = D3D11_DEPTH_WRITE_MASK_ZERO;
        dd.DepthFunc = D3D11_COMPARISON_ALWAYS;
        ID3D11Device_CreateDepthStencilState(s_dev, &dd, &s_ds_off);
    }
    {
        ID3DBlob *code = NULL;
        if (!compile("nv2a_ps_diffuse", s_ps_src, "ps_5_0", &code))
            return 0;
        gamepkg_timed_create_pixel_shader(s_dev, ID3D10Blob_GetBufferPointer(code),
                                       ID3D10Blob_GetBufferSize(code), NULL,
                                       &s_ps);
        ID3D10Blob_Release(code);
    }

    create_black_texture();
    fprintf(stderr, LOGP "device up: feature level 0x%X, swap chain %ux%u\n",
            (unsigned)fl, s_back_w, s_back_h);
    {
        ID3DBlob *code=NULL;
        if(!compile("nv2a_ps_diffuse_cube",gp_cube_basic_src,"ps_5_0",&code))return 0;
        HRESULT hr=gamepkg_timed_create_pixel_shader(s_dev,ID3D10Blob_GetBufferPointer(code),ID3D10Blob_GetBufferSize(code),NULL,&gp_cube_basic);
        ID3D10Blob_Release(code);if(FAILED(hr))return 0;
    }
    s_failed = 0;
    {
        IDXGIDevice *device = NULL; IDXGIAdapter *adapter = NULL;
        DXGI_ADAPTER_DESC description; char name[512] = "hardware D3D11 adapter";
        if (SUCCEEDED(ID3D11Device_QueryInterface(s_dev, &IID_IDXGIDevice, (void **)&device))) {
            if (SUCCEEDED(IDXGIDevice_GetAdapter(device, &adapter))) {
                if (SUCCEEDED(IDXGIAdapter_GetDesc(adapter, &description))) {
                    description.Description[127] = 0;
                    WideCharToMultiByte(CP_UTF8, 0, description.Description, -1, name, sizeof name, NULL, NULL);
                }
                IDXGIAdapter_Release(adapter);
            }
            IDXGIDevice_Release(device);
        }
        fprintf(stderr, LOGP "backend up on %s: hardware D3D11, %s window\n", name, visible ? "visible" : "headless");
    }
    if (visible) fputs("[PRESENT] window up: native D3D11\n", stderr);
    return 1;
}

/* ---- surfaces ------------------------------------------------------------- */

static void rt_release(RenderTarget *rt)
{
    s_sc_init = 0; /* Released views cannot be retained by the shadow. */
    if (s_drawn == rt)
        s_drawn = NULL;
    if (rt->srv)
        ID3D11ShaderResourceView_Release(rt->srv);
    ID3D11RenderTargetView_Release(rt->rtv);
    ID3D11Texture2D_Release(rt->tex);
    memset(rt, 0, sizeof *rt);
}

/* ---- render-target ownership (nv2a_backend_common.h) ----------------------
 *
 * A target is the title's surface only while the guest memory under it holds
 * what it held when the target was made. A title that frees a target's
 * memory and loads something else there (BLiNX 2's hub leaves two 512x512
 * targets whose megabytes stage 1 refills with textures) would otherwise get
 * the old image bound in place of the new texels. This backend never writes
 * guest memory, so a stale target is only dropped. The Metal backend's rule;
 * A/B: RECOMP_DEBUG=rt_alias_check=0. */
static uint64_t s_stale_drops, s_retired;
static uint64_t s_grows, s_partial_clears;   /* rt_grow, clear_region (as Metal's) */

static int alias_check_on(void)
{
    static int on = -1;
    if (on < 0) {
        const char *e = recomp_env(RENV_RT_ALIAS_CHECK);
        on = !(e && e[0] == '0');
    }
    return on;
}

/* Drop rt when the title has rewritten its memory; 1 if it was dropped. */
static int rt_drop_if_stale(RenderTarget *rt, const char *why)
{
    static int said;
    if (!alias_check_on() || !rt->tex || !rt->pitch
            || !nv2a_rt_own_stale(&rt->own, (const uint8_t *)xbox_GetMemoryOffset(),
                                  rt->addr, rt->pitch, rt->h, (uint32_t)s_presents))
        return 0;
    s_stale_drops++;
    if (said++ < 20)
        fprintf(stderr, LOGP "surface 0x%08X: %ux%u render target dropped (%s): the title"
                " rewrote its %u bytes; last drawn at flip %u, now %llu\n",
                rt->offset, rt->w, rt->h, why, rt->pitch * rt->h, rt->own.draw_flip,
                (unsigned long long)s_presents);
    rt_release(rt);
    return 1;
}

/* rt made w x h (guest pixels), what it holds kept at its top left. The new
 * area starts black: the guest bytes are there, but this backend never seeds
 * a target from guest memory (the title clears or draws it first). */
static int rt_grow(RenderTarget *rt, UINT w, UINT h)
{
    static const float black[4] = {0, 0, 0, 0};
    static int said;
    D3D11_TEXTURE2D_DESC td;
    ID3D11Texture2D *tex = NULL;
    ID3D11RenderTargetView *rtv = NULL;
    D3D11_BOX b;
    UINT hw, hh;

    nv2a_host_size(w, h, nv2a_host_render_scale(), D3D11_MAX_DIM, &hw, &hh);
    memset(&td, 0, sizeof td);
    td.Width = hw;
    td.Height = hh;
    td.MipLevels = 1;
    td.ArraySize = 1;
    td.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    td.SampleDesc.Count = 1;
    td.Usage = D3D11_USAGE_DEFAULT;
    td.BindFlags = D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE;
    if (FAILED(ID3D11Device_CreateTexture2D(s_dev, &td, NULL, &tex)))
        return 0;
    if (FAILED(ID3D11Device_CreateRenderTargetView(s_dev, (ID3D11Resource *)tex,
                                                   NULL, &rtv))) {
        ID3D11Texture2D_Release(tex);
        return 0;
    }
    ID3D11DeviceContext_ClearRenderTargetView(s_ctx, rtv, black);
    b.left = 0;
    b.top = 0;
    b.front = 0;
    b.right = rt->hw < hw ? rt->hw : hw;
    b.bottom = rt->hh < hh ? rt->hh : hh;
    b.back = 1;
    ID3D11DeviceContext_CopySubresourceRegion(s_ctx, (ID3D11Resource *)tex, 0, 0, 0, 0,
                                              (ID3D11Resource *)rt->tex, 0, &b);
    s_grows++;
    if (said++ < 20)
        fprintf(stderr, LOGP "surface 0x%08X: %ux%u render target grown to %ux%u\n",
                rt->offset, rt->w, rt->h, w, h);
    if (rt->srv) {
        ID3D11ShaderResourceView_Release(rt->srv);
        rt->srv = NULL;
    }
    ID3D11RenderTargetView_Release(rt->rtv);
    ID3D11Texture2D_Release(rt->tex);
    rt->tex = tex;
    rt->rtv = rtv;
    rt->w = w;
    rt->h = h;
    rt->hw = hw;
    rt->hh = hh;
    if (alias_check_on() && rt->pitch)
        nv2a_rt_own_reset(&rt->own, (const uint8_t *)xbox_GetMemoryOffset(),
                          rt->addr, rt->pitch, h, (uint32_t)s_presents);
    /* The draw path's state shadow may name the released view. */
    s_sc_init = 0;
    return 1;
}

static RenderTarget *surface(void)
{
    const struct nv2a_pb_gpu *g = nv2a_pb_gpu_state();
    UINT w = g->clip_x + g->clip_w, h = g->clip_y + g->clip_h;
    RenderTarget *free_rt = NULL, *lru = &s_rt[0];
    int i;

    if (!g->color_offset || !w || !h)
        return NULL;
    s_tick++;
    for (i = 0; i < RT_MAX; i++) {
        RenderTarget *rt = &s_rt[i];
        if (rt->tex && rt->offset == g->color_offset && rt->w == w && rt->h == h) {
            /* First use this flip of a target the title may have rewritten:
             * a stale one is made again. */
            if (rt_drop_if_stale(rt, "reuse"))
                break;
            rt->last_use = s_tick;
            nv2a_rt_own_drawn(&rt->own, (uint32_t)s_presents);
            return rt;
        }
    }
    /* The same surface under another clip extent: one target per surface,
     * as large as any clip it was drawn with. Burnout 3 draws its 640x480
     * back buffer with clips of 640x464, 623x401, 159x344 and more in one
     * frame; a target per extent split the frame across textures, the
     * overlap rule below dropped each in turn (this backend writes nothing
     * back), and the flip found no target of the frame's size: a black
     * title screen and a frozen race. Draws map guest pixels to NDC through
     * rt->w/h, so a larger target keeps every position. What that asks of
     * the rest: a clear covers only its clip and clear rect (clear_region),
     * and a flip shows only the frame's extent (present_extent). Another
     * pitch at the same offset is a new allocation, not another clip: it
     * gets a target of its own and the overlap rule retires this one. */
    for (i = 0; i < RT_MAX; i++) {
        RenderTarget *rt = &s_rt[i];
        if (!rt->tex || rt->offset != g->color_offset || rt->pitch != g->pitch)
            continue;
        if (rt_drop_if_stale(rt, "reuse"))
            break;
        if ((rt->w >= w && rt->h >= h)
                || rt_grow(rt, rt->w > w ? rt->w : w, rt->h > h ? rt->h : h)) {
            rt->last_use = s_tick;
            nv2a_rt_own_drawn(&rt->own, (uint32_t)s_presents);
            return rt;
        }
        break;
    }
    /* No two live targets share memory: one the new target overlaps is
     * dropped (this backend has nothing to write back). */
    if (alias_check_on()) {
        uint32_t addr = nv2a_pb_dma_resolve(g->color_offset);
        for (i = 0; i < RT_MAX; i++) {
            RenderTarget *rt = &s_rt[i];
            if (rt->tex && nv2a_rt_overlap(rt->addr, rt->pitch * rt->h, addr, g->pitch * h)) {
                if (s_retired++ < 20)
                    fprintf(stderr, LOGP "surface 0x%08X: %ux%u render target retired:"
                            " 0x%08X %ux%u overlaps it\n", rt->offset, rt->w, rt->h,
                            g->color_offset, w, h);
                rt_release(rt);
            }
        }
    }
    for (i = 0; i < RT_MAX; i++) {
        RenderTarget *rt = &s_rt[i];
        if (!rt->tex && !free_rt)
            free_rt = rt;
        if (rt->last_use < lru->last_use)
            lru = rt;
    }
    if (!free_rt) {
        free_rt = lru;
        rt_release(free_rt);
    }
    nv2a_host_size(w, h, nv2a_host_render_scale(), D3D11_MAX_DIM,
                   &free_rt->hw, &free_rt->hh);
    {
        D3D11_TEXTURE2D_DESC td;
        memset(&td, 0, sizeof td);
        td.Width = free_rt->hw;
        td.Height = free_rt->hh;
        td.MipLevels = 1;
        td.ArraySize = 1;
        td.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
        td.SampleDesc.Count = 1;
        td.Usage = D3D11_USAGE_DEFAULT;
        td.BindFlags = D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE;
        if (FAILED(ID3D11Device_CreateTexture2D(s_dev, &td, NULL, &free_rt->tex)))
            return NULL;
        if (FAILED(ID3D11Device_CreateRenderTargetView(
                    s_dev, (ID3D11Resource *)free_rt->tex, NULL, &free_rt->rtv))) {
            ID3D11Texture2D_Release(free_rt->tex);
            free_rt->tex = NULL;
            return NULL;
        }
    }
    free_rt->offset = g->color_offset;
    free_rt->addr = nv2a_pb_dma_resolve(g->color_offset);
    free_rt->w = w;
    free_rt->h = h;
    free_rt->pitch = g->pitch;
    free_rt->last_use = s_tick;
    if (alias_check_on() && free_rt->pitch)
        nv2a_rt_own_reset(&free_rt->own, (const uint8_t *)xbox_GetMemoryOffset(),
                          free_rt->addr, free_rt->pitch, h, (uint32_t)s_presents);
    nv2a_rt_own_drawn(&free_rt->own, (uint32_t)s_presents);
    if (free_rt->hw != w)
        fprintf(stderr, LOGP "surface 0x%08X: %ux%u render target (%ux%u host)\n",
                g->color_offset, w, h, free_rt->hw, free_rt->hh);
    else
        fprintf(stderr, LOGP "surface 0x%08X: %ux%u render target\n",
                g->color_offset, w, h);
    return free_rt;
}

/* ---- depth ------------------------------------------------------------------ */

/* One depth-stencil buffer per zeta offset and surface size. The depth half
 * is float, so the test compares the same normalised z (depth units / zmax)
 * the CPU path stores in its float buffer, rather than D24S8's 24-bit fixed
 * point; the 8 stencil bits are the guest's Z24S8 stencil. */
#define DT_MAX 4
typedef struct {
    uint32_t                zeta;
    UINT                    w, h;       /* guest size, as its colour target's */
    UINT                    hw, hh;     /* host size (render.scale) */
    ID3D11Texture2D        *tex;
    ID3D11DepthStencilView *dsv;
    uint64_t                last_use;
} DepthTarget;

static DepthTarget s_dt[DT_MAX];
#define DSS_MAX 64
static struct { uint64_t key; ID3D11DepthStencilState *dss; } s_dss[DSS_MAX];
static int s_dss_count;
static uint64_t s_zclears, s_draws_z;

static DepthTarget *depth_target(UINT w, UINT h)
{
    uint32_t zeta = nv2a_pb_vsh_state()->zeta_offset;
    DepthTarget *e = NULL, *lru = &s_dt[0];
    D3D11_TEXTURE2D_DESC td;
    int i;

    for (i = 0; i < DT_MAX; i++) {
        DepthTarget *d = &s_dt[i];
        if (d->tex && d->zeta == zeta && d->w == w && d->h == h) {
            d->last_use = s_tick;
            return d;
        }
        if (!d->tex && !e)
            e = d;
        if (d->last_use < lru->last_use)
            lru = d;
    }
    if (!e) {
        e = lru;
        s_sc_init = 0;
        ID3D11DepthStencilView_Release(e->dsv);
        ID3D11Texture2D_Release(e->tex);
        memset(e, 0, sizeof *e);
    }
    nv2a_host_size(w, h, nv2a_host_render_scale(), D3D11_MAX_DIM, &e->hw, &e->hh);
    memset(&td, 0, sizeof td);
    td.Width = e->hw;
    td.Height = e->hh;
    td.MipLevels = 1;
    td.ArraySize = 1;
    td.Format = DXGI_FORMAT_D32_FLOAT_S8X24_UINT;
    td.SampleDesc.Count = 1;
    td.Usage = D3D11_USAGE_DEFAULT;
    td.BindFlags = D3D11_BIND_DEPTH_STENCIL;
    if (FAILED(ID3D11Device_CreateTexture2D(s_dev, &td, NULL, &e->tex)))
        return NULL;
    if (FAILED(ID3D11Device_CreateDepthStencilView(s_dev, (ID3D11Resource *)e->tex,
                                                   NULL, &e->dsv))) {
        ID3D11Texture2D_Release(e->tex);
        e->tex = NULL;
        return NULL;
    }
    /* The CPU path starts its buffer at the far plane, stencil 0. */
    ID3D11DeviceContext_ClearDepthStencilView(s_ctx, e->dsv,
                                              D3D11_CLEAR_DEPTH | D3D11_CLEAR_STENCIL,
                                              1.0f, 0);
    e->zeta = zeta;
    e->w = w;
    e->h = h;
    e->last_use = s_tick;
    fprintf(stderr, LOGP "zeta 0x%08X: %ux%u depth buffer\n", zeta, w, h);
    return e;
}

/* GL's stencil op codes as D3D11_STENCIL_OP; anything else keeps, as on the
 * CPU. */
static uint32_t stencil_op(uint32_t op)
{
    return nv2a_stencil_op_from_gl(op) + D3D11_STENCIL_OP_KEEP;
}

/* SET_DEPTH_FUNC takes GL's codes, 0x200 NEVER .. 0x207 ALWAYS, in the order
 * D3D11_COMPARISON_* numbers them from 1, and SET_STENCIL_FUNC the same (the
 * walker keeps its low nibble). Anything else tests as ALWAYS, as on the CPU.
 * Depth and stencil each only when enabled; one state per distinct key. */
static ID3D11DepthStencilState *depth_state(int depth, int stencil)
{
    const struct nv2a_pb_vsh *v = nv2a_pb_vsh_state();
    uint32_t f = nv2a_cmp_from_gl(v->depth_func);
    uint64_t key = 0;
    D3D11_DEPTH_STENCIL_DESC dd;
    int i;

    if (depth)
        key |= 1u | f << 1 | (v->depth_mask ? 1u : 0u) << 4;
    if (stencil)
        key |= (uint64_t)(1u | (v->stencil_func & 7u) << 1
                          | stencil_op(v->stencil_op[0]) << 4
                          | stencil_op(v->stencil_op[1]) << 8
                          | stencil_op(v->stencil_op[2]) << 12
                          | (v->stencil_rmask & 0xFFu) << 16
                          | (v->stencil_wmask & 0xFFu) << 24) << 8;
    for (i = 0; i < s_dss_count; i++)
        if (s_dss[i].key == key)
            return s_dss[i].dss;
    memset(&dd, 0, sizeof dd);
    dd.DepthEnable = depth ? TRUE : FALSE;
    dd.DepthWriteMask = depth && v->depth_mask ? D3D11_DEPTH_WRITE_MASK_ALL
                                               : D3D11_DEPTH_WRITE_MASK_ZERO;
    dd.DepthFunc = depth ? (D3D11_COMPARISON_FUNC)(f + 1) : D3D11_COMPARISON_ALWAYS;
    if (stencil) {
        dd.StencilEnable = TRUE;
        dd.StencilReadMask = (UINT8)v->stencil_rmask;
        dd.StencilWriteMask = (UINT8)v->stencil_wmask;
        dd.FrontFace.StencilFailOp = (D3D11_STENCIL_OP)stencil_op(v->stencil_op[0]);
        dd.FrontFace.StencilDepthFailOp = (D3D11_STENCIL_OP)stencil_op(v->stencil_op[1]);
        dd.FrontFace.StencilPassOp = (D3D11_STENCIL_OP)stencil_op(v->stencil_op[2]);
        dd.FrontFace.StencilFunc = (D3D11_COMPARISON_FUNC)((v->stencil_func & 7u) + 1);
        dd.BackFace = dd.FrontFace;             /* GL one-sided stencil */
    }
    if (s_dss_count >= DSS_MAX)
        return s_ds_off;
    if (FAILED(ID3D11Device_CreateDepthStencilState(s_dev, &dd, &s_dss[s_dss_count].dss)))
        return s_ds_off;
    s_dss[s_dss_count].key = key;
    return s_dss[s_dss_count++].dss;
}

/* CLEAR_SURFACE with the Z bit: the clear value scaled as the CPU path
 * scales it, by the zeta format's range. The stencil bit: its low byte. */
static void d3d11_on_zclear(uint32_t param)
{
    const struct nv2a_pb_gpu *g = nv2a_pb_gpu_state();
    const struct nv2a_pb_vsh *v = nv2a_pb_vsh_state();
    RenderTarget *rt;
    DepthTarget *dt;
    float z;

    if (!(param & 3u) || !init())
        return;
    rt = surface();
    if (!rt)
        return;
    dt = depth_target(rt->w, rt->h);
    if (!dt)
        return;
    z = nv2a_zclear_depth(g->format, v->zclear);
    ID3D11DeviceContext_ClearDepthStencilView(s_ctx, dt->dsv,
        ((param & 1u) ? D3D11_CLEAR_DEPTH : 0u) | ((param & 2u) ? D3D11_CLEAR_STENCIL : 0u),
        z, (UINT8)(v->zclear & 0xFFu));
    if (param & 1u)
        s_zclears++;
}

static void clear_region(RenderTarget *rt, const struct nv2a_pb_gpu *g, const float c[4])
{
    uint32_t b[4];
    UINT x0, y0, x1, y1;
    D3D11_RECT r;

    /* The clip and clear rect (nv2a_clear_box): one target serves every
     * clip of its surface (rt_grow), so a whole-target clear would wipe the
     * parts of the frame another clip drew. */
    if (!nv2a_clear_box(g, rt->w, rt->h, b))
        return;
    x0 = b[0]; y0 = b[1]; x1 = b[2]; y1 = b[3];
    if ((x0 == 0 && y0 == 0 && x1 == rt->w && y1 == rt->h) || !s_ctx1) {
        static int said;
        if (!s_ctx1 && !said++ && !(x0 == 0 && y0 == 0 && x1 == rt->w && y1 == rt->h))
            fprintf(stderr, LOGP "no D3D 11.1 context: a partial clear clears the"
                    " whole target\n");
        ID3D11DeviceContext_ClearRenderTargetView(s_ctx, rt->rtv, c);
        return;
    }
    /* Host pixels: the target is rt->hw x rt->hh for rt->w x rt->h. */
    r.left = (LONG)((uint64_t)x0 * rt->hw / rt->w);
    r.top = (LONG)((uint64_t)y0 * rt->hh / rt->h);
    r.right = (LONG)((uint64_t)x1 * rt->hw / rt->w);
    r.bottom = (LONG)((uint64_t)y1 * rt->hh / rt->h);
    ID3D11DeviceContext1_ClearView(s_ctx1, (ID3D11View *)rt->rtv, c, &r, 1);
    s_partial_clears++;
}

#include "private_gpu_profile.h"

static void d3d11_on_clear(uint32_t param)
{
    const struct nv2a_pb_gpu *g = nv2a_pb_gpu_state();
    RenderTarget *rt;
    float c[4];

    if (!(param & 0xF0) || !init())
        return;
    GP_GPU_BEGIN();
    rt = surface();
    if (!rt)
        return;
    c[0] = (float)((g->clear_color >> 16) & 0xFF) / 255.0f;
    c[1] = (float)((g->clear_color >> 8) & 0xFF) / 255.0f;
    c[2] = (float)(g->clear_color & 0xFF) / 255.0f;
    c[3] = (float)(g->clear_color >> 24) / 255.0f;
    clear_region(rt, g, c);
    rt->last_clear = ++s_seq;
    s_clears++;
}

/* ---- vertices -------------------------------------------------------------- */

/* One attribute of one vertex as float4, decoded the way the CPU path's
 * fetch_attr does. 0 if the stream has no data for it. */
static int fetch(const struct nv2a_pb_draw *d, const VertexAttr *a,
                 uint32_t index, float out[4])
{
    const uint8_t *p;

    out[0] = out[1] = out[2] = 0.0f;
    out[3] = 1.0f;
    if (d->inline_data) {
        size_t at = (size_t)a->offset + (size_t)index * a->stride;
        if (at + 4 > d->inline_bytes)
            return 0;
        p = (const uint8_t *)d->inline_buf + at;
    } else {
        if (!a->offset)
            return 0;
        p = (const uint8_t *)xbox_GetMemoryOffset() + a->offset
          + (size_t)index * a->stride;
    }
    return nv2a_vtx_decode(a->type, a->size, p, out);
}

/* The DXGI format a stream can be read in as it is, so its bytes are copied
 * rather than expanded to float4 (the shader's float4 input gets the same
 * values fetch() would make, missing components (0,0,0,1)). 0 for the types
 * and sizes that have none: S32K (integers read as floats), CMP, and
 * three-component bytes and shorts. *bytes: the size of one element. */
static DXGI_FORMAT native_format(const VertexAttr *a, UINT *bytes)
{
    static const DXGI_FORMAT f32[5] = {0, DXGI_FORMAT_R32_FLOAT,
        DXGI_FORMAT_R32G32_FLOAT, DXGI_FORMAT_R32G32B32_FLOAT,
        DXGI_FORMAT_R32G32B32A32_FLOAT};
    static const DXGI_FORMAT u8[5] = {0, DXGI_FORMAT_R8_UNORM,
        DXGI_FORMAT_R8G8_UNORM, 0, DXGI_FORMAT_R8G8B8A8_UNORM};
    static const DXGI_FORMAT s16[5] = {0, DXGI_FORMAT_R16_SNORM,
        DXGI_FORMAT_R16G16_SNORM, 0, DXGI_FORMAT_R16G16B16A16_SNORM};

    if (a->size < 1 || a->size > 4)
        return 0;
    switch (a->type) {
    case NV2A_VTX_D3DCOLOR: *bytes = 4;           return DXGI_FORMAT_B8G8R8A8_UNORM;
    case NV2A_VTX_FLOAT:    *bytes = 4 * a->size; return f32[a->size];
    case NV2A_VTX_UBYTE:    *bytes = a->size;     return u8[a->size];
    case NV2A_VTX_S1:       *bytes = 2 * a->size; return s16[a->size];
    default:                return 0;
    }
}

#include "private_map_profile.h"

static void *ring_map(ID3D11Buffer *b, UINT size, UINT *pos, UINT need,
                      UINT align)
{
    D3D11_MAPPED_SUBRESOURCE m;
    D3D11_MAP how = D3D11_MAP_WRITE_NO_OVERWRITE;
    UINT at = (*pos + align - 1) / align * align;

    if (need > size)
        return NULL;
    if (at + need > size) {
        at = 0;
        how = D3D11_MAP_WRITE_DISCARD;
    }
    if (FAILED(ID3D11DeviceContext_Map(s_ctx, (ID3D11Resource *)b, 0, how, 0, &m)))
        return NULL;
    *pos = at + need;
    return (uint8_t *)m.pData + at;
}

/* A batch's vertex streams, written through one Map of s_vb: vb_begin
 * reserves an upper bound for all of them, vb_alloc hands out 16-aligned
 * pieces of it, vb_end unmaps and gives back what was not used. Without a
 * reservation (memos off, or the bound too large) each piece is its own
 * ring_map, as before. One mapping also means a wrap's DISCARD cannot land
 * between two streams of the same batch. */
static uint8_t *s_vbm_base;
static UINT     s_vbm_at, s_vbm_used, s_vbm_cap;

static void vb_begin(UINT bound)
{
    s_vbm_base = NULL;
    if (!memo_on() || !bound || bound > s_vb_size / 4)
        return;
    s_vbm_base = (uint8_t *)ring_map(s_vb, s_vb_size, &s_vb_pos, bound, 16);
    if (s_vbm_base) {
        s_vbm_at = s_vb_pos - bound;
        s_vbm_used = 0;
        s_vbm_cap = bound;
    }
}

static void *vb_alloc(UINT bytes, UINT *off)
{
    void *p;
    if (s_vbm_base) {
        UINT o = (s_vbm_used + 15u) & ~15u;
        if (o + bytes > s_vbm_cap)
            return NULL;
        s_vbm_used = o + bytes;
        *off = s_vbm_at + o;
        return s_vbm_base + o;
    }
    p = ring_map(s_vb, s_vb_size, &s_vb_pos, bytes, 16);
    if (p)
        *off = s_vb_pos - bytes;
    return p;
}

/* After each piece is written: unmapped now unless it is part of a batch. */
static void vb_written(void)
{
    if (!s_vbm_base)
        ID3D11DeviceContext_Unmap(s_ctx, (ID3D11Resource *)s_vb, 0);
}

static void vb_end(void)
{
    if (s_vbm_base) {
        ID3D11DeviceContext_Unmap(s_ctx, (ID3D11Resource *)s_vb, 0);
        s_vb_pos = s_vbm_at + s_vbm_used;
        s_vbm_base = NULL;
    }
}

/* Triangle, line and point lists from any NV097 primitive. Returns the
 * index count (into out, rebased by -base) and the topology. */
static uint32_t convert_indices(const struct nv2a_pb_draw *d, uint32_t base,
                                uint32_t *out, D3D11_PRIMITIVE_TOPOLOGY *topo)
{
    enum nv2a_topology t;
    uint32_t k = nv2a_prim_to_list(d->prim, d->idx, d->idx_count, base, out, &t);
    *topo = t == NV2A_TOPO_POINTS ? D3D11_PRIMITIVE_TOPOLOGY_POINTLIST
          : t == NV2A_TOPO_LINES  ? D3D11_PRIMITIVE_TOPOLOGY_LINELIST
          :                         D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
    return k;
}

/* ---- shaders --------------------------------------------------------------- */

static uint32_t program_hash(const struct nv2a_pb_vsh *v)
{
    uint32_t h = nv2a_vsh_program_hash(v, NULL);
    return h ? h : 1;
}

static VsEntry *vertex_shader(int program)
{
    static NV2AVshProgram parsed;
    static char hlsl[65536];
    const struct nv2a_pb_vsh *v = nv2a_pb_vsh_state();
    uint32_t hash = 0;
    VsEntry *e = NULL;
    ID3DBlob *code = NULL;
    const char *src;
    char name[32];
    int i;
    static struct { int valid, program; uint32_t dwords, start; VsEntry *e; } memo;

    if (memo_on() && memo.valid && memo.program == program
            && memo.dwords == v->prog_dwords && memo.start == v->start) {
#ifdef GAMEPKG_VERIFY_VS_CACHE
        gp_verify_counts.vertex++;
        uint32_t reference=program?program_hash(v):0;
        if(memo.e&&memo.e->hash!=reference) {
            fprintf(stderr,"[GamePKG] vertex memo key differs from original program hash\n");
            ExitProcess(86);
        }
        if(!memo.e) {
            int bad=0;
            for(int j=0;j<VS_MAX;j++)if(s_vs[j].used&&s_vs[j].hash==reference&&s_vs[j].bad)bad=1;
            if(!bad){fprintf(stderr,"[GamePKG] vertex memo skipped an original shader lookup\n");ExitProcess(87);}
        }
#endif
        return memo.e;
    }
    if (program)
        hash = program_hash(v);
    for (i = 0; i < VS_MAX; i++) {
        if (s_vs[i].used && s_vs[i].hash == hash) {
            memo.valid = 1;
            memo.program = program;
            memo.dwords = v->prog_dwords;
            memo.start = v->start;
            memo.e = s_vs[i].bad ? NULL : &s_vs[i];
            return memo.e;
        }
        if (!s_vs[i].used && !e)
            e = &s_vs[i];
    }
    if (!e)
        return NULL;    /* full; a title with this many programs wants LRU */
    e->used = 1;
    e->hash = hash;
    e->bad = 1;

    if (program) {
        d3d8_vsh_parse(&v->prog[v->start][0],
                       NV2A_VSH_SLOTS - (int)v->start, &parsed);
        if (d3d8_vsh_generate_hlsl_ex(&parsed, NV2A_VSH_HLSL_SCREEN_SPACE,
                                      hlsl, sizeof hlsl) <= 0) {
            fprintf(stderr, LOGP "program %08X: no HLSL\n", hash);
            return NULL;
        }
        src = hlsl;
        e->inputs = parsed.inputs_read;
        snprintf(name, sizeof name, "nv2a_vsh_%08X", hash);
    } else {
        src = s_vs_passthrough_src;
        e->inputs = (1u << 0) | (1u << 3) | (1u << 4) | (0xFu << 9);
        snprintf(name, sizeof name, "nv2a_vsh_passthrough");
    }
    if (!compile(name, src, "vs_5_0", &code))
        return NULL;
    if (FAILED(ID3D11Device_CreateVertexShader(
                s_dev, ID3D10Blob_GetBufferPointer(code),
                ID3D10Blob_GetBufferSize(code), NULL, &e->vs))) {
        ID3D10Blob_Release(code);
        return NULL;
    }
    e->code = code;     /* input layouts come per draw: input_layout() */
    fprintf(stderr, LOGP "compiled %s (inputs 0x%04X)\n", name, e->inputs);
    e->bad = 0;
    return e;
}

/* The input layout for a shader and the formats its streams arrive in this
 * batch (one input slot per ATTR), made the first time that pair is seen.
 * NULL if the shader reads nothing. */
static ID3D11InputLayout *input_layout(VsEntry *e, const uint8_t fmt[16])
{
    D3D11_INPUT_ELEMENT_DESC el[NV2A_VS_MAX_INPUTS];
    int i, n, k;

    for (k = 0; k < 8; k++)
        if (e->lay[k].il && !memcmp(e->lay[k].fmt, fmt, 16))
            return e->lay[k].il;
    for (i = 0, n = 0; i < NV2A_VS_MAX_INPUTS; i++) {
        if (!(e->inputs & (1u << i)))
            continue;
        memset(&el[n], 0, sizeof el[n]);
        el[n].SemanticName = "ATTR";
        el[n].SemanticIndex = (UINT)i;
        el[n].Format = (DXGI_FORMAT)fmt[i];
        el[n].InputSlot = (UINT)i;
        el[n].AlignedByteOffset = 0;
        el[n].InputSlotClass = D3D11_INPUT_PER_VERTEX_DATA;
        n++;
    }
    if (!n)
        return NULL;
    k = e->lay_next;
    e->lay_next = (k + 1) % 8;
    if (e->lay[k].il) {
        ID3D11InputLayout_Release(e->lay[k].il);
        s_state_gen++;
    }
    e->lay[k].il = NULL;
    memcpy(e->lay[k].fmt, fmt, 16);
    if (FAILED(ID3D11Device_CreateInputLayout(
                s_dev, el, (UINT)n, ID3D10Blob_GetBufferPointer(e->code),
                ID3D10Blob_GetBufferSize(e->code), &e->lay[k].il))) {
        fprintf(stderr, LOGP "vs %08X: CreateInputLayout failed\n", e->hash);
        e->lay[k].il = NULL;
    }
    return e->lay[k].il;
}

/* ---- rasteriser state ----------------------------------------------------- */

/* The state for the pgraph cull setting: CULL_FACE 0x404 FRONT, 0x405 BACK,
 * 0x408 FRONT_AND_BACK; FRONT_FACE 0x900 CW, 0x901 CCW. *all is set when the
 * draw culls every triangle (FRONT_AND_BACK), which D3D11 cannot express.
 * Character shadow volumes need it: each body part's shadow volume is
 * drawn twice into destination alpha, back faces (CULL_FRONT) added and
 * front faces (CULL_BACK) reverse-subtracted, which leaves alpha only where
 * the floor lies inside a volume. With no culling both passes cover the
 * whole volume and its screen footprint comes out as one dark wedge. */
static ID3D11RasterizerState *rasterizer_state(int *all)
{
    const struct nv2a_pb_vsh *v = nv2a_pb_vsh_state();
    static int off = -1;
    int c = 0, f = v->front_face != 0x900;

    static int flip = -1;
    if (off < 0)
        off = recomp_env(RENV_D3D11_NO_CULL) != NULL;  /* A/B: cull nothing */
    if (flip < 0)
        flip = recomp_env(RENV_D3D11_CULL_FLIP) != NULL;  /* A/B: other winding */
    f ^= flip;
    *all = 0;
    if (v->cull_enable && !off) {
        if (v->cull_face == 0x405)      c = 1;
        else if (v->cull_face == 0x404) c = 2;
        else if (v->cull_face == 0x408) *all = 1;
    }
    return s_rs[c][f];
}

/* ---- blending -------------------------------------------------------------- */

/* SET_BLEND_EQUATION as a D3D11 blend op. The signed variants (0xF005
 * REVERSE_SUBTRACT_SIGNED, 0xF006 ADD_SIGNED) have no D3D11 form and fall
 * back to their unsigned ones. */
static D3D11_BLEND_OP blend_op(uint32_t eq)
{
    switch (eq) {
    case 0x800A: return D3D11_BLEND_OP_SUBTRACT;
    case 0x800B:
    case 0xF005: return D3D11_BLEND_OP_REV_SUBTRACT;
    case 0x8007: return D3D11_BLEND_OP_MIN;
    case 0x8008: return D3D11_BLEND_OP_MAX;
    default:     return D3D11_BLEND_OP_ADD;
    }
}

static D3D11_BLEND blend_factor(uint32_t gl, int alpha)
{
    static const D3D11_BLEND map[NV2A_BF_UNKNOWN + 1] = {
        D3D11_BLEND_ZERO, D3D11_BLEND_ONE,
        D3D11_BLEND_SRC_COLOR, D3D11_BLEND_INV_SRC_COLOR,
        D3D11_BLEND_SRC_ALPHA, D3D11_BLEND_INV_SRC_ALPHA,
        D3D11_BLEND_DEST_ALPHA, D3D11_BLEND_INV_DEST_ALPHA,
        D3D11_BLEND_DEST_COLOR, D3D11_BLEND_INV_DEST_COLOR,
        D3D11_BLEND_SRC_ALPHA_SAT,
        /* No CONSTANT_ALPHA: blend_color() puts the right value in. */
        D3D11_BLEND_BLEND_FACTOR, D3D11_BLEND_INV_BLEND_FACTOR,
        D3D11_BLEND_BLEND_FACTOR, D3D11_BLEND_INV_BLEND_FACTOR,
        D3D11_BLEND_ONE,                        /* unknown */
    };
    uint32_t bf = nv2a_blend_from_gl(gl);
    return map[alpha ? nv2a_blend_for_alpha(bf) : bf];
}

static ID3D11BlendState *blend_state(void)
{
    const struct nv2a_pb_gpu *g = nv2a_pb_gpu_state();
    uint32_t key[5];
    D3D11_BLEND_DESC bd;
    ID3D11BlendState *bs = NULL;
    int i;

    key[0] = g->blend_enable ? 1 : 0;
    key[1] = key[0] ? g->blend_sfactor : 0;
    key[2] = key[0] ? g->blend_dfactor : 0;
    key[3] = g->color_keep;             /* SET_COLOR_MASK, inverted */
    key[4] = key[0] ? (uint32_t)blend_op(g->blend_equation) : 0;
    for (i = 0; i < s_blend_n; i++)
        if (!memcmp(s_blend[i].key, key, sizeof key))
            return s_blend[i].bs;
    memset(&bd, 0, sizeof bd);
    bd.RenderTarget[0].BlendEnable = key[0];
    bd.RenderTarget[0].SrcBlend = blend_factor(key[1], 0);
    bd.RenderTarget[0].DestBlend = blend_factor(key[2], 0);
    bd.RenderTarget[0].BlendOp = key[0] ? (D3D11_BLEND_OP)key[4] : D3D11_BLEND_OP_ADD;
    bd.RenderTarget[0].SrcBlendAlpha = blend_factor(key[1], 1);
    bd.RenderTarget[0].DestBlendAlpha = blend_factor(key[2], 1);
    bd.RenderTarget[0].BlendOpAlpha = bd.RenderTarget[0].BlendOp;
    bd.RenderTarget[0].RenderTargetWriteMask = (UINT8)(
        ((key[3] & 0x00FF0000u) ? 0 : D3D11_COLOR_WRITE_ENABLE_RED)
      | ((key[3] & 0x0000FF00u) ? 0 : D3D11_COLOR_WRITE_ENABLE_GREEN)
      | ((key[3] & 0x000000FFu) ? 0 : D3D11_COLOR_WRITE_ENABLE_BLUE)
      | ((key[3] & 0xFF000000u) ? 0 : D3D11_COLOR_WRITE_ENABLE_ALPHA));
    if (!key[0]) {
        bd.RenderTarget[0].SrcBlend = bd.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
        bd.RenderTarget[0].DestBlend = bd.RenderTarget[0].DestBlendAlpha = D3D11_BLEND_ZERO;
    }
    if (FAILED(ID3D11Device_CreateBlendState(s_dev, &bd, &bs)))
        return NULL;
    if (s_blend_n < BLEND_MAX) {
        i = s_blend_n++;
    } else {
        /* Full: replace one round robin, as the sampler cache does. A
         * released state's address can come back, so the bound-state
         * filter starts over. */
        static unsigned next;
        i = (int)(next++ % BLEND_MAX);
        ID3D11BlendState_Release(s_blend[i].bs);
        s_state_gen++;
    }
    memcpy(s_blend[i].key, key, sizeof key);
    s_blend[i].bs = bs;
    return bs;
}

/* The factor D3D11_BLEND_BLEND_FACTOR reads: SET_BLEND_COLOR, unpacked
 * from ARGB to RGBA. D3D11 has no CONSTANT_ALPHA, so when only the
 * _ALPHA factors are in use the alpha is replicated into all four; a
 * draw mixing _COLOR and _ALPHA keeps the colour (logged once). With
 * no constant factor the default (1,1,1,1) is used, so a colour the title
 * sets without using it does not rebind the blend state. Returns whether a
 * constant factor is in use. */
static int blend_color(float bf[4])
{
    const struct nv2a_pb_gpu *g = nv2a_pb_gpu_state();
    int mixed, on = nv2a_blend_constant(g->blend_enable, g->blend_sfactor,
                                        g->blend_dfactor, g->blend_color,
                                        bf, &mixed);
    if (mixed) {
        static int warned;
        if (!warned) {
            warned = 1;
            fprintf(stderr, LOGP "blend: CONSTANT_COLOR and CONSTANT_ALPHA mixed"
                    " (sfactor 0x%04X dfactor 0x%04X); the _ALPHA side uses the"
                    " colour\n", g->blend_sfactor, g->blend_dfactor);
        }
    }
    return on;
}

/* ---- textures -------------------------------------------------------------
 *
 * Four stages, decoded from the texture registers the title last wrote, the
 * way the CPU path's vsh_build_stages does: offset, format, size (from the
 * format word for swizzled and DXT, from IMAGE_RECT for linear), pitch,
 * wrap modes, enable, and a texture-shader mode from SET_SHADER_STAGE_PROGRAM.
 *
 * Each texture is decoded to A8R8G8B8 on the CPU -- the same per-format
 * arithmetic as the CPU sampler, so both paths see the same texels -- and
 * uploaded as B8G8R8A8_UNORM (the same bytes). Cache key and invalidation
 * follow nv2a_pb_state.h: offset + format + width + height + pitch; on a new
 * flip generation the whole texture is hashed again and re-uploaded if it
 * changed (the letterboxed movie changes only in the middle).
 *
 * Mipmaps: a swizzled or DXT texture brings the levels SET_TEXTURE_FORMAT
 * counts (clamped as xemu does: to CONTROL0's MAX_LOD_CLAMP + 1 and to the
 * level that reaches 1x1), packed one after another, each swizzled or
 * block-compressed at its own size (DXT levels round up to whole 4x4
 * blocks). The level count is part of the cache key and every level is in
 * the hash. Linear textures have one level. RECOMP_D3D11_NO_MIPS=1 uploads
 * level 0 only, for A/B.
 *
 * ponytail: DXT is decoded rather than uploaded as BC1-3; a texture
 * rewritten between two batches of one flip is not seen; 3D textures are
 * slice 0 only. Cube maps use native_cubemap.h and six real GPU faces. */

static Stage s_stage[4];
/* What each stage's SRV came from, for the batch trace. */
static const char   *s_stage_src[4];
static RenderTarget *s_stage_rt[4];

#include "kernel/nv2a_texscale.h"
#define TEX_MAX_TEXELS (2048u * 2048u)
#define TEX_CACHE 512   /* a demo frame binds ~100; 64 thrashed */
typedef struct {
    uint32_t offset, color, width, height, pitch, palette, levels;
    uint32_t gen, used;
    uint64_t hash;
    uint64_t up_key;
    uint32_t up_gen,up_flip;
    int up_failed;
    size_t up_bytes;
    ID3D11Texture2D *up_tex;
    ID3D11ShaderResourceView *up_srv;
    int      live;
    uint8_t *source_shadow;
    uint32_t shadow_alloc, shadow_tex, shadow_pal, shadow_next;
    int shadow_valid, shadow_wait;
    ID3D11Texture2D          *tex;
    ID3D11ShaderResourceView *srv;
} TexEntry;

static TexEntry  s_tc[TEX_CACHE];
static uint32_t  s_tc_clock;
/* Key hash -> entry + 1, checked before the scan. A live entry with a key
 * is the only one (one is made only when the scan finds none), so a hint
 * that matches is the entry the scan would find; a stale or colliding one
 * just falls through to it. */
#define TC_HINTS 2048
static uint16_t  s_tc_hint[TC_HINTS];

static uint32_t tc_slot(const Stage *t)
{
    uint32_t h = t->offset * 0x9E3779B1u;
    h ^= (t->color + (t->width << 8) + (t->height << 20)) * 0x85EBCA77u;
    h ^= (t->pitch ^ t->palette ^ (t->levels << 27)) * 0xC2B2AE3Du;
    return (h >> 16) & (TC_HINTS - 1);
}
static uint32_t *s_texels;
static size_t    s_texels_cap;
static ID3D11ShaderResourceView *s_black_srv;   /* too large: (0,0,0,1) */
static ID3D11ShaderResourceView *s_magenta_srv; /* format not decoded */

static int no_mips(void)
{
    static int off = -1;
    if (off < 0)
        off = recomp_env(RENV_D3D11_NO_MIPS) != NULL;
    return off;
}

static void build_stages(void)
{
    const uint32_t *regs = nv2a_pb_tex_regs();
    const uint8_t  *sets = nv2a_pb_tex_regs_set();
    const struct nv2a_pb_vsh *v = nv2a_pb_vsh_state();
    int n;

    for (n = 0; n < 4; n++) {
        Stage *t = &s_stage[n];
        int mode;

        nv2a_stage_decode(n, &regs[(0x40u * n) / 4], &sets[(0x40u * n) / 4],
                          v->shader_set, v->shader_prog, nv2a_pb_dma_resolve,
                          NV2A_STAGE_P8_PALETTE | NV2A_STAGE_PITCH_LINEAR
                          | (no_mips() ? 0u : NV2A_STAGE_MIPS), t);
        mode = t->raw_mode;
        if (mode && mode != 4 && (!t->on || !t->valid)) {
            /* The program asks for a texture this stage cannot give: it
             * reads (0, 0, 0, 1) now, as a NONE stage. Said once per stage, mode and format, since a
             * world drawn in flat vertex colour starts exactly here. */
            static uint8_t said[4][32][32];
            uint8_t bit = (uint8_t)(1u << (t->color & 7));
            if (!(said[n][mode & 31][(t->color >> 3) & 31] & bit)) {
                said[n][mode & 31][(t->color >> 3) & 31] |= bit;
                fprintf(stderr, LOGP "\x1b[1;35mstage %d: mode %d with no texture"
                        " (enabled %d, offset 0x%08X, format 0x%02X, %ux%u, pitch %u)"
                        "\x1b[0m\n", n, mode, t->on, t->offset, t->color,
                        t->width, t->height, t->pitch);
            }
        }
        if (t->mode == 5)
            t->mode = 0;    /* CLIP_PLANE: no texture, the register reads 0
                             * (ponytail: its per-pixel discard is not done) */
    }
}

/* Bytes of guest memory the texture occupies; 0 if it cannot be decoded. */
static uint32_t tex_extent(const Stage *t)
{
    if (!t->valid || (uint64_t)t->width * t->height > TEX_MAX_TEXELS)
        return 0;
    return nv2a_tex_extent(t->color, t->width, t->height, t->pitch, t->levels);
}

static uint64_t tex_hash(const uint8_t *p, uint32_t n)
{
    return nv2a_tex_hash(p, n, NV2A_TEX_HASH_SEED);
}

/* The whole texture into s_texels (A8R8G8B8): level 0 (width x height),
 * then each further level packed after it. */
static int decode_texture(const Stage *t)
{
    const uint8_t *mem = (const uint8_t *)xbox_GetMemoryOffset() + t->offset;
    size_t n = nv2a_tex_texels(t->width, t->height, t->levels);
    const uint32_t *pal = NULL;

    if (n > s_texels_cap) {
        uint32_t *p = (uint32_t *)realloc(s_texels, n * 4);
        if (!p)
            return 0;
        s_texels = p;
        s_texels_cap = n;
    }
    if (t->color == NV2A_TEX_P8)
        pal = (const uint32_t *)((const uint8_t *)xbox_GetMemoryOffset()
                                 + t->palette);
    nv2a_tex_decode(mem, t->color, t->width, t->height, t->pitch, t->levels,
                    pal, pal ? t->pal_len : 0, s_texels);
    return 1;
}

static void gp_ui_drop(TexEntry *e);
static int tex_upload(TexEntry *e, const Stage *t)
{
    const uint32_t *src;
    uint32_t l;
    gp_ui_drop(e);
    if (!decode_texture(t))
        return 0;
    if (!e->tex) {
        D3D11_TEXTURE2D_DESC td;
        memset(&td, 0, sizeof td);
        td.Width = t->width;
        td.Height = t->height;
        td.MipLevels = t->levels;
        td.ArraySize = 1;
        td.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
        td.SampleDesc.Count = 1;
        td.Usage = D3D11_USAGE_DEFAULT;
        td.BindFlags = D3D11_BIND_SHADER_RESOURCE;
        if (FAILED(ID3D11Device_CreateTexture2D(s_dev, &td, NULL, &e->tex)))
            return 0;
        if (FAILED(ID3D11Device_CreateShaderResourceView(
                    s_dev, (ID3D11Resource *)e->tex, NULL, &e->srv))) {
            ID3D11Texture2D_Release(e->tex);
            e->tex = NULL;
            return 0;
        }
    }
    for (l = 0, src = s_texels; l < t->levels; l++) {
        uint32_t w = nv2a_tex_level_dim(t->width, l), h = nv2a_tex_level_dim(t->height, l);
        ID3D11DeviceContext_UpdateSubresource(s_ctx, (ID3D11Resource *)e->tex,
                                              l, NULL, src, w * 4, 0);
        src += (size_t)w * h;
    }
    s_tex_uploads++;
    if (t->levels > 1)
        s_tex_mip_uploads++;
    return 1;
}

/* Separate scaled resources: guest uploads always retain their original extent. */
#define GP_UI_BUDGET (128u*1024u*1024u)
static size_t gp_ui_bytes;
static int gp_ui_draw;
static unsigned gp_ui_bound;
static uint64_t gp_ui_uploads;
static uint32_t *gp_ui_pixels;
static size_t gp_ui_capacity;
static void gp_ui_drop(TexEntry *e)
{
 if(e->up_srv)ID3D11ShaderResourceView_Release(e->up_srv);
 if(e->up_tex)ID3D11Texture2D_Release(e->up_tex);
 gp_ui_bytes-=e->up_bytes;e->up_bytes=0;e->up_srv=NULL;e->up_tex=NULL;
 e->up_key=0;e->up_gen=0;e->up_failed=0;e->up_flip=UINT32_MAX;s_sc_init=0;
}
static ID3D11ShaderResourceView *gp_ui_bind(TexEntry *e,const Stage *t)
{
 unsigned f=nv2a_texscale_factor();gp_ui_bound=0;
 if(!gp_ui_draw || !f || t->cube || nv2a_tex_signed_mask(t->filter) || !nv2a_texscale_tex_ok(nv2a_tex_size_from_format(t->color),t->width,t->height,t->levels))return e->srv;
 static int store_started;if(!store_started){store_started=1;const char *cfg=recomp_env(RENV_RENDER_SHADER_CACHE);char dir[1024],sub[1100];if(cfg&&*cfg&&strcmp(cfg,"none")&&strcmp(cfg,"off")&&strcmp(cfg,"0")&&nv2a_pcache_dir(cfg,nv2a_pcache_app(),dir,sizeof dir)&&snprintf(sub,sizeof sub,"%s/ui_scale",dir)<(int)sizeof sub)nv2a_texscale_cache_dir(sub);}
 uint32_t gen=nv2a_texscale_generation(),flip=nv2a_pb_gpu_state()->flips;
 if(e->up_gen!=gen){gp_ui_drop(e);e->up_gen=gen;}
 if(e->up_srv){gp_ui_bound=f;return e->up_srv;}
 if(e->up_failed || e->up_flip==flip)return e->srv;
 e->up_flip=flip;
 uint32_t w=t->width*f,h=t->height*f,levels=f==4?3:2;
 size_t count=nv2a_tex_texels(w,h,levels),bytes=count*4;
 if(bytes>gp_ui_capacity){uint32_t *room=realloc(gp_ui_pixels,bytes);if(!room)return e->srv;gp_ui_pixels=room;gp_ui_capacity=bytes;}
 uint32_t *pixels=gp_ui_pixels;
 const uint32_t *source=NULL;
 if(!e->up_key){if(!decode_texture(t)){return e->srv;}e->up_key=nv2a_texscale_key(s_texels,t->width,t->height);source=s_texels;}
 int r=nv2a_texscale_get(e->up_key,source,t->width,t->height,f,nv2a_texscale_algo(),0,pixels);
 /* A full worker queue can refuse the first request. Retry with source later. */
 if(r==NV2A_TEXSCALE_MISS)e->up_key=0;
 if(r==NV2A_TEXSCALE_FAILED)e->up_failed=1;
 if(r!=NV2A_TEXSCALE_READY){return e->srv;}
 nv2a_texscale_mips(pixels,w,h,levels);
 while(gp_ui_bytes+bytes>GP_UI_BUDGET){TexEntry *victim=NULL;for(unsigned i=0;i<TEX_CACHE;i++)if(s_tc[i].up_tex && &s_tc[i]!=e && (!victim || s_tc[i].used<victim->used))victim=&s_tc[i];if(!victim){e->up_key=0;return e->srv;}gp_ui_drop(victim);}
 D3D11_TEXTURE2D_DESC td={0};td.Width=w;td.Height=h;td.MipLevels=levels;td.ArraySize=1;td.Format=DXGI_FORMAT_B8G8R8A8_UNORM;td.SampleDesc.Count=1;td.Usage=D3D11_USAGE_DEFAULT;td.BindFlags=D3D11_BIND_SHADER_RESOURCE;
 D3D11_SUBRESOURCE_DATA data[3]={{0}};const uint32_t *p=pixels;
 for(unsigned l=0;l<levels;l++){unsigned lw=nv2a_tex_level_dim(w,l),lh=nv2a_tex_level_dim(h,l);data[l].pSysMem=p;data[l].SysMemPitch=lw*4;p+=(size_t)lw*lh;}
 ID3D11Texture2D *tex=NULL;ID3D11ShaderResourceView *srv=NULL;
 HRESULT hr=ID3D11Device_CreateTexture2D(s_dev,&td,data,&tex);
 if(SUCCEEDED(hr))hr=ID3D11Device_CreateShaderResourceView(s_dev,(ID3D11Resource*)tex,NULL,&srv);
 if(FAILED(hr)){if(tex)ID3D11Texture2D_Release(tex);e->up_key=0;return e->srv;}
 e->up_tex=tex;e->up_srv=srv;e->up_bytes=bytes;gp_ui_bytes+=bytes;gp_ui_bound=f;
 if(++gp_ui_uploads==1 || gp_ui_uploads%100==0)fprintf(stderr,LOGP "local UI upscale: %llu uploads, factor%u, GPU bytes%zu\n",(unsigned long long)gp_ui_uploads,f,gp_ui_bytes);
 return srv;
}

#define GP_TEX_SHADOW_BUDGET (128u * 1024u * 1024u)
static size_t s_gp_shadow_bytes;
static uint64_t s_gp_shadow_hits, s_gp_shadow_captures, s_gp_shadow_fallbacks;
#ifdef GAMEPKG_TEX_SHADOW_TEST
static int s_gp_shadow_fail_alloc;
#endif
static void gp_tex_shadow_free(TexEntry *e)
{
    if (e->source_shadow) {
        s_gp_shadow_bytes -= e->shadow_alloc;
        free(e->source_shadow);
    }
    e->source_shadow = NULL;
    e->shadow_alloc = e->shadow_tex = e->shadow_pal = 0;
    e->shadow_valid = 0;
}
static void gp_tex_shadow_backoff(TexEntry *e, uint32_t flip)
{
    gp_tex_shadow_free(e);
    e->shadow_wait = 1;
    e->shadow_next = flip + 120u;
}
static int gp_tex_shadow_due(const TexEntry *e, uint32_t flip)
{
    return !e->shadow_wait || (uint32_t)(flip - e->shadow_next) < 0x80000000u;
}

static void tex_free(TexEntry *e)
{
    gp_tex_shadow_free(e);
    gp_ui_drop(e);
    s_sc_init = 0;
    if (e->srv) ID3D11ShaderResourceView_Release(e->srv);
    if (e->tex) ID3D11Texture2D_Release(e->tex);
    memset(e, 0, sizeof *e);
}

/* The texels' hash, and for P8 the palette's folded in: a palette swap is a
 * new texture. */
static uint64_t tex_hash_pal(const uint8_t *mem, const Stage *t, uint32_t bytes)
{
    uint64_t h = tex_hash(mem + t->offset, bytes);
    if (t->color == 0x0B)
        h ^= tex_hash(mem + t->palette, t->pal_len * 4u) * 31u;
    return h;
}

/* The snapshot is accepted only after its complete hash, including any
 * palette, matches the original validation result. Comparisons inspect all
 * bytes at the same once-per-flip validation points as the original path. */
#if defined(__AVX2__)
#include <immintrin.h>
#endif
/* Equality only. Every load is wholly inside the caller's byte range. */
static int gamepkg_bytes_equal(const void *left, const void *right, size_t bytes)
{
#if defined(__AVX2__)
    const unsigned char *a = left, *b = right;
    while (bytes >= 128) {
        __m256i e0 = _mm256_cmpeq_epi8(_mm256_loadu_si256((const __m256i *)a),
                _mm256_loadu_si256((const __m256i *)b));
        __m256i e1 = _mm256_cmpeq_epi8(_mm256_loadu_si256((const __m256i *)(a + 32)),
                _mm256_loadu_si256((const __m256i *)(b + 32)));
        __m256i e2 = _mm256_cmpeq_epi8(_mm256_loadu_si256((const __m256i *)(a + 64)),
                _mm256_loadu_si256((const __m256i *)(b + 64)));
        __m256i e3 = _mm256_cmpeq_epi8(_mm256_loadu_si256((const __m256i *)(a + 96)),
                _mm256_loadu_si256((const __m256i *)(b + 96)));
#ifdef GAMEPKG_EQUAL_NEGATIVE
        e3 = e2; /* Deliberately ignore the fourth chunk; tests must reject it. */
#endif
        if (_mm256_movemask_epi8(_mm256_and_si256(_mm256_and_si256(e0, e1),
                _mm256_and_si256(e2, e3))) != -1) return 0;
        a += 128; b += 128; bytes -= 128;
    }
    while (bytes >= 32) {
        if (_mm256_movemask_epi8(_mm256_cmpeq_epi8(
                _mm256_loadu_si256((const __m256i *)a),
                _mm256_loadu_si256((const __m256i *)b))) != -1) return 0;
        a += 32; b += 32; bytes -= 32;
    }
    while (bytes >= 8) {
        uint64_t x, y;
        memcpy(&x, a, 8); memcpy(&y, b, 8);
        if (x != y) return 0;
        a += 8; b += 8; bytes -= 8;
    }
    while (bytes) {
        if (*a++ != *b++) return 0;
        bytes--;
    }
    return 1;
#else
    return bytes == 0 || memcmp(left, right, bytes) == 0;
#endif
}


static int gamepkg_verified_bytes_equal(const void *a, const void *b, size_t n)
{
 int result=gamepkg_bytes_equal(a,b,n);
#ifdef GAMEPKG_VERIFY_BYTE_EQUAL
 int expected=n==0 || memcmp(a,b,n)==0;
 if(result!=expected){fprintf(stderr,LOGP "FAIL full-byte equality at flip %u, bytes %zu\n",nv2a_pb_gpu_state()->flips,n);ExitProcess(89);}
#endif
 return result;
}

static int gp_tex_shadow_sizes(const Stage *t, uint32_t bytes, uint32_t *palette)
{
    if (!bytes || (t->color == 0x0B && t->pal_len > 256u)) return 0;
    *palette = t->color == 0x0B ? t->pal_len * 4u : 0;
    return (uint64_t)bytes + *palette <= GP_TEX_SHADOW_BUDGET;
}
static int gp_tex_shadow_matches(const TexEntry *e, const uint8_t *mem,
                                  const Stage *t, uint32_t bytes)
{
    uint32_t palette;
    if (!memo_on() || !e->shadow_valid || !e->source_shadow ||
            !gp_tex_shadow_sizes(t, bytes, &palette) ||
            e->shadow_tex != bytes || e->shadow_pal != palette) return 0;
    if (!gamepkg_verified_bytes_equal(e->source_shadow, mem + t->offset, bytes) ||
            (palette && !gamepkg_verified_bytes_equal(e->source_shadow + bytes, mem + t->palette, palette))) return 0;
#ifdef GAMEPKG_VERIFY_TEX_SHADOW
    if (tex_hash_pal(mem, t, bytes) != e->hash) {
        fprintf(stderr, LOGP "texture byte snapshot differs from original hash at flip %u\n", nv2a_pb_gpu_state()->flips);
        ExitProcess(86);
    }
#endif
    s_gp_shadow_hits++;
    return 1;
}
static void gp_tex_shadow_capture(TexEntry *e, const uint8_t *mem,
                                  const Stage *t, uint32_t bytes,
                                  uint64_t expected, uint32_t flip)
{
    uint32_t palette;
    if (!memo_on() || !gp_tex_shadow_due(e, flip)) return;
    if (!gp_tex_shadow_sizes(t, bytes, &palette)) {
        s_gp_shadow_fallbacks++;gp_tex_shadow_backoff(e, flip);return;
    }
    size_t total = (size_t)bytes + palette;
    if (e->shadow_alloc != total) {
        gp_tex_shadow_free(e);
        if (s_gp_shadow_bytes > GP_TEX_SHADOW_BUDGET - total) {
            s_gp_shadow_fallbacks++;gp_tex_shadow_backoff(e, flip);return;
        }
        uint8_t *copy;
#ifdef GAMEPKG_TEX_SHADOW_TEST
        copy = s_gp_shadow_fail_alloc ? NULL : (uint8_t *)malloc(total);
#else
        copy = (uint8_t *)malloc(total);
#endif
        if (!copy) {s_gp_shadow_fallbacks++;gp_tex_shadow_backoff(e, flip);return;}
        e->source_shadow = copy;e->shadow_alloc = (uint32_t)total;
        s_gp_shadow_bytes += total;
    }
    e->shadow_valid = 0;
    memcpy(e->source_shadow, mem + t->offset, bytes);
    if (palette) memcpy(e->source_shadow + bytes, mem + t->palette, palette);
    uint64_t hash = tex_hash(e->source_shadow, bytes);
    if (t->color == 0x0B) hash ^= tex_hash(e->source_shadow + bytes, palette) * 31u;
    if (hash != expected) {
        s_gp_shadow_fallbacks++;gp_tex_shadow_backoff(e, flip);return;
    }
    e->shadow_tex=bytes;e->shadow_pal=palette;e->shadow_valid=1;e->shadow_wait=0;
    s_gp_shadow_captures++;
}

/* The view for a stage's texture, decoded and uploaded if it is new or its
 * bytes changed since it was last checked (once per flip). */
static ID3D11ShaderResourceView *tex_bind(const Stage *t)
{
    const struct nv2a_pb_gpu *g = nv2a_pb_gpu_state();
    const uint8_t *mem = (const uint8_t *)xbox_GetMemoryOffset();
    uint32_t bytes = tex_extent(t), i;
    TexEntry *e = NULL, *victim = &s_tc[0];
    uint32_t slot;

    if (!bytes) {
        /* A format with no decoder is magenta, and said once, loudly: white
         * or black passes for a texture that decoded to that. */
        static uint8_t said[256];
        if ((uint64_t)t->width * t->height > TEX_MAX_TEXELS)
            return s_black_srv;
        if (!said[t->color & 0xFF]) {
            said[t->color & 0xFF] = 1;
            fprintf(stderr, LOGP "\x1b[1;35mtexture format 0x%02X (%ux%u at"
                    " 0x%08X) not decoded: drawn magenta\x1b[0m\n",
                    t->color, t->width, t->height, t->offset);
        }
        return s_magenta_srv ? s_magenta_srv : s_black_srv;
    }
    slot = tc_slot(t);
    if (memo_on() && s_tc_hint[slot]) {
        TexEntry *c = &s_tc[s_tc_hint[slot] - 1];
        if (c->live && c->offset == t->offset && c->color == t->color
                && c->width == t->width && c->height == t->height
                && c->pitch == t->pitch && c->palette == t->palette
                && c->levels == t->levels)
            e = c;
    }
    for (i = 0; !e && i < TEX_CACHE; i++) {
        TexEntry *c = &s_tc[i];
        if (c->live && c->offset == t->offset && c->color == t->color
                && c->width == t->width && c->height == t->height
                && c->pitch == t->pitch && c->palette == t->palette
                && c->levels == t->levels) {
            e = c;
            break;
        }
        if (!c->live || (victim->live && c->used < victim->used))
            victim = c;
    }
    if (e) {
        if (e->gen != g->flips) {
            int same = gp_tex_shadow_matches(e, mem, t, bytes);
            uint64_t h = same ? e->hash : tex_hash_pal(mem, t, bytes);
            s_tex_rehash++;
            e->gen = g->flips;
            if (h != e->hash) {
                gp_tex_shadow_backoff(e, g->flips);
                e->hash = h;
                s_tex_changed++;
                if (!tex_upload(e, t))
                    return s_black_srv;
            } else if (!same) {
                gp_tex_shadow_capture(e, mem, t, bytes, h, g->flips);
            }
        }
    } else {
        e = victim;
        tex_free(e);
        e->offset = t->offset;
        e->color = t->color;
        e->width = t->width;
        e->height = t->height;
        e->pitch = t->pitch;
        e->palette = t->palette;
        e->levels = t->levels;
        e->gen = g->flips;
        e->hash = tex_hash_pal(mem, t, bytes);
        if (!tex_upload(e, t)) {
            tex_free(e);
            return s_black_srv;
        }
        e->live = 1;
    }
    e->used = ++s_tc_clock;
    s_tc_hint[slot] = (uint16_t)(e - s_tc + 1);
    return gp_ui_bind(e,t);
}

/* NV2A wrap modes: 1 wrap, 2 mirror, 3 clamp to edge, 4 border, 5 clamp. */
static D3D11_TEXTURE_ADDRESS_MODE address_mode(uint32_t m)
{
    switch (m) {
    case 1:  return D3D11_TEXTURE_ADDRESS_WRAP;
    case 2:  return D3D11_TEXTURE_ADDRESS_MIRROR;
    default: return D3D11_TEXTURE_ADDRESS_CLAMP;
    }
}

/* SET_TEXTURE_FILTER: LOD bias in bits 0-12 (signed, 8 fraction bits), MIN
 * in bits 16-21, MAG in bits 24-27. MIN: 1 nearest and 2 linear on level 0,
 * 3/4 nearest/linear within the nearest level, 5/6 nearest/linear blended
 * between two levels, 7 convolution on level 0 (taken as linear). MAG: 1
 * nearest, 2 linear, 4 convolution (linear). The LOD range is CONTROL0's
 * clamps, as xemu's sampler: 0..0 when MIN does not mipmap. RECOMP_DEBUG=d3d11_point
 * forces nearest texels (the CPU path with RECOMP_DEBUG=pb_bilinear=0) for every
 * stage. */
#define filter_linear nv2a_tex_filter_linear   /* nv2a_backend_common.h */
#define filter_mip    nv2a_tex_filter_mip
#define lod_bias      nv2a_tex_lod_bias

#define SMP_MAX 256
static struct { uint64_t key; float bias; ID3D11SamplerState *s; } s_smp[SMP_MAX];
static int s_smp_n;
static int s_smp_last[4];   /* per stage: the entry it got last */

static ID3D11SamplerState *sampler(const Stage *t, float host_bias, unsigned ui_scale)
{
    static int point = -1;
    static ID3D11SamplerState *fallback;
    D3D11_SAMPLER_DESC sd;
    uint32_t u = t->addr_u & 0xF, v = t->addr_v & 0xF;
    int mn, mg, mip, i;
    uint64_t key;
    ID3D11SamplerState *st = NULL;

    if (point < 0)
        point = recomp_env(RENV_D3D11_POINT) != NULL;
    mn = !point && filter_linear(t->filter, 0);
    mg = !point && filter_linear(t->filter, 1);
    mip = ui_scale ? 2 : t->levels > 1 ? filter_mip(t->filter) : 0;
    if (!isfinite(host_bias)) host_bias=0.0f;
    float bias=mip ? lod_bias(t->filter)+host_bias : 0.0f;
    if(bias>15.99f)bias=15.99f;
    if(bias< -16.0f)bias= -16.0f;
    key = u | v << 4 | (uint64_t)mn << 8 | (uint64_t)mg << 9
        | (uint64_t)mip << 10 | (uint64_t)ui_scale << 40;
    if (mip)
        key |= (uint64_t)(t->filter & 0x1FFFu) << 12
             | (uint64_t)t->lod_min << 25 | (uint64_t)t->lod_max << 29;
    if (s_smp_last[t - s_stage] < s_smp_n
            && s_smp[s_smp_last[t - s_stage]].key == key
            && s_smp[s_smp_last[t - s_stage]].bias == bias)
        return s_smp[s_smp_last[t - s_stage]].s;
    for (i = 0; i < s_smp_n; i++)
        if (s_smp[i].key == key && s_smp[i].bias == bias) {
            s_smp_last[t - s_stage] = i;
            return s_smp[i].s;
        }
    memset(&sd, 0, sizeof sd);
    sd.Filter = (D3D11_FILTER)((mn ? 0x10 : 0) | (mg ? 0x4 : 0)
                               | (mip == 2 ? 0x1 : 0));
    sd.AddressU = address_mode(u);
    sd.AddressV = address_mode(v);
    sd.AddressW = D3D11_TEXTURE_ADDRESS_CLAMP;
    sd.ComparisonFunc = D3D11_COMPARISON_NEVER;
    sd.MipLODBias = bias;
    if(mip&&host_bias!=0.0f){static int said;if(!said++){fprintf(stderr,LOGP "host LOD bias active: %.3f, combined/clamped %.3f\n",host_bias,bias);}}
    sd.MinLOD = ui_scale ? 0.0f : mip ? (float)t->lod_min : 0.0f;
    sd.MaxLOD = ui_scale ? (ui_scale==4?2.0f:1.0f) : mip ? (float)t->lod_max : 0.0f;
    ID3D11Device_CreateSamplerState(s_dev, &sd, &st);
    if (!st)
        return fallback;
    if (s_smp_n < SMP_MAX) {
        i = s_smp_n++;
    } else {
        /* Full: replace one round robin. A released state's address can
         * come back, so the bound-state filter starts over. */
        static int next;
        i = next++ % SMP_MAX;
        if (s_smp[i].s == fallback)
            fallback = NULL;
        ID3D11SamplerState_Release(s_smp[i].s);
        s_state_gen++;
    }
    s_smp[i].key = key;
    s_smp[i].bias = bias;
    s_smp[i].s = st;
    s_smp_last[t - s_stage] = i;
    if (!fallback)
        fallback = st;
    return st;
}

static ID3D11ShaderResourceView *solid_texture(uint32_t argb)
{
    ID3D11ShaderResourceView *v = NULL;
    D3D11_TEXTURE2D_DESC td;
    D3D11_SUBRESOURCE_DATA sd;
    ID3D11Texture2D *t = NULL;
    memset(&td, 0, sizeof td);
    td.Width = td.Height = 1;
    td.MipLevels = td.ArraySize = 1;
    td.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    td.SampleDesc.Count = 1;
    td.Usage = D3D11_USAGE_IMMUTABLE;
    td.BindFlags = D3D11_BIND_SHADER_RESOURCE;
    sd.pSysMem = &argb;
    sd.SysMemPitch = 4;
    sd.SysMemSlicePitch = 4;
    if (SUCCEEDED(ID3D11Device_CreateTexture2D(s_dev, &td, &sd, &t))) {
        ID3D11Device_CreateShaderResourceView(s_dev, (ID3D11Resource *)t, NULL, &v);
        ID3D11Texture2D_Release(t);
    }
    return v;
}

static void create_black_texture(void)
{
    static const uint32_t black = 0xFF000000u;
    s_magenta_srv = solid_texture(0xFFFF00FFu);
    D3D11_TEXTURE2D_DESC td;
    D3D11_SUBRESOURCE_DATA sd;
    ID3D11Texture2D *t = NULL;
    memset(&td, 0, sizeof td);
    td.Width = td.Height = 1;
    td.MipLevels = td.ArraySize = 1;
    td.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    td.SampleDesc.Count = 1;
    td.Usage = D3D11_USAGE_IMMUTABLE;
    td.BindFlags = D3D11_BIND_SHADER_RESOURCE;
    sd.pSysMem = &black;
    sd.SysMemPitch = 4;
    sd.SysMemSlicePitch = 4;
    if (SUCCEEDED(ID3D11Device_CreateTexture2D(s_dev, &td, &sd, &t))) {
        ID3D11Device_CreateShaderResourceView(s_dev, (ID3D11Resource *)t, NULL,
                                              &s_black_srv);
        ID3D11Texture2D_Release(t);
    }
}

/* ---- pixel stage --------------------------------------------------------- */

/* Register-combiner pixel shaders, generated by d3d8_combiners.c from the
 * combiner registers and keyed by a hash of the parsed state (constants are
 * not part of it: they go in the constant buffer). */
#define RC_MAX 64
typedef struct {
    uint32_t           hash;
    int                used, bad;
    uint64_t           last_use;
    NV2ACombinerState  st;
    uint32_t input_key[36];
    int input_key_valid;
    ID3D11PixelShader *ps;
} RcEntry;
static RcEntry  s_rc[RC_MAX];
static uint64_t s_rc_compiles;
static RcEntry *s_rc_last;      /* the entry combiner_ps last returned from */
#define GAMEPKG_RC_KEY_HINTS 1024u
static uint16_t s_rc_key_hint[GAMEPKG_RC_KEY_HINTS];
static uint64_t s_rc_key_hits, s_rc_key_misses;
static uint32_t gamepkg_rc_key_slot(const uint32_t key[36])
{
    uint64_t h = nv2a_tex_hash((const uint8_t *)key, 36u * sizeof *key,
                              NV2A_TEX_HASH_SEED);
    return (uint32_t)(h ^ (h >> 32)) & (GAMEPKG_RC_KEY_HINTS - 1u);
}
static RcEntry *gamepkg_rc_key_find(const uint32_t key[36])
{
    if (!memo_on()) return NULL;
    uint16_t n = s_rc_key_hint[gamepkg_rc_key_slot(key)];
    if (n && n <= RC_MAX) {
        RcEntry *e = &s_rc[n - 1u];
        if (e->used && e->input_key_valid &&
                !memcmp(e->input_key, key, sizeof e->input_key)) {
            s_rc_key_hits++;
            return e;
        }
    }
    s_rc_key_misses++;
    return NULL;
}
static void gamepkg_rc_key_remember(RcEntry *e, const uint32_t key[36])
{
    if (!memo_on() || !e || !e->used) return;
    memcpy(e->input_key, key, sizeof e->input_key);
    e->input_key_valid = 1;
    s_rc_key_hint[gamepkg_rc_key_slot(key)] = (uint16_t)(e - s_rc + 1);
}


static uint32_t rc_hash(const NV2ACombinerState *st)
{
    const uint8_t *p = (const uint8_t *)st;
    uint32_t h = 0x811C9DC5u;
    size_t i;
    for (i = 0; i < sizeof *st; i++)
        h = (h ^ p[i]) * 0x01000193u;
    return h ? h : 1;
}

/* NULL when the shader cannot be built: the caller falls back to s_ps. */
static ID3D11PixelShader *combiner_ps(const NV2ACombinerState *st)
{
    static char src[32768];
    uint32_t h = rc_hash(st);
    RcEntry *e = NULL, *lru = &s_rc[0];
    ID3DBlob *code = NULL;
    int i;

    for (i = 0; i < RC_MAX; i++) {
        RcEntry *c = &s_rc[i];
        if (c->used && c->hash == h && !memcmp(&c->st, st, sizeof *st)) {
            c->last_use = s_tick;
            s_rc_last = c;
            return c->bad ? NULL : c->ps;
        }
        if (!c->used) {
            if (!e) e = c;
        } else if (c->last_use < lru->last_use) {
            lru = c;
        }
    }
    if (!e) {
        e = lru;
        if (e->ps) {
            ID3D11PixelShader_Release(e->ps);
            s_state_gen++;
        }
    }
    memset(e, 0, sizeof *e);
    s_rc_last = e;
    e->used = 1;
    e->hash = h;
    e->st = *st;
    e->last_use = s_tick;
    s_rc_compiles++;
    if (d3d8_combiners_generate_hlsl(st, src, (int)sizeof src) < 0
     || !compile("nv2a_ps_combiner", src, "ps_5_0", &code)) {
        fprintf(stderr, LOGP "combiner shader %08X failed (%d stages)\n",
                (unsigned)h, st->num_stages);
        e->bad = 1;
        return NULL;
    }
    if (FAILED(gamepkg_timed_create_pixel_shader(s_dev,
                   ID3D10Blob_GetBufferPointer(code),
                   ID3D10Blob_GetBufferSize(code), NULL, &e->ps)))
        e->bad = 1;
    ID3D10Blob_Release(code);
    if (recomp_env(RENV_D3D11_VERBOSE))
        fprintf(stderr, LOGP "combiner shader %08X: %d stages\n%s\n",
                (unsigned)h, st->num_stages, src);
    return e->bad ? NULL : e->ps;
}

/* Render to texture: a stage whose texture lies where a surface this backend
 * renders into starts samples that surface. The guest's copy of those bytes
 * is stale (nothing here writes guest memory), so decoding it would show
 * whatever was there before -- or nothing at all. A 32-bit texture at the
 * surface's address only, and never the surface being drawn into. */
/* The surface being drawn into, sampled by the same batch (a full-screen
 * post pass reading the frame so far). D3D11 cannot bind one texture as both
 * target and resource, and the guest memory under it is empty -- the frame
 * lives on the GPU -- so the batch gets a copy taken just before it draws.
 * The CPU path reads the same thing: the surface memory as drawn so far. */
static RenderTarget     s_self;         /* tex/srv/w/h of the copy */
static uint64_t         s_self_copies;

static RenderTarget *rt_self_copy(RenderTarget *rt)
{
    if (s_self.tex && (s_self.w != rt->w || s_self.h != rt->h
                       || s_self.hw != rt->hw || s_self.hh != rt->hh)) {
        s_sc_init = 0;
        if (s_self.srv) ID3D11ShaderResourceView_Release(s_self.srv);
        ID3D11Texture2D_Release(s_self.tex);
        s_self.tex = NULL;
        s_self.srv = NULL;
    }
    if (!s_self.tex) {
        D3D11_TEXTURE2D_DESC td;
        ID3D11Texture2D_GetDesc(rt->tex, &td);
        td.BindFlags = D3D11_BIND_SHADER_RESOURCE;
        td.MiscFlags = 0;
        if (FAILED(ID3D11Device_CreateTexture2D(s_dev, &td, NULL, &s_self.tex)))
            return NULL;
        if (FAILED(ID3D11Device_CreateShaderResourceView(
                    s_dev, (ID3D11Resource *)s_self.tex, NULL, &s_self.srv))) {
            ID3D11Texture2D_Release(s_self.tex);
            s_self.tex = NULL;
            return NULL;
        }
        s_self.w = rt->w;
        s_self.h = rt->h;
        s_self.hw = rt->hw;     /* GetDesc gave the copy the source's size */
        s_self.hh = rt->hh;
    }
    ID3D11DeviceContext_CopyResource(s_ctx, (ID3D11Resource *)s_self.tex,
                                     (ID3D11Resource *)rt->tex);
    s_self.offset = rt->offset;
    s_self.addr = rt->addr;
    s_self.last_draw = rt->last_draw;
    s_self.last_clear = rt->last_clear;
    s_self_copies++;
    {
        static int said;
        if (!said++)
            fprintf(stderr, LOGP "surface 0x%08X sampled while drawn into:"
                    " bound as a copy\n", rt->offset);
    }
    return &s_self;
}

static RenderTarget *rt_texture(const Stage *t)
{
    static int off = -1;
    RenderTarget *self = NULL;
    int i;

    if (off < 0)
        off = recomp_env(RENV_D3D11_NO_RTT) != NULL;   /* A/B: decode memory */
    if (off)
        return NULL;
    if (!t->offset || d3d8_format_dxt_block_bytes(t->color)
            || nv2a_tex_texel_bytes(nv2a_tex_linear_twin(t->color)) != 4)
        return NULL;
    for (i = 0; i < RT_MAX; i++) {
        RenderTarget *rt = &s_rt[i];
        /* Low 27 bits: the same memory through either window. */
        if (!rt->tex || ((rt->addr ^ t->offset) & 0x07FFFFFFu))
            continue;
        if (rt_drop_if_stale(rt, "bind"))
            continue;   /* the texture decodes from the title's bytes */
        if (rt == s_cur) {
            self = rt;
            continue;
        }
        if (!rt->srv && FAILED(ID3D11Device_CreateShaderResourceView(
                    s_dev, (ID3D11Resource *)rt->tex, NULL, &rt->srv)))
            return NULL;
        return rt;
    }
    return self ? rt_self_copy(self) : NULL;
}

#include "native_cubemap.h"

/* The combiner state of the batch being drawn (for the trace), and whether
 * it ran through combiners at all. */
static NV2ACombinerState s_last_rc;
static int               s_last_rc_on;

/* RECOMP_D3D11_DEBUG_PS=<what>: every batch draws one input of the pixel
 * stage instead of its real shader, to see which one goes wrong:
 *   d0 / d1     the interpolated diffuse / specular colour
 *   t0 .. t3    stage n's texture at its coordinates (alpha forced to 1)
 *   tc0 .. tc3  stage n's coordinates as colour (frac of s, t)
 * Unknown values draw white. */
/* RECOMP_D3D11_DEBUG_PS_SKIP_POST=1: batches that look like post passes --
 * depth test off, at most 8 vertices (a full-screen quad or two) -- keep
 * their real shader, so a glow or blur drawn over the scene does not
 * cover what the debug shader shows. Set per batch by on_draw. */
static int s_dbg_post;

static ID3D11PixelShader *debug_ps(void)
{
    static int tried;
    static ID3D11PixelShader *ps;
    const char *w;
    char src[4096], expr[256];
    ID3DBlob *code = NULL;

    if (tried)
        return ps;
    tried = 1;
    w = recomp_env(RENV_D3D11_DEBUG_PS);
    if (!w || !*w)
        return NULL;
    if (!strcmp(w, "d0"))
        snprintf(expr, sizeof expr, "float4(i.oD0.rgb, 1)");
    else if (!strcmp(w, "d1"))
        snprintf(expr, sizeof expr, "float4(i.oD1.rgb, 1)");
    else if (w[0] == 't' && w[1] >= '0' && w[1] <= '3' && !w[2])
        snprintf(expr, sizeof expr,
                 "float4(tex%c.Sample(samp%c, i.oT%c.xy * tex_scale[%c].xy).rgb, 1)",
                 w[1], w[1], w[1], w[1]);
    else if (w[0] == 't' && w[1] == 'c' && w[2] >= '0' && w[2] <= '3' && !w[3])
        snprintf(expr, sizeof expr, "float4(frac(i.oT%c.xy), 0, 1)", w[2]);
    else
        snprintf(expr, sizeof expr, "float4(1, 1, 1, 1)");
    snprintf(src, sizeof src, "%s%s"
             "Texture2D tex0 : register(t0); Texture2D tex1 : register(t1);\n"
             "Texture2D tex2 : register(t2); Texture2D tex3 : register(t3);\n"
             "SamplerState samp0 : register(s0); SamplerState samp1 : register(s1);\n"
             "SamplerState samp2 : register(s2); SamplerState samp3 : register(s3);\n"
             "float4 main(VS_OUT i) : SV_Target { return %s; }\n",
             VS_OUT_DECL, PS_CB_DECL, expr);
    if (compile("nv2a_ps_debug", src, "ps_5_0", &code)) {
        gamepkg_timed_create_pixel_shader(s_dev, ID3D10Blob_GetBufferPointer(code),
                                       ID3D10Blob_GetBufferSize(code), NULL, &ps);
        ID3D10Blob_Release(code);
    }
    fprintf(stderr, LOGP "d3d11_debug_ps=%s: every batch draws %s%s\n",
            w, expr, ps ? "" : " (FAILED to build)");
    return ps;
}

/* Stages, textures, samplers and PS constants for the batch about to be
 * drawn. Returns the pixel shader to use. */
static PsConsts s_cb_ps_shadow;
static int      s_cb_ps_ok;

#include "kernel/nv2a_texmip.h"
#include "private_pixel_profile.h"

static ID3D11PixelShader *setup_pixel(void)
{
    const struct nv2a_pb_vsh *v = nv2a_pb_vsh_state();
    ID3D11ShaderResourceView *srv[4];
    ID3D11SamplerState *smp[4];
    D3D11_MAPPED_SUBRESOURCE m;
    PsConsts pc;
    int n, textured = 0;

    GP_PIXEL_DIAG_BEGIN();
    build_stages();
    const struct nv2a_pb_present *pr=nv2a_pb_present_state();
    struct nv2a_texscale_draw draw={0,v->depth_enable,v->depth_enable&&v->depth_mask,s_cur?s_cur->w:0,s_cur?s_cur->h:0,pr->w,pr->h};
    gp_ui_draw=nv2a_texscale_draw_is_2d(&draw);
    float host_bias=nv2a_host_lod_bias();
    if(host_bias<0.0f && (nv2a_texmip_mode() & NV2A_TEXMIP_BIAS)
        && nv2a_texmip_cut_ref(v->atest_enable,v->atest_func,v->atest_ref))host_bias=0.0f;
    nv2a_ps_consts_fill(v, s_stage, &pc);
    for (n = 0; n < 4; n++) {
        const Stage *t = &s_stage[n];
        gp_ui_bound=0;
        int scaled = nv2a_tex_size_from_format(t->color);
        GP_PIXEL_DIAG_MARK(1);
        int cube=t->mode==3&&t->cube;
        RenderTarget *src = (t->mode && t->mode != 4 && !cube) ? rt_texture(t) : NULL;
        GP_PIXEL_DIAG_MARK(2);
        s_stage_rt[n] = src;
        s_stage_src[n] = !t->mode ? "none" : t->mode == 4 ? "passthru" : "cache";
        if(cube){srv[n]=gp_cube_bind(t);s_stage_src[n]="cube";textured=1;}
        else if (src) {
            s_stage_src[n] = src == &s_self ? "render-target(self copy)"
                                            : "render-target";
            /* Linear coordinates are texels of the surface as drawn. */
            if (!scaled) {
                pc.tex_scale[n][0] = 1.0f / (float)src->w;
                pc.tex_scale[n][1] = 1.0f / (float)src->h;
            }
            srv[n] = src->srv;
            s_rtt_binds++;
            textured = 1;
        } else if (t->mode && t->mode != 4) {
            srv[n] = tex_bind(t);
            if (srv[n] == s_magenta_srv) s_stage_src[n] = "MAGENTA(undecoded)";
            else if (srv[n] == s_black_srv) s_stage_src[n] = "BLACK(too large/failed)";
            textured = 1;
        } else {
            srv[n] = s_black_srv;
        }
        GP_PIXEL_DIAG_MARK(3);
        smp[n] = sampler(t,host_bias,gp_ui_bound);
    }
    GP_PIXEL_DIAG_MARK(4);
    if (memo_on() && s_cb_ps_ok && !memcmp(&s_cb_ps_shadow, &pc, sizeof pc)) {
        /* the buffer already holds these bytes */
    } else if (SUCCEEDED(ID3D11DeviceContext_Map(s_ctx, (ID3D11Resource *)s_cb_ps, 0,
                                          D3D11_MAP_WRITE_DISCARD, 0, &m))) {
        memcpy(m.pData, &pc, sizeof pc);
        ID3D11DeviceContext_Unmap(s_ctx, (ID3D11Resource *)s_cb_ps, 0);
        s_cb_ps_shadow = pc;
        s_cb_ps_ok = 1;
    }
    if (!sc_on() || memcmp(s_sc.ps_srv, srv, sizeof srv)) {
        ID3D11DeviceContext_PSSetShaderResources(s_ctx, 0, 4, srv);
        memcpy(s_sc.ps_srv, srv, sizeof srv);
    }
#ifdef GAMEPKG_D3D11_VERIFY_OUTPUT_CACHE
        gp_verify_counts.output++;
    {
        ID3D11ShaderResourceView *actual[4] = {0};
        ID3D11DeviceContext_PSGetShaderResources(s_ctx, 0, 4, actual);
        int ok = !memcmp(actual, srv, sizeof actual);
        for (int k=0;k<4;k++) if(actual[k]) ID3D11ShaderResourceView_Release(actual[k]);
        if (!ok) { fprintf(stderr, "[GamePKG] shader resources differ from uncached bindings\n"); ExitProcess(80); }
    }
#endif
    if (!sc_on() || memcmp(s_sc.smp, smp, sizeof smp)) {
        ID3D11DeviceContext_PSSetSamplers(s_ctx, 0, 4, smp);
        memcpy(s_sc.smp, smp, sizeof smp);
    }
    if (!sc_on() || s_sc.ps_cb != s_cb_ps) {
        ID3D11DeviceContext_PSSetConstantBuffers(s_ctx, 0, 1, &s_cb_ps);
        s_sc.ps_cb = s_cb_ps;
    }
    if (textured)
        s_draws_tex++;

    GP_PIXEL_DIAG_MARK(5);
    if (v->rc_set && (v->rc_ctl & 0xFF)) {
        NV2ACombinerState st;
        ID3D11PixelShader *ps;
        uint32_t modes = 0;
        for (n = 0; n < 4; n++) {
            uint32_t m=gp_native_texture_mode((unsigned)s_stage[n].mode,
                        (unsigned)s_stage[n].raw_mode,s_stage[n].cube);
            modes |= m << (5 * n);
        }
        /* The registers and modes that make the state: the same as the last
         * batch's give the same state and so the same shader. */
        static struct {
            uint32_t r[36];
            int      key_ok, ps_ok;   /* s_last_rc is r's state; ps is its shader */
            ID3D11PixelShader *ps;
            RcEntry *e;
            uint64_t compiles;
        } memo;
        uint32_t key[36];
        int hit;
        memcpy(key, v->rc_cicw, 32);
        memcpy(key + 8, v->rc_aicw, 32);
        memcpy(key + 16, v->rc_cocw, 32);
        memcpy(key + 24, v->rc_aocw, 32);
        key[32] = v->rc_ctl;
        key[33] = v->rc_fcw0;
        key[34] = v->rc_fcw1;
        key[35] = modes;
        hit = memo_on() && memo.key_ok && !memcmp(memo.r, key, sizeof key);
        if (!hit) {
            RcEntry *reuse = gamepkg_rc_key_find(key);
            if (reuse) {
#ifdef GAMEPKG_VERIFY_RC_KEY_CACHE
        gp_verify_counts.raw++;
                NV2ACombinerState reference;
                d3d8_combiners_from_regs(v->rc_cicw, v->rc_aicw, v->rc_cocw,
                        v->rc_aocw, v->rc_ctl, v->rc_fcw0, v->rc_fcw1,
                        modes, &reference);
                reference.fog_input = 1;
                if (memcmp(&reference, &reuse->st, sizeof reference)) {
                    fprintf(stderr, "[GamePKG] raw combiner key changed state\n");
                    ExitProcess(84);
                }
                uint64_t before = s_rc_compiles;
                ID3D11PixelShader *expected = combiner_ps(&reference);
                if (s_rc_last != reuse || expected != (reuse->bad ? NULL : reuse->ps)
                        || s_rc_compiles != before) {
                    fprintf(stderr, "[GamePKG] raw combiner cache differs from original lookup\n");
                    ExitProcess(85);
                }
#endif
                s_last_rc = reuse->st;
                memcpy(memo.r, key, sizeof key);
                memo.key_ok = memo.ps_ok = 1;
                memo.ps = reuse->bad ? NULL : reuse->ps;
                memo.e = reuse;
                memo.compiles = s_rc_compiles;
                hit = 1;
            }
        }

        if (!hit) {
            d3d8_combiners_from_regs(v->rc_cicw, v->rc_aicw, v->rc_cocw,
                                     v->rc_aocw, v->rc_ctl, v->rc_fcw0,
                                     v->rc_fcw1, modes, &st);
            st.fog_input = 1;   /* FOG.a is the interpolated vertex fog factor */
            s_last_rc = st;
            memcpy(memo.r, key, sizeof key);
            memo.key_ok = 1;
            memo.ps_ok = 0;
        }
        s_last_rc_on = 1;
        if (!s_dbg_post && (ps = debug_ps()) != NULL)
            GP_PIXEL_DIAG_RETURN(ps);
        /* A compile since may have evicted the entry remembered. */
        if (hit && memo.ps_ok && memo.compiles == s_rc_compiles) {
            memo.e->last_use = s_tick;   /* as combiner_ps's hit does */
            ps = memo.ps;
        } else {
            ps = combiner_ps(&s_last_rc);
            memo.ps = ps;
            memo.e = s_rc_last;
            memo.compiles = s_rc_compiles;
            memo.ps_ok = 1;
            gamepkg_rc_key_remember(memo.e, key);
        }
        if (ps)
            GP_PIXEL_DIAG_RETURN(ps);
    }
    if (!(v->rc_set && (v->rc_ctl & 0xFF)))
        s_last_rc_on = 0;
    {
        ID3D11PixelShader *ps = s_dbg_post ? NULL : debug_ps();
        if (ps)
            GP_PIXEL_DIAG_RETURN(ps);
    }
    GP_PIXEL_DIAG_RETURN(s_stage[0].mode==3&&s_stage[0].cube?gp_cube_basic:s_ps);
}


/* ---- batch trace ------------------------------------------------------------
 *
 * The full state of one batch, for the pixel trace below (RECOMP_TRACE=px):
 * the state, every stage and where its SRV came from, the input layout, the
 * vertex program's outputs for the first three vertices twice -- from the CPU
 * interpreter on the same inputs, and read back from the GPU through stream
 * output -- the generated HLSL (once per program/combiner), and the colour
 * of the first triangle's centroid before the draw. Lines start
 * "[D3D11-WHITE]", after the investigation it was written for. */

static uint32_t read_tex_pixel(ID3D11Texture2D *tex, UINT x, UINT y)
{
    static ID3D11Texture2D *st;
    D3D11_MAPPED_SUBRESOURCE m;
    D3D11_BOX b;
    uint32_t px = 0xDEADBEEFu;

    if (!st) {
        D3D11_TEXTURE2D_DESC td;
        memset(&td, 0, sizeof td);
        td.Width = td.Height = 1;
        td.MipLevels = td.ArraySize = 1;
        td.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
        td.SampleDesc.Count = 1;
        td.Usage = D3D11_USAGE_STAGING;
        td.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
        if (FAILED(ID3D11Device_CreateTexture2D(s_dev, &td, NULL, &st)))
            return px;
    }
    b.left = x; b.right = x + 1;
    b.top = y; b.bottom = y + 1;
    b.front = 0; b.back = 1;
    ID3D11DeviceContext_CopySubresourceRegion(s_ctx, (ID3D11Resource *)st, 0, 0, 0, 0,
                                              (ID3D11Resource *)tex, 0, &b);
    if (SUCCEEDED(ID3D11DeviceContext_Map(s_ctx, (ID3D11Resource *)st, 0,
                                          D3D11_MAP_READ, 0, &m))) {
        px = *(const uint32_t *)m.pData;   /* B,G,R,A bytes: 0xAARRGGBB */
        ID3D11DeviceContext_Unmap(s_ctx, (ID3D11Resource *)st, 0);
    }
    return px;
}

static uint32_t read_pixel(RenderTarget *rt, int x, int y)
{
    if (x < 0 || y < 0 || (UINT)x >= rt->w || (UINT)y >= rt->h)
        return 0xDEADBEEFu;
    /* Guest coordinates; a scaled target holds the pixel's first sample at
     * N times them. */
    return read_tex_pixel(rt->tex, (UINT)x * (rt->hw / rt->w), (UINT)y * (rt->hh / rt->h));
}

/* The first three vertices of the bound batch through the VS, read back by
 * stream output: 7 float4 each (SV_POSITION, COLOR0/1, TEXCOORD0..3). */
static int trace_stream_out(VsEntry *vs, D3D11_PRIMITIVE_TOPOLOGY topo,
                            float out[3][7][4])
{
    static const D3D11_SO_DECLARATION_ENTRY so[7] = {
        {0, "SV_POSITION", 0, 0, 4, 0}, {0, "COLOR", 0, 0, 4, 0},
        {0, "COLOR", 1, 0, 4, 0},       {0, "TEXCOORD", 0, 0, 4, 0},
        {0, "TEXCOORD", 1, 0, 4, 0},    {0, "TEXCOORD", 2, 0, 4, 0},
        {0, "TEXCOORD", 3, 0, 4, 0},
    };
    static ID3D11Buffer *buf, *stage;
    UINT stride = 7 * 16, zero = 0;
    ID3D11GeometryShader *gs = NULL;
    ID3D11Buffer *none = NULL;
    D3D11_MAPPED_SUBRESOURCE m;
    HRESULT hr;
    int ok = 0;

    if (!buf) {
        D3D11_BUFFER_DESC bd;
        memset(&bd, 0, sizeof bd);
        bd.ByteWidth = 3 * 7 * 16;
        bd.Usage = D3D11_USAGE_DEFAULT;
        bd.BindFlags = D3D11_BIND_STREAM_OUTPUT;
        if (FAILED(ID3D11Device_CreateBuffer(s_dev, &bd, NULL, &buf)))
            return 0;
        bd.Usage = D3D11_USAGE_STAGING;
        bd.BindFlags = 0;
        bd.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
        if (FAILED(ID3D11Device_CreateBuffer(s_dev, &bd, NULL, &stage)))
            return 0;
    }
    hr = ID3D11Device_CreateGeometryShaderWithStreamOutput(
        s_dev, ID3D10Blob_GetBufferPointer(vs->code), ID3D10Blob_GetBufferSize(vs->code),
        so, 7, &stride, 1, D3D11_SO_NO_RASTERIZED_STREAM, NULL, &gs);
    if (FAILED(hr)) {
        fprintf(stderr, "[D3D11-WHITE]  stream output: hr 0x%08lX\n", (unsigned long)hr);
        return 0;
    }
    ID3D11DeviceContext_SOSetTargets(s_ctx, 1, &buf, &zero);
    ID3D11DeviceContext_GSSetShader(s_ctx, gs, NULL, 0);
    ID3D11DeviceContext_IASetPrimitiveTopology(s_ctx, D3D11_PRIMITIVE_TOPOLOGY_POINTLIST);
    ID3D11DeviceContext_DrawIndexed(s_ctx, 3, 0, 0);
    ID3D11DeviceContext_SOSetTargets(s_ctx, 1, &none, &zero);
    ID3D11DeviceContext_GSSetShader(s_ctx, NULL, NULL, 0);
    ID3D11DeviceContext_IASetPrimitiveTopology(s_ctx, topo);
    ID3D11GeometryShader_Release(gs);
    ID3D11DeviceContext_CopyResource(s_ctx, (ID3D11Resource *)stage, (ID3D11Resource *)buf);
    if (SUCCEEDED(ID3D11DeviceContext_Map(s_ctx, (ID3D11Resource *)stage, 0,
                                          D3D11_MAP_READ, 0, &m))) {
        memcpy(out, m.pData, 3 * 7 * 16);
        ID3D11DeviceContext_Unmap(s_ctx, (ID3D11Resource *)stage, 0);
        ok = 1;
    }
    return ok;
}

static void trace_print_hlsl_once(uint32_t key, const char *what, const char *src)
{
    static uint32_t seen[64];
    static int n;
    int i;
    for (i = 0; i < n; i++)
        if (seen[i] == key)
            return;
    if (n < 64)
        seen[n++] = key;
    fprintf(stderr, "[D3D11-WHITE] ---- %s %08X ----\n%s\n[D3D11-WHITE] ---- end ----\n",
            what, key, src);
}

/* Before the draw: everything but the pixel. Returns the pixel to read back
 * after it in *px, *py (-1 if none). */
static void trace_batch(const struct nv2a_pb_draw *d, VsEntry *vs, RenderTarget *rt,
                        const uint8_t fmts[16], const UINT strides[16],
                        const UINT offs[16], D3D11_PRIMITIVE_TOPOLOGY topo,
                        int *px, int *py)
{
    const struct nv2a_pb_gpu *g = nv2a_pb_gpu_state();
    const struct nv2a_pb_vsh *v = nv2a_pb_vsh_state();
    float so[3][7][4];
    int k, i, have_so;

    *px = *py = -1;
    fprintf(stderr, "[D3D11-WHITE] batch prim %u n %u prog %08X start %u | rc %d ctl %08X"
            " shader %08X | depth %d func 0x%X mask %d | blend %d 0x%X 0x%X eq 0x%X"
            " | cull %d 0x%X front 0x%X | ctl0 %08X poly %d %g %g | atest %d"
            " 0x%X %u | surface 0x%08X %ux%u | fog en %d colour %08X\n",
            d->prim, d->idx_count, vs->hash, v->start, s_last_rc_on, v->rc_ctl,
            v->shader_prog, v->depth_enable, v->depth_func, v->depth_mask,
            g->blend_enable, g->blend_sfactor, g->blend_dfactor, g->blend_equation,
            v->cull_enable, v->cull_face, v->front_face, v->control0,
            v->poly_offset, v->poly_factor, v->poly_bias, v->atest_enable,
            v->atest_func, v->atest_ref, rt->offset, rt->w, rt->h, v->fog_enable,
            v->fog_color);
    for (k = 0; k < 4; k++) {
        const Stage *t = &s_stage[k];
        fprintf(stderr, "[D3D11-WHITE]  tex%d mode %d on %d fmt %02X %ux%u off %08X pitch %u"
                " addr %u/%u filter %08X pal %08X src %s", k, t->mode, t->on, t->color,
                t->width, t->height, t->offset, t->pitch, t->addr_u, t->addr_v,
                t->filter, t->palette,
                s_stage_src[k] ? s_stage_src[k] : "?");
        fprintf(stderr, " levels %u lod %u-%u", t->levels, t->lod_min, t->lod_max);
        if (s_stage_rt[k])
            fprintf(stderr, " (surface 0x%08X %ux%u, drawn %llu, cleared %llu)",
                    s_stage_rt[k]->offset, s_stage_rt[k]->w, s_stage_rt[k]->h,
                    (unsigned long long)s_stage_rt[k]->last_draw,
                    (unsigned long long)s_stage_rt[k]->last_clear);
        fprintf(stderr, "\n");
    }
    for (i = 0; i < NV2A_VS_MAX_INPUTS; i++) {
        const VertexAttr *a = &d->attr[i];
        if (!(vs->inputs & (1u << i)) && !a->size)
            continue;
        fprintf(stderr, "[D3D11-WHITE]  attr%d %s type %u size %u stride %u offset 0x%X"
                " inline %d | IA dxgi %u stride %u at %u | inl %g %g %g %g\n", i,
                (vs->inputs & (1u << i)) ? "read" : "unread", a->type, a->size,
                a->stride, a->offset, d->inline_data, fmts[i], strides[i], offs[i],
                v->inl[i][0], v->inl[i][1], v->inl[i][2], v->inl[i][3]);
    }
    fprintf(stderr, "[D3D11-WHITE]  c58 %g %g %g %g c59 %g %g %g %g\n",
            v->c[58][0], v->c[58][1], v->c[58][2], v->c[58][3],
            v->c[59][0], v->c[59][1], v->c[59][2], v->c[59][3]);
    {   /* RECOMP_D3D11_PX_CONSTS=a-b,c,...: those vertex constants too */
        const char *e = recomp_env(RENV_D3D11_PX_CONSTS);
        while (e && *e) {
            char *end;
            long lo = strtol(e, &end, 10), hi = lo, c;
            if (end == e)
                break;
            if (*end == '-')
                hi = strtol(end + 1, &end, 10);
            for (c = lo; c <= hi && c >= 0 && c < NV2A_VSH_CONSTANTS; c++)
                fprintf(stderr, "[D3D11-WHITE]  c%ld %g %g %g %g\n", c,
                        v->c[c][0], v->c[c][1], v->c[c][2], v->c[c][3]);
            e = *end == ',' ? end + 1 : NULL;
        }
    }

    have_so = trace_stream_out(vs, topo, so);
    {   /* RECOMP_D3D11_PX_VERTS=1: the program's first 16 slots and every
         * vertex's screen position, for working a draw out by hand */
        static int all = -1;
        if (all < 0)
            all = recomp_env(RENV_D3D11_PX_VERTS) != NULL;
        for (k = 0; all && k < 16; k++)
            fprintf(stderr, "[D3D11-P] %d %08X %08X %08X %08X\n", k,
                    v->prog[v->start + k][0], v->prog[v->start + k][1],
                    v->prog[v->start + k][2], v->prog[v->start + k][3]);
        for (k = 0; all && (uint32_t)k < d->idx_count; k++) {
            float in[NV2A_VSH_INPUTS][4];
            Nv2aVshOut o;
            memset(in, 0, sizeof in);
            for (i = 0; i < NV2A_VSH_INPUTS && i < NV2A_VS_MAX_INPUTS; i++) {
                const VertexAttr *a = &d->attr[i];
                if (!(a->size && a->stride) || !fetch(d, a, d->idx[k], in[i]))
                    memcpy(in[i], v->inl[i], 16);
            }
            nv2a_vsh_run((const uint32_t (*)[4])v->prog, v->start,
                         (float (*)[4])v->c, 0, (const float (*)[4])in, &o);
            fprintf(stderr, "[D3D11-V] %u %g %g %g %g\n", d->idx[k],
                    o.o[0][0], o.o[0][1], o.o[0][2], o.o[0][3]);
        }
    }
    for (k = 0; k < 3 && (uint32_t)k < d->idx_count; k++) {
        float in[NV2A_VSH_INPUTS][4];
        Nv2aVshOut o;
        uint32_t index = d->idx[k];
        memset(in, 0, sizeof in);
        for (i = 0; i < NV2A_VSH_INPUTS && i < NV2A_VS_MAX_INPUTS; i++) {
            const VertexAttr *a = &d->attr[i];
            if (!(a->size && a->stride) || !fetch(d, a, index, in[i]))
                memcpy(in[i], v->inl[i], 16);
        }
        nv2a_vsh_run((const uint32_t (*)[4])v->prog, v->start,
                     (float (*)[4])v->c, 0, (const float (*)[4])in, &o);
        fprintf(stderr, "[D3D11-WHITE]  vert %u (index %u):", k, index);
        for (i = 0; i < 16; i++)
            if (d->attr[i].size)
                fprintf(stderr, " v%d(%g %g %g %g)", i, in[i][0], in[i][1], in[i][2], in[i][3]);
        fprintf(stderr, "\n[D3D11-WHITE]   cpu pos %g %g %g %g D0 %g %g %g %g D1 %g %g %g %g"
                " T0 %g %g %g %g T1 %g %g %g %g\n",
                o.o[0][0], o.o[0][1], o.o[0][2], o.o[0][3],
                o.o[3][0], o.o[3][1], o.o[3][2], o.o[3][3],
                o.o[4][0], o.o[4][1], o.o[4][2], o.o[4][3],
                o.o[9][0], o.o[9][1], o.o[9][2], o.o[9][3],
                o.o[10][0], o.o[10][1], o.o[10][2], o.o[10][3]);
        if (have_so)
            fprintf(stderr, "[D3D11-WHITE]   gpu clip %g %g %g %g D0 %g %g %g %g D1 %g %g %g %g"
                    " T0 %g %g %g %g T1 %g %g %g %g\n",
                    so[k][0][0], so[k][0][1], so[k][0][2], so[k][0][3],
                    so[k][1][0], so[k][1][1], so[k][1][2], so[k][1][3],
                    so[k][2][0], so[k][2][1], so[k][2][2], so[k][2][3],
                    so[k][3][0], so[k][3][1], so[k][3][2], so[k][3][3],
                    so[k][4][0], so[k][4][1], so[k][4][2], so[k][4][3]);
    }
    if (have_so && d->idx_count >= 3) {
        float sx = 0, sy = 0;
        int ok = 1;
        for (k = 0; k < 3; k++) {
            float w = so[k][0][3];
            if (w <= 0.0f) { ok = 0; break; }
            sx += (so[k][0][0] / w * 0.5f + 0.5f) * (float)rt->w;
            sy += (0.5f - so[k][0][1] / w * 0.5f) * (float)rt->h;
        }
        if (ok) {
            *px = (int)(sx / 3.0f);
            *py = (int)(sy / 3.0f);
        }
    }
    {
        static NV2AVshProgram parsed;
        static char hlsl[65536];
        d3d8_vsh_parse(&v->prog[v->start][0],
                       NV2A_VSH_SLOTS - (int)v->start, &parsed);
        if (d3d8_vsh_generate_hlsl_ex(&parsed, NV2A_VSH_HLSL_SCREEN_SPACE,
                                      hlsl, sizeof hlsl) > 0)
            trace_print_hlsl_once(vs->hash, "vertex program", hlsl);
        if (s_last_rc_on && d3d8_combiners_generate_hlsl(&s_last_rc, hlsl, (int)sizeof hlsl) >= 0) {
            uint32_t h = 2166136261u;
            const char *c;
            for (c = hlsl; *c; c++) { h ^= (uint8_t)*c; h *= 16777619u; }
            fprintf(stderr, "[D3D11-WHITE]  combiner %08X\n", h);
            trace_print_hlsl_once(h, "combiner", hlsl);
        }
    }
    {
        for (k = 0; k < 8; k++)
            fprintf(stderr, "%s%08X/%08X%s", k ? " " : "[D3D11-WHITE]  factors c0/c1: ",
                    v->rc_f0[k], v->rc_f1[k], k == 7 ? "\n" : "");
        fprintf(stderr, "[D3D11-WHITE]  final cw %08X %08X, sf %08X %08X\n",
                v->rc_fcw0, v->rc_fcw1, v->rc_sf[0], v->rc_sf[1]);
        for (k = 0; k < (int)(v->rc_ctl & 0xFF) && k < 8; k++)
            fprintf(stderr, "[D3D11-WHITE]  stage %d icw %08X ocw %08X aicw %08X aocw %08X\n",
                    k, v->rc_cicw[k], v->rc_cocw[k], v->rc_aicw[k], v->rc_aocw[k]);
    }
    if (*px >= 0)
        fprintf(stderr, "[D3D11-WHITE]  px (%d,%d) before %08X\n", *px, *py,
                read_pixel(rt, *px, *py));
}

/* ---- draw ------------------------------------------------------------------ */

/* ---- pixel probe ------------------------------------------------------------
 *
 * RECOMP_D3D11_PX=x,y[;x,y...] (up to 4) with RECOMP_D3D11_PX_FLIPS=a-b
 * (this backend's present count, the "[D3D11] flip N" number; default 0-0
 * means the first frame only): every batch drawn in that window that covers
 * one of the pixels is redrawn twice into eight float targets, with the same
 * vertex shader, inputs, textures and samplers, depth tested LESS_EQUAL
 * without writes against the depth the real draw left, and the pixel's
 * interpolated inputs read back:
 *   pass A  oD0, oD1, oT0..oT3 as interpolated, SV_Position, fog
 *   pass B  stage 0..3 texels as the combiner samples them (q divide for
 *           true 2D stages, tex_scale) and the coordinates it samples at
 * Lines start "[D3D11-PX]": the colour before and after the real draw, then
 * the inputs; then the batch's full [D3D11-WHITE] block (stages with mode,
 * enable, address and filter, attributes, vertex outputs, combiner factors;
 * the HLSL once per program/combiner).
 * RECOMP_D3D11_PX_MAX caps the batches printed (default 400). Alpha test and
 * stencil are not applied to the probe, so "covered" can include a fragment
 * the real draw discarded: compare before/after. */
static int s_px_n = -1, s_px_x[4], s_px_y[4], s_px_max;
static uint64_t s_px_f0, s_px_f1;

static int px_on(void)
{
    if (s_px_n < 0) {
        const char *e = recomp_env(RENV_D3D11_PX), *f = recomp_env(RENV_D3D11_PX_FLIPS);
        const char *m = recomp_env(RENV_D3D11_PX_MAX);
        s_px_n = 0;
        while (e && *e && s_px_n < 4) {
            int x, y;
            if (sscanf(e, "%d,%d", &x, &y) != 2)
                break;
            s_px_x[s_px_n] = x;
            s_px_y[s_px_n] = y;
            s_px_n++;
            e = strchr(e, ';');
            if (e) e++;
        }
        if (f) {
            unsigned long long a = 0, b = 0;
            int k = sscanf(f, "%llu-%llu", &a, &b);
            s_px_f0 = a;
            s_px_f1 = k == 2 ? b : a;
        }
        s_px_max = m ? atoi(m) : 400;
        if (s_px_n)
            fprintf(stderr, LOGP "px: %d pixel(s), flips %llu-%llu\n",
                    s_px_n, (unsigned long long)s_px_f0,
                    (unsigned long long)s_px_f1);
    }
    return s_px_n > 0 && s_px_max > 0 && s_presents >= s_px_f0 && s_presents <= s_px_f1;
}

static ID3D11PixelShader *px_shader(int pass)
{
    static ID3D11PixelShader *ps[2];
    static int tried[2];
    char src[6144];
    ID3DBlob *code = NULL;

    if (tried[pass])
        return ps[pass];
    tried[pass] = 1;
    snprintf(src, sizeof src, "%s%s"
        "Texture2D tex0 : register(t0); Texture2D tex1 : register(t1);\n"
        "Texture2D tex2 : register(t2); Texture2D tex3 : register(t3);\n"
        "SamplerState samp0 : register(s0); SamplerState samp1 : register(s1);\n"
        "SamplerState samp2 : register(s2); SamplerState samp3 : register(s3);\n"
        "struct PO { float4 o0 : SV_Target0; float4 o1 : SV_Target1;\n"
        "    float4 o2 : SV_Target2; float4 o3 : SV_Target3; float4 o4 : SV_Target4;\n"
        "    float4 o5 : SV_Target5; float4 o6 : SV_Target6; float4 o7 : SV_Target7; };\n"
        "float2 uvof(float4 t, uint n) {\n"
        "    float2 uv = t.xy;\n"
        "    if (tex_mode[n] == 1u && t.w != 0.0 && t.w != 1.0) uv /= t.w;\n"
        "    return uv * tex_scale[n].xy;\n"
        "}\n"
        "PO main(VS_OUT i) {\n"
        "    PO o;\n"
        "%s"
        "    return o;\n"
        "}\n",
        VS_OUT_DECL, PS_CB_DECL,
        pass == 0
        ? "    o.o0 = i.oD0; o.o1 = i.oD1; o.o2 = i.oT0; o.o3 = i.oT1;\n"
          "    o.o4 = i.oT2; o.o5 = i.oT3; o.o6 = i.oPos;\n"
          "    o.o7 = float4(i.oFog, 1, 0, 0);\n"
        : "    float2 u0 = uvof(i.oT0, 0), u1 = uvof(i.oT1, 1);\n"
          "    float2 u2 = uvof(i.oT2, 2), u3 = uvof(i.oT3, 3);\n"
          "    o.o0 = tex0.Sample(samp0, u0); o.o1 = tex1.Sample(samp1, u1);\n"
          "    o.o2 = tex2.Sample(samp2, u2); o.o3 = tex3.Sample(samp3, u3);\n"
          "    o.o4 = float4(u0, 0, 0); o.o5 = float4(u1, 0, 0);\n"
          "    o.o6 = float4(u2, 0, 0); o.o7 = float4(u3, 0, 0);\n");
    if (compile("nv2a_ps_probe", src, "ps_5_0", &code)) {
        gamepkg_timed_create_pixel_shader(s_dev, ID3D10Blob_GetBufferPointer(code),
                                       ID3D10Blob_GetBufferSize(code), NULL, &ps[pass]);
        ID3D10Blob_Release(code);
    }
    if (!ps[pass])
        fprintf(stderr, LOGP "px: probe shader %d FAILED to build\n", pass);
    return ps[pass];
}

static void px_probe(const struct nv2a_pb_draw *d, VsEntry *vs, RenderTarget *rt,
                     const uint8_t fmts[16], const UINT strides[16], const UINT offs[16],
                     D3D11_PRIMITIVE_TOPOLOGY topo, UINT nidx,
                     ID3D11DepthStencilView *dsv, const uint32_t before[4])
{
    static ID3D11Texture2D *tex[8], *st;
    static ID3D11RenderTargetView *rtv[8];
    static UINT tw, th;
    static ID3D11DepthStencilState *dss_le;
    static const char *names[2][8] = {
        {"D0", "D1", "T0", "T1", "T2", "T3", "pos", "fog/cover"},
        {"tex0", "tex1", "tex2", "tex3", "uv0", "uv1", "uv2", "uv3"}};
    float val[2][4][8][4];
    const float zero[4] = {0, 0, 0, 0};
    D3D11_MAPPED_SUBRESOURCE m;
    int pass, k, n, any = 0;

    if (!px_shader(0) || !px_shader(1))
        return;
    /* At the target's host size: they are bound with its depth view, and
     * D3D11 wants every attachment the same size. */
    if (!tex[0] || tw != rt->hw || th != rt->hh) {
        D3D11_TEXTURE2D_DESC td;
        for (k = 0; k < 8; k++) {
            if (rtv[k]) ID3D11RenderTargetView_Release(rtv[k]);
            if (tex[k]) ID3D11Texture2D_Release(tex[k]);
            rtv[k] = NULL;
            tex[k] = NULL;
        }
        memset(&td, 0, sizeof td);
        td.Width = rt->hw;
        td.Height = rt->hh;
        td.MipLevels = td.ArraySize = 1;
        td.Format = DXGI_FORMAT_R32G32B32A32_FLOAT;
        td.SampleDesc.Count = 1;
        td.Usage = D3D11_USAGE_DEFAULT;
        td.BindFlags = D3D11_BIND_RENDER_TARGET;
        for (k = 0; k < 8; k++)
            if (FAILED(ID3D11Device_CreateTexture2D(s_dev, &td, NULL, &tex[k]))
                    || FAILED(ID3D11Device_CreateRenderTargetView(
                        s_dev, (ID3D11Resource *)tex[k], NULL, &rtv[k])))
                return;
        tw = rt->hw;
        th = rt->hh;
    }
    if (!st) {
        D3D11_TEXTURE2D_DESC td;
        memset(&td, 0, sizeof td);
        td.Width = 8;
        td.Height = 4;
        td.MipLevels = td.ArraySize = 1;
        td.Format = DXGI_FORMAT_R32G32B32A32_FLOAT;
        td.SampleDesc.Count = 1;
        td.Usage = D3D11_USAGE_STAGING;
        td.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
        if (FAILED(ID3D11Device_CreateTexture2D(s_dev, &td, NULL, &st)))
            return;
    }
    if (!dss_le) {
        D3D11_DEPTH_STENCIL_DESC dd;
        memset(&dd, 0, sizeof dd);
        dd.DepthEnable = TRUE;
        dd.DepthWriteMask = D3D11_DEPTH_WRITE_MASK_ZERO;
        dd.DepthFunc = D3D11_COMPARISON_LESS_EQUAL;
        ID3D11Device_CreateDepthStencilState(s_dev, &dd, &dss_le);
    }

    for (pass = 0; pass < 2; pass++) {
        for (k = 0; k < 8; k++)
            ID3D11DeviceContext_ClearRenderTargetView(s_ctx, rtv[k], zero);
        memset(s_sc.ps_srv, 0xFF, sizeof s_sc.ps_srv);
        ID3D11DeviceContext_OMSetRenderTargets(s_ctx, 8, rtv, dsv);
        ID3D11DeviceContext_OMSetDepthStencilState(s_ctx, dsv ? dss_le : s_ds_off, 0);
        ID3D11DeviceContext_OMSetBlendState(s_ctx, NULL, NULL, 0xFFFFFFFF);
        ID3D11DeviceContext_PSSetShader(s_ctx, px_shader(pass), NULL, 0);
        ID3D11DeviceContext_DrawIndexed(s_ctx, nidx, 0, 0);
        for (n = 0; n < s_px_n; n++) {
            D3D11_BOX b;
            if (s_px_x[n] < 0 || s_px_y[n] < 0
                    || (UINT)s_px_x[n] >= rt->w || (UINT)s_px_y[n] >= rt->h)
                continue;
            b.left = (UINT)s_px_x[n] * (rt->hw / rt->w); b.right = b.left + 1;
            b.top = (UINT)s_px_y[n] * (rt->hh / rt->h); b.bottom = b.top + 1;
            b.front = 0; b.back = 1;
            for (k = 0; k < 8; k++)
                ID3D11DeviceContext_CopySubresourceRegion(
                    s_ctx, (ID3D11Resource *)st, 0, (UINT)k, (UINT)n, 0,
                    (ID3D11Resource *)tex[k], 0, &b);
        }
        memset(val[pass], 0, sizeof val[pass]);
        if (SUCCEEDED(ID3D11DeviceContext_Map(s_ctx, (ID3D11Resource *)st, 0,
                                              D3D11_MAP_READ, 0, &m))) {
            for (n = 0; n < s_px_n; n++)
                memcpy(val[pass][n], (const uint8_t *)m.pData + (size_t)n * m.RowPitch,
                       sizeof val[pass][n]);
            ID3D11DeviceContext_Unmap(s_ctx, (ID3D11Resource *)st, 0);
        }
    }
    /* back to the real draw's state; the state-call filter's view of it
     * stays true for everything restored here */
    memset(s_sc.ps_srv, 0xFF, sizeof s_sc.ps_srv);
        ID3D11DeviceContext_OMSetRenderTargets(s_ctx, 1, &rt->rtv, dsv);
    {
        /* Recomputed, not s_sc.bs/.bf: setup_pixel can evict a shader
         * after the blend bind, and sc_on() then resets s_sc to 0xFF. The
         * pgraph state is unchanged since the draw, so these are the same. */
        float bf[4];
        blend_color(bf);
        ID3D11DeviceContext_OMSetBlendState(s_ctx, blend_state(), bf, 0xFFFFFFFF);
    }
    ID3D11DeviceContext_PSSetShader(s_ctx, s_sc.ps, NULL, 0);
    s_state_gen++;              /* and the next draw rebinds everything */

    for (n = 0; n < s_px_n; n++) {
        uint32_t after;
        if (val[0][n][7][1] != 1.0f)
            continue;           /* not covered */
        after = read_pixel(rt, s_px_x[n], s_px_y[n]);
        if (!any) {
            s_px_max--;
            fprintf(stderr, "[D3D11-PX] flip %llu draw %llu surface 0x%08X prog %08X"
                    " prim %u n %u\n", (unsigned long long)s_presents,
                    (unsigned long long)s_draws, rt->offset,
                    d->program ? vs->hash : 0u, d->prim, d->idx_count);
        }
        any = 1;
        fprintf(stderr, "[D3D11-PX]  px (%d,%d) before %08X after %08X\n",
                s_px_x[n], s_px_y[n], before[n], after);
        for (pass = 0; pass < 2; pass++)
            for (k = 0; k < 8; k++)
                fprintf(stderr, "[D3D11-PX]   %-9s %g %g %g %g\n", names[pass][k],
                        val[pass][n][k][0], val[pass][n][k][1],
                        val[pass][n][k][2], val[pass][n][k][3]);
    }
    if (any) {
        int px, py;
        trace_batch(d, vs, rt, fmts, strides, offs, topo, &px, &py);
    }
}

extern int gamepkg_nv2a_constants_dirty(void);
extern void gamepkg_nv2a_constants_clean(void);
/* Keep this as one call at the former memcmp site, retaining the surrounding
 * measured branch shape. On writes, keep the original byte comparison and
 * avoid needless GPU maps for identical constants. */
__attribute__((noinline)) static int gamepkg_constant_shadow_matches(
        const void *shadow, const void *current, size_t n)
{
    if (!gamepkg_nv2a_constants_dirty()) {
#ifdef GAMEPKG_VERIFY_CONSTANT_DIRTY
        gp_verify_counts.constants++;
        if (memcmp(shadow, current, n)) {
            fprintf(stderr, "[GamePKG] clean constant bank differs from GPU shadow\n");
            ExitProcess(83);
        }
#endif
        return 1;
    }
    if (memcmp(shadow, current, n))
        return 0;
    gamepkg_nv2a_constants_clean();
    return 1;
}

#include "private_draw_profile.h"

static void d3d11_on_draw(const struct nv2a_pb_draw *d)
{
    /* As Metal's: at most three list entries per index, grown to the
     * largest batch so far rather than sized to NV_MAX_INDICES. */
    static uint32_t *idx;
    static uint32_t idx_cap;
    const struct nv2a_pb_gpu *g = nv2a_pb_gpu_state();
    const struct nv2a_pb_vsh *v = nv2a_pb_vsh_state();
    D3D11_PRIMITIVE_TOPOLOGY topo;
    ID3D11Buffer *vbs[NV2A_VS_MAX_INPUTS];
    UINT strides[NV2A_VS_MAX_INPUTS], offs[NV2A_VS_MAX_INPUTS];
    RenderTarget *rt;
    VsEntry *vs;
    ID3D11RasterizerState *rs;
    int cull_all;
    uint8_t fmts[16];
    uint32_t lo = 0xFFFF, hi = 0, count, nidx, i;
    void *p;

    GP_DRAW_DIAG_BEGIN();
    if (!init())
        GP_DRAW_DIAG_RETURN();
    GP_GPU_BEGIN();
    rt = surface();
    vs = vertex_shader(d->program);
    if (!rt || !vs || !d->idx_count) {
        s_draws_skipped++;
        GP_DRAW_DIAG_RETURN();
    }
    nv2a_idx_range(d->idx, d->idx_count, &lo, &hi);
    count = hi - lo + 1;
    if (d->idx_count * 3u > idx_cap) {
        uint32_t *grown = (uint32_t *)realloc(idx, (size_t)d->idx_count * 12u);
        if (!grown) {
            s_draws_skipped++;
            GP_DRAW_DIAG_RETURN();
        }
        idx = grown;
        idx_cap = d->idx_count * 3u;
    }
    nidx = convert_indices(d, lo, idx, &topo);
    rs = rasterizer_state(&cull_all);
    if (!nidx) {
        s_draws_skipped++;
        GP_DRAW_DIAG_RETURN();
    }
    if (cull_all && (topo == D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST
                     || topo == D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP)) {
        s_draws_culled++;
        GP_DRAW_DIAG_RETURN();
    }

    GP_DRAW_DIAG_MARK(1);
    /* Streams: each one the shader reads. One in a format the input
     * assembler reads natively is copied as it is: vertices lo..hi of its
     * buffer in one block, shared by every stream interleaved in it (same
     * stride, within one stride of each other), bound at their own offsets.
     * The rest go as float4 per vertex, or one float4 at stride 0 (the
     * current SET_VERTEX_DATA value) if disabled. */
    memset(vbs, 0, sizeof vbs);
    memset(strides, 0, sizeof strides);
    memset(offs, 0, sizeof offs);
    memset(fmts, 0, sizeof fmts);
    {   /* the most the streams below can take, each 16-aligned */
        uint64_t bound = 0;
        for (i = 0; i < NV2A_VS_MAX_INPUTS; i++) {
            const VertexAttr *a = &d->attr[i];
            if (!(vs->inputs & (1u << i)))
                continue;
            bound += (a->size && a->stride)
                   ? (uint64_t)count * (a->stride > 16 ? a->stride : 16) + 48 : 32;
        }
        vb_begin(bound <= 0xFFFFFFFFu ? (UINT)bound : 0);
    }
    for (i = 0; i < NV2A_VS_MAX_INPUTS; i++) {
        const VertexAttr *a = &d->attr[i];
        int on = a->size && a->stride;
        UINT bytes = on ? count * 16u : 16u, esz, at;
        float *f;

        if (!(vs->inputs & (1u << i)) || fmts[i])
            continue;
        if (on && (d->inline_data || a->offset) && !(a->stride & 3)
                && native_format(a, &esz)) {
            uint32_t lo_off = a->offset, hi_end = a->offset + esz, group = 0, j;
            const uint8_t *base = d->inline_data
                ? (const uint8_t *)d->inline_buf
                : (const uint8_t *)xbox_GetMemoryOffset();
            for (j = i; j < NV2A_VS_MAX_INPUTS; j++) {
                const VertexAttr *b = &d->attr[j];
                UINT bsz;
                if (!(vs->inputs & (1u << j)) || fmts[j] || !b->size
                        || b->stride != a->stride || !native_format(b, &bsz)
                        || (!d->inline_data && !b->offset)
                        || (b->offset > a->offset ? b->offset - a->offset
                                                  : a->offset - b->offset) >= a->stride)
                    continue;
                group |= 1u << j;
                if (b->offset < lo_off) lo_off = b->offset;
                if (b->offset + bsz > hi_end) hi_end = b->offset + bsz;
            }
            bytes = (count - 1u) * a->stride + (hi_end - lo_off);
            if (!d->inline_data
                    || (size_t)lo_off + (size_t)lo * a->stride + bytes <= d->inline_bytes) {
                uint8_t *dst = (uint8_t *)vb_alloc(bytes, &at);
                if (!dst) {
                    vb_end();
                    s_draws_skipped++;
                    GP_DRAW_DIAG_RETURN();
                }
                memcpy(dst, base + lo_off + (size_t)lo * a->stride, bytes);
                vb_written();
                for (j = i; j < NV2A_VS_MAX_INPUTS; j++) {
                    if (!(group & (1u << j)))
                        continue;
                    fmts[j] = (uint8_t)native_format(&d->attr[j], &esz);
                    offs[j] = at + (d->attr[j].offset - lo_off);
                    strides[j] = a->stride;
                    vbs[j] = s_vb;
                }
                s_vb_native += bytes;
                continue;
            }
            bytes = count * 16u;
        }
        fmts[i] = (uint8_t)DXGI_FORMAT_R32G32B32A32_FLOAT;
        f = (float *)vb_alloc(bytes, &at);
        if (!f) {
            vb_end();
            s_draws_skipped++;
            GP_DRAW_DIAG_RETURN();
        }
        offs[i] = at;
        if (on) {
            int one = i;
            nv2a_vtx_gather(d, &one, 1, lo, count, v->inl,
                            (const uint8_t *)xbox_GetMemoryOffset(), f);
            strides[i] = 16;
        } else {
            memcpy(f, v->inl[i], 16);
            strides[i] = 0;
        }
        s_vb_expanded += bytes;
        vb_written();
        vbs[i] = s_vb;
    }
    vb_end();
    p = ring_map(s_ib, s_ib_size, &s_ib_pos, nidx * 4u, 4);
    if (!p) {
        s_draws_skipped++;
        GP_DRAW_DIAG_RETURN();
    }
    memcpy(p, idx, nidx * 4u);
    ID3D11DeviceContext_Unmap(s_ctx, (ID3D11Resource *)s_ib, 0);

    GP_DRAW_DIAG_MARK(2);
    /* Constants: the program's c[192] at b1 and the pixels->NDC mapping. */
    {
        static uint8_t consts_shadow[sizeof v->c];
        static uint8_t vp_shadow[80];
        static int consts_ok, vp_ok;
        D3D11_MAPPED_SUBRESOURCE m;
        struct nv2a_vp_consts vc;
        uint8_t vp[80];
        nv2a_vp_consts_fill(v, g->format, rt->w, rt->h, &vc);
        memcpy(vp, vc.c, sizeof vc.c);
        memcpy(vp + sizeof vc.c, vc.u, sizeof vc.u);
        if (!d->program
                || (memo_on() && consts_ok && gamepkg_constant_shadow_matches(consts_shadow, v->c, sizeof v->c))) {
            /* the buffer already holds these bytes */
        } else if (SUCCEEDED(ID3D11DeviceContext_Map(
                s_ctx, (ID3D11Resource *)s_cb_consts, 0,
                D3D11_MAP_WRITE_DISCARD, 0, &m))) {
            memcpy(m.pData, v->c, sizeof v->c);
            ID3D11DeviceContext_Unmap(s_ctx, (ID3D11Resource *)s_cb_consts, 0);
            memcpy(consts_shadow, v->c, sizeof v->c);
            consts_ok = 1;
            gamepkg_nv2a_constants_clean();
        }
        if (memo_on() && vp_ok && !memcmp(vp_shadow, vp, sizeof vp)) {
            /* unchanged */
        } else if (SUCCEEDED(ID3D11DeviceContext_Map(
                s_ctx, (ID3D11Resource *)s_cb_vp, 0,
                D3D11_MAP_WRITE_DISCARD, 0, &m))) {
            memcpy(m.pData, vp, sizeof vp);
            ID3D11DeviceContext_Unmap(s_ctx, (ID3D11Resource *)s_cb_vp, 0);
            memcpy(vp_shadow, vp, sizeof vp);
            vp_ok = 1;
        }
    }

    GP_DRAW_DIAG_MARK(3);
    {
        D3D11_VIEWPORT vp;
        ID3D11Buffer *cbs[3];
        vp.TopLeftX = 0;
        vp.TopLeftY = 0;
        vp.Width = (float)rt->hw;
        vp.Height = (float)rt->hh;
        vp.MinDepth = 0;
        vp.MaxDepth = 1;
        cbs[0] = NULL;
        cbs[1] = s_cb_consts;
        cbs[2] = s_cb_vp;
        ID3D11DepthStencilView *dsv = NULL;
        ID3D11DepthStencilState *dss = s_ds_off;
        /* Stencil only on a Z24S8 zeta surface, as on the CPU. */
        int use_s = v->stencil_enable && ((g->format >> 4) & 0xF) == 2;
        if (v->depth_enable || use_s) {
            DepthTarget *dt = depth_target(rt->w, rt->h);
            if (dt) {
                dsv = dt->dsv;
                dss = depth_state(v->depth_enable, use_s);
                s_draws_z++;
            }
        }
        int om_cached = sc_on();
        UINT om_ref = use_s ? v->stencil_ref & 0xFFu : 0;
        if (!om_cached || s_sc.om_rtv != rt->rtv || s_sc.om_dsv != dsv) {
            memset(s_sc.ps_srv, 0xFF, sizeof s_sc.ps_srv);
        ID3D11DeviceContext_OMSetRenderTargets(s_ctx, 1, &rt->rtv, dsv);
            s_sc.om_rtv = rt->rtv;
            s_sc.om_dsv = dsv;
        }
        if (!om_cached || s_sc.om_dss != dss || s_sc.om_ref != om_ref) {
            ID3D11DeviceContext_OMSetDepthStencilState(s_ctx, dss, om_ref);
            s_sc.om_dss = dss;
            s_sc.om_ref = om_ref;
        }
#ifdef GAMEPKG_D3D11_VERIFY_OUTPUT_CACHE
        gp_verify_counts.output++;
        {
            ID3D11RenderTargetView *actual_rt = NULL;
            ID3D11DepthStencilView *actual_ds = NULL;
            ID3D11DepthStencilState *actual_state = NULL;
            UINT actual_ref = 0;
            ID3D11DeviceContext_OMGetRenderTargets(s_ctx, 1, &actual_rt, &actual_ds);
            ID3D11DeviceContext_OMGetDepthStencilState(s_ctx, &actual_state, &actual_ref);
            int ok = actual_rt == rt->rtv && actual_ds == dsv
                   && actual_state == dss && actual_ref == om_ref;
            if (actual_rt) ID3D11RenderTargetView_Release(actual_rt);
            if (actual_ds) ID3D11DepthStencilView_Release(actual_ds);
            if (actual_state) ID3D11DepthStencilState_Release(actual_state);
            if (!ok) { fprintf(stderr, "[GamePKG] output state differs from uncached binding\n"); ExitProcess(79); }
        }
#endif
        {
            ID3D11BlendState *bs = blend_state();
            ID3D11InputLayout *il = input_layout(vs, fmts);
            int sc = sc_on();
            float bf[4];
            s_draws_bconst += (uint64_t)blend_color(bf);
            if (!sc || s_sc.bs != bs || memcmp(s_sc.bf, bf, sizeof bf)) {
                ID3D11DeviceContext_OMSetBlendState(s_ctx, bs, bf, 0xFFFFFFFF);
                s_sc.bs = bs;
                memcpy(s_sc.bf, bf, sizeof bf);
            }
            if (!sc || memcmp(&s_sc.vp, &vp, sizeof vp)) {
                ID3D11DeviceContext_RSSetViewports(s_ctx, 1, &vp);
                s_sc.vp = vp;
            }
            if (!sc || s_sc.rs != rs) {
                ID3D11DeviceContext_RSSetState(s_ctx, rs);
                s_sc.rs = rs;
            }
            if (!sc || s_sc.il != il) {
                ID3D11DeviceContext_IASetInputLayout(s_ctx, il);
                s_sc.il = il;
            }
            ID3D11DeviceContext_IASetVertexBuffers(s_ctx, 0, NV2A_VS_MAX_INPUTS,
                                                   vbs, strides, offs);
            ID3D11DeviceContext_IASetIndexBuffer(s_ctx, s_ib, DXGI_FORMAT_R32_UINT,
                                                 s_ib_pos - nidx * 4u);
            if (!sc || s_sc.topo != topo) {
                ID3D11DeviceContext_IASetPrimitiveTopology(s_ctx, topo);
                s_sc.topo = topo;
            }
            if (!sc || s_sc.vs != vs->vs) {
                ID3D11DeviceContext_VSSetShader(s_ctx, vs->vs, NULL, 0);
                s_sc.vs = vs->vs;
            }
            if (!sc || memcmp(s_sc.vs_cb, cbs, sizeof cbs)) {
                ID3D11DeviceContext_VSSetConstantBuffers(s_ctx, 0, 3, cbs);
                memcpy(s_sc.vs_cb, cbs, sizeof cbs);
            }
        }
        s_cur = rt;
        {
            static int skip_post = -1;
            ID3D11PixelShader *ps;
            if (skip_post < 0)
                skip_post = recomp_env(RENV_D3D11_DEBUG_PS_SKIP_POST) != NULL;
            s_dbg_post = skip_post && !v->depth_enable && d->idx_count <= 8;
    GP_DRAW_DIAG_MARK(4);
            ps = setup_pixel();
            /* setup_pixel may have compiled (and evicted) a shader */
            if (!sc_on() || s_sc.ps != ps) {
                ID3D11DeviceContext_PSSetShader(s_ctx, ps, NULL, 0);
                s_sc.ps = ps;
            }
        }
    GP_DRAW_DIAG_MARK(5);
        if (px_on()) {
            uint32_t before[4];
            int n;
            for (n = 0; n < s_px_n; n++)
                before[n] = read_pixel(rt, s_px_x[n], s_px_y[n]);
            ID3D11DeviceContext_DrawIndexed(s_ctx, nidx, 0, 0);
            px_probe(d, vs, rt, fmts, strides, offs, topo, nidx, dsv, before);
        } else {
            ID3D11DeviceContext_DrawIndexed(s_ctx, nidx, 0, 0);
        }
    }
    rt->last_draw = ++s_seq;
    s_cur = NULL;
    s_draws++;
    if (d->program)
        s_draws_prog++;
    GP_DRAW_DIAG_FINISH();
}

/* ---- flip ------------------------------------------------------------------ */

/* RECOMP_D3D11_DUMP=<prefix>: every 60th presented frame as <prefix>NNNN.bmp,
 * read back from the swap chain's buffer. Proof of what reached the screen.
 * The prefix is prepended to the file name as is; one ending in / or \ is a
 * directory, and the files in it are named frame_NNNN.bmp. The flips listed
 * in RECOMP_FB_DUMP_AT are written too, as <prefix>flip_NNNNN.bmp named by
 * present number (= flip number), so a golden run can pick its frames by an
 * event's flip rather than every 60th present. */
/* The frame of this flip as a BMP: the back buffer after the direct copy
 * (the stock path, as always), or the present RT itself when the flip went
 * through the scaling blit, so a window size or a filter never reaches the
 * dump (under render.scale the RT is host-sized: dumps are hw x hh). */
static void dump_back_buffer(const char *path)
{
    D3D11_TEXTURE2D_DESC td;
    ID3D11Texture2D *st = NULL, *src = s_back;
    D3D11_MAPPED_SUBRESOURCE m;
    FILE *f;
    UINT y;

    if (s_dump_rt && s_drawn && s_drawn->tex)
        src = s_drawn->tex;
    if (s_dump_rt && gp_post_shown_tex) src = gp_post_shown_tex;
    if (!src) {
        /* No back buffer after a failed resize (d3d11_on_flip said so). */
        return;
    }
    ID3D11Texture2D_GetDesc(src, &td);
    td.Usage = D3D11_USAGE_STAGING;
    td.BindFlags = 0;
    td.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
    td.MiscFlags = 0;
    if (FAILED(ID3D11Device_CreateTexture2D(s_dev, &td, NULL, &st)))
        return;
    ID3D11DeviceContext_CopyResource(s_ctx, (ID3D11Resource *)st,
                                     (ID3D11Resource *)src);
    if (SUCCEEDED(ID3D11DeviceContext_Map(s_ctx, (ID3D11Resource *)st, 0,
                                          D3D11_MAP_READ, 0, &m))) {
        f = fopen(path, "wb");
        if (f) {
            BITMAPFILEHEADER fh;
            BITMAPINFOHEADER ih;
            memset(&fh, 0, sizeof fh);
            memset(&ih, 0, sizeof ih);
            fh.bfType = 0x4D42;
            fh.bfOffBits = sizeof fh + sizeof ih;
            fh.bfSize = fh.bfOffBits + td.Width * td.Height * 4;
            ih.biSize = sizeof ih;
            ih.biWidth = (LONG)td.Width;
            ih.biHeight = -(LONG)td.Height;
            ih.biPlanes = 1;
            ih.biBitCount = 32;
            fwrite(&fh, sizeof fh, 1, f);
            fwrite(&ih, sizeof ih, 1, f);
            for (y = 0; y < td.Height; y++)
                fwrite((const uint8_t *)m.pData + (size_t)y * m.RowPitch,
                       td.Width * 4, 1, f);
            fclose(f);
            fprintf(stderr, LOGP "dumped %s\n", path);
        }
        ID3D11DeviceContext_Unmap(s_ctx, (ID3D11Resource *)st, 0);
    }
    ID3D11Texture2D_Release(st);
}

static void dump_frame(void)
{
    static const char *prefix;
    static int checked;
    char path[MAX_PATH];
    size_t n;
    int dir;

    if (!checked) {
        checked = 1;
        prefix = recomp_env(RENV_D3D11_DUMP);
    }
    if (!prefix) {
        /* fb_dump_at leaves the listed flips to this backend, which writes
         * them only next to its d3d11_dump frames. */
        static int warned;
        if (!warned && nv2a_pb_dump_at_listed((uint32_t)s_presents)) {
            warned = 1;
            fprintf(stderr, LOGP "WARNING: fb_dump_at is set without d3d11_dump;"
                    " no flips will be dumped\n");
        }
        return;
    }
    n = strlen(prefix);
    dir = n && (prefix[n - 1] == '/' || prefix[n - 1] == '\\');
    if ((s_presents % 60) == 1) {
        snprintf(path, sizeof path, "%s%s%04u.bmp", prefix, dir ? "frame_" : "",
                 (unsigned)(s_presents / 60));
        dump_back_buffer(path);
    }
    if (nv2a_pb_dump_at_listed((uint32_t)s_presents)) {
        snprintf(path, sizeof path, "%sflip_%05u.bmp", prefix,
                 (unsigned)s_presents);
        dump_back_buffer(path);
    }
}

static struct nv2a_occ s_occ;           /* see "Visibility tests" below */

/* The surface a flip puts on screen: the walker's pick (present_pick in
 * nv2a_pb_exec.c, the same for every backend), as the render target of that
 * offset and size. The walker hands its offset over as `surface_offset`.
 * With no render target for it here yet, the frame on screen stays. */
static RenderTarget *present_target(uint32_t surface_offset)
{
    const struct nv2a_pb_present *p = nv2a_pb_present_state();
    int i;

    for (i = 0; i < RT_MAX; i++) {
        RenderTarget *rt = &s_rt[i];
        if (rt->tex && surface_offset && rt->offset == surface_offset
                && (!p->offset || (rt->w >= p->w && rt->h >= p->h)))
            return rt;
    }
    return s_drawn;
}

/* RECOMP_FLIP_LOG: a line per flip, just before the walker's "[GPU] flip"
 * one, with what this backend did since the last. The walker's own "batches"
 * counts the CPU rasteriser's batches, so under this backend it reads 0;
 * "batches" here is the same quantity for D3D11 (3D frames have them, movie
 * frames do not); "vb" is vertex bytes copied as they are / expanded to
 * float4; the flip number is this backend's present count. */
static void flip_log(uint32_t surface_offset)
{
    static int on = -1;
    static uint64_t p_draws, p_prog, p_tex, p_rtt, p_clears, p_up, p_chg,
                    p_re, p_vn, p_ve, p_skip, p_cull, p_self, p_mip,
                    p_occ, p_occ_vis, p_occ_us, p_occ_late, p_occ_super;
    if (on < 0)
        on = recomp_env(RENV_FLIP_LOG) != NULL;
    if (!on)
        return;
    fprintf(stderr, "[D3D11] flip %llu batches %llu prog %llu tex %llu rtt %llu (self %llu)"
            " skipped %llu culled %llu clears %llu uploads %llu (mip %llu) changed %llu rehash %llu"
            " vb %llu/%llu bytes occ %llu (visible %llu, pending %d, late %llu, superseded %llu, sync %llu us)"
            " present 0x%08X walker 0x%08X\n",
            (unsigned long long)s_presents,
            (unsigned long long)(s_draws - p_draws),
            (unsigned long long)(s_draws_prog - p_prog),
            (unsigned long long)(s_draws_tex - p_tex),
            (unsigned long long)(s_rtt_binds - p_rtt),
            (unsigned long long)(s_self_copies - p_self),
            (unsigned long long)(s_draws_skipped - p_skip),
            (unsigned long long)(s_draws_culled - p_cull),
            (unsigned long long)(s_clears - p_clears),
            (unsigned long long)(s_tex_uploads - p_up),
            (unsigned long long)(s_tex_mip_uploads - p_mip),
            (unsigned long long)(s_tex_changed - p_chg),
            (unsigned long long)(s_tex_rehash - p_re),
            (unsigned long long)(s_vb_native - p_vn),
            (unsigned long long)(s_vb_expanded - p_ve),
            (unsigned long long)(s_occ.reports - p_occ),
            (unsigned long long)(s_occ.vis - p_occ_vis), s_occ.npend,
            (unsigned long long)(s_occ.late - p_occ_late),
            (unsigned long long)(s_occ.super - p_occ_super),
            (unsigned long long)(s_occ.sync_us - p_occ_us),
            s_drawn ? s_drawn->offset : 0, surface_offset);
    p_draws = s_draws; p_prog = s_draws_prog; p_tex = s_draws_tex;
    p_rtt = s_rtt_binds; p_skip = s_draws_skipped; p_cull = s_draws_culled; p_clears = s_clears;
    p_up = s_tex_uploads; p_chg = s_tex_changed; p_re = s_tex_rehash;
    p_vn = s_vb_native; p_ve = s_vb_expanded; p_self = s_self_copies;
    p_mip = s_tex_mip_uploads;
    p_occ = s_occ.reports; p_occ_vis = s_occ.vis; p_occ_us = s_occ.sync_us;
    p_occ_late = s_occ.late; p_occ_super = s_occ.super;
}

/* ---- present: the scaling blit and the resize (E1) ------------------------
 *
 * Stock (render.scale 1, nearest, the window as created) presents by copy,
 * as it always did. Anything else -- a scaled frame, a resized or
 * fullscreen window, linear or integer filtering -- draws the present RT
 * into nv2a_present_rect's rectangle of a cleared back buffer with one
 * full-screen triangle. */
static const char s_blit_vs_src[] =
    "struct VO { float4 p : SV_Position; float2 t : TEXCOORD0; };\n"
    "cbuffer C : register(b0) { float4 uv_scale; };\n"
    "VO main(uint i : SV_VertexID) {\n"
    "    VO o;\n"
    "    o.t = float2((i << 1) & 2, i & 2);\n"
    "    o.p = float4(o.t * float2(2, -2) + float2(-1, 1), 0, 1);\n"
    "    o.t *= uv_scale.xy;\n"
    "    return o;\n"
    "}\n";
static const char s_blit_ps_src[] =
    "Texture2D t0 : register(t0); SamplerState s0 : register(s0);\n"
    "float4 main(float4 p : SV_Position, float2 t : TEXCOORD0) : SV_Target {\n"
    "    return float4(t0.Sample(s0, t).rgb, 1);\n"
    "}\n";
static ID3D11VertexShader *s_blit_vs;
static ID3D11PixelShader  *s_blit_ps;
static ID3D11SamplerState *s_blit_smp[2];   /* point, linear */
static ID3D11Buffer       *s_blit_cb;       /* b0: the share of the target shown */
static int                 s_blit_failed;

static int blit_init(void)
{
    ID3DBlob *code = NULL;
    D3D11_SAMPLER_DESC sd;
    int k;

    if (s_blit_ps || s_blit_failed)
        return s_blit_ps != NULL;
    s_blit_failed = 1;
    if (!compile("nv2a_present_vs", s_blit_vs_src, "vs_5_0", &code))
        return 0;
    ID3D11Device_CreateVertexShader(s_dev, ID3D10Blob_GetBufferPointer(code),
                                    ID3D10Blob_GetBufferSize(code), NULL, &s_blit_vs);
    ID3D10Blob_Release(code);
    if (!s_blit_vs || !compile("nv2a_present_ps", s_blit_ps_src, "ps_5_0", &code))
        return 0;
    gamepkg_timed_create_pixel_shader(s_dev, ID3D10Blob_GetBufferPointer(code),
                                   ID3D10Blob_GetBufferSize(code), NULL, &s_blit_ps);
    ID3D10Blob_Release(code);
    for (k = 0; k < 2; k++) {
        memset(&sd, 0, sizeof sd);
        sd.Filter = k ? D3D11_FILTER_MIN_MAG_MIP_LINEAR : D3D11_FILTER_MIN_MAG_MIP_POINT;
        sd.AddressU = sd.AddressV = sd.AddressW = D3D11_TEXTURE_ADDRESS_CLAMP;
        sd.ComparisonFunc = D3D11_COMPARISON_NEVER;
        ID3D11Device_CreateSamplerState(s_dev, &sd, &s_blit_smp[k]);
        if (!s_blit_smp[k])
            return 0;
    }
    s_blit_cb = make_buffer(16, D3D11_BIND_CONSTANT_BUFFER);
    if (!s_blit_ps || !s_blit_cb)
        return 0;
    s_blit_failed = 0;
    return 1;
}

/* WM_SIZE recorded a client size the swap chain does not have: resize it
 * (FLIP_DISCARD keeps no contents worth keeping). */
static void present_resize(void)
{
    UINT cw = s_client_w, ch = s_client_h;
    HRESULT hr;

    if (!cw || !ch || (cw == s_back_w && ch == s_back_h))
        return;
    s_sc_init = 0;
    memset(s_sc.ps_srv, 0xFF, sizeof s_sc.ps_srv);
        ID3D11DeviceContext_OMSetRenderTargets(s_ctx, 0, NULL, NULL);
    if (s_back_rtv) {
        ID3D11RenderTargetView_Release(s_back_rtv);
        s_back_rtv = NULL;
    }
    if (s_back) {
        ID3D11Texture2D_Release(s_back);
        s_back = NULL;
    }
    hr = IDXGISwapChain_ResizeBuffers(s_swap, 0, cw, ch, DXGI_FORMAT_UNKNOWN, s_swap_flags);
    if (FAILED(hr))
        fprintf(stderr, LOGP "ResizeBuffers %ux%u hr=0x%08lX\n", cw, ch, (unsigned long)hr);
    IDXGISwapChain_GetBuffer(s_swap, 0, &IID_ID3D11Texture2D, (void **)&s_back);
    if (s_back) {
        D3D11_TEXTURE2D_DESC td;
        ID3D11Texture2D_GetDesc(s_back, &td);
        s_back_w = td.Width;
        s_back_h = td.Height;
    }
    fprintf(stderr, LOGP "window resized: swap chain %ux%u\n", s_back_w, s_back_h);
}

/* The part of rt a flip shows, in guest pixels: the walker's present extent
 * when it names this surface. A target grown for a larger clip (rt_grow) can
 * be bigger than the frame; the rest of it, and its black grow margin, is not
 * the frame. */
static void present_extent(const RenderTarget *rt, UINT *w, UINT *h)
{
    const struct nv2a_pb_present *p = nv2a_pb_present_state();
    *w = rt->w;
    *h = rt->h;
    if (p->offset && p->offset == rt->offset && p->w && p->h) {
        if (p->w < *w) *w = p->w;
        if (p->h < *h) *h = p->h;
    }
}

/* Same reference SMAA HIGH and shared CAS/FXAA source as the Vulkan presenter.
 * Runs on the native-resolution final frame, before window scaling. */
#include "video/smaa.h"
#include "video/post_shaders.h"
enum { GP_POST_EDGE, GP_POST_WEIGHT, GP_POST_BLEND, GP_POST_CAS, GP_POST_FXAA, GP_POST_CAS_BLIT, GP_POST_COUNT };
static struct {
    int tried, ok;
    ID3D11VertexShader *vs;
    ID3D11PixelShader *ps[GP_POST_COUNT];
    ID3D11Buffer *cb;
    ID3D11SamplerState *sampler[2];
    ID3D11ShaderResourceView *lut[2];
    RenderTarget coords;
    RenderTarget target[5]; /* crop, edges, weights, AA, sharpen */
    UINT w,h;
    uint64_t passes[GP_POST_COUNT];
} gp_post;
#include "private_edge_mask.h"

static const char gp_post_head[] =
    "cbuffer Post : register(b0) { float4 rtm; float4 strength; };\n"
    "struct V { float4 pos:SV_Position; float2 uv:TEXCOORD0; };\n";
static const char gp_post_vs[] =
    "struct V { float4 pos:SV_Position; float2 uv:TEXCOORD0; };\n"
    "V main(uint id:SV_VertexID){ V o; o.uv=float2((id<<1)&2,id&2);"
    "o.pos=float4(o.uv.x*2-1,1-o.uv.y*2,0,1);return o;}\n";
static const char *const gp_post_smaa_tail[3] = {
    "Texture2D colorTex:register(t0);\n"
    "float4 main(V i):SV_Target {float4 o[3];SMAAEdgeDetectionVS(i.uv,o);"
    "return float4(SMAALumaEdgeDetectionPS(i.uv,o,colorTex),0,0);}\n",
    "Texture2D edgesTex:register(t0);Texture2D areaTex:register(t1);Texture2D searchTex:register(t2);\n"
    "float4 main(V i):SV_Target {float4 o[3];float2 p;SMAABlendingWeightCalculationVS(i.uv,p,o);"
    "return SMAABlendingWeightCalculationPS(i.uv,p,o,edgesTex,areaTex,searchTex,float4(0,0,0,0));}\n",
    "Texture2D colorTex:register(t0);Texture2D blendTex:register(t1);\n"
    "float4 main(V i):SV_Target {float4 o;SMAANeighborhoodBlendingVS(i.uv,o);"
    "return float4(SMAANeighborhoodBlendingPS(i.uv,o,colorTex,blendTex).rgb,1);}\n"
};
#define GP_POST_FILTER_HEAD \
    "Texture2D tex0:register(t0);SamplerState LinearSampler:register(s0);\n" \
    "#define mix lerp\n#define FETCH(p) tex0.SampleLevel(LinearSampler,(p),0).rgb\n" \
    "#define RETURN(c) return float4((c),1)\n" \
    "float4 main(V gpInput):SV_Target {float2 uv=gpInput.uv;float2 rcp=rtm.xy;float sharp=strength.x;\n"
static const char gp_post_cas[] = GP_POST_FILTER_HEAD POST_CAS_BODY "}\n";
static const char gp_post_cas_blit[] = "Texture2D tex0:register(t0);Texture2D coordTex:register(t1);SamplerState LinearSampler:register(s0);SamplerState coordSampler:register(s1);\n#define mix lerp\n#define FETCH(p) tex0.SampleLevel(LinearSampler,(p),0).rgb\n#define RETURN(c) return float4((c),1)\nfloat4 main(V gpInput):SV_Target {float2 uv=coordTex.SampleLevel(coordSampler,gpInput.uv,0).xy;float2 rcp=rtm.xy;float sharp=strength.x;\n" POST_CAS_BODY "}\n";
static const char gp_post_fxaa[] = GP_POST_FILTER_HEAD POST_FXAA_BODY "}\n";
static int gp_post_texture(UINT w,UINT h,DXGI_FORMAT fmt,const void *bytes,UINT stride,
                           RenderTarget *rt)
{
    D3D11_TEXTURE2D_DESC d={0};D3D11_SUBRESOURCE_DATA data={0};
    d.Width=w;d.Height=h;d.MipLevels=d.ArraySize=d.SampleDesc.Count=1;
    d.Format=fmt;d.Usage=bytes?D3D11_USAGE_IMMUTABLE:D3D11_USAGE_DEFAULT;
    d.BindFlags=D3D11_BIND_SHADER_RESOURCE|(bytes?0:D3D11_BIND_RENDER_TARGET);
    data.pSysMem=bytes;data.SysMemPitch=stride;
    if(FAILED(ID3D11Device_CreateTexture2D(s_dev,&d,bytes?&data:NULL,&rt->tex)))return 0;
    if(FAILED(ID3D11Device_CreateShaderResourceView(s_dev,(ID3D11Resource*)rt->tex,NULL,&rt->srv)))return 0;
    if(!bytes && FAILED(ID3D11Device_CreateRenderTargetView(s_dev,(ID3D11Resource*)rt->tex,NULL,&rt->rtv)))return 0;
    rt->hw=rt->w=w;rt->hh=rt->h=h;return 1;
}
static int gp_post_shader(int n,const char *body)
{
    size_t len=strlen(gp_post_head)+strlen(body)+1;char *src=malloc(len);ID3DBlob *code=NULL;
    if(!src)return 0;snprintf(src,len,"%s%s",gp_post_head,body);
    char name[40];snprintf(name,sizeof name,"gamepkg_post_%d",n);
    int ok=compile(name,src,"ps_5_0",&code);free(src);
    if(!ok)return 0;
    HRESULT hr=ID3D11Device_CreatePixelShader(s_dev,ID3D10Blob_GetBufferPointer(code),ID3D10Blob_GetBufferSize(code),NULL,&gp_post.ps[n]);
    ID3D10Blob_Release(code);return SUCCEEDED(hr);
}
static int gp_post_init(void)
{
    if(gp_post.tried)return gp_post.ok;gp_post.tried=1;
    ID3DBlob *code=NULL;
    if(!compile("gamepkg_post_vs",gp_post_vs,"vs_5_0",&code))return 0;
    HRESULT hr=ID3D11Device_CreateVertexShader(s_dev,ID3D10Blob_GetBufferPointer(code),ID3D10Blob_GetBufferSize(code),NULL,&gp_post.vs);ID3D10Blob_Release(code);if(FAILED(hr))return 0;
    gp_post.cb=make_buffer(32,D3D11_BIND_CONSTANT_BUFFER);if(!gp_post.cb)return 0;
    D3D11_SAMPLER_DESC sd={0};sd.AddressU=sd.AddressV=sd.AddressW=D3D11_TEXTURE_ADDRESS_CLAMP;
    sd.ComparisonFunc=D3D11_COMPARISON_NEVER;sd.MaxLOD=0;
    for(int k=0;k<2;k++){sd.Filter=k?D3D11_FILTER_MIN_MAG_MIP_POINT:D3D11_FILTER_MIN_MAG_LINEAR_MIP_POINT;
        if(FAILED(ID3D11Device_CreateSamplerState(s_dev,&sd,&gp_post.sampler[k])))return 0;}
    const char *lib=smaa_glsl_library();
    const char *begin=strstr(lib,"SamplerState LinearSampler {");const char *end=begin?strstr(begin,"#define SMAATexture2D(tex) Texture2D tex"):NULL;
    if(!begin||!end)return 0;
    /* Explicit registers replace effect-file sampler initializers; shader-only
     * D3DCompile does not create/bind those sampler states. */
    const char *decl="SamplerState LinearSampler:register(s0);SamplerState PointSampler:register(s1);\n";
    const char *defines="#define SMAA_HLSL_4\n#define SMAA_PRESET_HIGH\n#define SMAA_RT_METRICS rtm\n";
    for(int k=0;k<3;k++){
        size_t len=strlen(defines)+strlen(lib)+(size_t)256+strlen(gp_post_smaa_tail[k]);char *src=malloc(len);if(!src)return 0;
        snprintf(src,len,"%s%.*s%s%s%s",defines,(int)(begin-lib),lib,decl,end,gp_post_smaa_tail[k]);
        int ok=gp_post_shader(k,src);free(src);if(!ok)return 0;
    }
    if(!gp_post_shader(GP_POST_CAS,gp_post_cas)||!gp_post_shader(GP_POST_FXAA,gp_post_fxaa)||!gp_post_shader(GP_POST_CAS_BLIT,gp_post_cas_blit))return 0;
    RenderTarget lut={0};
    if(!gp_post_texture(SMAA_AREA_W,SMAA_AREA_H,DXGI_FORMAT_R8G8_UNORM,smaa_area_bytes,SMAA_AREA_W*2,&lut))return 0;
    gp_post.lut[0]=lut.srv;ID3D11Texture2D_Release(lut.tex);memset(&lut,0,sizeof lut);
    if(!gp_post_texture(SMAA_SEARCH_W,SMAA_SEARCH_H,DXGI_FORMAT_R8_UNORM,smaa_search_bytes,SMAA_SEARCH_W,&lut))return 0;
    gp_post.lut[1]=lut.srv;ID3D11Texture2D_Release(lut.tex);gp_post.ok=1;
    fprintf(stderr,LOGP "post shaders ready: reference SMAA HIGH, shared FXAA and CAS\n");return 1;
}
static void gp_post_targets_free(void)
{
    ID3D11ShaderResourceView *none[4]={0};ID3D11DeviceContext_PSSetShaderResources(s_ctx,0,4,none);
    ID3D11DeviceContext_OMSetRenderTargets(s_ctx,0,NULL,NULL);
    for(int k=0;k<6;k++){RenderTarget *t=k<5?&gp_post.target[k]:&gp_post.coords;if(t->rtv)ID3D11RenderTargetView_Release(t->rtv);if(t->srv)ID3D11ShaderResourceView_Release(t->srv);if(t->tex)ID3D11Texture2D_Release(t->tex);memset(t,0,sizeof *t);}
#ifdef GAMEPKG_POST_EDGE_MASK
    gp_edge_mask_free();
#endif
    gp_post.w=gp_post.h=0;s_sc_init=0;
}
static int gp_post_size(UINT w,UINT h)
{
    if(gp_post.w==w&&gp_post.h==h)return 1;gp_post_targets_free();
    for(int k=0;k<5;k++)if(!gp_post_texture(w,h,k==1?DXGI_FORMAT_R8G8_UNORM:k==2?DXGI_FORMAT_R8G8B8A8_UNORM:DXGI_FORMAT_B8G8R8A8_UNORM,NULL,0,&gp_post.target[k])){gp_post_targets_free();return 0;}
    /* Ask the same point sampler to choose an exact native pixel center.
     * Its fixed-point addressing can differ from floor(float UV*size). */
    float *coords=malloc((size_t)w*h*2*sizeof(float));if(!coords){gp_post_targets_free();return 0;}
    for(UINT y=0;y<h;y++)for(UINT x=0;x<w;x++){size_t i=((size_t)y*w+x)*2;coords[i]=((float)x+.5f)/(float)w;coords[i+1]=((float)y+.5f)/(float)h;}
    int ok=gp_post_texture(w,h,DXGI_FORMAT_R32G32_FLOAT,coords,w*8,&gp_post.coords);free(coords);if(!ok){gp_post_targets_free();return 0;}
#ifdef GAMEPKG_POST_EDGE_MASK
    (void)gp_edge_mask_size(w,h);
#endif
    gp_post.w=w;gp_post.h=h;return 1;
}
static void gp_post_pass(int shader,RenderTarget *dst,ID3D11ShaderResourceView *a,ID3D11ShaderResourceView *b,ID3D11ShaderResourceView *c)
{
    static const float zero[4]={0};ID3D11ShaderResourceView *none[4]={0},*src[3]={a,b,c};
    ID3D11DeviceContext_PSSetShaderResources(s_ctx,0,4,none);
#ifdef GAMEPKG_POST_EDGE_MASK
    ID3D11DepthStencilView *gp_mask_view=gp_edge_mask_bind(shader,dst->hw,dst->hh);
    ID3D11DeviceContext_OMSetRenderTargets(s_ctx,1,&dst->rtv,gp_mask_view);
#else
    ID3D11DeviceContext_OMSetRenderTargets(s_ctx,1,&dst->rtv,NULL);
#endif
    if(shader==GP_POST_EDGE||shader==GP_POST_WEIGHT)ID3D11DeviceContext_ClearRenderTargetView(s_ctx,dst->rtv,zero);
    ID3D11DeviceContext_PSSetShaderResources(s_ctx,0,3,src);
    ID3D11DeviceContext_PSSetShader(s_ctx,gp_post.ps[shader],NULL,0);
    ID3D11DeviceContext_Draw(s_ctx,3,0);gp_post.passes[shader]++;
    ID3D11DeviceContext_PSSetShaderResources(s_ctx,0,4,none);
}
static RenderTarget *gp_post_run_impl(RenderTarget *rt,int aa,float sharp,UINT cw,UINT ch,int defer_cas)
{
    int sm=aa==NV2A_AA_SMAA,fx=aa==NV2A_AA_FXAA,cas=sharp>0.001f;
    if(!(sm||fx||cas))return rt;
    UINT w=(UINT)((uint64_t)cw*rt->hw/rt->w),h=(UINT)((uint64_t)ch*rt->hh/rt->h);
    if(!w||!h||!gp_post_init()||!gp_post_size(w,h))return rt;
    if(!rt->srv && FAILED(ID3D11Device_CreateShaderResourceView(s_dev,(ID3D11Resource*)rt->tex,NULL,&rt->srv)))return rt;
    ID3D11ShaderResourceView *source=rt->srv;RenderTarget *out=rt;
    s_sc_init=0; /* Also valid if a later Map fails and we use the original frame. */
    ID3D11ShaderResourceView *none[4]={0};ID3D11DeviceContext_PSSetShaderResources(s_ctx,0,4,none);ID3D11DeviceContext_OMSetRenderTargets(s_ctx,0,NULL,NULL);
    if(w!=rt->hw||h!=rt->hh){D3D11_BOX box={0,0,0,w,h,1};ID3D11DeviceContext_CopySubresourceRegion(s_ctx,(ID3D11Resource*)gp_post.target[0].tex,0,0,0,0,(ID3D11Resource*)rt->tex,0,&box);source=gp_post.target[0].srv;}
    D3D11_MAPPED_SUBRESOURCE m;
    if(FAILED(ID3D11DeviceContext_Map(s_ctx,(ID3D11Resource*)gp_post.cb,0,D3D11_MAP_WRITE_DISCARD,0,&m)))return rt;
    float constants[8]={1.0f/w,1.0f/h,(float)w,(float)h,sharp,0,0,0};memcpy(m.pData,constants,sizeof constants);ID3D11DeviceContext_Unmap(s_ctx,(ID3D11Resource*)gp_post.cb,0);
    D3D11_VIEWPORT vp={0,0,(float)w,(float)h,0,1};ID3D11DeviceContext_RSSetViewports(s_ctx,1,&vp);ID3D11DeviceContext_RSSetState(s_ctx,s_rs[0][0]);
    ID3D11DeviceContext_OMSetBlendState(s_ctx,NULL,NULL,0xffffffff);ID3D11DeviceContext_OMSetDepthStencilState(s_ctx,s_ds_off,0);
    ID3D11DeviceContext_IASetInputLayout(s_ctx,NULL);ID3D11DeviceContext_IASetPrimitiveTopology(s_ctx,D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    ID3D11DeviceContext_VSSetShader(s_ctx,gp_post.vs,NULL,0);ID3D11DeviceContext_PSSetConstantBuffers(s_ctx,0,1,&gp_post.cb);ID3D11DeviceContext_PSSetSamplers(s_ctx,0,2,gp_post.sampler);
    if(sm){gp_post_pass(GP_POST_EDGE,&gp_post.target[1],source,NULL,NULL);gp_post_pass(GP_POST_WEIGHT,&gp_post.target[2],gp_post.target[1].srv,gp_post.lut[0],gp_post.lut[1]);gp_post_pass(GP_POST_BLEND,&gp_post.target[3],source,gp_post.target[2].srv,NULL);out=&gp_post.target[3];source=out->srv;}
    else if(fx){gp_post_pass(GP_POST_FXAA,&gp_post.target[3],source,NULL,NULL);out=&gp_post.target[3];source=out->srv;}
    if(cas&&!defer_cas){gp_post_pass(GP_POST_CAS,&gp_post.target[4],source,NULL,NULL);out=&gp_post.target[4];}
    ID3D11DeviceContext_OMSetRenderTargets(s_ctx,0,NULL,NULL);s_sc_init=0;
    /* A synthetic view keeps guest extent while storage is native scale. */
    {static int last=-1;static float last_sharp=-1;
     if(last!=aa||last_sharp!=sharp){last=aa;last_sharp=sharp;
      fprintf(stderr,LOGP "post passes active: AA %s, sharpen %.2f, native %ux%u\n",sm?"SMAA HIGH":fx?"FXAA":"off",sharp,w,h);}}
    out->w=cw;out->h=ch;return out;
}

static RenderTarget *gp_post_run_config(RenderTarget *rt,int aa,float sharp,UINT cw,UINT ch)
{return gp_post_run_impl(rt,aa,sharp,cw,ch,0);}

/* rt's present extent into the back buffer at nv2a_present_rect's place,
 * filtered. */
static void present_blit(RenderTarget *rt, int filter)
{
    static const float black[4] = {0, 0, 0, 1};
    ID3D11ShaderResourceView *none = NULL;
    struct nv2a_rect r;
    D3D11_VIEWPORT vp;
    UINT cw, ch, hcw, hch;
    int whole;

    if (!s_back || !blit_init())
        return;
    if (!s_back_rtv && FAILED(ID3D11Device_CreateRenderTargetView(
                s_dev, (ID3D11Resource *)s_back, NULL, &s_back_rtv)))
        return;
    if (!rt->srv && FAILED(ID3D11Device_CreateShaderResourceView(
                s_dev, (ID3D11Resource *)rt->tex, NULL, &rt->srv)))
        return;
    present_extent(rt, &cw, &ch);
    hcw = (UINT)((uint64_t)cw * rt->hw / rt->w);
    hch = (UINT)((uint64_t)ch * rt->hh / rt->h);
    whole = nv2a_present_rect(hcw, hch, s_back_w, s_back_h, filter, &r);
    {
        D3D11_MAPPED_SUBRESOURCE m;
        float uv[4] = {(float)cw / (float)rt->w, (float)ch / (float)rt->h, 0, 0};
        if (FAILED(ID3D11DeviceContext_Map(s_ctx, (ID3D11Resource *)s_blit_cb, 0,
                                           D3D11_MAP_WRITE_DISCARD, 0, &m)))
            return;
        memcpy(m.pData, uv, sizeof uv);
        ID3D11DeviceContext_Unmap(s_ctx, (ID3D11Resource *)s_blit_cb, 0);
    }
    memset(s_sc.ps_srv, 0xFF, sizeof s_sc.ps_srv);
        ID3D11DeviceContext_OMSetRenderTargets(s_ctx, 1, &s_back_rtv, NULL);
    ID3D11DeviceContext_ClearRenderTargetView(s_ctx, s_back_rtv, black);
    vp.TopLeftX = (float)r.x;
    vp.TopLeftY = (float)r.y;
    vp.Width = (float)r.w;
    vp.Height = (float)r.h;
    vp.MinDepth = 0.0f;
    vp.MaxDepth = 1.0f;
    ID3D11DeviceContext_RSSetViewports(s_ctx, 1, &vp);
    ID3D11DeviceContext_RSSetState(s_ctx, s_rs[0][0]);
    ID3D11DeviceContext_OMSetBlendState(s_ctx, NULL, NULL, 0xFFFFFFFF);
    ID3D11DeviceContext_OMSetDepthStencilState(s_ctx, s_ds_off, 0);
    ID3D11DeviceContext_IASetInputLayout(s_ctx, NULL);
    ID3D11DeviceContext_IASetPrimitiveTopology(s_ctx, D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    ID3D11DeviceContext_VSSetShader(s_ctx, s_blit_vs, NULL, 0);
    ID3D11DeviceContext_VSSetConstantBuffers(s_ctx, 0, 1, &s_blit_cb);
    ID3D11DeviceContext_PSSetShader(s_ctx, gp_post_fused?gp_post.ps[GP_POST_CAS_BLIT]:s_blit_ps, NULL, 0);
    if(gp_post_fused)ID3D11DeviceContext_PSSetConstantBuffers(s_ctx,0,1,&gp_post.cb);
    ID3D11ShaderResourceView *blit_src[2]={rt->srv,gp_post_fused?gp_post.coords.srv:NULL};
    ID3D11DeviceContext_PSSetShaderResources(s_ctx,0,2,blit_src);
    /* Point for nearest and whole multiples; linear for linear and for an
     * integer frame that had to shrink to fit. */
    ID3D11DeviceContext_PSSetSamplers(s_ctx, 0, gp_post_fused?2:1,
        gp_post_fused?&gp_post.sampler[0]:&s_blit_smp[filter == NV2A_PRESENT_LINEAR
                    || (filter == NV2A_PRESENT_INTEGER && !whole)]);
    ID3D11DeviceContext_Draw(s_ctx, 3, 0);
    ID3D11ShaderResourceView *blit_none[2]={none,none};
    ID3D11DeviceContext_PSSetShaderResources(s_ctx,0,2,blit_none);
    /* Every binding above went behind the draw path's state shadow. */
    s_sc_init = 0;
}

static UINT s_shown_w, s_shown_h;   /* the last flip's present extent */
/* nv2a_pb_d3d11_back_pixel: a back-buffer pixel read at each flip before
 * Present, since FLIP_DISCARD leaves nothing to read after it. */
static int s_probe_x = -1, s_probe_y;
static uint32_t s_probe_px = 0xDEADBEEFu;

/* FLIP_STALL reached: hand the frame's work to the GPU now, so it is drawn
 * during the vblank hold, not after it (BO-74). */
static void d3d11_on_flip_ready(void)
{
    GP_GPU_MAIN_END();
    if (s_ctx)
        ID3D11DeviceContext_Flush(s_ctx);
}

static int gp_post_can_fuse(const RenderTarget *rt,UINT cw,UINT ch,int aa,float sharp)
{
 if(aa!=NV2A_AA_SMAA||!(sharp>0.001f)||nv2a_host_opts()->present_filter!=NV2A_PRESENT_NEAREST||recomp_env(RENV_D3D11_DUMP)||!rt->w||!rt->h)return 0;
 UINT w=(UINT)((uint64_t)cw*rt->hw/rt->w),h=(UINT)((uint64_t)ch*rt->hh/rt->h);struct nv2a_rect r;
 if(!w||!h||!s_back_w||!s_back_h)return 0;
 nv2a_present_rect(w,h,s_back_w,s_back_h,NV2A_PRESENT_NEAREST,&r);
 /* Measured wins for <=75% native area. Larger output retains the full
  * original path, avoiding extra CAS work while upscaling. */
 return r.w>0&&r.h>0&&(uint64_t)r.w*r.h*4u<=(uint64_t)w*h*3u;
}

static void d3d11_on_flip(uint32_t surface_offset, uint32_t pitch)
{
    HRESULT hr;

    if (!init())
        return;
    pump();
    if(gamepkg_native_quit_ms && GetTickCount64()-gamepkg_native_started_ms>=gamepkg_native_quit_ms) {
        gamepkg_native_quit_ms=0;
        PostMessageW(s_hwnd,WM_CLOSE,0,0);
        pump();
        return;
    }
    {
        wchar_t tb[192];
        if (xbox_FramebufferWindowTitleText(tb, 192, &s_title_clock))
            SetWindowTextW(s_hwnd, tb);
    }
    GP_GPU_POST_BEGIN();
    if (s_occ.ops)
        nv2a_occ_poll(&s_occ, 0);
    present_resize();
    s_drawn = present_target(surface_offset);
    s_dump_rt = 0;
    gp_post_shown_tex = NULL;
    RenderTarget *shown = s_drawn;
    gp_post_fused=0;
    if(shown){UINT cw,ch;present_extent(shown,&cw,&ch);int aa=nv2a_host_post_aa();float sharp=nv2a_host_sharpen();
        int defer=gp_post_can_fuse(shown,cw,ch,aa,sharp);
        shown=gp_post_run_impl(shown,aa,sharp,cw,ch,defer);
        gp_post_fused=defer&&shown!=s_drawn&&gp_post.ok;
        if(shown!=s_drawn)gp_post_shown_tex=shown->tex;
        if(gp_post_fused){static int said;if(!said++){fprintf(stderr,LOGP "CAS fused with nearest presentation: native-center samples, SMAA unchanged\n");}}
    }
    if (shown) {
        static int said;
        const struct nv2a_host_opts *eo = nv2a_host_opts();
        /* Stock: the frame as the guest drew it, a copy into the unresized
         * windowed back buffer (whole or, for another size, its overlap). */
        int stock = nv2a_host_render_scale() == 1
                 && eo->present_filter == NV2A_PRESENT_NEAREST
                 && !eo->fullscreen
                 && s_back_w == s_made_w && s_back_h == s_made_h;
        UINT cw, ch;
        int same;
        present_extent(shown, &cw, &ch);
        s_shown_w = cw;
        s_shown_h = ch;
        same = cw == shown->w && ch == shown->h
            && shown->hw == s_back_w && shown->hh == s_back_h;
        int blit = gp_post_fused || (!stock && !(same && eo->present_filter != NV2A_PRESENT_LINEAR));
        if (!s_back) {
            /* A failed ResizeBuffers/GetBuffer left no back buffer. */
            static int said_nb;
            if (!said_nb++)
                fprintf(stderr, LOGP "no back buffer (resize failed): frame not shown\n");
        } else if (blit) {
            present_blit(shown, eo->present_filter);
            s_dump_rt = 1;
        } else if (same) {
            ID3D11DeviceContext_CopyResource(s_ctx, (ID3D11Resource *)s_back,
                                             (ID3D11Resource *)shown->tex);
        } else {
            static const float black[4] = {0, 0, 0, 1};
            D3D11_BOX b;
            b.left = 0;
            b.top = 0;
            b.front = 0;
            b.right = cw < s_back_w ? cw : s_back_w;
            b.bottom = ch < s_back_h ? ch : s_back_h;
            b.back = 1;
            /* FLIP_DISCARD hands back a buffer whose contents are undefined
             * (DXVK keeps an old frame), so the band the copy leaves -- a
             * frame cropped smaller than its grown target, or than the
             * window -- is black, as present_blit leaves it. */
            if ((b.right < s_back_w || b.bottom < s_back_h)
                    && (s_back_rtv || SUCCEEDED(ID3D11Device_CreateRenderTargetView(
                            s_dev, (ID3D11Resource *)s_back, NULL, &s_back_rtv))))
                ID3D11DeviceContext_ClearRenderTargetView(s_ctx, s_back_rtv, black);
            ID3D11DeviceContext_CopySubresourceRegion(
                s_ctx, (ID3D11Resource *)s_back, 0, 0, 0, 0,
                (ID3D11Resource *)shown->tex, 0, &b);
        }
        if (s_back && !said++)
            fprintf(stderr, LOGP "first present: back buffer %ux%u, frame %ux%u"
                    " (%ux%u host), %s\n", s_back_w, s_back_h, shown->w, shown->h,
                    shown->hw, shown->hh,
                    blit ? "scaling blit" : same ? "copy" : "overlap copy");
    }
    if (s_back && s_probe_x >= 0)
        s_probe_px = (UINT)s_probe_x < s_back_w && (UINT)s_probe_y < s_back_h
                   ? read_tex_pixel(s_back, (UINT)s_probe_x, (UINT)s_probe_y)
                   : 0xDEADBEEFu;
    GP_GPU_FINISH(nv2a_pb_gpu_state()->flips);
    s_presents++;
#ifdef GAMEPKG_CUBE_TIMING
    gp_cp_frame(nv2a_pb_gpu_state()->flips);
#endif
#ifdef GAMEPKG_PIXEL_TIMING
    gp_pixel_profile_frame(nv2a_pb_gpu_state()->flips);
#endif
#ifdef GAMEPKG_DRAW_TIMING
    gp_draw_profile_frame(nv2a_pb_gpu_state()->flips);
#endif
#ifdef GAMEPKG_MAP_TIMING
    gp_map_profile_frame(nv2a_pb_gpu_state()->flips);
#endif
    dump_frame();
    if (s_latency_wait)   /* a queue slot free: the display's back-pressure */
        WaitForSingleObjectEx(s_latency_wait, 100, TRUE);
    hr = IDXGISwapChain_Present(s_swap, s_sync_interval, 0);
    xbox_PacingPresent(0);   /* RECOMP_TRACE=pacing=all: host present times */
    if (xbox_LatOn())
        xbox_LatRow('P', nv2a_pb_gpu_state()->flips, 0, NULL);
    if (FAILED(hr) && s_presents < 4)
        fprintf(stderr, LOGP "Present hr=0x%08lX\n", (unsigned long)hr);
    /* FLIP_DISCARD unbinds the back buffer; the next frame rebinds its own
     * render target anyway. */
    flip_log(surface_offset);
#if defined(GAMEPKG_VERIFY_CONSTANT_DIRTY) || defined(GAMEPKG_VERIFY_RC_KEY_CACHE) || defined(GAMEPKG_VERIFY_VS_CACHE) || defined(GAMEPKG_D3D11_VERIFY_OUTPUT_CACHE)
    if(s_presents==1||s_presents%300==0)
        fprintf(stderr,"[cache-verify] constants=%llu raw=%llu vertex=%llu output=%llu\n",
            (unsigned long long)gp_verify_counts.constants,(unsigned long long)gp_verify_counts.raw,
            (unsigned long long)gp_verify_counts.vertex,(unsigned long long)gp_verify_counts.output);
#endif
    if (s_presents == 1 || (s_presents % 300) == 0)
        fprintf(stderr, LOGP "present #%llu: surface 0x%08X (walker said 0x%08X),"
                " %llu clears, %llu draws, %llu skipped, %llu culled, %llu texture uploads,"
                " %llu rehashes, %llu combiner shaders, %llu zclears,"
                " %llu depth-tested draws, %llu render-target textures,"
                " %llu stale drops, %llu target grows, %llu partial clears\n",
                (unsigned long long)s_presents,
                s_drawn ? s_drawn->offset : 0, surface_offset,
                (unsigned long long)s_clears, (unsigned long long)s_draws,
                (unsigned long long)s_draws_skipped,
                (unsigned long long)s_draws_culled,
                (unsigned long long)s_tex_uploads,
                (unsigned long long)s_tex_rehash,
                (unsigned long long)s_rc_compiles,
                (unsigned long long)s_zclears, (unsigned long long)s_draws_z,
                (unsigned long long)s_rtt_binds, (unsigned long long)s_stale_drops,
                (unsigned long long)s_grows, (unsigned long long)s_partial_clears);
    if (s_presents == 1 || (s_presents % 300) == 0)
        fprintf(stderr,LOGP "cube resources: %llu binds, %llu GPU face copies, %llu memory face uploads\n",
                (unsigned long long)gp_cube_binds,(unsigned long long)gp_cube_copies,(unsigned long long)gp_cube_uploads);
    if (s_presents == 1 || (s_presents % 300) == 0)
        fprintf(stderr, LOGP "blend: %llu draws with a constant factor,"
                " %u SET_BLEND_COLOR writes (last 0x%08X)\n",
                (unsigned long long)s_draws_bconst,
                nv2a_pb_gpu_state()->blend_color_writes,
                nv2a_pb_gpu_state()->blend_color);
    if (s_presents == 1 || (s_presents % 300) == 0)
        fprintf(stderr, LOGP "raw combiner keys: %llu hits, %llu misses\n",
                (unsigned long long)s_rc_key_hits, (unsigned long long)s_rc_key_misses);
    if (s_presents == 1 || (s_presents % 300) == 0)
        fprintf(stderr, LOGP "texture byte cache: %llu hits, %llu captures, %llu fallbacks, %zu bytes\n",
                (unsigned long long)s_gp_shadow_hits, (unsigned long long)s_gp_shadow_captures,
                (unsigned long long)s_gp_shadow_fallbacks, s_gp_shadow_bytes);
}

/* ---- registration ---------------------------------------------------------- */

/* ---- Visibility tests: D3D11 occlusion queries ----------------------
 *
 * BeginVisibilityTest is CLEAR_REPORT_VALUE + SET_ZPASS_PIXEL_COUNT_ENABLE(1),
 * EndVisibilityTest is ENABLE(0) + GET_REPORT. Each stretch with counting on
 * is one D3D11_QUERY_OCCLUSION (begun at enable, or at a clear while
 * enabled; ended at disable or the report), and a report sums the stretches
 * since the last clear. The count is samples passing depth and stencil with
 * the pixel shader's alpha-test discard applied, the GPU's version of the
 * CPU path's put_pixel count.
 *
 * Latency: a report is not completed at GET_REPORT. Its queries go on a
 * pending list, polled (GetData without a flush) at every batch and flip
 * and, with a flush, on the ack thread's idle tick (on_poll), so a title
 * spinning on the result still gets one. Until then the status keeps D3D's
 * 0xFFFFFFFF, which GetVisibilityTestResult reads as D3DERR_TESTINCOMPLETE.
 * When done, count then status 0 are written. RECOMP_D3D11_OCC=sync waits
 * for the result at GET_REPORT instead (the stall the flip log's "occ"
 * shows), =fixed leaves reports to the walker (RECOMP_ZPASS_FIXED, every
 * test visible). A report still pending after 250 ms, or one with too many
 * stretches, completes as visible (0x10000); the flip log's "visible"
 * (s_occ_vis) counts those fallbacks too.
 *
 * Reused reports: the XDK recycles its report slots, so a GET_REPORT can name
 * a va that still has a report pending. The new one supersedes it, before
 * anything is polled: the old
 * queries go back to the pool unread and nothing is written for the old
 * test, so its late completion cannot land over the title's reset status or
 * the new count. */
#define OCC_POOL    (NV2A_OCC_PENDING * 2 + NV2A_OCC_SEG)
static ID3D11Query *s_occ_pool[OCC_POOL];
static int          s_occ_pool_n, s_occ_made;
static int          s_occ_mode = -1;         /* 0 defer, 1 sync, 2 fixed */

static int occ_mode(void)
{
    if (s_occ_mode < 0) {
        const char *e = recomp_env(RENV_D3D11_OCC);
        s_occ_mode = !e ? 0 : !_stricmp(e, "sync") ? 1 : !_stricmp(e, "fixed") ? 2 : 0;
    }
    return s_occ_mode;
}

static uint64_t occ_now_us(void *ctx)
{
    static LARGE_INTEGER f;
    LARGE_INTEGER c;
    (void)ctx;
    if (!f.QuadPart)
        QueryPerformanceFrequency(&f);
    QueryPerformanceCounter(&c);
    return (uint64_t)(c.QuadPart * 1000000.0 / (double)f.QuadPart);
}

static void *occ_get(void *ctx)
{
    ID3D11Query *q = NULL;
    D3D11_QUERY_DESC qd;
    (void)ctx;
    if (s_occ_pool_n)
        return s_occ_pool[--s_occ_pool_n];
    if (s_occ_made >= OCC_POOL)
        return NULL;
    qd.Query = D3D11_QUERY_OCCLUSION;
    qd.MiscFlags = 0;
    if (FAILED(ID3D11Device_CreateQuery(s_dev, &qd, &q)))
        return NULL;
    s_occ_made++;
    return q;
}

static void occ_put(void *ctx, void *q)
{
    (void)ctx;
    if (q && s_occ_pool_n < OCC_POOL)
        s_occ_pool[s_occ_pool_n++] = (ID3D11Query *)q;
    else if (q)
        ID3D11Query_Release((ID3D11Query *)q);
}

static void occ_begin(void *ctx, void *q)
{
    (void)ctx;
    ID3D11DeviceContext_Begin(s_ctx, (ID3D11Asynchronous *)q);
}

static void occ_end(void *ctx, void *q)
{
    (void)ctx;
    ID3D11DeviceContext_End(s_ctx, (ID3D11Asynchronous *)q);
}

/* GetData without a flush polls; with one, it pushes the GPU along. */
static int occ_result(void *ctx, void *q, int flush, uint64_t *count)
{
    UINT64 v = 0;
    (void)ctx;
    if (ID3D11DeviceContext_GetData(s_ctx, (ID3D11Asynchronous *)q, &v, sizeof v,
                                    flush ? 0 : D3D11_ASYNC_GETDATA_DONOTFLUSH) != S_OK)
        return 0;
    *count = v;
    return 1;
}

static const struct nv2a_occ_ops s_occ_ops = {
    occ_get, occ_put, occ_begin, occ_end, occ_result, occ_now_us, NULL
};

/* The tracker, set up on first use (init() has made the device). */
static int occ_ready_on(void)
{
    if (occ_mode() == 2 || !init())
        return 0;
    if (!s_occ.ops)
        nv2a_occ_init(&s_occ, &s_occ_ops, (uint8_t *)xbox_GetMemoryOffset());
    return 1;
}

static void d3d11_on_zpass(int enable)
{
    if (occ_ready_on())
        nv2a_occ_zpass(&s_occ, enable);
}

static void d3d11_on_zpass_clear(void)
{
    if (occ_ready_on())
        nv2a_occ_zpass_clear(&s_occ);
}

static int d3d11_on_report(uint32_t va)
{
    if (!occ_ready_on())
        return 0;
    nv2a_occ_report(&s_occ, va, occ_mode() == 1);
    return 1;
}

static void d3d11_on_poll(void)
{
    if (s_occ.npend && s_dev)
        nv2a_occ_poll(&s_occ, 1);
}

static const struct nv2a_pb_backend s_d3d11_backend = {
    d3d11_on_clear,
    d3d11_on_zclear,
    d3d11_on_draw,
    d3d11_on_flip,
    d3d11_on_zpass,
    d3d11_on_zpass_clear,
    d3d11_on_report,
    d3d11_on_poll,
    0,                  /* writes_back */
    NULL,               /* sync_guest */
    d3d11_on_flip_ready,
};

/* For tests/d3d11_backend_smoke: the size of the target at a colour offset
 * and the part of it the last flip showed, 0 when there is none. */
int nv2a_pb_d3d11_shown(uint32_t offset, uint32_t *tw, uint32_t *th,
                        uint32_t *sw, uint32_t *sh)
{
    int i;
    for (i = 0; i < RT_MAX; i++)
        if (s_rt[i].tex && s_rt[i].offset == offset) {
            *tw = s_rt[i].w;
            *th = s_rt[i].h;
            *sw = s_shown_w;
            *sh = s_shown_h;
            return 1;
        }
    return 0;
}

/* For tests/d3d11_backend_smoke: the back-buffer pixel the last flip read,
 * 0xAARRGGBB, at the point the previous call set (0xDEADBEEF before a flip
 * has read one there); then (x, y) is the point the next flips read, and
 * (-1, -1) stops the reads (each is a copy and a Map stall per flip). */
uint32_t nv2a_pb_d3d11_back_pixel(int x, int y)
{
    uint32_t px = s_probe_px;
    if (x != s_probe_x || y != s_probe_y)
        s_probe_px = 0xDEADBEEFu;
    s_probe_x = x;
    s_probe_y = y;
    return px;
}

/* For tests/d3d11_backend_smoke: the pixel at guest (x, y) of the target
 * at a colour offset, 0xAARRGGBB, or 0xDEADBEEF when there is none. */
uint32_t nv2a_pb_d3d11_pixel(uint32_t offset, int x, int y)
{
    int i;
    for (i = 0; i < RT_MAX; i++)
        if (s_rt[i].tex && s_rt[i].offset == offset)
            return read_pixel(&s_rt[i], x, y);
    return 0xDEADBEEFu;
}

const struct nv2a_pb_backend *nv2a_pb_d3d11_backend(void)
{
    return &s_d3d11_backend;
}

/* RECOMP_PB_BACKEND=d3d11 routes the executor here. Called once from the
 * executor's first method, on the ack thread. */
void nv2a_pb_d3d11_register_from_env(void)
{
    const char *b = recomp_env(RENV_PB_BACKEND);
    if (b && !_stricmp(b, "d3d11")) {
        nv2a_pb_set_backend(&s_d3d11_backend);
        fprintf(stderr, LOGP "RECOMP_PB_BACKEND=d3d11: executor draws through D3D11\n");
    }
}

#endif /* _WIN32 */

#ifdef GAMEPKG_DXBC_TEST
int gamepkg_test_compile(const char *source,const char *target,ID3DBlob **out) {
 if(!s_compile){HMODULE dll=LoadLibraryA("d3dcompiler_47.dll");if(dll)s_compile=(compile_fn)(void (*)(void))GetProcAddress(dll,"D3DCompile");}
 if(!s_compile)return 0;
 return compile("gamepkg_test",source,target,out);
}
uint64_t gamepkg_test_cache_hits(void){return gp_cache_hits;}
uint64_t gamepkg_test_cache_misses(void){return gp_cache_misses;}
int gamepkg_test_cache_payload(const void *p,uint32_t n,const char *s,const char *t){const void *b;uint32_t nb;return gp_payload(p,n,s,t,&b,&nb);}
#endif

#ifdef GAMEPKG_RC_KEY_TEST
int gamepkg_rc_key_unit_test(void)
{
    uint32_t key[36], changed[36], rng = 0x17983253u;
    unsigned checks = 0;
#define KC_CHECK(x) do { checks++; if (!(x)) { fprintf(stderr,"FAIL RC key line %d: %s\n",__LINE__,#x); return 1; } } while(0)
    memset(s_rc, 0, sizeof s_rc);
    memset(s_rc_key_hint, 0, sizeof s_rc_key_hint);
    s_memo = 1;
    memset(key,0,sizeof key);s_rc[0].used=1;s_rc_key_hint[gamepkg_rc_key_slot(key)]=1;
    KC_CHECK(!gamepkg_rc_key_find(key));
    memset(s_rc,0,sizeof s_rc);memset(s_rc_key_hint,0,sizeof s_rc_key_hint);
    for (unsigned iteration=0; iteration<2048; iteration++) {
        for (unsigned j=0;j<36;j++) { rng=rng*1664525u+1013904223u;key[j]=rng; }
        RcEntry *e=&s_rc[iteration%RC_MAX];
        memset(e,0,sizeof *e);e->used=1;e->bad=iteration&1;
        e->ps=(ID3D11PixelShader *)(uintptr_t)(iteration+1u);
        KC_CHECK(!gamepkg_rc_key_find(key));
        gamepkg_rc_key_remember(e,key);
        KC_CHECK(gamepkg_rc_key_find(key)==e);
        KC_CHECK((gamepkg_rc_key_find(key)->bad ? NULL : gamepkg_rc_key_find(key)->ps)==(e->bad?NULL:e->ps));
        for(unsigned byte=0;byte<sizeof key;byte++) {
            memcpy(changed,key,sizeof key);((uint8_t *)changed)[byte]^=0x80;
            KC_CHECK(!gamepkg_rc_key_find(changed));
        }
        uint32_t oldslot=gamepkg_rc_key_slot(key);
        memcpy(changed,key,sizeof key);
        do { changed[35]++; } while(gamepkg_rc_key_slot(changed)!=oldslot);
        KC_CHECK(memcmp(changed,key,sizeof key)!=0);
        KC_CHECK(!gamepkg_rc_key_find(changed));
        /* Same slot, different complete key: collision safely misses. */
        gamepkg_rc_key_remember(e,changed);
        KC_CHECK(!gamepkg_rc_key_find(key));
        KC_CHECK(gamepkg_rc_key_find(changed)==e);
        /* Reuse or eviction clears validity before a new shader can live here. */
        memset(e,0,sizeof *e);e->used=1;
        KC_CHECK(!gamepkg_rc_key_find(changed));
        gamepkg_rc_key_remember(e,key);e->used=0;
        KC_CHECK(!gamepkg_rc_key_find(key));
        e->used=1;s_memo=0;
        KC_CHECK(!gamepkg_rc_key_find(key));
        gamepkg_rc_key_remember(e,changed);
        KC_CHECK(!memcmp(e->input_key,key,sizeof key));
        s_memo=1;KC_CHECK(gamepkg_rc_key_find(key)==e);
    }
    memset(s_rc,0,sizeof s_rc);memset(s_rc_key_hint,0,sizeof s_rc_key_hint);
    printf("PASS %u actual RC-key cache checks: full keys, all byte changes, forced hash collisions, failed shaders, alias replacement, eviction, disabled memo\n",checks);
#undef KC_CHECK
    return 0;
}

#endif

#ifdef GAMEPKG_TEX_SHADOW_TEST
int gamepkg_tex_shadow_unit_test(void)
{
    unsigned checks=0;uint32_t rng=0x38475196u;
    uint8_t *mem=(uint8_t *)malloc(16384);if(!mem)return 2;
#define SH_CHECK(x) do{checks++;if(!(x)){fprintf(stderr,"FAIL shadow line %d: %s\n",__LINE__,#x);return 1;}}while(0)
    s_memo=1;
    for(unsigned iteration=0;iteration<256;iteration++){
        for(unsigned j=0;j<16384;j++){rng=rng*1664525u+1013904223u;mem[j]=(uint8_t)(rng>>24);}
        TexEntry e;Stage t;memset(&e,0,sizeof e);memset(&t,0,sizeof t);
        uint32_t bytes=1+(iteration*17u)%4096u;t.offset=128;t.color=(iteration%3)?0x0B:0x12;t.palette=8192;t.pal_len=(iteration%4==0)?0:(iteration%2?32:256);
        if(iteration%7==0)t.palette=t.offset+bytes/2;
        uint64_t hash=tex_hash_pal(mem,&t,bytes);e.hash=hash;
        SH_CHECK(!gp_tex_shadow_matches(&e,mem,&t,bytes));
        gp_tex_shadow_capture(&e,mem,&t,bytes,hash,1);
        SH_CHECK(e.shadow_valid);SH_CHECK(gp_tex_shadow_matches(&e,mem,&t,bytes));
        SH_CHECK(s_gp_shadow_bytes==e.shadow_alloc);
        for(unsigned j=0;j<bytes;j++){
            mem[t.offset+j]^=0x80;SH_CHECK(!gp_tex_shadow_matches(&e,mem,&t,bytes));mem[t.offset+j]^=0x80;
        }
        if(t.color==0x0B)for(unsigned j=0;j<t.pal_len*4u;j++){
            mem[t.palette+j]^=0x40;SH_CHECK(!gp_tex_shadow_matches(&e,mem,&t,bytes));mem[t.palette+j]^=0x40;
        }
        SH_CHECK(gp_tex_shadow_matches(&e,mem,&t,bytes));
        s_memo=0;SH_CHECK(!gp_tex_shadow_matches(&e,mem,&t,bytes));s_memo=1;
        gp_tex_shadow_free(&e);SH_CHECK(!e.shadow_valid);SH_CHECK(s_gp_shadow_bytes==0);
    }
    TexEntry e;Stage t;memset(&e,0,sizeof e);memset(&t,0,sizeof t);t.offset=128;t.color=0x12;uint32_t bytes=1024;uint64_t hash=tex_hash_pal(mem,&t,bytes);e.hash=hash;
    /* A source update between the original hash and snapshot cannot validate it. */
    mem[t.offset]^=1;gp_tex_shadow_capture(&e,mem,&t,bytes,hash,0);SH_CHECK(!e.shadow_valid);SH_CHECK(!e.source_shadow);SH_CHECK(s_gp_shadow_bytes==0);mem[t.offset]^=1;
    SH_CHECK(!gp_tex_shadow_due(&e,119));SH_CHECK(gp_tex_shadow_due(&e,120));
    gp_tex_shadow_capture(&e,mem,&t,bytes,hash,120);SH_CHECK(e.shadow_valid);
    gp_tex_shadow_backoff(&e,0xFFFFFFFFu-60u);SH_CHECK(!gp_tex_shadow_due(&e,0xFFFFFFFFu));SH_CHECK(!gp_tex_shadow_due(&e,58));SH_CHECK(gp_tex_shadow_due(&e,59));SH_CHECK(s_gp_shadow_bytes==0);
    memset(&e,0,sizeof e);s_gp_shadow_fail_alloc=1;gp_tex_shadow_capture(&e,mem,&t,bytes,hash,0);SH_CHECK(!e.shadow_valid);SH_CHECK(s_gp_shadow_bytes==0);s_gp_shadow_fail_alloc=0;
    memset(&e,0,sizeof e);s_gp_shadow_bytes=GP_TEX_SHADOW_BUDGET;gp_tex_shadow_capture(&e,mem,&t,bytes,hash,0);SH_CHECK(!e.shadow_valid);SH_CHECK(!e.source_shadow);s_gp_shadow_bytes=0;
    memset(&e,0,sizeof e);gp_tex_shadow_capture(&e,mem,&t,0xFFFFFFFFu,hash,0);SH_CHECK(!e.shadow_valid);SH_CHECK(!e.source_shadow);
    memset(&e,0,sizeof e);t.color=0x0B;t.pal_len=0xFFFFFFFFu;gp_tex_shadow_capture(&e,mem,&t,bytes,hash,0);SH_CHECK(!e.shadow_valid);SH_CHECK(!e.source_shadow);
    free(mem);printf("PASS %u texture shadow checks: all source/palette byte changes, overlap, empty palettes, source races, failed allocation, budget fallback, eviction, frame wrap, disabled memo\n",checks);
#undef SH_CHECK
    return 0;
}

#endif

#ifdef GAMEPKG_POST_TEST
static int gp_post_read(RenderTarget *rt,uint32_t *pixels)
{
 D3D11_TEXTURE2D_DESC d;D3D11_MAPPED_SUBRESOURCE m;ID3D11Texture2D *st=NULL;
 ID3D11Texture2D_GetDesc(rt->tex,&d);d.Usage=D3D11_USAGE_STAGING;d.BindFlags=0;d.CPUAccessFlags=D3D11_CPU_ACCESS_READ;d.MiscFlags=0;
 if(FAILED(ID3D11Device_CreateTexture2D(s_dev,&d,NULL,&st)))return 0;
 ID3D11DeviceContext_OMSetRenderTargets(s_ctx,0,NULL,NULL);ID3D11DeviceContext_CopyResource(s_ctx,(ID3D11Resource*)st,(ID3D11Resource*)rt->tex);
 HRESULT hr=ID3D11DeviceContext_Map(s_ctx,(ID3D11Resource*)st,0,D3D11_MAP_READ,0,&m);
 if(SUCCEEDED(hr)){for(UINT y=0;y<d.Height;y++)memcpy(pixels+y*d.Width,(char*)m.pData+y*m.RowPitch,d.Width*4);ID3D11DeviceContext_Unmap(s_ctx,(ID3D11Resource*)st,0);}
 ID3D11Texture2D_Release(st);return SUCCEEDED(hr);
}
static float gp_post_ref_channel(const uint32_t *p,int w,int h,int x,int y,int c)
{if(x<0)x=0;if(x>=w)x=w-1;if(y<0)y=0;if(y>=h)y=h-1;return ((p[y*w+x]>>(c*8))&255)/255.0f;}
static uint32_t gp_post_ref_cas(const uint32_t *p,int w,int h,int x,int y,float sharp)
{
 uint32_t pixel=0xff000000u;
 for(int ch=0;ch<3;ch++){
  float v[9];for(int yy=-1;yy<=1;yy++)for(int xx=-1;xx<=1;xx++)v[(yy+1)*3+xx+1]=gp_post_ref_channel(p,w,h,x+xx,y+yy,ch);
  float lo=v[4],hi=v[4];for(int j=1;j<9;j+=2){lo=fminf(lo,v[j]);hi=fmaxf(hi,v[j]);}
  float lo2=lo,hi2=hi;for(int j=0;j<9;j+=2){lo2=fminf(lo2,v[j]);hi2=fmaxf(hi2,v[j]);}
  lo+=lo2;hi+=hi2;float amp=fminf(lo,2-hi)/fmaxf(hi,.0001f);amp=fminf(1,fmaxf(0,amp));float weight=-sqrtf(amp)/(8-3*sharp);
  float value=(weight*(v[1]+v[3]+v[5]+v[7])+v[4])/(1+4*weight);value=fminf(1,fmaxf(0,value));pixel|=(uint32_t)floorf(value*255+.5f)<<(ch*8);
 }
 return pixel;
}
int gamepkg_post_unit_test(void)
{
 if(!init()||!gp_post_init()){fprintf(stderr,"[post-test] initialization failed\n");return 94;}
 FILE *records=fopen("post-pixels.bin","wb");if(!records)return 94;
 static const UINT sizes[][2]={{1,1},{2,2},{7,5},{31,19},{64,48}};
 uint64_t checks=0,changed=0;
 for(int size=0;size<5;size++)for(int pattern=0;pattern<4;pattern++){
  UINT w=sizes[size][0],h=sizes[size][1];size_t n=(size_t)w*h;uint32_t *input=malloc(n*4),*aa=malloc(n*4),*got=malloc(n*4);if(!input||!aa||!got)return 94;
  for(UINT y=0;y<h;y++)for(UINT x=0;x<w;x++){
   uint32_t c=pattern==0?0x5a5a5a:pattern==1?(x>y*2?0xd0d0d0:0x202020):pattern==2?(((x*37+y*13)&255)*0x010101):(((x*29+y*11)&255)|(((x*7+y*43)&255)<<8)|(((x*71+y*3)&255)<<16));input[y*w+x]=0xff000000|c;
  }
  RenderTarget rt={0};if(!gp_post_texture(w,h,DXGI_FORMAT_B8G8R8A8_UNORM,input,w*4,&rt))return 94;
  if(gp_post_run_config(&rt,0,0,w,h)!=&rt)return 94;
  for(int mode=0;mode<3;mode++){
   RenderTarget *out=gp_post_run_config(&rt,mode,0,w,h);
   if(mode && out==&rt)return 94;
   if(!gp_post_read(out,aa))return 94;
   uint32_t record[5]={w,h,(uint32_t)pattern,(uint32_t)mode,0};if(fwrite(record,sizeof record,1,records)!=1||fwrite(aa,4,n,records)!=n)return 94;
   for(size_t i=0;i<n;i++){if(mode==2&&aa[i]!=input[i])changed++;if((mode==0||pattern==0)&&aa[i]!=input[i]){fprintf(stderr,"[post-test] constant/off mismatch size%u mode%d\n",w,mode);return 94;}checks++;}
   out=gp_post_run_config(&rt,mode,.1f,w,h);if(out==&rt||!gp_post_read(out,got))return 94;
   for(UINT y=0;y<h;y++)for(UINT x=0;x<w;x++){
    uint32_t ref=gp_post_ref_cas(aa,w,h,x,y,.1f),actual=got[y*w+x];
    for(int ch=0;ch<4;ch++){int diff=(int)((actual>>(ch*8))&255)-(int)((ref>>(ch*8))&255);if(diff < -2||diff>2){fprintf(stderr,"[post-test] CAS mismatch %ux%u pattern%d mode%d pixel%u,%u ch%d got%08x ref%08x\n",w,h,pattern,mode,x,y,ch,actual,ref);return 94;}checks++;}
   }
   record[4]=1;if(fwrite(record,sizeof record,1,records)!=1||fwrite(got,4,n,records)!=n)return 94;
   /* A visible crop must not sample the padded render target's margin. */
   UINT pw=w+3,ph=h+2;uint32_t *padding=malloc((size_t)pw*ph*4);if(!padding)return 94;
   for(UINT y=0;y<ph;y++)for(UINT x=0;x<pw;x++)padding[y*pw+x]=x<w&&y<h?input[y*w+x]:0xffff00ff;
   RenderTarget padded={0};if(!gp_post_texture(pw,ph,DXGI_FORMAT_B8G8R8A8_UNORM,padding,pw*4,&padded))return 94;
   RenderTarget *cropped=gp_post_run_config(&padded,mode,.1f,w,h);if(cropped==&padded||!gp_post_read(cropped,aa)||memcmp(got,aa,n*4)){fprintf(stderr,"[post-test] padded crop mismatch\n");return 94;}
   checks+=n;ID3D11ShaderResourceView_Release(padded.srv);ID3D11Texture2D_Release(padded.tex);free(padding);
  }
  ID3D11DeviceContext_OMSetRenderTargets(s_ctx,0,NULL,NULL);ID3D11ShaderResourceView *none[4]={0};ID3D11DeviceContext_PSSetShaderResources(s_ctx,0,4,none);
  if(rt.rtv)ID3D11RenderTargetView_Release(rt.rtv);ID3D11ShaderResourceView_Release(rt.srv);ID3D11Texture2D_Release(rt.tex);free(input);free(aa);free(got);
 }
 if(fclose(records))return 94;
 gp_post_targets_free();if(!changed)return 94;
 fprintf(stderr,"[post-test] PASS %llu pixel/channel checks, %llu SMAA changed pixels; CAS CPU reference tolerance2/255, constant/off and resize/toggle preserved\n",(unsigned long long)checks,(unsigned long long)changed);
 return FAILED(ID3D11Device_GetDeviceRemovedReason(s_dev))?94:0;
}
#endif

#ifdef GAMEPKG_POST_TEST
static void gp_fused_draw(RenderTarget *input,UINT cw,UINT ch,float strength,int fused)
{
 gp_post_fused=0;RenderTarget *out=gp_post_run_impl(input,NV2A_AA_SMAA,strength,cw,ch,fused);
 if(out==input){fprintf(stderr,"[fused-test] post failed\n");ExitProcess(97);}gp_post_fused=fused;
 present_blit(out,NV2A_PRESENT_NEAREST);gp_post_fused=0;
}
static double gp_fused_time(RenderTarget *input,UINT cw,UINT ch,int fused,int frames)
{
 ID3D11Query *d=NULL,*q[2]={0};D3D11_QUERY_DESC desc={D3D11_QUERY_TIMESTAMP_DISJOINT,0};
 if(FAILED(ID3D11Device_CreateQuery(s_dev,&desc,&d)))return -1;desc.Query=D3D11_QUERY_TIMESTAMP;
 for(int i=0;i<2;i++)if(FAILED(ID3D11Device_CreateQuery(s_dev,&desc,&q[i])))return -1;
 ID3D11DeviceContext_Begin(s_ctx,(ID3D11Asynchronous*)d);ID3D11DeviceContext_End(s_ctx,(ID3D11Asynchronous*)q[0]);
 for(int i=0;i<frames;i++)gp_fused_draw(input,cw,ch,.1f,fused);
 ID3D11DeviceContext_End(s_ctx,(ID3D11Asynchronous*)q[1]);ID3D11DeviceContext_End(s_ctx,(ID3D11Asynchronous*)d);ID3D11DeviceContext_Flush(s_ctx);
 D3D11_QUERY_DATA_TIMESTAMP_DISJOINT value;uint64_t t[2];DWORD start=GetTickCount();
 for(;;){HRESULT a=ID3D11DeviceContext_GetData(s_ctx,(ID3D11Asynchronous*)d,&value,sizeof value,D3D11_ASYNC_GETDATA_DONOTFLUSH),b=ID3D11DeviceContext_GetData(s_ctx,(ID3D11Asynchronous*)q[0],&t[0],sizeof t[0],D3D11_ASYNC_GETDATA_DONOTFLUSH),c=ID3D11DeviceContext_GetData(s_ctx,(ID3D11Asynchronous*)q[1],&t[1],sizeof t[1],D3D11_ASYNC_GETDATA_DONOTFLUSH);if(a==S_OK&&b==S_OK&&c==S_OK)break;if(FAILED(a)||FAILED(b)||FAILED(c)||GetTickCount()-start>10000)return -1;Sleep(1);}
 ID3D11Query_Release(d);for(int i=0;i<2;i++)ID3D11Query_Release(q[i]);if(value.Disjoint||!value.Frequency||t[1]<t[0])return -1;return (double)(t[1]-t[0])*1000.0/value.Frequency/frames;
}
int gamepkg_fused_unit_test(void)
{
 ID3D11Texture2D *saved_back=s_back;ID3D11RenderTargetView *saved_rtv=s_back_rtv;UINT saved_w=s_back_w,saved_h=s_back_h;
 struct nv2a_host_opts saved_opts=*nv2a_host_opts(),opts=saved_opts;
 static const UINT dimensions[][8]={{7,5,7,5,320,240},{32,24,64,48,640,480},{640,480,1280,960,640,480},{640,480,1280,960,1280,800},{640,480,1280,960,1280,960},{640,480,1280,960,1920,1080},{640,480,1288,968,640,480,644,484}};
 uint64_t channels=0,changed=0;unsigned maximum=0;
 for(unsigned test=0;test<sizeof dimensions/sizeof dimensions[0];test++)for(int aspect=0;aspect<2;aspect++){
  UINT cw=dimensions[test][0],ch=dimensions[test][1],w=dimensions[test][2],h=dimensions[test][3],bw=dimensions[test][4],bh=dimensions[test][5];
  RenderTarget back={0};if(!gp_post_texture(bw,bh,DXGI_FORMAT_B8G8R8A8_UNORM,NULL,0,&back))return 97;s_back=back.tex;s_back_rtv=back.rtv;s_back_w=bw;s_back_h=bh;
  opts.present_aspect=aspect?NV2A_ASPECT_16_9:NV2A_ASPECT_FRAME;nv2a_host_opts_set(&opts);
  size_t n=(size_t)w*h,bn=(size_t)bw*bh;uint32_t *input=malloc(n*4),*a=malloc(bn*4),*b=malloc(bn*4);if(!input||!a||!b)return 97;
  for(int pattern=0;pattern<3;pattern++){
   uint32_t rng=0x512a64bf;for(UINT y=0;y<h;y++)for(UINT x=0;x<w;x++){rng^=rng<<13;rng^=rng>>17;rng^=rng<<5;input[(size_t)y*w+x]=0xff000000|(pattern==0?(x>2*y?0xd0d0d0:0x202020):pattern==1?(((x*13+y*37)&255)*0x010101):(rng&0xffffff));}
   RenderTarget rt={0};if(!gp_post_texture(w,h,DXGI_FORMAT_B8G8R8A8_UNORM,input,w*4,&rt))return 97;rt.w=dimensions[test][6]?dimensions[test][6]:cw;rt.h=dimensions[test][7]?dimensions[test][7]:ch;
   int expected=test==2||test==3||test==6||(test==4&&aspect==1);if(gp_post_can_fuse(&rt,cw,ch,NV2A_AA_SMAA,.1f)!=expected){fprintf(stderr,"[fused-test] wrong output-size eligibility test%u aspect%d\n",test,aspect);return 97;}
   gp_fused_draw(&rt,cw,ch,.1f,0);if(!gp_post_read(&back,a))return 97;gp_fused_draw(&rt,cw,ch,.1f,1);if(!gp_post_read(&back,b))return 97;
   for(size_t i=0;i<bn;i++)for(unsigned c=0;c<4;c++){int diff=(int)((a[i]>>(c*8))&255)-(int)((b[i]>>(c*8))&255);unsigned d=diff<0?-diff:diff;if(d>maximum)maximum=d;changed+=d!=0;channels++;if(d>2){fprintf(stderr,"[fused-test] FAIL source%ux%u back%ux%u aspect%d pattern%d xy%u,%u channel%u old%08x new%08x max%u\n",w,h,bw,bh,aspect,pattern,(unsigned)(i%bw),(unsigned)(i/bw),c,a[i],b[i],d);return 97;}}
   if((test==2||test==6)&&pattern==2){const float strengths[]={.01f,.5f,1.0f};for(unsigned k=0;k<3;k++){
    gp_fused_draw(&rt,cw,ch,strengths[k],0);if(!gp_post_read(&back,a))return 97;gp_fused_draw(&rt,cw,ch,strengths[k],1);if(!gp_post_read(&back,b))return 97;
    for(size_t i=0;i<bn;i++)for(unsigned c=0;c<4;c++){int diff=(int)((a[i]>>(c*8))&255)-(int)((b[i]>>(c*8))&255);unsigned d=diff<0?-diff:diff;if(d>maximum)maximum=d;changed+=d!=0;channels++;if(d>2){fprintf(stderr,"[fused-test] strength/crop mismatch test%u strength%.2f xy%u,%u old%08x new%08x\n",test,strengths[k],(unsigned)(i%bw),(unsigned)(i/bw),a[i],b[i]);return 97;}}
   }}
   if((test==2||test==3||test==4||test==5)&&aspect==1&&pattern==2){const int order[8]={0,1,1,0,1,0,0,1};for(int round=-2;round<8;round++){int kind=round<0?round+2:order[round];double ms=gp_fused_time(&rt,cw,ch,kind,60);if(ms<0)return 97;fprintf(stderr,"[fused-bench] back%ux%u round%d %s 60 full outputs %.6f ms/output\n",bw,bh,round,kind?"fused":"original",ms);}}
   ID3D11ShaderResourceView_Release(rt.srv);ID3D11Texture2D_Release(rt.tex);
  }
  free(input);free(a);free(b);ID3D11DeviceContext_OMSetRenderTargets(s_ctx,0,NULL,NULL);ID3D11RenderTargetView_Release(back.rtv);ID3D11ShaderResourceView_Release(back.srv);ID3D11Texture2D_Release(back.tex);
 }
 gp_post_targets_free();s_back=saved_back;s_back_rtv=saved_rtv;s_back_w=saved_w;s_back_h=saved_h;gp_post_fused=0;nv2a_host_opts_set(&saved_opts);s_sc_init=0;
 fprintf(stderr,"[fused-test] PASS %llu output channels, changed%llu max%u/255; full window/blackbands/aspects/downscale/upscale comparisons\n",(unsigned long long)channels,(unsigned long long)changed,maximum);return 0;
}
#endif

#ifdef GAMEPKG_POST_TEST
int gamepkg_lod_unit_test(void)
{
 if(!init())return 95;Stage saved=s_stage[0],*t=&s_stage[0];unsigned checks=0;
 const float host[]={0,-.5f,.5f,-4,4,NAN,INFINITY,-INFINITY};
 for(int mip=0;mip<3;mip++)for(int guest=-4096;guest<=4095;guest+=127)for(unsigned h=0;h<sizeof host/sizeof host[0];h++){
  memset(t,0,sizeof *t);t->addr_u=t->addr_v=3;t->levels=6;t->lod_min=1;t->lod_max=5;
  t->filter=0x01000000u|((mip==0?1u:mip==1?3u:6u)<<16)|((uint32_t)guest&0x1fffu);
  float expected=mip ? (float)guest/256.0f+(isfinite(host[h])?host[h]:0) : 0;
  if(expected>15.99f)expected=15.99f;if(expected< -16)expected= -16;
  ID3D11SamplerState *s=sampler(t,host[h],0);if(!s)return 95;D3D11_SAMPLER_DESC d;ID3D11SamplerState_GetDesc(s,&d);
  if(d.MipLODBias!=expected || d.MinLOD!=(mip?1.0f:0.0f) || d.MaxLOD!=(mip?5.0f:0.0f)){fprintf(stderr,"[lod-test] wrong state guest%d host%g mip%d got%g expected%g\n",guest,host[h],mip,d.MipLODBias,expected);return 95;}
  if(sampler(t,host[h],0)!=s)return 95;checks+=4;
 }
 s_stage[0]=saved;s_sc_init=0;fprintf(stderr,"[lod-test] PASS %u actual GPU sampler descriptor/cache checks, finite/nonfinite, guest/host bias and clamps\n",checks);return 0;
}
#endif

#ifdef GAMEPKG_POST_TEST
static int gp_ui_check_texture(ID3D11Texture2D *tex,const uint32_t *expected,unsigned w,unsigned h,unsigned levels,uint64_t *checks)
{
 D3D11_TEXTURE2D_DESC d;ID3D11Texture2D_GetDesc(tex,&d);if(d.Width!=w||d.Height!=h||d.MipLevels!=levels)return 0;
 d.Usage=D3D11_USAGE_STAGING;d.BindFlags=0;d.CPUAccessFlags=D3D11_CPU_ACCESS_READ;ID3D11Texture2D *st=NULL;
 if(FAILED(ID3D11Device_CreateTexture2D(s_dev,&d,NULL,&st)))return 0;
 ID3D11DeviceContext_CopyResource(s_ctx,(ID3D11Resource*)st,(ID3D11Resource*)tex);
 for(unsigned l=0;l<levels;l++){
  unsigned lw=nv2a_tex_level_dim(w,l),lh=nv2a_tex_level_dim(h,l);D3D11_MAPPED_SUBRESOURCE m;
  if(FAILED(ID3D11DeviceContext_Map(s_ctx,(ID3D11Resource*)st,l,D3D11_MAP_READ,0,&m))){ID3D11Texture2D_Release(st);return 0;}
  int same=1;for(unsigned y=0;y<lh;y++){if(memcmp((char*)m.pData+y*m.RowPitch,expected+y*lw,lw*4))same=0;*checks+=lw;}
  ID3D11DeviceContext_Unmap(s_ctx,(ID3D11Resource*)st,l);if(!same){ID3D11Texture2D_Release(st);return 0;}expected+=(size_t)lw*lh;
 }
 ID3D11Texture2D_Release(st);return 1;
}
int gamepkg_ui_unit_test(void)
{
 if(!init())return 99;
 unsigned saved=nv2a_texscale_factor();enum nv2a_texscale_algo oldalgo=nv2a_texscale_algo();uint64_t checks=0;
 nv2a_texscale_cache_dir(NULL);nv2a_texscale_set_sync(1);gp_ui_draw=1;
 static const unsigned dims[][2]={{1,1},{2,2},{7,5},{32,32},{64,32}};
 for(unsigned size=0;size<5;size++)for(unsigned f=2;f<=4;f+=2)for(int algo=0;algo<2;algo++)for(int pattern=0;pattern<3;pattern++){
  Stage t={0};t.width=dims[size][0];t.height=dims[size][1];t.levels=1;t.color=0x06;
  unsigned sw=t.width*f,sh=t.height*f,levels=f==4?3:2;size_t n=(size_t)t.width*t.height,ns=nv2a_tex_texels(sw,sh,levels);
  uint32_t *source=malloc(n*4),*ref=malloc(ns*4);if(!source||!ref)return 99;
  for(size_t i=0;i<n;i++)source[i]=(pattern==0?0xff000000u:pattern==1?((i%3)?0xff000000u:0):((i*17u)&255u)<<24)|((i*7717u)&0xffffffu);
  TexEntry e={0};D3D11_TEXTURE2D_DESC td={0};td.Width=t.width;td.Height=t.height;td.ArraySize=td.MipLevels=1;td.Format=DXGI_FORMAT_B8G8R8A8_UNORM;td.SampleDesc.Count=1;td.Usage=D3D11_USAGE_DEFAULT;td.BindFlags=D3D11_BIND_SHADER_RESOURCE;D3D11_SUBRESOURCE_DATA data={source,t.width*4,0};
  if(FAILED(ID3D11Device_CreateTexture2D(s_dev,&td,&data,&e.tex))||FAILED(ID3D11Device_CreateShaderResourceView(s_dev,(ID3D11Resource*)e.tex,NULL,&e.srv)))return 99;
  nv2a_texscale_set_factor(f);nv2a_texscale_set_algo((enum nv2a_texscale_algo)algo);
  /* Prime the shared store, avoiding assumptions about guest texture swizzle. */
  e.up_gen=nv2a_texscale_generation();e.up_flip=UINT32_MAX;e.up_key=nv2a_texscale_key(source,t.width,t.height);
  if(nv2a_texscale_get(e.up_key,source,t.width,t.height,f,(enum nv2a_texscale_algo)algo,0,NULL)!=NV2A_TEXSCALE_PENDING)return 99;
  if(!nv2a_texscale_flush(10000))return 99;
  ID3D11ShaderResourceView *out=gp_ui_bind(&e,&t);if(out==e.srv||!e.up_tex||gp_ui_bound!=f)return 99;
  if(!nv2a_texscale(source,t.width,t.height,f,(enum nv2a_texscale_algo)algo,ref))return 99;nv2a_texscale_mips(ref,sw,sh,levels);
  if(!gp_ui_check_texture(e.up_tex,ref,sw,sh,levels,&checks)||!gp_ui_check_texture(e.tex,source,t.width,t.height,1,&checks))return 99;
  gp_ui_draw=0;if(gp_ui_bind(&e,&t)!=e.srv||gp_ui_bound)return 99;gp_ui_draw=1;
  t.cube=1;if(gp_ui_bind(&e,&t)!=e.srv)return 99;t.cube=0;
  t.filter=0x10000000;if(gp_ui_bind(&e,&t)!=e.srv)return 99;t.filter=0;
  nv2a_texscale_set_factor(0);if(gp_ui_bind(&e,&t)!=e.srv)return 99;
  gp_ui_drop(&e);if(e.up_srv||e.up_tex||e.up_key||gp_ui_bytes)return 99;
  ID3D11ShaderResourceView_Release(e.srv);ID3D11Texture2D_Release(e.tex);free(source);free(ref);checks+=7;
 }
 /* Exercise real guest decode/upload, invalidation, generation changes and raw extent. */
 {
  uint32_t *guest=VirtualAlloc((void*)(uintptr_t)0x85000000u,65536,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);if(!guest)return 99;
  Stage t={0};t.width=t.height=8;t.levels=1;t.color=0x06;t.valid=1;t.offset=(uint32_t)((uintptr_t)guest-(uintptr_t)xbox_GetMemoryOffset());
  TexEntry e={0};gp_ui_draw=1;
  for(int version=0;version<3;version++){
   uint32_t colour=version==0?0xff3050a0u:version==1?0x80408020u:0xffffa030u;
   for(unsigned i=0;i<64;i++)guest[i]=colour;
   if(!tex_upload(&e,&t)||e.up_tex||e.up_srv||e.up_key)return 99;
   unsigned f=version==1?4:2;nv2a_texscale_set_factor(f);
   if(gp_ui_bind(&e,&t)==e.srv||gp_ui_bound!=f)return 99;
   unsigned levels=f==4?3:2;size_t count=nv2a_tex_texels(8*f,8*f,levels);uint32_t *ref=malloc(count*4);if(!ref)return 99;
   if(!nv2a_texscale(guest,8,8,f,nv2a_texscale_algo(),ref))return 99;nv2a_texscale_mips(ref,8*f,8*f,levels);
   if(!gp_ui_check_texture(e.up_tex,ref,8*f,8*f,levels,&checks)||!gp_ui_check_texture(e.tex,guest,8,8,1,&checks))return 99;free(ref);
  }
  tex_free(&e);if(gp_ui_bytes||e.up_tex||e.tex)return 99;VirtualFree(guest,0,MEM_RELEASE);
 }
 nv2a_texscale_set_sync(0);nv2a_texscale_set_factor(saved);nv2a_texscale_set_algo(oldalgo);s_sc_init=0;
 fprintf(stderr,"[ui-test] PASS %llu exact original/scaled GPU texels plus cache, raw/2D/3D/cube/signed/off resource checks\n",(unsigned long long)checks);return 0;
}
#endif


#if defined(GAMEPKG_POST_TEST) && defined(GAMEPKG_POST_EDGE_MASK)
int gamepkg_edge_mask_unit_test(void){
 if(!init()||!gp_post_init())return 96;
 static const UINT sizes[][2]={{1,1},{2,3},{17,11},{64,48},{320,240},{1280,960}};
 uint64_t checks=0;int saved=gp_edge_mask_requested;
 for(unsigned size=0;size<sizeof sizes/sizeof sizes[0];size++)for(unsigned pattern=0;pattern<6;pattern++){
  UINT w=sizes[size][0],h=sizes[size][1];size_t n=(size_t)w*h;uint32_t *input=malloc(n*4),*plain=malloc(n*4),*masked=malloc(n*4);if(!input||!plain||!masked)return 96;
  uint32_t rng=0x4d231872;for(UINT y=0;y<h;y++)for(UINT x=0;x<w;x++){rng^=rng<<13;rng^=rng>>17;rng^=rng<<5;uint32_t c=pattern==0?0x555555:pattern==1?((x>2*y)?0xd0d0d0:0x202020):pattern==2?(((x/12+y/12)&1)?0xffffff:0):pattern==3?((x==0||y==h-1)?0xffffff:0):pattern==4?(((x*13+y*37)&255)*0x010101):rng&0xffffff;input[(size_t)y*w+x]=0xff000000|c;}
  RenderTarget rt={0};if(!gp_post_texture(w,h,DXGI_FORMAT_B8G8R8A8_UNORM,input,w*4,&rt))return 96;
  for(int cas=0;cas<2;cas++){
   gp_edge_mask_requested=0;RenderTarget*out=gp_post_run_config(&rt,NV2A_AA_SMAA,cas?.1f:0,w,h);if(out==&rt||!gp_post_read(out,plain))return 96;
   gp_edge_mask_requested=1;if(getenv("GAMEPKG_TEST_BREAK_EDGE_MASK"))gp_edge_mask_test_reference=2;
   out=gp_post_run_config(&rt,NV2A_AA_SMAA,cas?.1f:0,w,h);if(!gp_edge_mask.view||gp_edge_mask.w!=w||gp_edge_mask.h!=h||out==&rt||!gp_post_read(out,masked))return 96;
   for(size_t i=0;i<n;i++){if(plain[i]!=masked[i]){fprintf(stderr,"[edge-mask-test] FAIL %ux%u pattern%u CAS%d pixel%zu plain%08x masked%08x\n",w,h,pattern,cas,i,plain[i],masked[i]);return 96;}checks+=4;}
  }
  if(size==5&&(pattern==0||pattern==1||pattern==5)){
   struct nv2a_host_opts saved_opts=*nv2a_host_opts(),opts=saved_opts;opts.present_aspect=NV2A_ASPECT_16_9;nv2a_host_opts_set(&opts);rt.w=640;rt.h=480;
   static const int order[8]={0,1,1,0,1,0,0,1};for(int round=-2;round<8;round++){int kind=round<0?round+2:order[round];gp_edge_mask_requested=kind;double ms=gp_fused_time(&rt,640,480,1,60);if(ms<0)return 96;fprintf(stderr,"[edge-mask-bench] pattern%u round%d %s 60 outputs %.6f ms/output\n",pattern,round,kind?"masked":"original",ms);}
   nv2a_host_opts_set(&saved_opts);
  }
  ID3D11DeviceContext_OMSetRenderTargets(s_ctx,0,NULL,NULL);ID3D11ShaderResourceView*none[4]={0};ID3D11DeviceContext_PSSetShaderResources(s_ctx,0,4,none);if(rt.rtv)ID3D11RenderTargetView_Release(rt.rtv);ID3D11ShaderResourceView_Release(rt.srv);ID3D11Texture2D_Release(rt.tex);free(input);free(plain);free(masked);
 }
 gp_edge_mask_requested=saved;gp_post_targets_free();s_sc_init=0;fprintf(stderr,"[edge-mask-test] PASS %llu exact RGBA channels, original versus masked SMAA HIGH, CAS off/on, blank/sparse/dense/random edges, resize\n",(unsigned long long)checks);return FAILED(ID3D11Device_GetDeviceRemovedReason(s_dev))?96:0;
}
#endif

#include "native_cubemap_test.h"
