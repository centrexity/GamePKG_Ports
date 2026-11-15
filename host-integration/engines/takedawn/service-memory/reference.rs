// Maintainer runtime conformance; this is not a game or device emulator.
fn main() {
    let unsupported = local::runtime::Runtime::new(
        &[local::runtime::Window {
            address: 0x20000000,
            bytes: 67108864,
        }],
        vec![],
        256 * 1024 * 1024,
        8,
        1048576,
    )
    .unwrap();
    local::install(unsupported).unwrap();
    let root = memory::Memory::new().unwrap();
    assert!(matches!(layout::Layout::layout_current(&root), Err(-5)));
    client::local_record_release(root.call_handle()).unwrap();
    local::shutdown();
    for round in 0..2 {
        let runtime = local::runtime::Runtime::new(
            &[
                local::runtime::Window {
                    address: 0x20000000,
                    bytes: 67108864,
                },
                local::runtime::Window {
                    address: 0x24000000,
                    bytes: 67108864,
                },
                local::runtime::Window {
                    address: 0x28000000,
                    bytes: 1048576,
                },
            ],
            vec![],
            256 * 1024 * 1024,
            8,
            1048576,
        )
        .unwrap();
        local::install(runtime).unwrap();
        assert_eq!(layout_checks::layout_verify(), Ok(()), "round {round}");
        local::shutdown();
    }
    println!("PASS: 29 main-RAM views, contiguous/tiled alias, independent flash, all 32 spans, protection/decommit/reuse and unbacked device holes; synthetic only");
}
