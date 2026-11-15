// Maintainer-only reference state. Selected original bodies are injected below
// unchanged from the pinned kit. Never admitted as production game source.
type int8_t=i8; type int16_t=i16; type int32_t=i32; type int64_t=i64;
type uint8_t=u8; type uint16_t=u16; type uint32_t=u32; type uint64_t=u64;
type uintptr_t=usize; type ptrdiff_t=isize; type size_t=usize;
static mut ORIGINAL:crate::registers::Registers=crate::registers::Registers {
 eax:0,ebx:0,ecx:0,edx:0,esi:0,edi:0,ebp:0,esp:0,seh_ebp:0,df:0,fp_top:0,fp_cc:0,fp_stack:[0.0;8],
};
static mut g_xbox_mem_offset:ptrdiff_t=0;
pub unsafe fn setup(bytes:&[u8],ram:*mut u8) {
 g_xbox_mem_offset=ram as ptrdiff_t;
 (*(&raw mut ORIGINAL)).decode(bytes).unwrap();
}
pub unsafe fn capture()->Vec<u8>{(*(&raw const ORIGINAL)).encode()}
unsafe fn gamepkg_tls_g_eax()->*mut uint32_t {&raw mut ORIGINAL.eax}
unsafe fn gamepkg_tls_g_ebx()->*mut uint32_t {&raw mut ORIGINAL.ebx}
unsafe fn gamepkg_tls_g_ecx()->*mut uint32_t {&raw mut ORIGINAL.ecx}
unsafe fn gamepkg_tls_g_edx()->*mut uint32_t {&raw mut ORIGINAL.edx}
unsafe fn gamepkg_tls_g_esi()->*mut uint32_t {&raw mut ORIGINAL.esi}
unsafe fn gamepkg_tls_g_edi()->*mut uint32_t {&raw mut ORIGINAL.edi}
unsafe fn gamepkg_tls_g_ebp()->*mut uint32_t {&raw mut ORIGINAL.ebp}
unsafe fn gamepkg_tls_g_esp()->*mut uint32_t {&raw mut ORIGINAL.esp}
unsafe fn gamepkg_tls_g_seh_ebp()->*mut uint32_t {&raw mut ORIGINAL.seh_ebp}
unsafe fn gamepkg_tls_g_df()->*mut i32 {&raw mut ORIGINAL.df}
unsafe fn gamepkg_tls_g_fp_stack()->*mut [f64;8] {&raw mut ORIGINAL.fp_stack}
unsafe fn gamepkg_tls_g_fp_top()->*mut i32 {&raw mut ORIGINAL.fp_top}
unsafe fn gamepkg_tls_g_fp_cc()->*mut u16 {&raw mut ORIGINAL.fp_cc}
