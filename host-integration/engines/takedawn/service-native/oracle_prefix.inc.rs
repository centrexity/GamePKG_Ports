// Maintainer-only reference state. Selected original bodies are injected below
// unchanged from the pinned kit. Never admitted as production game source.
type int8_t=i8; type int16_t=i16; type int32_t=i32; type int64_t=i64;
type uint8_t=u8; type uint16_t=u16; type uint32_t=u32; type uint64_t=u64;
type uintptr_t=usize; type ptrdiff_t=isize; type size_t=usize;
static mut ORIGINAL:crate::registers::Registers=crate::registers::Registers {
 fs_base:0,eax:0,ebx:0,ecx:0,edx:0,esi:0,edi:0,ebp:0,esp:0,seh_ebp:0,df:0,fp_top:0,fp_cc:0,fp_control_word:0x037f,fp_stack:[0.0;8],xmm0:[0;4],xmm1:[0;4],xmm2:[0;4],xmm3:[0;4],xmm4:[0;4],xmm5:[0;4],xmm6:[0;4],xmm7:[0;4],
};
static mut g_xbox_mem_offset:ptrdiff_t=0;
#[derive(Copy,Clone)]
#[repr(C)]
pub union RecompXmm {pub f:[f32;4],pub d:[f64;2],pub u:[u32;4],pub i:[i32;4],pub q:[u64;2]}
static mut XMM0:RecompXmm=RecompXmm{u:[0;4]};
unsafe fn gamepkg_tls_g_xmm0()->*mut RecompXmm {&raw mut XMM0}
static mut XMM1:RecompXmm=RecompXmm{u:[0;4]};
unsafe fn gamepkg_tls_g_xmm1()->*mut RecompXmm {&raw mut XMM1}
static mut XMM2:RecompXmm=RecompXmm{u:[0;4]};
unsafe fn gamepkg_tls_g_xmm2()->*mut RecompXmm {&raw mut XMM2}
static mut XMM3:RecompXmm=RecompXmm{u:[0;4]};
unsafe fn gamepkg_tls_g_xmm3()->*mut RecompXmm {&raw mut XMM3}
static mut XMM4:RecompXmm=RecompXmm{u:[0;4]};
unsafe fn gamepkg_tls_g_xmm4()->*mut RecompXmm {&raw mut XMM4}
static mut XMM5:RecompXmm=RecompXmm{u:[0;4]};
unsafe fn gamepkg_tls_g_xmm5()->*mut RecompXmm {&raw mut XMM5}
static mut XMM6:RecompXmm=RecompXmm{u:[0;4]};
unsafe fn gamepkg_tls_g_xmm6()->*mut RecompXmm {&raw mut XMM6}
static mut XMM7:RecompXmm=RecompXmm{u:[0;4]};
unsafe fn gamepkg_tls_g_xmm7()->*mut RecompXmm {&raw mut XMM7}
pub unsafe fn setup(bytes:&[u8],ram:*mut u8) {
 g_xbox_mem_offset=ram as ptrdiff_t;
 (*(&raw mut ORIGINAL)).decode(bytes).unwrap();
 XMM0=RecompXmm{u:ORIGINAL.xmm0};
 XMM1=RecompXmm{u:ORIGINAL.xmm1};
 XMM2=RecompXmm{u:ORIGINAL.xmm2};
 XMM3=RecompXmm{u:ORIGINAL.xmm3};
 XMM4=RecompXmm{u:ORIGINAL.xmm4};
 XMM5=RecompXmm{u:ORIGINAL.xmm5};
 XMM6=RecompXmm{u:ORIGINAL.xmm6};
 XMM7=RecompXmm{u:ORIGINAL.xmm7};

}
pub unsafe fn capture()->Vec<u8>{ORIGINAL.xmm0=XMM0.u;ORIGINAL.xmm1=XMM1.u;ORIGINAL.xmm2=XMM2.u;ORIGINAL.xmm3=XMM3.u;ORIGINAL.xmm4=XMM4.u;ORIGINAL.xmm5=XMM5.u;ORIGINAL.xmm6=XMM6.u;ORIGINAL.xmm7=XMM7.u;(*(&raw const ORIGINAL)).encode()}
unsafe fn gamepkg_tls_g_eax()->*mut uint32_t {&raw mut ORIGINAL.eax}
unsafe fn gamepkg_tls_g_ebx()->*mut uint32_t {&raw mut ORIGINAL.ebx}
unsafe fn gamepkg_tls_g_ecx()->*mut uint32_t {&raw mut ORIGINAL.ecx}
unsafe fn gamepkg_tls_g_edx()->*mut uint32_t {&raw mut ORIGINAL.edx}
unsafe fn gamepkg_tls_g_esi()->*mut uint32_t {&raw mut ORIGINAL.esi}
unsafe fn gamepkg_tls_g_edi()->*mut uint32_t {&raw mut ORIGINAL.edi}
unsafe fn gamepkg_tls_g_ebp()->*mut uint32_t {&raw mut ORIGINAL.ebp}
unsafe fn gamepkg_tls_g_esp()->*mut uint32_t {&raw mut ORIGINAL.esp}
unsafe fn gamepkg_tls_g_seh_ebp()->*mut uint32_t {&raw mut ORIGINAL.seh_ebp}
unsafe fn gamepkg_tls_g_fs_base()->*mut u32 {&raw mut ORIGINAL.fs_base}
unsafe fn gamepkg_tls_g_df()->*mut i32 {&raw mut ORIGINAL.df}
unsafe fn gamepkg_tls_g_fp_stack()->*mut [f64;8] {&raw mut ORIGINAL.fp_stack}
unsafe fn gamepkg_tls_g_fp_top()->*mut i32 {&raw mut ORIGINAL.fp_top}
unsafe fn gamepkg_tls_g_fp_cc()->*mut u16 {&raw mut ORIGINAL.fp_cc}
#[cfg_attr(unix,link(name="m"))]
unsafe extern "C" {fn sqrtf(value:f32)->f32;fn fabs(value:f64)->f64;fn frexp(value:f64,exponent:*mut i32)->f64;fn ldexp(value:f64,exponent:i32)->f64;}

