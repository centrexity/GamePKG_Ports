//! Maintainer-only whole-kit inventory. No game runtime or policy exception.
use super::*;
use std::io::Read;
const INVENTORY: &str = include_str!("../reviewed_units.tsv");
const MAX_UNIT: u64 = 16 * 1024 * 1024;
const ENTRY: &str = "xbe_entry_point";
const ENTRY_UNIT: &str = "recomp_0004_013.rs";
const ENTRY_ADDRESS: u32 = 0x001d2807;
fn route_address(name: &str) -> Option<u32> {
    if name == ENTRY {
        return Some(ENTRY_ADDRESS);
    }
    let hex = name.strip_prefix("sub_")?;
    (hex.len() == 8 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(hex, 16).ok())
        .flatten()
}
struct Candidate {
    unit: String,
    code: String,
    dependencies: BTreeSet<String>,
    runtime_calls: u32,
}
fn definitions(source: syn::File) -> impl Iterator<Item = ItemFn> {
    source.items.into_iter().filter_map(|item| match item {
        Item::Fn(function)
            if function.sig.ident == ENTRY
                || function
                    .sig
                    .ident
                    .to_string()
                    .strip_prefix("sub_")
                    .is_some_and(|n| n.len() == 8 && n.bytes().all(|b| b.is_ascii_hexdigit())) =>
        {
            Some(function)
        }
        _ => None,
    })
}
fn read_unit(
    root: &Path,
    name: &str,
    expected: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let path = root.join(name);
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.len() > MAX_UNIT {
        return Err("unit is not a bounded regular file".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(&path)?
        .take(MAX_UNIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_UNIT || format!("{:x}", Sha256::digest(&bytes)) != expected {
        return Err(format!("unreviewed unit hash: {name}").into());
    }
    Ok(bytes)
}
fn close_graph(
    candidates: &mut BTreeMap<String, Candidate>,
    rejected: &mut BTreeMap<String, serde_json::Value>,
) {
    loop {
        let missing = candidates
            .iter()
            .filter_map(|(name, candidate)| {
                let absent = candidate
                    .dependencies
                    .iter()
                    .filter(|dependency| !candidates.contains_key(*dependency))
                    .cloned()
                    .collect::<Vec<_>>();
                (!absent.is_empty()).then(|| (name.clone(), absent))
            })
            .collect::<Vec<_>>();
        if missing.is_empty() {
            break;
        }
        for (name, absent) in missing {
            let candidate = candidates.remove(&name).unwrap();
            rejected.insert(
                name,
                serde_json::json!({"unit":candidate.unit,"unconverted_dependencies":absent}),
            );
        }
    }
}
fn reject_duplicates(
    owners: &BTreeMap<String, Vec<String>>,
    accepted: &mut BTreeMap<String, Candidate>,
    rejected: &mut BTreeMap<String, serde_json::Value>,
) -> BTreeMap<String, Vec<String>> {
    let mut duplicates = BTreeMap::new();
    for (name, locations) in owners {
        if locations.len() > 1 {
            accepted.remove(name);
            rejected.insert(
                name.clone(),
                serde_json::json!({"duplicate_definitions":locations}),
            );
            duplicates.insert(name.clone(), locations.clone());
        }
    }
    duplicates
}
struct Remap<'a>(&'a BTreeMap<String, usize>);
impl VisitMut for Remap<'_> {
    fn visit_expr_path_mut(&mut self, expression: &mut syn::ExprPath) {
        let parts = expression
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>();
        if parts.len() == 3 && parts[0] == "crate" && parts[1] == "game" {
            if let Some(index) = self.0.get(&parts[2]) {
                let unit = format_ident!("unit_{index:04}");
                let name = format_ident!("{}", parts[2]);
                expression.path = parse_quote!(crate::game::#unit::#name);
            }
        }
        visit_mut::visit_expr_path_mut(self, expression);
    }
}
fn branch(first: usize, last: usize, addresses: &[u32], nodes: &mut Vec<String>) -> String {
    if last - first == 1 {
        return format!("crate::game::unit_{first:04}::route(native_state,guest_ram,address)");
    }
    let middle = first + (last - first) / 2;
    let left = branch(first, middle, addresses, nodes);
    let right = branch(middle, last, addresses, nodes);
    let id = nodes.len();
    let bound = addresses[middle];
    nodes.push(format!("pub fn branch_{id}(native_state:&mut crate::registers::Registers,guest_ram:&mut crate::memory::Memory,address:u32)->Result<(),i32>{{if address<{bound}u32{{{left}}}else{{{right}}}}}"));
    format!(
        "crate::dispatch::nodes_{:04}::branch_{id}(native_state,guest_ram,address)",
        id / 128
    )
}
fn token_count(tokens: proc_macro2::TokenStream) -> usize {
    let mut pending = vec![tokens];
    let mut count = 0;
    while let Some(tokens) = pending.pop() {
        for token in tokens {
            if let proc_macro2::TokenTree::Group(group) = token {
                count += 2;
                pending.push(group.stream());
            } else {
                count += 1;
            }
        }
    }
    count
}
fn source_partitions(
    accepted: &BTreeMap<String, Candidate>,
) -> Result<Vec<Vec<&String>>, Box<dyn std::error::Error>> {
    let mut partitions = Vec::new();
    let mut current = Vec::new();
    let mut bytes = 0;
    let mut tokens = 0;
    // Named entrypoint spelling is not address order. Reject address aliases
    // before emitting anything and partition every route in numeric order.
    let mut ordered = BTreeMap::new();
    for (name, candidate) in accepted {
        if name == ENTRY && candidate.unit != ENTRY_UNIT {
            return Err("named entrypoint has an unreviewed source unit".into());
        }
        let address = route_address(name).ok_or("unreviewed routable function name")?;
        if ordered.insert(address, (name, candidate)).is_some() {
            return Err("duplicate native route address".into());
        }
    }
    // Leave space for the direct route and path remapping below. The app's
    // independent source token/complexity limits are not changed or bypassed.
    for (name, candidate) in ordered.into_values() {
        let weight = candidate.code.len() + candidate.dependencies.len() * 16;
        let count = token_count(candidate.code.parse()?) + candidate.dependencies.len() * 4;
        if weight > 1024 * 1024 - 4096 || count > 120000 {
            return Err(format!("native function {name} needs structural source partitioning ({weight} bytes, {count} tokens)").into());
        }
        if current.len() == 8 || bytes + weight > 1024 * 1024 - 4096 || tokens + count > 120000 {
            partitions.push(current);
            current = Vec::new();
            bytes = 0;
            tokens = 0;
        }
        current.push(name);
        bytes += weight;
        tokens += count;
    }
    if !current.is_empty() {
        partitions.push(current);
    }
    Ok(partitions)
}
fn emit_closed(
    accepted: &BTreeMap<String, Candidate>,
    output: &Path,
) -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    if accepted.is_empty() {
        return Err("no closed functions to prepare".into());
    }
    let partitions = source_partitions(accepted)?;
    let mapping = partitions
        .iter()
        .enumerate()
        .flat_map(|(index, names)| names.iter().map(move |name| ((*name).clone(), index)))
        .collect::<BTreeMap<_, _>>();
    let mut files = BTreeMap::<String, String>::new();
    let mut game = String::new();
    let mut addresses = Vec::new();
    for (index, chunk) in partitions.iter().enumerate() {
        game.push_str(&format!("pub mod unit_{index:04};\n"));
        addresses.push(route_address(chunk[0]).ok_or("unreviewed native route")?);
        let mut body = String::from(
            "// Maintainer candidate from the pinned Takedawn kit; not an admitted game.\n",
        );
        let mut arms = String::new();
        for name in chunk {
            let mut function: syn::ItemFn = syn::parse_str(&accepted[*name].code)?;
            Remap(&mapping).visit_item_fn_mut(&mut function);
            body.push_str(&function.to_token_stream().to_string());
            body.push('\n');
            let address = route_address(name).ok_or("unreviewed native route")?;
            arms.push_str(&format!("{address}u32=>{name}(native_state,guest_ram),"));
        }
        body.push_str(&format!("pub fn route(native_state:&mut crate::registers::Registers,guest_ram:&mut crate::memory::Memory,address:u32)->Result<(),i32>{{match address{{{arms}_=>Err(-5)}}}}"));
        if body.len() > 1024 * 1024 {
            return Err(
                format!("candidate unit {index} exceeds its byte bound after remapping").into(),
            );
        }
        files.insert(format!("game/unit_{index:04}.rs"), body);
    }
    files.insert("game.rs".into(), game);
    let mut nodes = Vec::new();
    let root = branch(0, addresses.len(), &addresses, &mut nodes);
    let mut dispatch = String::new();
    for (index, chunk) in nodes.chunks(128).enumerate() {
        dispatch.push_str(&format!("pub mod nodes_{index:04};\n"));
        files.insert(format!("dispatch/nodes_{index:04}.rs"), chunk.join("\n"));
    }
    dispatch.push_str(&format!("pub fn invoke(native_state:&mut crate::registers::Registers,guest_ram:&mut crate::memory::Memory,address:u32)->Result<(),i32>{{if !native_state.valid(){{return Err(-1);}}{root}}}"));
    dispatch.push_str(&format!(
        "pub fn entry_available()->bool{{{}}}",
        accepted.contains_key(ENTRY)
    ));
    files.insert("dispatch.rs".into(), dispatch);
    if files.len() > 8192 || files.values().map(String::len).sum::<usize>() > 128 * 1024 * 1024 {
        return Err("candidate sources exceed reviewed preparation bounds".into());
    }
    fs::create_dir(output)?;
    for (name, text) in &files {
        let path = output.join(name);
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, text)?;
    }
    let paths = files
        .keys()
        .map(|name| output.join(name))
        .collect::<Vec<_>>();
    // Do not let rustfmt follow the module tree in addition to formatting these
    // exact inventoried files. Bound the subprocess argument batch as well.
    for chunk in paths.chunks(64) {
        if !std::process::Command::new("rustfmt")
            .args(["--edition", "2021", "--config", "skip_children=true"])
            .args(chunk)
            .status()?
            .success()
        {
            return Err("candidate formatting failed".into());
        }
    }
    let mut hashes = BTreeMap::new();
    for name in files.keys() {
        hashes.insert(
            name.clone(),
            format!("{:x}", Sha256::digest(fs::read(output.join(name))?)),
        );
    }
    Ok(hashes)
}
/// Hash-check every input before parsing or creating output. Each unit is then
/// parsed independently; no unbounded whole-kit AST or unsafe reference is kept.
pub(super) fn run(
    input: &Path,
    output: &Path,
    emit: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if output.exists() {
        return Err("batch output must not exist".into());
    }
    let units = INVENTORY
        .lines()
        .map(|line| line.split_once('\t').ok_or("invalid pinned inventory"))
        .collect::<Result<Vec<_>, _>>()?;
    if units.len() != 485 {
        return Err("unexpected pinned unit count".into());
    }
    let mut unique = BTreeSet::new();
    for (name, hash) in &units {
        if !name.starts_with("recomp_")
            || !name.ends_with(".rs")
            || name.contains(['/', '\\'])
            || hash.len() != 64
            || !unique.insert(*name)
        {
            return Err("invalid pinned unit identity".into());
        }
        read_unit(input, name, hash)?;
    }
    let mut owners = BTreeMap::<String, Vec<String>>::new();
    let mut accepted = BTreeMap::new();
    let mut rejected = BTreeMap::new();
    let mut locally_convertible = 0usize;
    for (index, (name, hash)) in units.iter().enumerate() {
        let bytes = read_unit(input, name, hash)?;
        let ast = syn::parse_file(std::str::from_utf8(&bytes)?)?;
        for function in definitions(ast) {
            let id = function.sig.ident.to_string();
            if id == ENTRY && *name != ENTRY_UNIT {
                return Err("named entrypoint has an unreviewed source unit".into());
            }
            owners
                .entry(id.clone())
                .or_default()
                .push((*name).to_owned());
            match lower(function) {
                Ok((mut function, dependencies)) => {
                    let runtime_calls = icall::count(&mut function);
                    locally_convertible += 1;
                    accepted.insert(
                        id,
                        Candidate {
                            unit: (*name).to_owned(),
                            code: function.to_token_stream().to_string(),
                            dependencies,
                            runtime_calls,
                        },
                    );
                }
                Err(errors) => {
                    rejected.insert(id, serde_json::json!({"unit":name,"reasons":errors}));
                }
            }
        }
        if index % 50 == 0 {
            eprintln!("Inventory: {}/{} units", index + 1, units.len());
        }
    }
    // Never choose a duplicate definition by archive order. Even identical
    // bodies require explicit ownership review before any caller may use them.
    let duplicates = reject_duplicates(&owners, &mut accepted, &mut rejected);
    close_graph(&mut accepted, &mut rejected);
    let graph=accepted.iter().map(|(name,c)| (name.clone(),serde_json::json!({"unit":c.unit,"dependencies":c.dependencies,"runtime_calls":c.runtime_calls,"lowered_sha256":format!("{:x}",Sha256::digest(c.code.as_bytes()))}))).collect::<BTreeMap<_,_>>();
    let total = owners.values().map(Vec::len).sum::<usize>();
    let emitted = if emit {
        emit_closed(&accepted, output)?
    } else {
        fs::create_dir(output)?;
        BTreeMap::new()
    };
    let report = serde_json::json!({
        "schema":1,"production_ready":false,"register_record_version":4,"register_record_bytes":244,"debug_adapter_sha256":format!("{:x}",Sha256::digest(include_bytes!("../../debug.rs"))),"debug_review_sha256":format!("{:x}",Sha256::digest(include_bytes!("../reviewed_debug_service.json"))),"registers_source_sha256":format!("{:x}",Sha256::digest(include_bytes!("../../registers.rs"))),"source_kit_sha256":"6b37d24c0984c686c7ed6fd73cd3a5b252b5631ceefa2798e5b7a6e0adff8dc4",
        "inventory_sha256":format!("{:x}",Sha256::digest(INVENTORY.as_bytes())),
        "converter_main_sha256":format!("{:x}",Sha256::digest(include_bytes!("main.rs"))),
        "converter_batch_sha256":format!("{:x}",Sha256::digest(include_bytes!("batch.rs"))),
        "converter_icall_sha256":format!("{:x}",Sha256::digest(include_bytes!("icall.rs"))),
        "converter_icall_compacted_sha256":format!("{:x}",Sha256::digest(include_bytes!("icall_compacted.rs"))),"converter_icall_compacted_pattern_sha256":format!("{:x}",Sha256::digest(include_bytes!("icall_compacted_template.inc"))),"converter_icall_guarded_pattern_sha256":format!("{:x}",Sha256::digest(include_bytes!("icall_guarded_template.inc"))),"converter_icall_jump_pattern_sha256":format!("{:x}",Sha256::digest(include_bytes!("icall_jump_template.inc"))),"converter_icall_jump_compacted_pattern_sha256":format!("{:x}",Sha256::digest(include_bytes!("icall_jump_compacted_template.inc"))),"converter_icall_pattern_sha256":format!("{:x}",Sha256::digest(include_bytes!("icall_template.inc"))),
        "converter_lock_sha256":format!("{:x}",Sha256::digest(include_bytes!("../Cargo.lock"))),
        "units":units.iter().map(|(name,hash)|(*name,*hash)).collect::<BTreeMap<_,_>>(),
        "definitions":total,"unique_definitions":owners.len(),"locally_convertible_definitions":locally_convertible,
        "definition_scope":"sub_XXXXXXXX plus the exact unit-bound xbe_entry_point; five _gen helpers still require inclusion",
        "named_entrypoint":{"name":ENTRY,"unit":ENTRY_UNIT,"address":ENTRY_ADDRESS,"retained":accepted.contains_key(ENTRY)},
        "closed_definitions":accepted.len(),"duplicates":duplicates,"closed_graph":graph,"rejected":rejected,
        "emitted_files":emitted,
        "runtime_call_functions":accepted.iter().filter(|(_,c)|c.runtime_calls!=0).map(|(n,c)|(n,c.runtime_calls)).collect::<BTreeMap<_,_>>(),
        "closed_graph_scope":"literal direct dependencies only; runtime native routes remain partial and must fault on known but unmigrated targets",
        "qualification":"Maintainer lowering only; emitted candidates still require source admission, separate object audits and differential checks; no full-game behavior or device qualification"
    });
    fs::write(
        output.join("inventory.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{total} definitions; {} locally convertible; {} closed definitions; {} duplicate symbols; maintainer candidate evidence only",locally_convertible,accepted.len(),duplicates.len());
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_the_exact_named_root_is_included_and_address_aliases_fail_before_output() {
        let names=definitions(syn::parse_file("fn sub_00000100(){}fn xbe_entry_point(){}fn xbe_entry_point_gen(){}fn sub_001537C0_gen(){}fn unrelated(){}").unwrap()).map(|f|f.sig.ident.to_string()).collect::<Vec<_>>();
        assert_eq!(names, vec!["sub_00000100", "xbe_entry_point"]);
        let mut root = candidate(&[]);
        root.unit = ENTRY_UNIT.into();
        root.code = "pub fn xbe_entry_point(){}".into();
        let mut aliases = BTreeMap::from([(ENTRY.into(), root)]);
        aliases.insert("sub_001D2807".into(), candidate(&[]));
        assert!(source_partitions(&aliases)
            .unwrap_err()
            .to_string()
            .contains("duplicate"));
        aliases.remove("sub_001D2807");
        aliases.get_mut(ENTRY).unwrap().unit = "recomp_manual.rs".into();
        assert!(source_partitions(&aliases)
            .unwrap_err()
            .to_string()
            .contains("source unit"));
        assert_eq!(route_address("xbe_entry_point_gen"), None);
        assert_eq!(route_address("sub_001537C0_gen"), None);
    }
    #[test]
    fn named_entrypoint_uses_numeric_order_across_partitions_and_preserves_direct_calls() {
        let root = std::env::temp_dir().join(format!(
            "gamepkg-entry-emission-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let output = root.join("source");
        let mut candidates = BTreeMap::new();
        for base in [0x100u32, 0x00f00000] {
            for offset in 0..8 {
                let name = format!("sub_{:08X}", base + offset);
                candidates.insert(name.clone(),Candidate{unit:"test.rs".into(),code:format!("pub fn {name}(native_state:&mut crate::registers::Registers,guest_ram:&mut crate::memory::Memory)->Result<(),i32>{{native_state.value+=1;Ok(())}}"),dependencies:BTreeSet::new(),runtime_calls:0});
            }
        }
        candidates.insert(ENTRY.into(),Candidate{unit:ENTRY_UNIT.into(),code:"pub fn xbe_entry_point(native_state:&mut crate::registers::Registers,guest_ram:&mut crate::memory::Memory)->Result<(),i32>{crate::game::sub_00F00007(native_state,guest_ram)?;native_state.value+=77;Ok(())}".into(),dependencies:BTreeSet::from(["sub_00F00007".into()]),runtime_calls:0});
        emit_closed(&candidates, &output).unwrap();
        let harness = r#"#![allow(dead_code,unused_variables,non_snake_case)]
mod game;mod dispatch;
mod memory{pub struct Memory;}
mod registers{pub struct Registers{pub value:u32}impl Registers{pub fn valid(&self)->bool{true}}}
fn main(){let mut state=registers::Registers{value:0};let mut ram=memory::Memory;
assert!(dispatch::entry_available());assert_eq!(dispatch::invoke(&mut state,&mut ram,0x1d2807),Ok(()));assert_eq!(state.value,78);
assert_eq!(dispatch::invoke(&mut state,&mut ram,0x100),Ok(()));assert_eq!(state.value,79);
assert_eq!(dispatch::invoke(&mut state,&mut ram,0x1d2806),Err(-5));assert_eq!(dispatch::invoke(&mut state,&mut ram,0x1d2808),Err(-5));assert_eq!(state.value,79);
assert_eq!(dispatch::invoke(&mut state,&mut ram,0xf00007),Ok(()));assert_eq!(state.value,80);}
"#;
        fs::write(output.join("main.rs"), harness).unwrap();
        let exe = root.join("entry-check");
        assert!(std::process::Command::new("rustc")
            .args(["--edition=2021", "-D", "warnings"])
            .arg(output.join("main.rs"))
            .arg("-o")
            .arg(&exe)
            .status()
            .unwrap()
            .success());
        assert!(std::process::Command::new(&exe).status().unwrap().success());
        fs::remove_dir_all(root).unwrap();
    }
    fn candidate(deps: &[&str]) -> Candidate {
        Candidate {
            unit: "test.rs".into(),
            code: String::new(),
            dependencies: deps.iter().map(|s| (*s).to_owned()).collect(),
            runtime_calls: 0,
        }
    }
    #[test]
    fn source_partitioning_splits_large_groups_and_rejects_oversized_functions_before_output() {
        let mut candidates = BTreeMap::new();
        for (index, size) in [(0, 600000), (1, 600000)] {
            let name = format!("sub_{index:08X}");
            candidates.insert(
                name.clone(),
                Candidate {
                    unit: "synthetic.rs".into(),
                    code: format!("pub fn {name}(){{let padding=\"{}\";}}", "x".repeat(size)),
                    dependencies: BTreeSet::new(),
                    runtime_calls: 0,
                },
            );
        }
        let groups = source_partitions(&candidates).unwrap();
        assert_eq!(groups.iter().map(Vec::len).collect::<Vec<_>>(), vec![1, 1]);
        let candidate = candidates.get_mut("sub_00000000").unwrap();
        candidate.code = format!(
            "pub fn sub_00000000(){{let padding=\"{}\";}}",
            "x".repeat(1024 * 1024)
        );
        assert!(source_partitions(&candidates)
            .unwrap_err()
            .to_string()
            .contains("sub_00000000"));
        let root = std::env::temp_dir().join(format!(
            "gamepkg-oversized-candidate-{}",
            std::process::id()
        ));
        let output = root.join("must-not-exist");
        assert!(!output.exists());
        assert!(emit_closed(&candidates, &output).is_err());
        assert!(!output.exists());
        candidate_token_budget_test(&mut candidates);
    }
    fn candidate_token_budget_test(candidates: &mut BTreeMap<String, Candidate>) {
        candidates.get_mut("sub_00000000").unwrap().code =
            format!("pub fn sub_00000000(){{{}}}", "let n=1;".repeat(25000));
        assert!(source_partitions(candidates)
            .unwrap_err()
            .to_string()
            .contains("tokens"));
    }
    #[test]
    fn graph_removes_transitive_missing_calls_and_preserves_closed_cycles() {
        let mut candidates = BTreeMap::from([
            ("a".into(), candidate(&["b"])),
            ("b".into(), candidate(&["missing"])),
            ("c".into(), candidate(&["d"])),
            ("d".into(), candidate(&["c"])),
            ("leaf".into(), candidate(&[])),
        ]);
        let mut rejected = BTreeMap::new();
        close_graph(&mut candidates, &mut rejected);
        assert_eq!(
            candidates.keys().map(String::as_str).collect::<Vec<_>>(),
            vec!["c", "d", "leaf"]
        );
        assert_eq!(
            rejected["a"]["unconverted_dependencies"],
            serde_json::json!(["b"])
        );
        assert_eq!(
            rejected["b"]["unconverted_dependencies"],
            serde_json::json!(["missing"])
        );
    }
    #[test]
    fn duplicate_owners_revoke_every_definition_and_all_dependent_callers() {
        let owners = BTreeMap::from([
            ("duplicate".into(), vec!["a.rs".into(), "b.rs".into()]),
            ("leaf".into(), vec!["a.rs".into()]),
        ]);
        let mut accepted = BTreeMap::from([
            ("duplicate".into(), candidate(&[])),
            ("caller".into(), candidate(&["duplicate"])),
            ("leaf".into(), candidate(&[])),
        ]);
        let mut rejected = BTreeMap::new();
        let duplicates = reject_duplicates(&owners, &mut accepted, &mut rejected);
        close_graph(&mut accepted, &mut rejected);
        assert_eq!(
            accepted.keys().map(String::as_str).collect::<Vec<_>>(),
            vec!["leaf"]
        );
        assert_eq!(duplicates["duplicate"], vec!["a.rs", "b.rs"]);
        assert!(rejected["duplicate"]["duplicate_definitions"].is_array());
        assert_eq!(
            rejected["caller"]["unconverted_dependencies"],
            serde_json::json!(["duplicate"])
        );
    }
    #[test]
    fn emission_preserves_cross_module_native_calls_unknown_entries_and_source_literals() {
        let root =
            std::env::temp_dir().join(format!("gamepkg-batch-emission-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let output = root.join("source");
        let mut candidates = BTreeMap::new();
        for index in 0..9 {
            let name = format!("sub_{:08X}", 0x100 + index);
            let body = if index == 0 {
                "crate::game::sub_00000108(native_state,guest_ram)?;native_state.value+=1;"
                    .to_owned()
            } else {
                format!("native_state.value+={index};")
            };
            candidates.insert(name.clone(),Candidate{unit:"test.rs".into(),runtime_calls:0,code:format!("pub fn {name}(native_state:&mut crate::registers::Registers,guest_ram:&mut crate::memory::Memory)->Result<(),i32>{{{body}Ok(())}}"),dependencies:if index==0 {BTreeSet::from(["sub_00000108".into()])}else{BTreeSet::new()}});
        }
        let hashes = emit_closed(&candidates, &output).unwrap();
        assert_eq!(hashes.len(), 5);
        let first = fs::read_to_string(output.join("game/unit_0000.rs")).unwrap();
        assert!(first.contains("crate::game::unit_0001::sub_00000108"));
        let literal = "crate :: game :: sub_00000108";
        let mut function: syn::ItemFn =
            syn::parse_str(&format!("fn sample(){{let text=\"{literal}\";}} ")).unwrap();
        Remap(&BTreeMap::from([("sub_00000108".into(), 1)])).visit_item_fn_mut(&mut function);
        assert!(function.to_token_stream().to_string().contains(literal));
        let harness = r#"#![allow(dead_code,unused_variables)]
mod game;mod dispatch;
mod memory{pub struct Memory;}
mod registers{pub struct Registers{pub value:u32,pub allowed:bool}impl Registers{pub fn valid(&self)->bool{self.allowed}}}
fn main(){let mut state=registers::Registers{value:0,allowed:true};let mut ram=memory::Memory;
assert_eq!(dispatch::invoke(&mut state,&mut ram,0x100),Ok(()));assert_eq!(state.value,9);
assert_eq!(dispatch::invoke(&mut state,&mut ram,0x99),Err(-5));assert_eq!(dispatch::invoke(&mut state,&mut ram,0x109),Err(-5));assert_eq!(state.value,9);
state.allowed=false;assert_eq!(dispatch::invoke(&mut state,&mut ram,0x108),Err(-1));assert_eq!(state.value,9);}
"#;
        fs::write(output.join("main.rs"), harness).unwrap();
        let executable = root.join("native-router");
        assert!(std::process::Command::new("rustc")
            .args(["--edition=2021", "-D", "warnings"])
            .arg(output.join("main.rs"))
            .arg("-o")
            .arg(&executable)
            .status()
            .unwrap()
            .success());
        assert!(std::process::Command::new(&executable)
            .status()
            .unwrap()
            .success());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn pinned_inputs_cannot_be_changed_symlinked_or_written_to_an_existing_output() {
        let root = std::env::temp_dir().join(format!("gamepkg-batch-unit-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let path = root.join("recomp_test.rs");
        fs::write(&path, b"test").unwrap();
        let hash = format!("{:x}", Sha256::digest(b"test"));
        assert_eq!(read_unit(&root, "recomp_test.rs", &hash).unwrap(), b"test");
        assert!(read_unit(&root, "recomp_test.rs", &"0".repeat(64)).is_err());
        assert!(run(&root, &root, false).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&path, root.join("alias.rs")).unwrap();
            assert!(read_unit(&root, "alias.rs", &hash).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
