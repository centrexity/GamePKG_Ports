/* SPDX-License-Identifier: Apache-2.0
 * Window-message bridge into the existing SDK's unchanged action mapping. */
#ifndef GAMEPKG_NATIVE_WIN32_KEYBOARD_H
#define GAMEPKG_NATIVE_WIN32_KEYBOARD_H
#include "gamepkg_win32_keyboard.h"
static inline void gamepkg_native_win32_key_down(WPARAM key, LPARAM flags) {
    int accepted = gamepkg_win32_keyboard_message(WM_KEYDOWN, key, flags);
    if (accepted == GAMEPKG_WIN32_KEY_RESET) xbox_InputKeysClear();
    else if (accepted == GAMEPKG_WIN32_KEY_ACCEPT) {
        uint32_t label = gamepkg_win32_keyboard_label((uint32_t)key, (uint32_t)flags);
        xbox_InputKeySet((int)label, gamepkg_win32_keyboard_held(label));
    }
}
static inline void gamepkg_native_win32_key_up(WPARAM key, LPARAM flags) {
    uint32_t label = gamepkg_win32_keyboard_label((uint32_t)key, (uint32_t)flags);
    if (gamepkg_win32_keyboard_message(WM_KEYUP, key, flags) == GAMEPKG_WIN32_KEY_ACCEPT)
        xbox_InputKeySet((int)label, gamepkg_win32_keyboard_held(label));
}
static inline void gamepkg_native_win32_focus(int focused) {
    gamepkg_win32_keyboard_focus(focused);
    if (!focused) xbox_InputKeysClear();
    xbox_InputSetBackground(!focused);
}
/* Call before returning from a consumed fullscreen key message. */
static inline void gamepkg_native_win32_shortcut_release(WPARAM key, LPARAM flags) {
    uint32_t label = gamepkg_win32_keyboard_label((uint32_t)key, (uint32_t)flags);
    gamepkg_win32_keyboard_release((uint32_t)key, (uint32_t)flags);
    xbox_InputKeySet((int)label, gamepkg_win32_keyboard_held(label));
}
#endif
