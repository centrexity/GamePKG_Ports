# Preserve the original USB device and worker; remove only its startup cutoff.
if(NOT DEFINED INPUT OR NOT DEFINED OUTPUT)
 message(FATAL_ERROR "Pinned OHCI input/output required")
endif()
get_filename_component(input_real "${INPUT}" REALPATH)
get_filename_component(output_real "${OUTPUT}" REALPATH)
if(input_real STREQUAL output_real)
 message(FATAL_ERROR "Never overwrite the pinned USB source")
endif()
file(SHA256 "${INPUT}" hash)
if(NOT hash STREQUAL "565cf6168f67cce7502bdf492fbefed576ccc48e2ced23b5ec277561102f05a4")
 message(FATAL_ERROR "Unreviewed USB source")
endif()
file(READ "${INPUT}" source)
set(before [==[        if ((control & 0xC0u) != 0x80u) {
            if (++waited > 1500)              /* 30 s and it never started */
                break;
            continue;
        }]==])
set(after [==[        if ((control & 0xC0u) != 0x80u) {
            /* Startup can include local shader compilation. Keep waiting for
             * the actual driver rather than permanently losing every pad.
             * Saturate the diagnostic count; the process owns this worker. */
            if (waited < UINT32_MAX / 20u) waited++;
            continue;
        }]==])
string(REPLACE "${before}" "${after}" adapted "${source}")
if(adapted STREQUAL source)
 message(FATAL_ERROR "Pinned USB startup branch missing")
endif()
file(WRITE "${OUTPUT}.tmp" "${adapted}")
execute_process(COMMAND "${CMAKE_COMMAND}" -E copy_if_different "${OUTPUT}.tmp" "${OUTPUT}" COMMAND_ERROR_IS_FATAL ANY)
file(REMOVE "${OUTPUT}.tmp")
