# Read the pinned shader manifest through the admitted read-only image.
if(NOT DEFINED INPUT OR NOT DEFINED OUTPUT)
 message(FATAL_ERROR "Pinned shader cache input/output required")
endif()
get_filename_component(input_real "${INPUT}" REALPATH)
get_filename_component(output_real "${OUTPUT}" REALPATH)
if(input_real STREQUAL output_real)
 message(FATAL_ERROR "Never overwrite the pinned shader cache source")
endif()
file(SHA256 "${INPUT}" actual)
if(NOT actual STREQUAL "7af5171c93d1139fd3146b5902fc5158414282c574e4b6d6e8d48ce21793ce67")
 message(FATAL_ERROR "Pinned shader cache source hash mismatch")
endif()
file(READ "${INPUT}" source)
set(before [==[(xbe = slurp(s_man_xbe, &xsize))]==])
set(after [==[(xbe = gpw_pcache_xbe(s_man_xbe, &xsize))]==])
string(REPLACE "${before}" "${after}" adapted "${source}")
if(adapted STREQUAL source)
 message(FATAL_ERROR "Pinned manifest image read missing")
endif()
set(adapted "#include \"windows_stream.h\"\n${adapted}")
set(before [==[struct man_vs { uint64_t hash; uint32_t n; uint32_t *slots; };]==])
set(after [==[/* The same reviewed XBE pixel-definition scan as the shader-build scenes.
 * Keep only functional combiner states in this player's local coverage keys;
 * no program bytes are bundled, and live shader state is never changed. */
static void gamepkg_manifest_pixel(void *context, const void *state, uint32_t length)
{
    if (length) nv2a_pcache_put((nv2a_pcache *)context, NV2A_PC_FS, state, length);
}
struct man_vs { uint64_t hash; uint32_t n; uint32_t *slots; };]==])
string(REPLACE "${before}" "${after}" next "${adapted}")
if(next STREQUAL adapted)
 message(FATAL_ERROR "Pinned local pixel shader scan site missing")
endif()
set(adapted "${next}")
set(before [==[        nv2a_pcache_scan_vsh(xbe, xsize, man_vs_add, &scan);
        man_scan_raw(b, size, 16 + n, xbe, xsize, &scan);]==])
set(after [==[        nv2a_pcache_scan_vsh(xbe, xsize, man_vs_add, &scan);
        man_scan_raw(b, size, 16 + n, xbe, xsize, &scan);
        size_t definitions = nv2a_pcache_scan_psdef(xbe, xsize, gamepkg_manifest_pixel, dst);
        fprintf(stderr, "[pcache] local ISO pixel definition scan: %zu definitions\n", definitions);]==])
string(REPLACE "${before}" "${after}" next "${adapted}")
if(next STREQUAL adapted)
 message(FATAL_ERROR "Pinned local pixel shader scan site missing")
endif()
set(adapted "${next}")
file(WRITE "${OUTPUT}.tmp" "${adapted}")
execute_process(COMMAND "${CMAKE_COMMAND}" -E copy_if_different "${OUTPUT}.tmp" "${OUTPUT}" COMMAND_ERROR_IS_FATAL ANY)
file(REMOVE "${OUTPUT}.tmp")
