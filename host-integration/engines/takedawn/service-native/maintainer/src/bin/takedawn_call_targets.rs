//! Maintainer-only exact dispatch inventory. No native function addresses emitted.
use quote::ToTokens;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
use syn::{Expr, Item, Member, Type};
const PINS: &str = include_str!("../../reviewed_units.tsv");
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn symbol(expr: &Expr) -> Option<String> {
    let Expr::Path(p) = expr else {
        return None;
    };
    if p.qself.is_some()
        || !p.attrs.is_empty()
        || p.path.leading_colon.is_some()
        || p.path.segments.len() != 1
        || !matches!(p.path.segments[0].arguments, syn::PathArguments::None)
    {
        return None;
    }
    Some(p.path.segments[0].ident.to_string())
}
fn sub_name(name: &str) -> bool {
    name.strip_prefix("sub_")
        .is_some_and(|s| s.len() == 8 && s.bytes().all(|b| b.is_ascii_hexdigit()))
}
fn literal(expr: &Expr) -> Option<u32> {
    match expr {
        Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(i),
            attrs,
        }) if attrs.is_empty() => i.base10_parse().ok(),
        _ => None,
    }
}
fn table(source: &str) -> Result<Vec<(u32, String)>, String> {
    let ast = syn::parse_file(source).map_err(|e| e.to_string())?;
    let tables = ast
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Static(v) if v.ident == "g_recomp_table" => Some(v),
            _ => None,
        })
        .collect::<Vec<_>>();
    if tables.len() != 1 {
        return Err("one exact dispatch table required".into());
    }
    let item = tables[0];
    if !item.attrs.is_empty() {
        return Err("dispatch attributes".into());
    }
    let Type::Array(ty) = &*item.ty else {
        return Err("dispatch array type".into());
    };
    if ty.elem.to_token_stream().to_string() != "recomp_entry_t" {
        return Err("dispatch entry type".into());
    }
    let Some(length) = literal(&ty.len) else {
        return Err("dispatch array bound".into());
    };
    let Expr::Array(array) = &*item.expr else {
        return Err("dispatch literal array".into());
    };
    if array.elems.len() != length as usize || length == 0 || length > 32768 {
        return Err("dispatch array length".into());
    }
    let mut rows = Vec::new();
    let mut names = BTreeSet::new();
    for expr in &array.elems {
        let Expr::Struct(entry) = expr else {
            return Err("dispatch entry shape".into());
        };
        if entry.path.to_token_stream().to_string() != "recomp_entry_t"
            || entry.rest.is_some()
            || !entry.attrs.is_empty()
            || entry.fields.len() != 2
        {
            return Err("dispatch entry fields".into());
        }
        let mut address = None;
        let mut function = None;
        for field in &entry.fields {
            if !field.attrs.is_empty() || field.colon_token.is_none() {
                return Err("dispatch field attributes".into());
            }
            let Member::Named(name) = &field.member else {
                return Err("dispatch field name".into());
            };
            if name == "xbox_va" && address.is_none() {
                let Expr::Cast(c) = &field.expr else {
                    return Err("dispatch address cast".into());
                };
                if c.ty.to_token_stream().to_string() != "uint32_t" || !c.attrs.is_empty() {
                    return Err("dispatch address width".into());
                }
                address = literal(&c.expr);
            } else if name == "func" && function.is_none() {
                let Expr::Call(call) = &field.expr else {
                    return Err("dispatch Some".into());
                };
                if symbol(&call.func).as_deref() != Some("Some")
                    || call.args.len() != 1
                    || !call.attrs.is_empty()
                {
                    return Err("dispatch Some shape".into());
                }
                let Expr::Cast(c) = &call.args[0] else {
                    return Err("dispatch function cast".into());
                };
                if !c.attrs.is_empty()
                    || c.ty.to_token_stream().to_string()
                        != quote::quote!(unsafe extern "C" fn() -> ()).to_string()
                {
                    return Err("dispatch function signature".into());
                }
                function = symbol(&c.expr);
            } else {
                return Err("unknown or repeated dispatch field".into());
            }
        }
        let (Some(address), Some(name)) = (address, function) else {
            return Err("incomplete dispatch entry".into());
        };
        if (!sub_name(&name) && name != "xbe_entry_point")
            || !names.insert(name.clone())
            || rows
                .last()
                .is_some_and(|(previous, _)| *previous >= address)
        {
            return Err("ambiguous or unsorted dispatch target".into());
        }
        if sub_name(&name)
            && u32::from_str_radix(&name[4..], 16).map_err(|e| e.to_string())? != address
        {
            return Err("dispatch address/symbol mismatch".into());
        }
        rows.push((address, name));
    }
    Ok(rows)
}
fn sub_definitions(source: &str) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
    let ast = syn::parse_file(source)?;
    let mut names = BTreeSet::new();
    for item in ast.items {
        if let Item::Fn(f) = item {
            let name = f.sig.ident.to_string();
            if sub_name(&name) && !names.insert(name) {
                return Err("duplicate source definition".into());
            }
        }
    }
    Ok(names)
}
fn branch(
    first: usize,
    last: usize,
    rows: &[(u32, String)],
    functions: &mut Vec<String>,
) -> String {
    if last - first == 1 {
        return format!(
            "crate::targets::chunks_{:04}::leaf_{first:04}(address)",
            first / 8
        );
    }
    let middle = first + (last - first) / 2;
    let left = branch(first, middle, rows, functions);
    let right = branch(middle, last, rows, functions);
    let id = functions.len();
    let bound = rows[middle * 128].0;
    functions.push(format!(
        "fn branch_{id}(address:u32)->u32{{if address<{bound}u32{{{left}}}else{{{right}}}}}"
    ));
    format!("branch_{id}(address)")
}
fn emitted(
    rows: &[(u32, String)],
    manual: &BTreeSet<String>,
    stubs: &BTreeSet<String>,
) -> BTreeMap<String, String> {
    let mut files = BTreeMap::<String, String>::new();
    for (index, chunk) in rows.chunks(128).enumerate() {
        let mut arms = String::new();
        for (offset, (address, name)) in chunk.iter().enumerate() {
            let kind = if stubs.contains(name) {
                6
            } else if manual.contains(name) {
                2
            } else if name == "xbe_entry_point" {
                3
            } else {
                1
            };
            let tag = (kind << 24) | (index * 128 + offset) as u32;
            arms.push_str(&format!("{address}u32=>{tag}u32,"));
        }
        files
            .entry(format!("targets/chunks_{:04}.rs", index / 8))
            .or_default()
            .push_str(&format!(
                "pub fn leaf_{index:04}(address:u32)->u32{{match address{{{arms}_=>0}}}}\n"
            ));
    }
    let mut functions = Vec::new();
    let root = branch(0, rows.len().div_ceil(128), rows, &mut functions);
    let modules = (0..rows.len().div_ceil(1024))
        .map(|i| format!("pub mod chunks_{i:04};\n"))
        .collect::<String>();
    // These exact constants/semantics were reviewed from the separately pinned
    // soundtrack/kernel sources below. Kernel lookup intentionally accepts
    // unaligned bytes; soundtrack lookup rejects them.
    let entry=format!("pub fn original_target(address:u32)->u32{{if address>=0xfe7f5700 && address<0xfe7f5728 && address&3==0{{return 0x04000000|((address-0xfe7f5700)/4);}}let found={root};if found!=0{{return found;}}if address>=0xfe000000 && address<0xfe0005e8{{return 0x05000000|((address-0xfe000000)/4);}}0}}");
    files.insert(
        "targets.rs".into(),
        format!("{modules}{}\n{entry}", functions.join("\n")),
    );
    files
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 3 {
        return Err("requires pinned-input-directory new-output-directory".into());
    }
    let input = Path::new(&args[1]);
    let output = Path::new(&args[2]);
    if output.exists() {
        return Err("output must not exist".into());
    }
    let mut expected = PINS
        .lines()
        .map(|line| line.split_once('\t').unwrap())
        .collect::<BTreeMap<_, _>>();
    expected.extend([
        (
            "soundtrack.c",
            "597dc38f2d52321ae98fca04db9622e2a65b0db37b63bdcc8961e8cb60522cc8",
        ),
        (
            "soundtrack.h",
            "b771544f7eb09ad6d30fe5fcd134f8f8703b21e5b43140784da26cc77977a52e",
        ),
        (
            "kernel_bridge.c",
            "bf62f8787a2e78a0abdb28e10ad8f79c5267dc655491353af231067747ce446e",
        ),
        (
            "kernel.h",
            "efae4f1b5aab8a2a420d00b8fb5fd75a1fb9a717704a709c34baddcc254bd17e",
        ),
    ]);
    let names = [
        "recomp_dispatch.rs",
        "recomp_manual.rs",
        "recomp_stubs_unresolved.rs",
        "soundtrack.c",
        "soundtrack.h",
        "kernel_bridge.c",
        "kernel.h",
    ];
    let mut sources = BTreeMap::new();
    let mut hashes = BTreeMap::new();
    for name in names {
        let path = input.join(name);
        let meta = fs::symlink_metadata(&path)?;
        if !meta.is_file() || meta.len() > 16 * 1024 * 1024 {
            return Err("bounded regular input required".into());
        }
        let data = fs::read(path)?;
        let hash = digest(&data);
        if expected[name] != hash {
            return Err(format!("changed reviewed input {name}").into());
        }
        hashes.insert(name, hash);
        sources.insert(name, String::from_utf8(data)?);
    }
    let rows = table(&sources["recomp_dispatch.rs"])?;
    if rows.len() != 24001
        || rows
            .iter()
            .filter(|(_, name)| name == "xbe_entry_point")
            .count()
            != 1
    {
        return Err("changed reviewed dispatch coverage".into());
    }
    let manual = sub_definitions(&sources["recomp_manual.rs"])?;
    let stubs = sub_definitions(&sources["recomp_stubs_unresolved.rs"])?;
    if manual.len() != 5
        || stubs.len() != 13
        || rows
            .iter()
            .find(|(_, name)| name == "xbe_entry_point")
            .unwrap()
            .0
            != 0x1d2807
    {
        return Err("changed reviewed manual/stub/entrypoint inventory".into());
    }
    let files = emitted(&rows, &manual, &stubs);
    let mut emitted_hashes = BTreeMap::new();
    fs::create_dir(output)?;
    for (name, text) in files {
        let path = output.join(&name);
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(&path, text)?;
        if !std::process::Command::new("rustfmt")
            .args(["--edition", "2021", "--config", "skip_children=true"])
            .arg(&path)
            .status()?
            .success()
        {
            return Err("formatting failed".into());
        }
        emitted_hashes.insert(name, digest(&fs::read(path)?));
    }
    let report = serde_json::json!({"schema":1,"production_ready":false,"source_kit_sha256":"6b37d24c0984c686c7ed6fd73cd3a5b252b5631ceefa2798e5b7a6e0adff8dc4","tool_sha256":digest(include_bytes!("takedawn_call_targets.rs")),"lock_sha256":digest(include_bytes!("../../Cargo.lock")),"inputs":hashes,"rows":rows,"manual_wrappers":manual,"unresolved_stubs":stubs,"emitted_files":emitted_hashes,"kernel_slots":378,"soundtrack_slots":10,"qualification":"Original lookup identity only; no native handler pointers, handler execution, resolver integration or gameplay qualification"});
    fs::write(
        output.join("call_targets.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("Verified {} original lookup rows; 5 manual wrappers, named entrypoint, 13 unresolved source stubs; numeric tags only",rows.len());
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn input(rows: &str, count: usize) -> String {
        format!("static mut g_recomp_table:[recomp_entry_t;{count}]=[{rows}];")
    }
    fn row(address: &str, name: &str) -> String {
        format!("recomp_entry_t{{xbox_va:{address} as uint32_t,func:Some({name} as unsafe extern \"C\" fn()->())}},")
    }
    #[test]
    fn only_exact_sorted_unique_direct_function_entries_are_accepted() {
        let good = row("0x11000", "sub_00011000");
        assert_eq!(
            table(&input(&good, 1)).unwrap(),
            vec![(0x11000, "sub_00011000".into())]
        );
        for text in [
            input(&(good.clone() + &good), 2),
            input(&row("0x11001", "sub_00011000"), 1),
            input(&good, 2),
            input(&good.replace("Some(", "Some(("), 1),
            input(&good.replace("uint32_t", "uint64_t"), 1),
            input(&good.replace("fn()", "fn(u32)"), 1),
            input(&good.replace("sub_00011000", "other"), 1),
            input(&good.replace("func:", "unknown:"), 1),
        ] {
            assert!(table(&text).is_err(), "{text}");
        }
    }
    #[test]
    fn named_entrypoint_is_retained_instead_of_assuming_a_hex_function_name() {
        assert_eq!(
            table(&input(&row("0x1d2807", "xbe_entry_point"), 1)).unwrap()[0].1,
            "xbe_entry_point"
        );
    }
    #[test]
    fn emitted_tags_preserve_manual_and_unresolved_identity_without_callbacks() {
        let rows = vec![
            (1, "sub_00000001".into()),
            (2, "sub_00000002".into()),
            (3, "xbe_entry_point".into()),
            (4, "sub_00000004".into()),
        ];
        let files = emitted(
            &rows,
            &BTreeSet::from(["sub_00000002".into()]),
            &BTreeSet::from(["sub_00000004".into()]),
        );
        let source = &files["targets/chunks_0000.rs"];
        for tag in [0x01000000u32, 0x02000001, 0x03000002, 0x06000003] {
            assert!(source.contains(&tag.to_string()));
        }
        assert!(!source.contains("sub_"));
        assert!(!source.contains("unsafe"));
    }
}