unsafe fn gamepkg_tls_g_fp_control_word()->*mut u16 {&raw mut ORIGINAL.fp_control_word}

// Reference-only libc copy used by unchanged pinned function bodies.
unsafe extern "C" {fn memcpy(destination:*mut ::core::ffi::c_void,source:*const ::core::ffi::c_void,bytes:size_t)->*mut ::core::ffi::c_void;}

// Exact pinned helper dependencies; maintainer reference only.
const INT64_MIN:i64=i64::MIN;
#[cfg_attr(unix,link(name="m"))]
unsafe extern "C" {fn floor(value:f64)->f64;fn ceil(value:f64)->f64;fn trunc(value:f64)->f64;fn fmod(first:f64,second:f64)->f64;fn copysign(first:f64,second:f64)->f64;}

// Original ICALL global values. The original native function and logger bodies
// remain unchanged; the maintainer console adapter collects their numeric data.
static mut g_icall_trace:[u32;16]=[0;16];
static mut g_icall_trace_idx:u32=0;
static mut g_icall_count:u64=0;
static mut g_xbox_code_lo:u32=0;
static mut g_xbox_code_hi:u32=0;
const ICALL_TRACE_SIZE:i32=16;
struct C2Rust_Unnamed(u32);
impl C2Rust_Unnamed {const SLOTS:Self=Self(16);}
type FILE=::core::ffi::c_void;
type recomp_func_t=Option<unsafe extern "C" fn()>;
unsafe extern "C" {
    fn gamepkg_native_stderr()->*mut FILE;
    fn gamepkg_native_fflush(stream:*mut FILE)->i32;
    fn gamepkg_native_fprintf(stream:*mut FILE,format:*const ::core::ffi::c_char,...)->i32;
}
unsafe fn recomp_lookup_manual(_:u32)->recomp_func_t {None}
unsafe fn recomp_lookup_kernel(_:u32)->recomp_func_t {None}
unsafe fn recomp_lookup(address:u32)->recomp_func_t {
    match address {0x15590=>Some(sub_00015590),0x155a0=>Some(sub_000155A0),0x155e0=>Some(sub_000155E0),0x15600=>Some(sub_00015600),_=>None}
}
pub unsafe fn call_bounds(low:u32,high:u32) {g_xbox_code_lo=low;g_xbox_code_hi=high;}
pub unsafe fn call_snapshot()->([u32;16],u32,u64) {(g_icall_trace,g_icall_trace_idx,g_icall_count)}
