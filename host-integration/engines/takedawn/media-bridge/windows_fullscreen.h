/* Private Win32 presentation adapter. The owner keeps its original pump. */
#ifndef GAMEPKG_WINDOWS_FULLSCREEN_H
#define GAMEPKG_WINDOWS_FULLSCREEN_H
#include <windows.h>
#include <stdint.h>
#include <string.h>
struct gamepkg_window_mode {
    HWND owner;
    int held, saved, visible;
    LONG_PTR style, exstyle;
    WINDOWPLACEMENT placement;
};
static int gamepkg_set_style(HWND window, int slot, LONG_PTR value) {
    SetLastError(ERROR_SUCCESS);
    return SetWindowLongPtrW(window, slot, value) != 0 || GetLastError() == ERROR_SUCCESS;
}
static int gamepkg_native_fullscreen(struct gamepkg_window_mode *state, HWND window, int full) {
    MONITORINFO monitor;
    WINDOWPLACEMENT current;
    LONG_PTR style = GetWindowLongPtrW(window, GWL_STYLE);
    LONG_PTR extended = GetWindowLongPtrW(window, GWL_EXSTYLE);
    if (!IsWindow(window) || window != state->owner) return 0;
    memset(&monitor, 0, sizeof monitor); monitor.cbSize = sizeof monitor;
    if (!GetMonitorInfoW(MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST), &monitor)) return 0;
    memset(&current, 0, sizeof current); current.length = sizeof current;
    if (!GetWindowPlacement(window, &current)) return 0;
    if (full) {
        state->style = style; state->exstyle = extended; state->placement = current;
        state->visible = IsWindowVisible(window);
        if (!gamepkg_set_style(window, GWL_STYLE, style & ~(LONG_PTR)WS_OVERLAPPEDWINDOW)) return 0;
        if (!SetWindowPos(window, NULL, monitor.rcMonitor.left, monitor.rcMonitor.top,
            monitor.rcMonitor.right-monitor.rcMonitor.left, monitor.rcMonitor.bottom-monitor.rcMonitor.top,
            SWP_NOACTIVATE|SWP_NOZORDER|SWP_FRAMECHANGED)) {
            gamepkg_set_style(window, GWL_STYLE, style); return 0;
        }
        state->saved = 1;
    } else {
        LONG_PTR next = state->saved ? state->style : (style & ~(LONG_PTR)WS_POPUP) | WS_OVERLAPPEDWINDOW;
        LONG_PTR next_ex = state->saved ? state->exstyle : extended;
        if (!gamepkg_set_style(window, GWL_STYLE, next) || !gamepkg_set_style(window,GWL_EXSTYLE,next_ex)) {
            gamepkg_set_style(window,GWL_STYLE,style); gamepkg_set_style(window,GWL_EXSTYLE,extended); return 0;
        }
        if (state->saved) {
            /* Do not activate, reveal or maximize a hidden/minimized test window. */
            WINDOWPLACEMENT restore = state->placement;
            if (!state->visible) restore.showCmd = SW_HIDE;
            if (!SetWindowPlacement(window,&restore) ||
                !SetWindowPos(window,NULL,0,0,0,0,
                    SWP_NOACTIVATE|SWP_NOZORDER|SWP_NOMOVE|SWP_NOSIZE|SWP_FRAMECHANGED)) {
                gamepkg_set_style(window,GWL_STYLE,style);
                gamepkg_set_style(window,GWL_EXSTYLE,extended);
                SetWindowPlacement(window,&current);
                return 0;
            }
        } else {
            RECT rect={0,0,1280,720};
            if (!AdjustWindowRectEx(&rect,(DWORD)next,FALSE,(DWORD)next_ex)) return 0;
            int width=rect.right-rect.left,height=rect.bottom-rect.top;
            int available_w=monitor.rcWork.right-monitor.rcWork.left, available_h=monitor.rcWork.bottom-monitor.rcWork.top;
            if(width>available_w)width=available_w;if(height>available_h)height=available_h;
            if (!SetWindowPos(window,NULL,monitor.rcWork.left+(available_w-width)/2,
                monitor.rcWork.top+(available_h-height)/2,width,height,
                SWP_NOACTIVATE|SWP_NOZORDER|SWP_FRAMECHANGED)) return 0;
        }
    }
    return 1;
}
/* 0 = ordinary message, 1 = consumed, 2 = first Alt+Enter transition. */
static int gamepkg_native_fullscreen_key(struct gamepkg_window_mode *state, HWND window,
                                        UINT message, WPARAM key, LPARAM bits) {
    if (state->owner != window) {
        if(state->owner && message!=WM_NCCREATE && message!=WM_CREATE)return 0;
        memset(state,0,sizeof *state);state->owner=window;
    }
    if(message==WM_KILLFOCUS){state->held=0;return 0;}
    if(message==WM_NCDESTROY){memset(state,0,sizeof *state);return 0;}
    if(key!=VK_RETURN)return 0;
    if(message==WM_SYSCHAR)return state->held||((uintptr_t)bits&((uintptr_t)1<<29))?1:0;
    if(message==WM_KEYUP||message==WM_SYSKEYUP){int held=state->held;state->held=0;return held;}
    if(message!=WM_KEYDOWN&&message!=WM_SYSKEYDOWN)return 0;
    if(state->held)return 1;
    if(!((uintptr_t)bits & ((uintptr_t)1<<29)))return 0;
    state->held=1;
    return ((uintptr_t)bits & ((uintptr_t)1<<30)) ? 1 : 2;
}
#endif
