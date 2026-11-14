# Stage only the reviewed native Windows startup/font transport connections.
# No conversion, extraction or edits to upstream occur here.
if(NOT DEFINED INPUT OR NOT DEFINED OUTPUT OR NOT DEFINED KIND)
    message(FATAL_ERROR "Set INPUT, OUTPUT and KIND (main or font)")
endif()
get_filename_component(input_real "${INPUT}" REALPATH)
get_filename_component(output_real "${OUTPUT}" REALPATH)
if(input_real STREQUAL output_real)
    message(FATAL_ERROR "Never overwrite upstream startup/font source")
endif()
file(SHA256 "${INPUT}" hash)
file(READ "${INPUT}" source)
if(KIND STREQUAL "main")
    if(NOT hash STREQUAL "f1df9ac7aa232d23e8e21c8d77608724177e647a43588ce41cf604cf19c658ae")
        message(FATAL_ERROR "Unreviewed Takedawn main.c")
    endif()
    set(load_before [==[static BOOL load_xbe(const char *path, void **out_data, size_t *out_size)
{
    FILE *f = fopen(path, "rb");
    long size;
    void *data;

    if (!f) {
        XR_ERROR( "Cannot open XBE: %s\n", path);
        return FALSE;
    }
    fseek(f, 0, SEEK_END);
    size = ftell(f);
    fseek(f, 0, SEEK_SET);
    if (size <= 0 || !(data = malloc((size_t)size))) {
        fclose(f);
        return FALSE;
    }
    if (fread(data, 1, (size_t)size, f) != (size_t)size) {
        free(data);
        fclose(f);
        return FALSE;
    }
    fclose(f);
    *out_data = data;
    *out_size = (size_t)size;
    return TRUE;
}
]==])
    set(load_after [[static BOOL load_xbe(const char *path, void **out_data, size_t *out_size)
{
    return gpw_load_xbe(path, out_data, out_size);
}
]])
    string(REPLACE "${load_before}" "${load_after}" adapted "${source}")
    if(adapted STREQUAL source)
        message(FATAL_ERROR "Pinned XBE startup loader is missing")
    endif()
    set(report_before [==[    FILE *f = fopen(s_xbe_path, "rb");
    long size = -1;

    if (f) {
        if (fseek(f, 0, SEEK_END) == 0)
            size = ftell(f);
        fclose(f);
    }
]==])
    set(report_after [[    int64_t bytes = gpw_file_size(s_xbe_path);
    long size = bytes >= 0 && bytes <= LONG_MAX ? (long)bytes : -1;

]])
    string(REPLACE "${report_before}" "${report_after}" with_report "${adapted}")
    if(with_report STREQUAL adapted)
        message(FATAL_ERROR "Pinned XBE report read is missing")
    endif()
    string(REPLACE "int main(int argc, char **argv)" "int gamepkg_original_main(int argc, char **argv)" adapted "${with_report}")
    if(adapted STREQUAL with_report)
        message(FATAL_ERROR "Pinned native entry is missing")
    endif()
elseif(KIND STREQUAL "font")
    if(NOT hash STREQUAL "1613e239cfe8a58330cf3ee42ae5e19e27fd59b84c00108247271866050e762b")
        message(FATAL_ERROR "Unreviewed xboxrecomp overlay_gamefont.cpp")
    endif()
    string(REPLACE "SDL_RWFromFile(path, \"rb\")" "gpw_SDL_RWFromFile(path, \"rb\")" adapted "${source}")
    if(adapted STREQUAL source)
        message(FATAL_ERROR "Pinned SDL font read is missing")
    endif()
else()
    message(FATAL_ERROR "Unsupported native source kind")
