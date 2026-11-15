// Pinned recomp_0007_002.rs/sub_00246622, entry's final two groups + six arms.
// Attribution: service-native/UPSTREAM_LICENSES.txt.
match c2rust_current_block_30 {
    C2Rust_Block_30::S_350 => {
        { _fn = recomp_lookup_manual(_va); c2rust_current_block_30 = C2Rust_Block_30::S_360; };
        { if _fn.is_none() { c2rust_current_block_30 = C2Rust_Block_30::S_362; }
          else { c2rust_current_block_30 = C2Rust_Block_30::S_366; } };
    }
    C2Rust_Block_30::S_362 => {
        _fn = recomp_lookup(_va);
        c2rust_current_block_30 = C2Rust_Block_30::S_366;
    }
    C2Rust_Block_30::S_366 => {
        if _fn.is_none() { c2rust_current_block_30 = C2Rust_Block_30::S_368; }
        else { c2rust_current_block_30 = C2Rust_Block_30::S_372; }
    }
    C2Rust_Block_30::S_368 => {
        _fn = recomp_lookup_kernel(_va);
        c2rust_current_block_30 = C2Rust_Block_30::S_372;
    }
    C2Rust_Block_30::S_372 => {
        if _fn.is_some() { c2rust_current_block_30 = C2Rust_Block_30::S_378; }
        else { c2rust_current_block_30 = C2Rust_Block_30::S_385; }
    }
    C2Rust_Block_30::S_378 => {
        _fn.expect("non-null function pointer")();
        c2rust_current_block_30 = C2Rust_Block_30::S_399;
    }
    C2Rust_Block_30::S_385 => {
        { recomp_icall_fail_log(_va); c2rust_current_block_30 = C2Rust_Block_30::S_387; };
        { (*__gamepkg_tls_g_esp) = (*__gamepkg_tls_g_esp).wrapping_add(4 as uint32_t);
          c2rust_current_block_30 = C2Rust_Block_30::S_389; };
        { (*__gamepkg_tls_g_eax) = 0 as uint32_t;
          c2rust_current_block_30 = C2Rust_Block_30::S_399; };
    }
}
