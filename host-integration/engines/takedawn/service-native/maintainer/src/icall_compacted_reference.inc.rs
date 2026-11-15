// Maintainer-only controlled handlers for the graph-reconstruction comparison.
// No part of this reference environment is admitted into a game.
type uint32_t=u32;
type uint64_t=u64;
type recomp_func_t=Option<unsafe extern "C" fn()>;
const ICALL_TRACE_SIZE:i32=16;
static mut EAX:u32=0;
static mut ESP:u32=0;
static mut g_icall_trace:[u32;16]=[0;16];
static mut g_icall_trace_idx:u32=0;
static mut g_icall_count:u64=0;
static mut g_xbox_code_lo:u32=0;
static mut g_xbox_code_hi:u32=0;
static mut LOG:u64=0;
static mut SELECTED:u32=0;
unsafe fn gamepkg_tls_g_eax()->*mut u32 {&raw mut EAX}
unsafe fn gamepkg_tls_g_esp()->*mut u32 {&raw mut ESP}
unsafe extern "C" fn manual(){EAX=EAX.wrapping_mul(13);ESP=ESP.wrapping_add(4);SELECTED=1;}
unsafe extern "C" fn generated(){EAX=EAX.rotate_left(7);ESP=ESP.wrapping_sub(4);SELECTED=2;}
unsafe extern "C" fn kernel(){EAX=EAX.wrapping_sub(31);ESP=ESP.wrapping_add(8);SELECTED=3;}
unsafe fn recomp_lookup_manual(target:u32)->recomp_func_t{if target==0x155a0{Some(manual)}else{None}}
unsafe fn recomp_lookup(target:u32)->recomp_func_t{if target==0x155a0 || target==0x155a4{Some(generated)}else{None}}
unsafe fn recomp_lookup_kernel(target:u32)->recomp_func_t{if target==0xfe000001{Some(kernel)}else{None}}
unsafe fn recomp_icall_not_code_log(target:u32){LOG=0x1111111111111111 ^ u64::from(target) ^ g_icall_count.rotate_left(7);}
unsafe fn recomp_icall_fail_log(target:u32){LOG=0x2222222222222222 ^ u64::from(target) ^ g_icall_count.rotate_left(7);}
pub unsafe fn run(target:u32,stack:u32,low:u32,high:u32,seed:u32)->(u32,u32,[u32;16],u32,u64,u64,u32){
 EAX=target;ESP=stack;g_xbox_code_lo=low;g_xbox_code_hi=high;
 g_icall_trace_idx=seed;g_icall_count=if seed==u32::MAX{u64::MAX}else{u64::MAX-u64::from(seed)};
 for index in 0..16{g_icall_trace[index]=seed.wrapping_mul(index as u32+1);}
 LOG=0;SELECTED=0;sub_100();(EAX,ESP,g_icall_trace,g_icall_trace_idx,g_icall_count,LOG,SELECTED)
}
