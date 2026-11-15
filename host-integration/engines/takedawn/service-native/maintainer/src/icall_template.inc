// Maintainer AST pattern from pinned recomp_0005_013.rs/sub_001FBAE0.
// Upstream attribution: ../UPSTREAM_LICENSES.txt in service-native.
    ::core::ptr::write_volatile(
        &raw mut g_icall_trace[(::core::ptr::read_volatile::<
            uint32_t,
        >(&raw const g_icall_trace_idx)
            & (ICALL_TRACE_SIZE - 1 as ::core::ffi::c_int) as uint32_t) as usize],
        _va,
    );
    ::core::ptr::write_volatile(
        &raw mut g_icall_trace_idx,
        ::core::ptr::read_volatile::<uint32_t>(&raw const g_icall_trace_idx)
            .wrapping_add(1),
    );
    ::core::ptr::write_volatile(
        &raw mut g_icall_count,
        ::core::ptr::read_volatile::<uint64_t>(&raw const g_icall_count).wrapping_add(1),
    );
    's_200: {
        's_151: {
            if _va >= 0xfe000000 as uint32_t || g_xbox_code_hi == 0 as uint32_t
                || _va >= g_xbox_code_lo && _va < g_xbox_code_hi
            {
                break 's_151;
            }
            recomp_icall_not_code_log(_va);
            (*__gamepkg_tls_g_esp) = _icall_esp;
            (*__gamepkg_tls_g_eax) = 0 as uint32_t;
            break 's_200;
        }
        let mut _fn: recomp_func_t = recomp_lookup_manual(_va);
        's_159: {
            if !_fn.is_none() {
                break 's_159;
            }
            _fn = recomp_lookup(_va);
        }
        's_165: {
            if !_fn.is_none() {
                break 's_165;
            }
            _fn = recomp_lookup_kernel(_va);
        }
        's_171: {
            if _fn.is_some() {
                break 's_171;
            }
            recomp_icall_fail_log(_va);
            (*__gamepkg_tls_g_esp) = _icall_esp;
            (*__gamepkg_tls_g_eax) = 0 as uint32_t;
            break 's_200;
        }
        _fn.expect("non-null function pointer")();
    }
