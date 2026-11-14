# Stage only the reviewed original; never mutate the pinned source tree.
if(NOT DEFINED INPUT OR NOT DEFINED OUTPUT)
 message(FATAL_ERROR "Shader builder input/output required")
endif()
get_filename_component(input_real "${INPUT}" REALPATH)
get_filename_component(output_real "${OUTPUT}" REALPATH)
if(input_real STREQUAL output_real)
 message(FATAL_ERROR "Never overwrite the pinned shader builder source")
endif()
file(SHA256 "${INPUT}" actual)
if(NOT actual STREQUAL "43ec5a7244ad0a227304bd1df2090dc3e4894d656cf20539c2a01e74ea54ba5c")
 message(FATAL_ERROR "Pinned shader builder source hash mismatch")
endif()
file(READ "${INPUT}" source)
set(before [==[#ifdef XBOXRECOMP_HAVE_OVERLAY
#include "xbox_shader_build.h"
#include "nv2a_texscale.h"               /* the scene fills graphics.ui_scale's cache too */
#include "platform/recomp_save_dir.h"
#include "enhance/recomp_cfg.h"
#endif]==])
set(after [==[/* port: the shader builder uses these models even without the overlay UI. */
#include "nv2a_texscale.h"
#include "platform/recomp_save_dir.h"
#include "enhance/recomp_cfg.h"
#ifdef XBOXRECOMP_HAVE_OVERLAY
#include "xbox_shader_build.h"
#endif]==])
string(REPLACE "${before}" "${after}" adapted "${source}")
if(adapted STREQUAL source)
 message(FATAL_ERROR "Pinned shader builder model includes missing")
endif()
file(WRITE "${OUTPUT}.tmp" "${adapted}")
file(RENAME "${OUTPUT}.tmp" "${OUTPUT}")
