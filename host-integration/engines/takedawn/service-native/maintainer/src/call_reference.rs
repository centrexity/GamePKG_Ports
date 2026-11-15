//! Maintainer-only native call-graph comparisons with controlled handlers.
use super::*;
pub(super) fn compare(source: &str, reconstructed: &ItemFn, tag: &str) {
    let helpers = include_str!("icall_compacted_reference.inc.rs");
    let text = format!(
        r#"#![allow(dead_code,non_snake_case,non_camel_case_types,non_upper_case_globals,unused_mut,unused_variables,unused_assignments,unused_parens)]
mod original{{{helpers}{source}}}
mod reconstructed{{{helpers}{reconstructed}}}
fn main(){{let mut count=0;for target in [0u32,1,0xffff,0x10000,0x155a0,0x155a4,0x1ffff,0x20000,0xfdffffff,0xfe000000,0xfe000001,0xffffffff]{{for (low,high) in [(0,0),(0x10000,0x20000),(0x16000,0x20000),(1,0),(0,1)]{{for value in 0..128u32{{let seed=if value&1==0{{u32::MAX-value}}else{{value}};let stack=value.wrapping_mul(0x12345678);let expected=unsafe{{original::run(target,stack,low,high,seed)}};let observed=unsafe{{reconstructed::run(target,stack,low,high,seed)}};assert_eq!(observed,expected);count+=1;}}}}}}println!("PASS: {{count}} original/reconstructed {tag} graph comparisons with controlled handlers; no gameplay");}}
"#,
        reconstructed = reconstructed.to_token_stream()
    );
    let directory =
        std::env::temp_dir().join(format!("gamepkg-icall-{tag}-native-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let main = directory.join("main.rs");
    fs::write(&main, text).unwrap();
    for target in [
        "x86_64-unknown-linux-gnu",
        "i686-unknown-linux-gnu",
        "x86_64-pc-windows-gnu",
        "i686-pc-windows-gnu",
    ] {
        let exe = directory.join(format!(
            "compare-{target}{}",
            if target.contains("windows") {
                ".exe"
            } else {
                ""
            }
        ));
        let mut build = std::process::Command::new("rustc");
        build
            .args(["--edition=2021", "-D", "warnings", "-O", "--target", target])
            .arg(&main)
            .arg("-o")
            .arg(&exe);
        if target == "i686-unknown-linux-gnu" {
            build.args(["-C", "linker=/usr/bin/gcc-11"]);
        }
        assert!(build.status().unwrap().success());
        let mut run = if target.contains("windows") {
            let mut c = std::process::Command::new("wine");
            c.env("WINEDEBUG", "-all")
                .env("WINEPREFIX", "/tmp/gamepkg-port-replace-wine")
                .arg(&exe);
            c
        } else {
            std::process::Command::new(&exe)
        };
        assert!(run.status().unwrap().success());
    }
    fs::remove_dir_all(directory).unwrap();
}
