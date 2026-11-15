// Reviewed reconstruction of recomp_0007_002.rs/sub_00246622's untraced lookup.
// Attribution: service-native/UPSTREAM_LICENSES.txt.
's_jump_0: {
    let mut _fn: recomp_func_t = recomp_lookup_manual(_va);
    's_jump_1: {
        if !_fn.is_none() { break 's_jump_1; }
        _fn = recomp_lookup(_va);
    }
    's_jump_2: {
        if !_fn.is_none() { break 's_jump_2; }
        _fn = recomp_lookup_kernel(_va);
    }
    's_jump_3: {
        if _fn.is_some() { break 's_jump_3; }
        recomp_icall_fail_log(_va);
        (*__gamepkg_tls_g_esp) = (*__gamepkg_tls_g_esp).wrapping_add(4 as uint32_t);
        (*__gamepkg_tls_g_eax) = 0 as uint32_t;
        break 's_jump_0;
    }
    _fn.expect("non-null function pointer")();
}
