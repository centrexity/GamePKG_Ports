// Maintainer AST pattern from pinned recomp_0002_001.rs/sub_000CDDD0.
// Upstream attribution: service-native/UPSTREAM_LICENSES.txt.
match c2rust_current_block_54 {
C2Rust_Block_54::S_124 => {
                {
                    ::core::ptr::write_volatile(
                        &raw mut g_icall_trace[(::core::ptr::read_volatile::<
                            uint32_t,
                        >(&raw const g_icall_trace_idx)
                            & (ICALL_TRACE_SIZE - 1 as ::core::ffi::c_int) as uint32_t)
                            as usize],
                        _va,
                    );
                    c2rust_current_block_54 = C2Rust_Block_54::S_156;
                };
                {
                    ::core::ptr::write_volatile(
                        &raw mut g_icall_trace_idx,
                        ::core::ptr::read_volatile::<
                            uint32_t,
                        >(&raw const g_icall_trace_idx)
                            .wrapping_add(1),
                    );
                    c2rust_current_block_54 = C2Rust_Block_54::S_158;
                };
                {
                    ::core::ptr::write_volatile(
                        &raw mut g_icall_count,
                        ::core::ptr::read_volatile::<uint64_t>(&raw const g_icall_count)
                            .wrapping_add(1),
                    );
                    c2rust_current_block_54 = C2Rust_Block_54::S_160;
                };
                {
                    if !(_va >= 0xfe000000 as uint32_t || g_xbox_code_hi == 0 as uint32_t
                        || _va >= g_xbox_code_lo && _va < g_xbox_code_hi)
                    {
                        c2rust_current_block_54 = C2Rust_Block_54::S_163;
                    } else {
                        c2rust_current_block_54 = C2Rust_Block_54::S_175;
                    }
                };
            }
            C2Rust_Block_54::S_163 => {
                {
                    recomp_icall_not_code_log(_va);
                    c2rust_current_block_54 = C2Rust_Block_54::S_165;
                };
                {
                    (*__gamepkg_tls_g_esp) = _icall_esp;
                    c2rust_current_block_54 = C2Rust_Block_54::S_167;
                };
                {
                    (*__gamepkg_tls_g_eax) = 0 as uint32_t;
                    c2rust_current_block_54 = C2Rust_Block_54::S_224;
                };
            }
            C2Rust_Block_54::S_175 => {
                {
                    _fn = recomp_lookup_manual(_va);
                    c2rust_current_block_54 = C2Rust_Block_54::S_177;
                };
                {
                    if _fn.is_none() {
                        c2rust_current_block_54 = C2Rust_Block_54::S_179;
                    } else {
                        c2rust_current_block_54 = C2Rust_Block_54::S_183;
                    }
                };
            }
            C2Rust_Block_54::S_179 => {
                _fn = recomp_lookup(_va);
                c2rust_current_block_54 = C2Rust_Block_54::S_183;
            }
            C2Rust_Block_54::S_183 => {
                if _fn.is_none() {
                    c2rust_current_block_54 = C2Rust_Block_54::S_185;
                } else {
                    c2rust_current_block_54 = C2Rust_Block_54::S_189;
                }
            }
            C2Rust_Block_54::S_185 => {
                _fn = recomp_lookup_kernel(_va);
                c2rust_current_block_54 = C2Rust_Block_54::S_189;
            }
            C2Rust_Block_54::S_189 => {
                if _fn.is_some() {
                    c2rust_current_block_54 = C2Rust_Block_54::S_195;
                } else {
                    c2rust_current_block_54 = C2Rust_Block_54::S_202;
                }
            }
            C2Rust_Block_54::S_195 => {
                _fn.expect("non-null function pointer")();
                c2rust_current_block_54 = C2Rust_Block_54::S_224;
            }
            C2Rust_Block_54::S_202 => {
                {
                    recomp_icall_fail_log(_va);
                    c2rust_current_block_54 = C2Rust_Block_54::S_204;
                };
                {
                    (*__gamepkg_tls_g_esp) = _icall_esp;
                    c2rust_current_block_54 = C2Rust_Block_54::S_206;
                };
                {
                    (*__gamepkg_tls_g_eax) = 0 as uint32_t;
                    c2rust_current_block_54 = C2Rust_Block_54::S_224;
                };

}
}
