//! Maintainer-only AST converter, not a runtime or game dependency.
use proc_macro2::Span;
use quote::{format_ident, quote, ToTokens};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
use syn::{
    parse_quote,
    visit_mut::{self, VisitMut},
    Expr, Item, ItemFn, Stmt, Type,
};

fn plain(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(p) => plain(&p.expr),
        Expr::Group(g) => plain(&g.expr),
        _ => expr,
    }
}
fn path(expr: &Expr) -> Option<String> {
    if let Expr::Path(p) = plain(expr) {
        if p.qself.is_none() {
            return Some(
                p.path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::"),
            );
        }
    }
    None
}
fn primitive(ty: &Type) -> Option<&'static str> {
    let name = ty.to_token_stream().to_string().replace(' ', "");
    Some(match name.as_str() {
        "uint8_t" | "u8" | "::core::ffi::c_uchar" => "u8",
        "int8_t" | "i8" | "::core::ffi::c_schar" => "i8",
        "uint16_t" | "u16" | "::core::ffi::c_ushort" => "u16",
        "int16_t" | "i16" | "::core::ffi::c_short" => "i16",
        "uint32_t" | "u32" | "::core::ffi::c_uint" => "u32",
        "int32_t" | "i32" | "::core::ffi::c_int" => "i32",
        "uint64_t" | "u64" | "::core::ffi::c_ulonglong" => "u64",
        "int64_t" | "i64" | "::core::ffi::c_longlong" => "i64",
        "f32" | "::core::ffi::c_float" => "f32",
        "f64" | "::core::ffi::c_double" => "f64",
        _ => return None,
    })
}
fn address(expr: &Expr) -> Option<Expr> {
    // Accept precisely the pinned C2Rust guest-address + host-relocation idiom.
    // Native casts, pointer arithmetic and other provenance expressions reject.
    let Expr::Call(pointer) = plain(expr) else {
        return None;
    };
    if !matches!(
        path(&pointer.func)?.as_str(),
        "core::ptr::with_exposed_provenance_mut" | "core::ptr::with_exposed_provenance"
    ) || pointer.args.len() != 1
    {
        return None;
    }
    let Expr::Cast(outer) = plain(pointer.args.first()?) else {
        return None;
    };
    if outer.ty.to_token_stream().to_string() != "usize" {
        return None;
    }
    let Expr::MethodCall(add) = plain(&outer.expr) else {
        return None;
    };
    if add.method != "wrapping_add" || add.args.len() != 1 {
        return None;
    }
    let Expr::Cast(offset) = plain(add.args.first()?) else {
        return None;
    };
    if path(&offset.expr).as_deref() != Some("g_xbox_mem_offset")
        || offset.ty.to_token_stream().to_string() != "uintptr_t"
    {
        return None;
    }
    let Expr::Cast(base) = plain(&add.receiver) else {
        return None;
    };
    if base.ty.to_token_stream().to_string() != "uintptr_t" {
        return None;
    }
    Some(*base.expr.clone())
}
fn pointer_primitive(expr: &Expr) -> Option<&'static str> {
    let Expr::Call(call) = plain(expr) else {
        return None;
    };
    let Expr::Path(p) = plain(&call.func) else {
        return None;
    };
    let syn::PathArguments::AngleBracketed(args) = &p.path.segments.last()?.arguments else {
        return None;
    };
    if args.args.len() != 1 {
        return None;
    }
    let syn::GenericArgument::Type(ty) = args.args.first()? else {
        return None;
    };
    primitive(ty)
}
#[derive(Default)]
struct Lower {
    tls: BTreeMap<String, String>,
    locals: BTreeMap<String, String>,
    dependencies: BTreeSet<String>,
    errors: BTreeSet<String>,
}
impl Lower {
    fn reject(&mut self, message: impl Into<String>) {
        self.errors.insert(message.into());
    }
    fn tls_local(&mut self, statement: &Stmt) -> bool {
        let Stmt::Local(local) = statement else {
            return false;
        };
        let syn::Pat::Ident(binding) = &local.pat else {
            return false;
        };
        let Some(init) = &local.init else {
            return false;
        };
        let Expr::Unsafe(block) = &*init.expr else {
            return false;
        };
        if block.block.stmts.len() != 1 {
            return false;
        }
        let Stmt::Expr(Expr::Call(call), None) = &block.block.stmts[0] else {
            return false;
        };
        let Some(name) = path(&call.func) else {
            return false;
        };
        if !name.starts_with("gamepkg_tls_g_") {
            return false;
        }
        let field = name.trim_start_matches("gamepkg_tls_g_");
        if !call.args.is_empty() || binding.ident != format!("__{name}") {
            self.reject("unreviewed TLS declaration");
            return false;
        }
        if ![
            "eax", "ebx", "ecx", "edx", "esi", "edi", "ebp", "esp", "seh_ebp", "df", "fp_stack",
            "fp_top", "fp_cc",
        ]
        .contains(&field)
        {
            self.reject(format!("TLS field {field}"));
            return false;
        }
        self.tls.insert(binding.ident.to_string(), field.into());
        true
    }
    fn guest_u32(&self, expr: &Expr) -> bool {
        match plain(expr) {
            Expr::Cast(c) => primitive(&c.ty) == Some("u32"),
            Expr::Unary(u) if matches!(u.op, syn::UnOp::Deref(_)) => path(&u.expr)
                .and_then(|n| self.tls.get(&n))
                .is_some_and(|n| {
                    [
                        "eax", "ebx", "ecx", "edx", "esi", "edi", "ebp", "esp", "seh_ebp",
                    ]
                    .contains(&n.as_str())
                }),
            Expr::Path(_) => path(expr)
                .and_then(|n| self.locals.get(&n))
                .is_some_and(|n| n == "u32"),
            Expr::MethodCall(m) => {
                [
                    "wrapping_add",
                    "wrapping_sub",
                    "wrapping_mul",
                    "wrapping_shl",
                    "wrapping_shr",
                ]
                .iter()
                .any(|n| m.method == *n)
                    && self.guest_u32(&m.receiver)
            }
            Expr::Binary(b) => self.guest_u32(&b.left) && self.guest_u32(&b.right),
            Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Int(n),
                ..
            }) => n.suffix() == "u32",
            _ => false,
        }
    }
    fn memory_call(&mut self, call: &syn::ExprCall, name: &str) -> Option<Expr> {
        let read = name == "core::ptr::read_volatile";
        if call.args.len() != if read { 1 } else { 2 } {
            self.reject("memory arity");
            return None;
        }
        let pointer = call.args.first()?;
        let Some(mut addr) = address(pointer) else {
            self.reject("unreviewed guest pointer");
            return None;
        };
        if !self.guest_u32(&addr) {
            self.reject("guest address not proven u32");
            return None;
        }
        let Some(ty) = pointer_primitive(pointer) else {
            self.reject("unreviewed memory width");
            return None;
        };
        if read {
            let Expr::Path(p) = plain(&call.func) else {
                return None;
            };
            let syn::PathArguments::AngleBracketed(args) = &p.path.segments.last()?.arguments
            else {
                self.reject("missing load width");
                return None;
            };
            if args.args.len() != 1
                || !matches!(args.args.first(), Some(syn::GenericArgument::Type(t)) if primitive(t)==Some(ty))
            {
                self.reject("mismatched load/pointer width");
                return None;
            }
        }
        self.visit_expr_mut(&mut addr);
        let method = format_ident!("{}_{}", if read { "read" } else { "write" }, ty);
        if read {
            Some(parse_quote!(guest_ram.#method((#addr) as u32)?))
        } else {
            let mut value = call.args[1].clone();
            self.visit_expr_mut(&mut value);
            Some(parse_quote!(guest_ram.#method((#addr) as u32, #value)?))
        }
    }
}
impl VisitMut for Lower {
    fn visit_macro_mut(&mut self, _: &mut syn::Macro) {
        self.reject("macro");
    }
    fn visit_attribute_mut(&mut self, _: &mut syn::Attribute) {
        self.reject("nested attribute");
    }
    fn visit_item_mut(&mut self, _: &mut syn::Item) {
        self.reject("nested item or alias");
    }
    fn visit_local_mut(&mut self, local: &mut syn::Local) {
        if let syn::Pat::Type(p) = &local.pat {
            if let syn::Pat::Ident(binding) = &*p.pat {
                if let Some(ty) = primitive(&p.ty) {
                    self.locals.insert(binding.ident.to_string(), ty.into());
                }
            }
        }
        visit_mut::visit_local_mut(self, local);
    }
    fn visit_block_mut(&mut self, block: &mut syn::Block) {
        block.stmts.retain(|stmt| !self.tls_local(stmt));
        visit_mut::visit_block_mut(self, block);
    }
    fn visit_type_mut(&mut self, ty: &mut Type) {
        if let Some(name) = primitive(ty) {
            *ty = syn::parse_str(name).unwrap();
            return;
        }
        match ty {
            Type::Ptr(_) | Type::BareFn(_) => self.reject("raw/function pointer type"),
            Type::Path(p)
                if p.path.segments.iter().any(|s| {
                    matches!(
                        s.ident.to_string().as_str(),
                        "uintptr_t" | "ptrdiff_t" | "size_t" | "RecompXmm" | "RecompMmx"
                    )
                }) =>
            {
                self.reject(format!("unreviewed type {}", p.to_token_stream()))
            }
            _ => {}
        }
        visit_mut::visit_type_mut(self, ty);
    }
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        if let Expr::Unary(unary) = expr {
            if matches!(unary.op, syn::UnOp::Deref(_)) {
                if let Some(field) = path(&unary.expr).and_then(|n| self.tls.get(&n).cloned()) {
                    let name = syn::Ident::new(&field, Span::call_site());
                    *expr = parse_quote!(native_state.#name);
                    return;
                }
                self.reject("unreviewed dereference");
            }
        }
        if let Expr::Unsafe(block) = expr {
            let mut body = block.block.clone();
            self.visit_block_mut(&mut body);
            *expr = Expr::Block(syn::ExprBlock {
                attrs: vec![],
                label: None,
                block: body,
            });
            return;
        }
        if let Expr::Return(ret) = expr {
            if let Some(value) = &mut ret.expr {
                self.visit_expr_mut(value);
                *expr = parse_quote!(return {#value; Ok(())});
            } else {
                *expr = parse_quote!(return Ok(()));
            }
            return;
        }
        if let Expr::Call(call) = expr {
            let Some(name) = path(&call.func) else {
                self.reject("indirect call");
                return;
            };
            if matches!(
                name.as_str(),
                "core::ptr::read_volatile" | "core::ptr::write_volatile"
            ) {
                if let Some(lowered) = self.memory_call(call, &name) {
                    *expr = lowered;
                }
                return;
            }
            if name.starts_with("sub_")
                && name[4..].bytes().all(|b| b.is_ascii_hexdigit())
                && call.args.is_empty()
            {
                self.dependencies.insert(name.clone());
                let callee = format_ident!("{name}");
                *expr = parse_quote!(crate::game::#callee(native_state,guest_ram)?);
                return;
            }
            if [
                "fabs", "fabsf", "sin", "sinf", "cos", "cosf", "sqrt", "sqrtf",
            ]
            .contains(&name.as_str())
                && call.args.len() == 1
            {
                let mut value = call.args[0].clone();
                self.visit_expr_mut(&mut value);
                let method = format_ident!(
                    "{}",
                    match name.as_str() {
                        "fabs" | "fabsf" => "abs",
                        "sinf" => "sin",
                        "cosf" => "cos",
                        "sqrtf" => "sqrt",
                        other => other,
                    }
                );
                *expr = parse_quote!((#value).#method());
                return;
            }
            self.reject(format!("unreviewed call {name}"));
        }
        match expr {
            Expr::RawAddr(_) | Expr::Macro(_) => self.reject("raw address or macro"),
            Expr::Path(p)
                if p.path
                    .segments
                    .first()
                    .is_some_and(|s| s.ident.to_string().starts_with("g_")) =>
            {
                self.reject(format!("unreviewed global {}", p.to_token_stream()))
            }
            _ => {}
        }
        visit_mut::visit_expr_mut(self, expr);
    }
}
fn lower(mut function: ItemFn) -> Result<(ItemFn, BTreeSet<String>), BTreeSet<String>> {
    let mut pass = Lower::default();
    for attr in &function.attrs {
        if !attr.path().is_ident("no_mangle") && !attr.path().is_ident("inline") {
            pass.reject("unreviewed function attribute");
        }
    }
    if !function.sig.inputs.is_empty()
        || function.sig.variadic.is_some()
        || !matches!(function.sig.output, syn::ReturnType::Default)
    {
        pass.reject("unreviewed signature");
    }
    pass.visit_block_mut(&mut function.block);
    if !pass.errors.is_empty() {
        return Err(pass.errors);
    }
    function.attrs.clear();
    function.sig.unsafety = None;
    function.sig.abi = None;
    function.sig.inputs = parse_quote!(native_state:&mut crate::registers::Registers,guest_ram:&mut crate::memory::Memory);
    function.sig.output = parse_quote!(->Result<(),i32>);
    function
        .block
        .stmts
        .push(Stmt::Expr(parse_quote!(Ok(())), None));
    Ok((function, pass.dependencies))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 3 {
        return Err("requires input-unit.rs new-output-directory".into());
    }
    let source = fs::read(&args[1])?;
    let source_hash = format!("{:x}", Sha256::digest(&source));
    if source_hash != "442f7812516f3c5157ee74e15aede23d0ac423105acb7d973733bbe9f01ad94d" {
        return Err("unreviewed input unit hash".into());
    }
    let ast = syn::parse_file(std::str::from_utf8(&source)?)?;
    let mut original = BTreeMap::new();
    let mut accepted = BTreeMap::new();
    let mut rejected = BTreeMap::<String, Vec<String>>::new();
    for item in ast.items {
        let Item::Fn(function) = item else {
            continue;
        };
        let name = function.sig.ident.to_string();
        if !name.starts_with("sub_") || !name[4..].bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        if original.insert(name.clone(), function.clone()).is_some() {
            return Err("duplicate generated definition".into());
        }
        match lower(function) {
            Ok((function, dependencies)) => {
                accepted.insert(name, (function, dependencies));
            }
            Err(errors) => {
                rejected.insert(name, errors.into_iter().collect());
            }
        }
    }
    loop {
        let missing = accepted
            .iter()
            .filter_map(|(name, (_, deps))| {
                let absent = deps
                    .iter()
                    .filter(|n| !accepted.contains_key(*n))
                    .cloned()
                    .collect::<Vec<_>>();
                (!absent.is_empty()).then(|| (name.clone(), absent))
            })
            .collect::<Vec<_>>();
        if missing.is_empty() {
            break;
        }
        for (name, absent) in missing {
            accepted.remove(&name);
            rejected.insert(
                name,
                vec![format!("unconverted dependencies: {}", absent.join(","))],
            );
        }
    }
    let output = Path::new(&args[2]);
    fs::create_dir(output)?;
    let originals = accepted.keys().map(|n| &original[n]).collect::<Vec<_>>();
    fs::write(output.join("oracle.rs"), quote!(#(#originals)*).to_string())?;
    let mut routes = Vec::new();
    for (index, chunk) in accepted.keys().collect::<Vec<_>>().chunks(12).enumerate() {
        let router = format_ident!("route_{index}");
        let addresses = chunk
            .iter()
            .map(|n| u32::from_str_radix(&n[4..], 16).unwrap())
            .collect::<Vec<_>>();
        let names = chunk
            .iter()
            .map(|n| format_ident!("{n}"))
            .collect::<Vec<_>>();
        routes.push(quote!(fn #router(native_state:&mut crate::registers::Registers,guest_ram:&mut crate::memory::Memory,address:u32)->Result<(),i32>{match address{#(#addresses=>crate::game::#names(native_state,guest_ram),)*_=>Err(-5)}}));
    }
    let route_names = (0..routes.len())
        .map(|i| format_ident!("route_{i}"))
        .collect::<Vec<_>>();
    let dispatch = quote!(#(#routes)* pub fn invoke(native_state:&mut crate::registers::Registers,guest_ram:&mut crate::memory::Memory,address:u32)->Result<(),i32>{if !native_state.valid(){return Err(-1);}#(match #route_names(native_state,guest_ram,address){Err(-5)=>{},result=>return result})*Err(-5)});
    fs::write(output.join("dispatch.rs"), dispatch.to_string())?;
    let functions = accepted.values().map(|(f, _)| f).collect::<Vec<_>>();
    fs::write(
        output.join("game.rs"),
        format!(
            "// Maintainer-generated from pinned Takedawn; see LICENSE and provenance.json.\n{}\n",
            quote!(#(#functions)*)
        ),
    )?;
    let formatter = std::process::Command::new("rustfmt")
        .arg("--version")
        .output()?;
    if !formatter.status.success() {
        return Err("rustfmt version check failed".into());
    }
    if !std::process::Command::new("rustfmt")
        .args(["--edition", "2021"])
        .arg(output.join("game.rs"))
        .arg(output.join("dispatch.rs"))
        .status()?
        .success()
    {
        return Err("generated Rust formatting failed".into());
    }
    let provenance = serde_json::json!({"schema":1,"generator_source_sha256":format!("{:x}",Sha256::digest(include_bytes!("main.rs"))),"generator_lock_sha256":format!("{:x}",Sha256::digest(include_bytes!("../Cargo.lock"))),"formatter_version":String::from_utf8(formatter.stdout)?.trim(),"game_sha256":format!("{:x}",Sha256::digest(fs::read(output.join("game.rs"))?)),"dispatch_sha256":format!("{:x}",Sha256::digest(fs::read(output.join("dispatch.rs"))?)),"input_sha256":source_hash,"source_kit_sha256":"6b37d24c0984c686c7ed6fd73cd3a5b252b5631ceefa2798e5b7a6e0adff8dc4","converted":accepted.keys().collect::<Vec<_>>(),"rejected":rejected,"production_ready":false});
    fs::write(
        output.join("provenance.json"),
        serde_json::to_vec_pretty(&provenance)?,
    )?;
    println!(
        "{} converted function definitions; {} explicitly unconverted",
        accepted.len(),
        provenance["rejected"].as_object().unwrap().len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lowered(body: &str) -> Result<(ItemFn, BTreeSet<String>), BTreeSet<String>> {
        lower(syn::parse_str(&format!("pub unsafe extern \"C\" fn sub_100(){{{body}}}")).unwrap())
    }
    fn load(address: &str, width: &str) -> String {
        format!("::core::ptr::read_volatile::<{width}>(::core::ptr::with_exposed_provenance_mut::<uint32_t>((({address}) as uintptr_t).wrapping_add(g_xbox_mem_offset as uintptr_t) as usize));")
    }
    #[test]
    fn guest_addresses_require_proven_fixed_u32_width_and_exact_relocation_shape() {
        assert!(lowered(&load("0x1234 as uint32_t", "uint32_t")).is_ok());
        let errors = lowered(&format!(
            "let a:uint64_t=0x100001234;{}",
            load("a", "uint32_t")
        ))
        .unwrap_err();
        assert!(errors.contains("guest address not proven u32"));
        assert!(lowered(&load("0x1234 as uint32_t", "uint16_t"))
            .unwrap_err()
            .contains("mismatched load/pointer width"));
        assert!(lowered(
            &load("0x1234 as uint32_t", "uint32_t")
                .replace("g_xbox_mem_offset as uintptr_t", "1 as uintptr_t")
        )
        .is_err());
    }
    #[test]
    fn native_pointer_paths_unknown_tls_and_aliases_are_not_converted() {
        for body in [
            "let p:*mut u32=0 as *mut u32;*p=1;",
            "let other=unsafe{gamepkg_tls_g_eax()};",
            "let __gamepkg_tls_g_xmm0=unsafe{gamepkg_tls_g_xmm0()};",
            "let value=SDL_GetTicks();",
            "let f=some_function;f();",
            "let f=some_function;f.unwrap()();",
            "core::arch::asm!(\"nop\");",
            "type Hidden=u32;",
        ] {
            assert!(lowered(body).is_err(), "{body}");
        }
    }
    #[test]
    fn original_direct_calls_record_dependencies_instead_of_using_callbacks() {
        let (converted, deps) = lowered("sub_200();return;").unwrap();
        assert_eq!(deps, BTreeSet::from(["sub_200".into()]));
        assert!(converted
            .to_token_stream()
            .to_string()
            .contains("crate :: game :: sub_200"));
        assert!(converted.sig.unsafety.is_none() && converted.sig.abi.is_none());
    }
    #[test]
    fn only_original_zero_argument_signatures_and_known_attributes_are_candidates() {
        for source in [
            "pub unsafe extern \"C\" fn sub_100(x:u32){}",
            "#[cfg(windows)]pub unsafe extern \"C\" fn sub_100(){}",
            "pub unsafe extern \"C\" fn sub_100()->u32{0}",
        ] {
            assert!(lower(syn::parse_str(source).unwrap()).is_err());
        }
    }
}
