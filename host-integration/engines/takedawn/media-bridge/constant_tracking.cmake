# Applied after the original source hash has been verified.
if(TRACK_CONSTANTS)
set(before [==[static struct nv2a_pb_vsh s_vsh;]==])
set(after [==[static struct nv2a_pb_vsh s_vsh;
/* port: all bank writes and snapshot restores invalidate the GPU shadow.
 * One executor/render thread owns this state. Dirty-but-identical bytes
 * still take the renderer's existing comparison, avoiding redundant maps. */
static int gamepkg_constants_dirty = 1;
int gamepkg_nv2a_constants_dirty(void) { return gamepkg_constants_dirty; }
void gamepkg_nv2a_constants_clean(void) { gamepkg_constants_dirty = 0; }
]==])
string(REPLACE "${before}" "${after}" next "${adapted}")
if(next STREQUAL adapted)
 message(FATAL_ERROR "Pinned constant-bank site missing")
endif()
set(adapted "${next}")
set(before [==[s_vsh.c[s_vsh.cload][comp] = u2f(param);]==])
set(after [==[s_vsh.c[s_vsh.cload][comp] = u2f(param);
            gamepkg_constants_dirty = 1;]==])
string(REPLACE "${before}" "${after}" next "${adapted}")
if(next STREQUAL adapted)
 message(FATAL_ERROR "Pinned constant-bank site missing")
endif()
set(adapted "${next}")
set(before [==[s_vsh.c[XF_VPOFF][k] = u2f(param);]==])
set(after [==[s_vsh.c[XF_VPOFF][k] = u2f(param);
            gamepkg_constants_dirty = 1;]==])
string(REPLACE "${before}" "${after}" next "${adapted}")
if(next STREQUAL adapted)
 message(FATAL_ERROR "Pinned constant-bank site missing")
endif()
set(adapted "${next}")
set(before [==[s_vsh.c[XF_VPSCL][k] = u2f(param);]==])
set(after [==[s_vsh.c[XF_VPSCL][k] = u2f(param);
            gamepkg_constants_dirty = 1;]==])
string(REPLACE "${before}" "${after}" next "${adapted}")
if(next STREQUAL adapted)
 message(FATAL_ERROR "Pinned constant-bank site missing")
endif()
set(adapted "${next}")
set(before [==[s_vsh.c[row][slot % 4] = u2f(param);]==])
set(after [==[s_vsh.c[row][slot % 4] = u2f(param);
            gamepkg_constants_dirty = 1;]==])
string(REPLACE "${before}" "${after}" next "${adapted}")
if(next STREQUAL adapted)
 message(FATAL_ERROR "Pinned constant-bank site missing")
endif()
set(adapted "${next}")
set(before [==[    nv2a_vsh_run((const uint32_t (*)[4])s_vsh.prog, s_vsh.start, s_vsh.c,]==])
set(after [==[    if (s_vsh.cxt_write) gamepkg_constants_dirty = 1;
    nv2a_vsh_run((const uint32_t (*)[4])s_vsh.prog, s_vsh.start, s_vsh.c,]==])
string(REPLACE "${before}" "${after}" next "${adapted}")
if(next STREQUAL adapted)
 message(FATAL_ERROR "Pinned constant-bank site missing")
endif()
set(adapted "${next}")
set(before [==[    s_gpu = gpu0; s_vsh = vsh0; s_zpass_count = zpass0;]==])
set(after [==[    s_gpu = gpu0; s_vsh = vsh0; s_zpass_count = zpass0;
    gamepkg_constants_dirty = 1;]==])
string(REPLACE "${before}" "${after}" next "${adapted}")
if(next STREQUAL adapted)
 message(FATAL_ERROR "Pinned constant-bank site missing")
endif()
set(adapted "${next}")
endif()