endif()
if(KIND STREQUAL "main")
    include("${CMAKE_CURRENT_LIST_DIR}/presentation_policy.cmake")
    gamepkg_takedawn_main_presentation(adapted adapted)
    # Installation admission is replaced by the already verified image view.
    set(data_before "rc = bo3_data_enter(data_arg, xbe_named,")
    string(REPLACE "${data_before}" "rc = gpw_game_data_enter(data_arg, xbe_named ? s_xbe_path : NULL," with_data "${adapted}")
    if(with_data STREQUAL adapted)
        message(FATAL_ERROR "Pinned native data admission site is missing")
    endif()
    set(adapted "${with_data}")

    string(PREPEND adapted "#include \"video/video_player.h\"\n")
    set(quit_before [==[#if defined(BO3_HAVE_REPORTS)
    recomp_env_set(RENV_WINDOW_QUIT_AFTER, seconds);
#elif defined(_WIN32)
    _putenv_s("RECOMP_WINDOW_QUIT_AFTER", seconds);
#else
    setenv("RECOMP_WINDOW_QUIT_AFTER", seconds, 1);
#endif]==])
    string(REPLACE "${quit_before}" "    recomp_env_set(RENV_WINDOW_QUIT_AFTER, seconds);" with_quit "${adapted}")
    if(with_quit STREQUAL adapted)
        message(FATAL_ERROR "Pinned quit-after cache update is missing")
    endif()
    set(adapted "${with_quit}")
    # Native D3D11 owns its window too; --seconds requests its normal close.
    string(REPLACE "#if !defined(_WIN32) || defined(XBOXRECOMP_WIN_VULKAN)\nstatic void set_quit_after"
        "#if 1 /* port: all native presenters honor the explicit test duration */\nstatic void set_quit_after" adapted "${adapted}")
    string(REPLACE "    if (record)\n        record_start(record, seconds);\n    return host_main();"
        "    if (record)\n        record_start(record, seconds);\n    if (seconds) set_quit_after(seconds_text);\n    xbox_HostWindowSetTitle(\"Burnout 3: Takedown (XBOX version)\");\n    return host_main();" adapted "${adapted}")

    # The settings model also serves the native Win32 renderer without ImGui.
    string(REPLACE "#ifdef XBOXRECOMP_HAVE_OVERLAY\n    xbox_SettingsGameDefault(name, value);"
        "#if defined(XBOXRECOMP_HAVE_OVERLAY) || defined(GAMEPKG_HAVE_SETTINGS)\n    xbox_SettingsGameDefault(name, value);" adapted "${adapted}")
    string(REPLACE "    /* The opt-in guest patches of config/patches.json (BO-79):"
        "    /* The opt-in guest patches of config/patches.json (BO-79):" adapted "${adapted}")
    string(REPLACE "#ifdef XBOXRECOMP_HAVE_OVERLAY\n    /* The overlay's settings.toml"
        "#if defined(GAMEPKG_HAVE_SETTINGS) && !defined(XBOXRECOMP_HAVE_OVERLAY)\n    xbox_SettingsInit(BO3_USER_DIR);\n#endif\n#ifdef XBOXRECOMP_HAVE_OVERLAY\n    /* The overlay's settings.toml" adapted "${adapted}")
    string(REPLACE "#ifdef XBOXRECOMP_HAVE_OVERLAY\n    xbox_SettingsFinish();"
        "#if defined(XBOXRECOMP_HAVE_OVERLAY) || defined(GAMEPKG_HAVE_SETTINGS)\n    xbox_SettingsFinish();" adapted "${adapted}")
    string(PREPEND adapted "#ifdef GAMEPKG_HAVE_SETTINGS\n#include \"xbox_settings.h\"\n#endif\n")

endif()
file(TO_CMAKE_PATH "${CMAKE_CURRENT_LIST_DIR}/windows_stream.h" header)
string(PREPEND adapted "#include \"${header}\"\n#include <limits.h>\n")
file(WRITE "${OUTPUT}.tmp" "${adapted}")
execute_process(COMMAND "${CMAKE_COMMAND}" -E copy_if_different "${OUTPUT}.tmp" "${OUTPUT}" COMMAND_ERROR_IS_FATAL ANY)
file(REMOVE "${OUTPUT}.tmp")
