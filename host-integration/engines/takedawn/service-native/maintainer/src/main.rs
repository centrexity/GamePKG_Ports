//! Maintainer-only AST converter, not a runtime or game dependency.
mod batch;
#[cfg(test)]
mod call_reference;
mod icall;
mod icall_compacted;
use proc_macro2::Span;
use quote::{format_ident, quote, ToTokens};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    sync::Arc,
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
fn small_integer(expr: &Expr) -> Option<u32> {
    match plain(expr) {
        Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(lit),
            ..
        }) => lit.base10_parse().ok(),
        Expr::Cast(c) if primitive(&c.ty).is_some() => small_integer(&c.expr),
        _ => None,
    }
}
fn plain_callee(call: &syn::ExprCall) -> bool {
    matches!(plain(&call.func),Expr::Path(p) if p.qself.is_none() && p.path.leading_colon.is_none() && p.path.segments.len()==1 && matches!(p.path.segments[0].arguments,syn::PathArguments::None))
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
    reviewed_indexed_copy: bool,
    thread_patch_pattern: Option<syn::Block>,
    tls: BTreeMap<String, String>,
    locals: BTreeMap<String, String>,
    enums: BTreeMap<String, Arc<BTreeMap<String, u32>>>,
    dependencies: BTreeSet<String>,
    errors: BTreeSet<String>,
}
#[derive(Clone, Copy)]
enum FlowLabel<'a> {
    Block(&'a str),
    Loop(&'a str),
}
impl Lower {
    fn lower_thread_patch(&mut self, statements: &[Stmt]) -> Option<[Stmt; 2]> {
        let pattern = self.thread_patch_pattern.as_ref()?;
        if !statements.starts_with(&pattern.stmts)
            || self.locals.get("_icall_esp_1").map(String::as_str) != Some("u32")
            || self.locals.get("_pv_8").map(String::as_str) != Some("u32")
            || !self.locals.contains_key("c2rust_current_block_45")
            || [
                ("__gamepkg_tls_g_esi", "esi"),
                ("__gamepkg_tls_g_esp", "esp"),
                ("__gamepkg_tls_g_ebx", "ebx"),
            ]
            .iter()
            .any(|(binding, field)| self.tls.get(*binding).map(String::as_str) != Some(*field))
        {
            return None;
        }
        let successor = self.enum_variant(&parse_quote!(C2Rust_Block_45::S_658))?;
        self.thread_patch_pattern = None;
        let successor =
            syn::LitInt::new(&format!("{successor}u32"), proc_macro2::Span::call_site());
        Some([
            parse_quote!(_icall_esp_1 = crate::irq_delivery::irq_thread_patch_current(guest_ram, native_state)?;),
            parse_quote!(c2rust_current_block_45 = #successor;),
        ])
    }
    fn lower_block(&mut self, block: &mut syn::Block, label: Option<FlowLabel<'_>>) {
        let outer_locals = self.locals.clone();
        let outer_tls = self.tls.clone();
        let outer_enums = self.enums.clone();
        let mut statements = Vec::new();
        let mut remaining = std::mem::take(&mut block.stmts).into_iter();
        while !remaining.as_slice().is_empty() {
            if let Some(statement) = icall::rewrite_flat_jump(remaining.as_slice(), self) {
                statements.push(statement);
                break;
            }
            if let Some(replacement) = self.lower_thread_patch(remaining.as_slice()) {
                for _ in 0..7 {
                    remaining.next();
                }
                statements.extend(replacement);
                continue;
            }
            if let Some(statement) = icall::rewrite_jump(remaining.as_slice(), self) {
                remaining.next();
                statements.push(statement);
                continue;
            }
            if let Some(replacement) = icall::rewrite_leaving(remaining.as_slice(), self) {
                for _ in 0..10 {
                    remaining.next();
                }
                statements.extend(replacement);
                continue;
            }
            if let Some(label) = label {
                let replacement = match label {
                    FlowLabel::Block(name) => icall::rewrite_tail(remaining.as_slice(), self, name),
                    FlowLabel::Loop(name) => {
                        icall::rewrite_loop_tail(remaining.as_slice(), self, name)
                    }
                };
                if let Some(statement) = replacement {
                    for _ in 0..9 {
                        remaining.next();
                    }
                    statements.push(statement);
                    continue;
                }
            }

            if let Some(statement) = icall::rewrite(remaining.as_slice(), self) {
                for _ in 0..4 {
                    remaining.next();
                }
                statements.push(statement);
                continue;
            }
            let mut statement = remaining.next().unwrap();
            if self.tls_local(&statement) || self.enum_local(&statement) {
                continue;
            }
            self.visit_stmt_mut(&mut statement);
            statements.push(statement);
        }
        block.stmts = statements;
        self.locals = outer_locals;
        self.tls = outer_tls;
        self.enums = outer_enums;
    }
    fn reject(&mut self, message: impl Into<String>) {
        self.errors.insert(message.into());
    }
    fn enum_local(&mut self, statement: &Stmt) -> bool {
        let Stmt::Item(Item::Enum(item)) = statement else {
            return false;
        };
        let name = item.ident.to_string();
        let decimal = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
        if !name.strip_prefix("C2Rust_Block_").is_some_and(decimal)
            || !item.attrs.is_empty()
            || !matches!(item.vis, syn::Visibility::Inherited)
            || !item.generics.params.is_empty()
            || item.generics.where_clause.is_some()
            || item.variants.is_empty()
            || item.variants.len() > 65536
        {
            self.reject("unreviewed control-flow enum");
            return false;
        }
        let mut variants = BTreeMap::new();
        for (ordinal, variant) in item.variants.iter().enumerate() {
            let id = variant.ident.to_string();
            if !id.strip_prefix("S_").is_some_and(decimal)
                || !variant.attrs.is_empty()
                || !matches!(variant.fields, syn::Fields::Unit)
                || variant.discriminant.is_some()
                || variants.insert(id, ordinal as u32).is_some()
            {
                self.reject("unreviewed control-flow variant");
                return false;
            }
        }
        self.enums.insert(name, Arc::new(variants));
        true
    }
    fn enum_variant(&self, expression: &syn::Path) -> Option<u32> {
        if expression.leading_colon.is_some()
            || expression.segments.len() != 2
            || expression
                .segments
                .iter()
                .any(|s| !matches!(s.arguments, syn::PathArguments::None))
        {
            return None;
        }
        let owner = expression.segments[0].ident.to_string();
        self.enums
            .get(&owner)?
            .get(&expression.segments[1].ident.to_string())
            .copied()
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
            "eax",
            "ebx",
            "ecx",
            "edx",
            "esi",
            "edi",
            "ebp",
            "esp",
            "seh_ebp",
            "fs_base",
            "df",
            "eflags",
            "fp_cmp",
            "fp_stack",
            "fp_top",
            "fp_cc",
            "fp_control_word",
            "xmm0",
            "xmm1",
            "xmm2",
            "xmm3",
            "xmm4",
            "xmm5",
            "xmm6",
            "xmm7",
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
                        "fs_base",
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
            Expr::Binary(b) => {
                matches!(
                    b.op,
                    syn::BinOp::Add(_)
                        | syn::BinOp::Sub(_)
                        | syn::BinOp::Mul(_)
                        | syn::BinOp::Div(_)
                        | syn::BinOp::Rem(_)
                        | syn::BinOp::BitXor(_)
                        | syn::BinOp::BitAnd(_)
                        | syn::BinOp::BitOr(_)
                        | syn::BinOp::Shl(_)
                        | syn::BinOp::Shr(_)
                ) && self.guest_u32(&b.left)
                    && self.guest_u32(&b.right)
            }
            Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Int(n),
                ..
            }) => n.suffix() == "u32",
            _ => false,
        }
    }
    fn float32(&self, expr: &Expr) -> bool {
        if self
            .vector_lane(expr)
            .is_some_and(|(_, _, view)| view == "f")
        {
            return true;
        }
        match plain(expr) {
            Expr::Cast(c) => primitive(&c.ty) == Some("f32"),
            Expr::Path(_) => path(expr)
                .and_then(|n| self.locals.get(&n))
                .is_some_and(|n| n == "f32"),
            Expr::Binary(b)
                if matches!(
                    b.op,
                    syn::BinOp::Add(_) | syn::BinOp::Sub(_) | syn::BinOp::Mul(_)
                ) =>
            {
                self.float32(&b.left) && self.float32(&b.right)
            }
            Expr::Call(c) => path(&c.func).as_deref() == Some("sqrtf"),
            _ => false,
        }
    }
    fn vector_value(&self, expr: &Expr) -> bool {
        match plain(expr) {
            Expr::Unary(u) if matches!(u.op, syn::UnOp::Deref(_)) => path(&u.expr)
                .and_then(|n| self.tls.get(&n))
                .is_some_and(|n| n.starts_with("xmm")),
            Expr::Path(_) => path(expr)
                .and_then(|n| self.locals.get(&n))
                .is_some_and(|n| n == "xmm"),
            _ => false,
        }
    }
    fn vector_lane(&self, expr: &Expr) -> Option<(Expr, Expr, String)> {
        let Expr::Index(index) = plain(expr) else {
            return None;
        };
        let Expr::Field(field) = plain(&index.expr) else {
            return None;
        };
        if !self.vector_value(&field.base) {
            return None;
        }
        let syn::Member::Named(view) = &field.member else {
            return None;
        };
        Some((*field.base.clone(), *index.index.clone(), view.to_string()))
    }
    fn control_u16(&self, expr: &Expr) -> bool {
        match plain(expr) {
            Expr::Cast(c) => primitive(&c.ty) == Some("u16"),
            Expr::Path(_) => path(expr)
                .and_then(|n| self.locals.get(&n))
                .is_some_and(|n| n == "u16"),
            Expr::Unary(u) if matches!(u.op, syn::UnOp::Deref(_)) => path(&u.expr)
                .and_then(|n| self.tls.get(&n))
                .is_some_and(|n| n == "fp_control_word"),
            Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Int(lit),
                ..
            }) => lit.suffix() == "u16",
            _ => false,
        }
    }
    fn scalar_call(&mut self, call: &syn::ExprCall, name: &str) -> Option<Expr> {
        if !call.attrs.is_empty()
            || !plain_callee(call)
            || self.locals.contains_key(name)
            || self.tls.contains_key(name)
        {
            self.reject("unproven scalar helper binding");
            return None;
        }
        if name == "recomp_fist" || name == "recomp_frndint" {
            let expected = if name == "recomp_fist" { 3 } else { 2 };
            if call.args.len() != expected || !self.control_u16(&call.args[1]) {
                self.reject("integer rounding arity or control-word width");
                return None;
            }
            let width = if name == "recomp_fist" {
                let Some(width) = small_integer(&call.args[2]) else {
                    self.reject("integer-store width must be a reviewed constant");
                    return None;
                };
                if ![16, 32, 64].contains(&width) {
                    self.reject("integer-store width");
                    return None;
                }
                width
            } else {
                0
            };
            let mut value = call.args[0].clone();
            let mut control = call.args[1].clone();
            self.visit_expr_mut(&mut value);
            self.visit_expr_mut(&mut control);
            return Some(if name == "recomp_fist" {
                parse_quote!(crate::scalar::integer_store(#value,#control,#width)?)
            } else {
                parse_quote!(crate::scalar::round_integer(#value,#control))
            });
        }
        if name == "RC_ROT" {
            if call.args.len() != 5
                || !self.guest_u32(&call.args[0])
                || !self.guest_u32(&call.args[1])
            {
                self.reject("rotate-carry arity or value width");
                return None;
            }
            let (Some(width), Some(direction)) =
                (small_integer(&call.args[3]), small_integer(&call.args[4]))
            else {
                self.reject("rotate-carry width/direction must be reviewed constants");
                return None;
            };
            if ![8, 16, 32].contains(&width) || direction > 1 {
                self.reject("rotate-carry width/direction");
                return None;
            }
            let Expr::RawAddr(reference) = plain(&call.args[2]) else {
                self.reject("rotate-carry requires exact local mutable carry");
                return None;
            };
            let Some(binding) = path(&reference.expr) else {
                self.reject("rotate-carry carry is not local");
                return None;
            };
            if !reference.attrs.is_empty()
                || !matches!(plain(&reference.expr),Expr::Path(p) if p.qself.is_none() && p.attrs.is_empty() && p.path.segments.len()==1 && matches!(p.path.segments[0].arguments,syn::PathArguments::None))
                || !matches!(reference.mutability, syn::PointerMutability::Mut(_))
                || self.locals.get(&binding).map(String::as_str) != Some("i32")
            {
                self.reject("rotate-carry carry type or rights");
                return None;
            }
            let mut value = call.args[0].clone();
            let mut count = call.args[1].clone();
            self.visit_expr_mut(&mut value);
            self.visit_expr_mut(&mut count);
            let carry = syn::Ident::new(&binding, Span::call_site());
            let left = direction == 1;
            return Some(
                parse_quote!(crate::scalar::rotate_carry(#value,#count,&mut #carry,#width,#left)?),
            );
        }
        if call.args.len() != 1 {
            self.reject("scalar helper arity");
            return None;
        }
        if name == "recomp_parity8" && !self.guest_u32(&call.args[0]) {
            self.reject("parity value not proven u32");
            return None;
        }
        let mut value = call.args[0].clone();
        self.visit_expr_mut(&mut value);
        Some(if name == "recomp_parity8" {
            parse_quote!(crate::scalar::parity8(#value))
        } else {
            parse_quote!(crate::scalar::round24(#value))
        })
    }
    fn pointer_name(&self, expr: &Expr) -> Option<String> {
        let Expr::Path(p) = plain(expr) else {
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
        let name = p.path.segments[0].ident.to_string();
        (self.locals.get(&name).map(String::as_str) == Some("guest-byte-pointer")).then_some(name)
    }
    fn pointer_value(&mut self, expr: &Expr) -> Option<Expr> {
        if self.pointer_name(expr).is_some() {
            return Some(expr.clone());
        }
        let Expr::Call(call) = plain(expr) else {
            return None;
        };
        if !call.attrs.is_empty() || pointer_primitive(expr) != Some("u8") {
            return None;
        }
        let name = path(&call.func)?;
        if name == "core::ptr::null_mut" && call.args.is_empty() {
            return Some(parse_quote!(None));
        }
        if !matches!(
            name.as_str(),
            "core::ptr::with_exposed_provenance_mut" | "core::ptr::with_exposed_provenance"
        ) {
            return None;
        }
        let mut value = address(expr)?;
        if !self.guest_u32(&value) {
            return None;
        }
        self.visit_expr_mut(&mut value);
        Some(parse_quote!(Some(#value)))
    }
    fn pointer_index(&mut self, expr: &Expr) -> Option<(syn::Ident, Expr)> {
        let Expr::MethodCall(call) = plain(expr) else {
            return None;
        };
        if call.method != "offset"
            || call.args.len() != 1
            || !call.attrs.is_empty()
            || call.turbofish.is_some()
        {
            return None;
        }
        let name = self.pointer_name(&call.receiver)?;
        let Expr::Cast(count) = plain(&call.args[0]) else {
            return None;
        };
        if count.ty.to_token_stream().to_string() != "isize" || !self.guest_u32(&count.expr) {
            return None;
        }
        let mut length = *count.expr.clone();
        self.visit_expr_mut(&mut length);
        Some((format_ident!("{name}"), length))
    }
    fn pointer_operand(&mut self, expr: &Expr) -> Option<Expr> {
        if let Some(name) = self.pointer_name(expr) {
            let name = format_ident!("{name}");
            return Some(parse_quote!(crate::memory::guest_position(#name)?));
        }
        let (name, length) = self.pointer_index(expr)?;
        Some(parse_quote!(crate::memory::guest_offset(#name,#length)?))
    }
    fn indexed_byte(&mut self, expr: &Expr) -> Option<Expr> {
        if !self.reviewed_indexed_copy {
            return None;
        }
        let Expr::Unary(unary) = plain(expr) else {
            return None;
        };
        if !unary.attrs.is_empty() || !matches!(unary.op, syn::UnOp::Deref(_)) {
            return None;
        }
        let (name, length) = self.pointer_index(&unary.expr)?;
        Some(parse_quote!(crate::memory::guest_byte_address(#name,#length)?))
    }
    fn fill_call(&mut self, call: &syn::ExprCall) -> Option<Expr> {
        struct Attributes(bool);
        impl VisitMut for Attributes {
            fn visit_attribute_mut(&mut self, _: &mut syn::Attribute) {
                self.0 = true;
            }
        }
        let mut attributes = Attributes(false);
        attributes.visit_expr_call_mut(&mut call.clone());
        if attributes.0 {
            return None;
        }
        if !plain_callee(call)
            || !call.attrs.is_empty()
            || call.args.len() != 3
            || self.locals.contains_key("memset")
            || self.tls.contains_key("memset")
            || self.locals.contains_key("g_xbox_mem_offset")
            || self.tls.contains_key("g_xbox_mem_offset")
        {
            return None;
        }
        let Expr::Call(pointer) = plain(&call.args[0]) else {
            return None;
        };
        if path(&pointer.func).as_deref() != Some("core::ptr::with_exposed_provenance_mut")
            || !pointer.attrs.is_empty()
        {
            return None;
        }
        let Expr::Path(function) = plain(&pointer.func) else {
            return None;
        };
        let syn::PathArguments::AngleBracketed(arguments) =
            &function.path.segments.last()?.arguments
        else {
            return None;
        };
        if arguments.args.len() != 1
            || !matches!(arguments.args.first(),Some(syn::GenericArgument::Type(ty)) if ty.to_token_stream().to_string().replace(' ',"")=="::core::ffi::c_void")
        {
            return None;
        }
        let mut destination = address(&call.args[0])?;
        if !self.guest_u32(&destination) {
            return None;
        }
        let Expr::Cast(integer) = plain(&call.args[1]) else {
            return None;
        };
        let Expr::Cast(byte) = plain(&integer.expr) else {
            return None;
        };
        if primitive(&integer.ty) != Some("i32")
            || primitive(&byte.ty) != Some("u8")
            || !self.guest_u32(&byte.expr)
        {
            return None;
        }
        let Expr::Cast(length) = plain(&call.args[2]) else {
            return None;
        };
        if length.ty.to_token_stream().to_string() != "size_t" || !self.guest_u32(&length.expr) {
            return None;
        }
        let mut length = *length.expr.clone();
        let mut value = *byte.expr.clone();
        self.visit_expr_mut(&mut destination);
        self.visit_expr_mut(&mut length);
        self.visit_expr_mut(&mut value);
        Some(parse_quote!(guest_ram.fill(#destination,(#value) as u8,#length)?))
    }
    fn copy_call(&mut self, call: &syn::ExprCall) -> Option<Expr> {
        if !plain_callee(call)
            || !call.attrs.is_empty()
            || call.args.len() != 3
            || self.locals.contains_key("memcpy")
            || self.tls.contains_key("memcpy")
        {
            return None;
        }
        let mut addresses = Vec::new();
        for (index, argument) in call.args.iter().take(2).enumerate() {
            let Expr::Cast(cast) = plain(argument) else {
                return None;
            };
            let Type::Ptr(pointer) = &*cast.ty else {
                return None;
            };
            if pointer.mutability.is_some() != (index == 0)
                || pointer.elem.to_token_stream().to_string().replace(' ', "")
                    != "::core::ffi::c_void"
            {
                return None;
            }
            let name = self.pointer_name(&cast.expr)?;
            let name = format_ident!("{name}");
            addresses.push(parse_quote!(crate::memory::guest_address(#name)?));
        }
        let Expr::Cast(length) = plain(&call.args[2]) else {
            return None;
        };
        if length.ty.to_token_stream().to_string() != "size_t" || !self.guest_u32(&length.expr) {
            return None;
        }
        let mut length = *length.expr.clone();
        self.visit_expr_mut(&mut length);
        let destination: &Expr = &addresses[0];
        let source: &Expr = &addresses[1];
        Some(parse_quote!(guest_ram.copy_nonoverlapping(#destination,#source,#length)?))
    }
    fn vector_call(&mut self, call: &syn::ExprCall, name: &str) -> Option<Expr> {
        let (helper, count, access) = match name {
            "XMM_ZERO" => ("zero", 0, 0),
            "XMM_SCALAR" => ("scalar", 1, 0),
            "XMM_MEM" => ("load", 1, 1),
            "XMM_STORE" => ("store", 2, 2),
            "XMM_ADD" => ("add", 2, 0),
            "XMM_SUB" => ("subtract", 2, 0),
            "XMM_MUL" => ("multiply", 2, 0),
            "XMM_XOR" => ("xor", 2, 0),
            "XMM_SHUFFLE" => ("shuffle", 3, 0),
            _ => return None,
        };
        if call.args.len() != count {
            self.reject("vector helper arity");
            return None;
        }
        if access != 0 && !self.guest_u32(&call.args[0]) {
            self.reject("vector address not proven u32");
            return None;
        }
        let mut args = call.args.clone();
        for expr in &mut args {
            self.visit_expr_mut(expr);
        }
        let method = format_ident!("{helper}");
        Some(if access == 0 {
            parse_quote!(crate::vector::#method(#args))
        } else {
            parse_quote!(crate::vector::#method(guest_ram,#args)?)
        })
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
    fn visit_stmt_mut(&mut self, statement: &mut Stmt) {
        if let Stmt::Expr(Expr::Call(call), Some(_)) = statement {
            if path(&call.func).as_deref() == Some("recomp_debug_service") {
                struct Attributes(bool);
                impl VisitMut for Attributes {
                    fn visit_attribute_mut(&mut self, _: &mut syn::Attribute) {
                        self.0 = true;
                    }
                }
                let mut attributes = Attributes(false);
                attributes.visit_expr_call_mut(&mut call.clone());
                if !plain_callee(call)
                    || attributes.0
                    || call.args.len() != 2
                    || self.locals.contains_key("recomp_debug_service")
                    || self.tls.contains_key("recomp_debug_service")
                    || !call.args.iter().all(|a| self.guest_u32(a))
                {
                    self.reject("unreviewed debug-service call");
                    return;
                }
                let mut service = call.args[0].clone();
                let mut argument = call.args[1].clone();
                self.visit_expr_mut(&mut service);
                self.visit_expr_mut(&mut argument);
                *statement = parse_quote!(crate::debug::service(guest_ram,#service,#argument)?;);
                return;
            }
            if path(&call.func).as_deref() == Some("memset") {
                if let Some(value) = self.fill_call(call) {
                    *statement = Stmt::Expr(value, Some(Default::default()));
                } else {
                    self.reject("unreviewed guest memset arguments");
                }
                return;
            }
            if path(&call.func).as_deref() == Some("memcpy") {
                if let Some(value) = self.copy_call(call) {
                    *statement = Stmt::Expr(value, Some(Default::default()));
                } else {
                    self.reject("unreviewed guest memcpy arguments");
                }
                return;
            }
        }
        if let Stmt::Macro(m) = statement {
            if m.attrs.is_empty() && compacted_trap(&m.mac) {
                *statement = Stmt::Expr(parse_quote!(return Err(-1)), Some(Default::default()));
                return;
            }
        }
        visit_mut::visit_stmt_mut(self, statement);
    }
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
        if let syn::Pat::Type(pattern) = &local.pat {
            if let Type::Ptr(pointer) = &*pattern.ty {
                if pointer.mutability.is_some() && primitive(&pointer.elem) == Some("u8") {
                    let syn::Pat::Ident(binding) = &*pattern.pat else {
                        self.reject("guest pointer binding pattern");
                        return;
                    };
                    if !pattern.attrs.is_empty()
                        || !binding.attrs.is_empty()
                        || binding.by_ref.is_some()
                        || binding.subpat.is_some()
                    {
                        self.reject("guest pointer binding attributes or destructuring");
                        return;
                    }
                    let name = binding.ident.to_string();
                    let initializer = if let Some(init) = &local.init {
                        if init.diverge.is_some() {
                            self.reject("guest pointer divergence");
                            return;
                        }
                        let Some(value) = self.pointer_value(&init.expr) else {
                            self.reject("unproven guest pointer initializer");
                            return;
                        };
                        Some(value)
                    } else {
                        None
                    };
                    for attr in &mut local.attrs {
                        self.visit_attribute_mut(attr);
                    }
                    if let syn::Pat::Type(pattern) = &mut local.pat {
                        pattern.ty = Box::new(parse_quote!(Option<u32>));
                    }
                    if let (Some(init), Some(value)) = (&mut local.init, initializer) {
                        init.expr = Box::new(value);
                    }
                    self.locals
                        .insert(name.clone(), "guest-byte-pointer".into());
                    self.tls.remove(&name);
                    return;
                }
            }
        }
        let (name, ty) = match &local.pat {
            syn::Pat::Ident(binding) => (binding.ident.to_string(), None),
            syn::Pat::Type(p) => match &*p.pat {
                syn::Pat::Ident(binding) => (
                    binding.ident.to_string(),
                    primitive(&p.ty).or_else(|| {
                        (p.ty.to_token_stream().to_string() == "RecompXmm").then_some("xmm")
                    }),
                ),
                _ => {
                    self.reject("unreviewed binding pattern");
                    return;
                }
            },
            _ => {
                self.reject("unreviewed binding pattern");
                return;
            }
        };
        // A let initializer still sees its outer binding, not the new shadow.
        for attribute in &mut local.attrs {
            self.visit_attribute_mut(attribute);
        }
        if let Some(init) = &mut local.init {
            self.visit_expr_mut(&mut init.expr);
            if let Some((_, diverge)) = &mut init.diverge {
                self.visit_expr_mut(diverge);
            }
        }
        self.visit_pat_mut(&mut local.pat);
        self.locals
            .insert(name.clone(), ty.unwrap_or("unknown").into());
        self.tls.remove(&name);
    }
    fn visit_block_mut(&mut self, block: &mut syn::Block) {
        self.lower_block(block, None);
    }
    fn visit_expr_loop_mut(&mut self, node: &mut syn::ExprLoop) {
        for attribute in &mut node.attrs {
            self.visit_attribute_mut(attribute);
        }
        if let Some(label) = &node.label {
            let name = label.name.ident.to_string();
            self.lower_block(&mut node.body, Some(FlowLabel::Loop(&name)));
            if !icall::label_used(&mut node.body, &name) {
                node.label = None;
            }
        } else {
            self.visit_block_mut(&mut node.body);
        }
    }
    fn visit_type_mut(&mut self, ty: &mut Type) {
        if let Type::Path(p) = ty {
            if p.qself.is_none()
                && p.path.leading_colon.is_none()
                && p.path.segments.len() == 1
                && matches!(p.path.segments[0].arguments, syn::PathArguments::None)
                && self
                    .enums
                    .contains_key(&p.path.segments[0].ident.to_string())
            {
                *ty = parse_quote!(u32);
                return;
            }
        }
        if let Some(name) = primitive(ty) {
            *ty = syn::parse_str(name).unwrap();
            return;
        }
        if ty.to_token_stream().to_string() == "RecompXmm" {
            *ty = parse_quote!([u32; 4]);
            return;
        }
        match ty {
            Type::Ptr(_) | Type::BareFn(_) => self.reject("raw/function pointer type"),
            Type::Path(p)
                if p.path.segments.iter().any(|s| {
                    matches!(
                        s.ident.to_string().as_str(),
                        "uintptr_t"
                            | "ptrdiff_t"
                            | "size_t"
                            | "RecompXmm"
                            | "RecompMmx"
                            | "recomp_func_t"
                    )
                }) =>
            {
                self.reject(format!("unreviewed type {}", p.to_token_stream()))
            }
            _ => {}
        }
        visit_mut::visit_type_mut(self, ty);
    }
    fn visit_pat_mut(&mut self, pattern: &mut syn::Pat) {
        if let syn::Pat::Path(p) = pattern {
            if p.qself.is_none() && p.attrs.is_empty() {
                if let Some(value) = self.enum_variant(&p.path) {
                    let literal = syn::LitInt::new(&format!("{value}u32"), Span::call_site());
                    *pattern = parse_quote!(#literal);
                    return;
                }
            }
        }
        visit_mut::visit_pat_mut(self, pattern);
    }
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        if let Expr::Block(node) = expr {
            if node.attrs.is_empty() && node.label.is_some() {
                let name = node.label.as_ref().unwrap().name.ident.to_string();
                self.lower_block(&mut node.block, Some(FlowLabel::Block(&name)));
                if !icall::label_used(&mut node.block, &name) {
                    node.label = None;
                }
                return;
            }
        }
        let mut enum_fallback = false;
        let compacted_loop = if let Expr::Loop(node) = expr {
            if node.body.stmts.len() == 1 {
                if let Stmt::Expr(Expr::Match(m), _) = &node.body.stmts[0] {
                    matches!(plain(&m.expr),Expr::Path(p) if p.qself.is_none() && p.attrs.is_empty() && p.path.segments.len()==1 && p.path.segments[0].ident.to_string().starts_with("c2rust_current_block_"))
                        && m.arms.iter().any(|arm| enum_pattern(&arm.pat, self))
                        && m.arms.iter().all(|arm| arm.guard.is_none())
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };
        if let Expr::Assign(assignment) = expr {
            if let Some(destination) = self.indexed_byte(&assignment.left) {
                if let Some(source) = self.indexed_byte(&assignment.right) {
                    // Rust evaluates the source before the destination place.
                    *expr = parse_quote!(crate::memory::guest_store_byte(
                        guest_ram.read_u8(#source)?, #destination, guest_ram
                    )?);
                } else {
                    self.reject("unreviewed indexed guest byte source");
                }
                return;
            }
            if self.pointer_name(&assignment.left).is_some() {
                if let Some(value) = self.pointer_value(&assignment.right) {
                    assignment.right = Box::new(value);
                } else {
                    self.reject("unproven guest pointer assignment");
                }
                return;
            }
        }
        if let Expr::MethodCall(call) = expr {
            if let Some(name) = self.pointer_name(&call.receiver) {
                if call.method == "is_null"
                    && call.args.is_empty()
                    && call.turbofish.is_none()
                    && call.attrs.is_empty()
                {
                    let name = format_ident!("{name}");
                    *expr = parse_quote!(#name.is_none());
                    return;
                }
            }
        }
        if let Expr::Binary(binary) = expr {
            if matches!(
                binary.op,
                syn::BinOp::Lt(_) | syn::BinOp::Le(_) | syn::BinOp::Gt(_) | syn::BinOp::Ge(_)
            ) {
                let left = self.pointer_operand(&binary.left);
                let right = self.pointer_operand(&binary.right);
                if left.is_some() || right.is_some() {
                    if let (Some(left), Some(right)) = (left, right) {
                        binary.left = Box::new(left);
                        binary.right = Box::new(right);
                    } else {
                        self.reject("unproven guest pointer comparison");
                    }
                    return;
                }
            }
        }
        if self.pointer_name(expr).is_some() {
            self.reject("guest pointer escaped reviewed operations");
            return;
        }
        if let Expr::Path(p) = expr {
            if p.qself.is_none() && p.attrs.is_empty() {
                if let Some(value) = self.enum_variant(&p.path) {
                    let literal = syn::LitInt::new(&format!("{value}u32"), Span::call_site());
                    *expr = parse_quote!(#literal);
                    return;
                }
            }
        }
        if let Expr::Match(m) = expr {
            let enum_match = m.arms.iter().any(|arm| enum_pattern(&arm.pat, self));
            let last = m.arms.len().saturating_sub(1);
            for (index, arm) in m.arms.iter_mut().enumerate() {
                if index == last
                    && arm.guard.is_none()
                    && compacted_trap_body(&arm.body)
                    && enum_pattern(&arm.pat, self)
                {
                    // Every listed original impossible tag faults. Integer tags
                    // outside the original enum are impossible too, never success.
                    arm.pat = parse_quote!(_);
                }
            }
            enum_fallback = enum_match
                && !m
                    .arms
                    .iter()
                    .any(|arm| arm.guard.is_none() && matches!(arm.pat, syn::Pat::Wild(_)));
        }
        if let Expr::Macro(m) = expr {
            if m.attrs.is_empty() && compacted_trap(&m.mac) {
                *expr = parse_quote!(return Err(-1));
                return;
            }
        }
        if let Expr::Binary(binary) = expr {
            if self.float32(&binary.left) && self.float32(&binary.right) {
                let helper = match binary.op {
                    syn::BinOp::Add(_) => Some("add_float"),
                    syn::BinOp::Sub(_) => Some("subtract_float"),
                    syn::BinOp::Mul(_) => Some("multiply_float"),
                    _ => None,
                };
                if let Some(helper) = helper {
                    let method = format_ident!("{helper}");
                    let mut first = *binary.left.clone();
                    let mut second = *binary.right.clone();
                    self.visit_expr_mut(&mut first);
                    self.visit_expr_mut(&mut second);
                    *expr = parse_quote!(crate::vector::#method(#first,#second));
                    return;
                }
            }
        }
        if let Expr::Assign(assignment) = expr {
            if let Some((mut base, mut index, view)) = self.vector_lane(&assignment.left) {
                let mut value = *assignment.right.clone();
                self.visit_expr_mut(&mut value);
                self.visit_expr_mut(&mut base);
                self.visit_expr_mut(&mut index);
                let setter = match view.as_str() {
                    "f" => "set_float",
                    "u" => "set_word",
                    "i" => "set_signed",
                    _ => {
                        self.reject("unreviewed vector lane width");
                        return;
                    }
                };
                let method = format_ident!("{setter}");
                *expr = parse_quote!({let converted_lane_value=#value;crate::vector::#method(&mut #base,#index,converted_lane_value)?;});
                return;
            }
        }
        if let Some((mut base, mut index, view)) = self.vector_lane(expr) {
            self.visit_expr_mut(&mut base);
            self.visit_expr_mut(&mut index);
            let getter = match view.as_str() {
                "f" => "float",
                "u" => "word",
                "i" => "signed",
                _ => {
                    self.reject("unreviewed vector lane width");
                    return;
                }
            };
            let method = format_ident!("{getter}");
            *expr = parse_quote!(crate::vector::#method(#base,#index)?);
            return;
        }
        if let Expr::Field(field) = expr {
            if self.vector_value(&field.base) {
                self.reject("unreviewed whole vector view");
                return;
            }
        }
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
            if name == "xbox_ReadTimeStampCounter" {
                if !call.attrs.is_empty()
                    || !plain_callee(call)
                    || !call.args.is_empty()
                    || !matches!(&*call.func, Expr::Path(p) if p.attrs.is_empty())
                    || self.locals.contains_key(&name)
                    || self.tls.contains_key(&name)
                {
                    self.reject("unproven guest TSC binding or arity");
                } else {
                    *expr = parse_quote!(crate::kernel_pool::kernel_tsc_read(guest_ram)?);
                }
                return;
            }
            if [
                "recomp_fp_round24",
                "recomp_parity8",
                "RC_ROT",
                "recomp_fist",
                "recomp_frndint",
            ]
            .contains(&name.as_str())
            {
                if let Some(value) = self.scalar_call(call, &name) {
                    *expr = value;
                }
                return;
            }
            if name == "core::mem::size_of" && call.args.is_empty() {
                if let Expr::Path(p) = plain(&call.func) {
                    if let syn::PathArguments::AngleBracketed(arguments) =
                        &p.path.segments.last().unwrap().arguments
                    {
                        if arguments.args.len() == 1 {
                            if let Some(syn::GenericArgument::Type(ty)) = arguments.args.first() {
                                let bytes = match primitive(ty) {
                                    Some("u8" | "i8") => Some(1),
                                    Some("u16" | "i16") => Some(2),
                                    Some("u32" | "i32" | "f32") => Some(4),
                                    Some("u64" | "i64" | "f64") => Some(8),
                                    _ => None,
                                };
                                if let Some(bytes) = bytes {
                                    let literal = syn::LitInt::new(
                                        &format!("{bytes}usize"),
                                        Span::call_site(),
                                    );
                                    *expr = parse_quote!(#literal);
                                    return;
                                }
                            }
                        }
                    }
                }
                self.reject("size_of requires a reviewed fixed-width primitive");
                return;
            }
            if name.starts_with("XMM_") {
                if let Some(value) = self.vector_call(call, &name) {
                    *expr = value;
                } else {
                    self.reject(format!("unreviewed vector helper {name}"));
                }
                return;
            }
            if matches!(
                name.as_str(),
                "core::ptr::read_volatile" | "core::ptr::write_volatile"
            ) {
                if let Some(lowered) = self.memory_call(call, &name) {
                    *expr = lowered;
                }
                return;
            }
            if (name == "xbe_entry_point"
                || name.starts_with("sub_") && name[4..].bytes().all(|b| b.is_ascii_hexdigit()))
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
        if enum_fallback {
            if let Expr::Match(m) = expr {
                // Insert after recursive lowering so this error is not mistaken
                // for an original void return and discarded as success.
                m.arms.push(parse_quote!(_ => return Err(-1)));
            }
        }
        if compacted_loop {
            if let Expr::Loop(node) = expr {
                if let Stmt::Expr(Expr::Match(m), _) = &node.body.stmts[0] {
                    let selector = m.expr.clone();
                    let mut statements = Vec::new();
                    for arm in &m.arms {
                        let mut arm = arm.clone();
                        let body = arm.body;
                        arm.body = Box::new(parse_quote!({#body;continue;}));
                        let wildcard = arm.guard.is_none() && matches!(arm.pat, syn::Pat::Wild(_));
                        let mut group: syn::ExprMatch = parse_quote!(match #selector {#arm});
                        if !wildcard {
                            group.arms.push(parse_quote!(_=>{}));
                        }
                        statements.push(Stmt::Expr(Expr::Match(group), Some(Default::default())));
                    }
                    // Each successful original arm finishes this iteration,
                    // even if it changed the selector. Nonmatching guards and
                    // patterns retain their order and have no extra effects.
                    statements.push(Stmt::Expr(
                        parse_quote!(return Err(-1)),
                        Some(Default::default()),
                    ));
                    node.body.stmts = statements;
                }
            }
        }
    }
}
fn compacted_trap(mac: &syn::Macro) -> bool {
    mac.path.is_ident("unreachable")
        && matches!(mac.delimiter, syn::MacroDelimiter::Paren(_))
        && syn::parse2::<syn::LitStr>(mac.tokens.clone())
            .is_ok_and(|text| text.value() == "GamePKG compacted dispatch state reached")
}
fn compacted_trap_body(expr: &Expr) -> bool {
    match plain(expr) {
        Expr::Macro(m) => m.attrs.is_empty() && compacted_trap(&m.mac),
        Expr::Block(b) if b.attrs.is_empty() && b.label.is_none() && b.block.stmts.len() == 1 => {
            match &b.block.stmts[0] {
                Stmt::Expr(e, _) => compacted_trap_body(e),
                Stmt::Macro(m) => m.attrs.is_empty() && compacted_trap(&m.mac),
                _ => false,
            }
        }
        _ => false,
    }
}
fn enum_pattern(pattern: &syn::Pat, lower: &Lower) -> bool {
    match pattern {
        syn::Pat::Path(p) => {
            p.qself.is_none() && p.attrs.is_empty() && lower.enum_variant(&p.path).is_some()
        }
        syn::Pat::Or(p) => {
            p.attrs.is_empty()
                && !p.cases.is_empty()
                && p.cases.iter().all(|p| enum_pattern(p, lower))
        }
        _ => false,
    }
}
fn thread_patch_pattern() -> syn::Block {
    // The retail STI shadow is the complete PUSH EBX, including its RAM store.
    // Match the exact generated prefix, including every state transition.
    syn::parse_str(
        r#"{
        {
            recomp_unimpl(
                b"cli\0".as_ptr() as *const ::core::ffi::c_char,
                0x1d3fce as uint32_t,
            );
            c2rust_current_block_45 = C2Rust_Block_45::S_633;
        };
        {
            ::core::ptr::write_volatile(
                ::core::ptr::with_exposed_provenance_mut::<uint32_t,>(
                    ((*__gamepkg_tls_g_esi) as uintptr_t)
                        .wrapping_add(g_xbox_mem_offset as uintptr_t) as usize,
                ),
                0x28c5048d as uint32_t,
            );
            c2rust_current_block_45 = C2Rust_Block_45::S_635;
        };
        {
            recomp_unimpl(
                b"sti\0".as_ptr() as *const ::core::ffi::c_char,
                0x1d3fd5 as uint32_t,
            );
            c2rust_current_block_45 = C2Rust_Block_45::S_638;
        };
        {
            _icall_esp_1 = (*__gamepkg_tls_g_esp);
            c2rust_current_block_45 = C2Rust_Block_45::S_644;
        };
        {
            _pv_8 = (*__gamepkg_tls_g_ebx);
            c2rust_current_block_45 = C2Rust_Block_45::S_646;
        };
        {
            (*__gamepkg_tls_g_esp) = (*__gamepkg_tls_g_esp).wrapping_sub(4 as uint32_t);
            c2rust_current_block_45 = C2Rust_Block_45::S_648;
        };
        {
            ::core::ptr::write_volatile(
                ::core::ptr::with_exposed_provenance_mut::<uint32_t,>(
                    ((*__gamepkg_tls_g_esp) as uintptr_t)
                        .wrapping_add(g_xbox_mem_offset as uintptr_t) as usize,
                ),
                _pv_8,
            );
            c2rust_current_block_45 = C2Rust_Block_45::S_658;
        };
    }"#,
    )
    .expect("reviewed thread shadow template")
}
fn lower(mut function: ItemFn) -> Result<(ItemFn, BTreeSet<String>), BTreeSet<String>> {
    let mut pass = Lower {
        // Whole-kit input is hash pinned. Other byte-copy routines stay out
        // until their newly reachable dependencies fit source admission.
        reviewed_indexed_copy: function.sig.ident == "sub_001D1818",
        thread_patch_pattern: (function.sig.ident == "sub_001D3F2F").then(thread_patch_pattern),
        ..Lower::default()
    };
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
    icall_compacted::rewrite(&mut function);
    pass.visit_block_mut(&mut function.block);
    if pass.thread_patch_pattern.is_some() {
        pass.reject("unreviewed thread-start interrupt shadow");
    }
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
    if args.len() == 4 && args[1] == "--prepare-tsc-leaves" {
        return batch::prepare_tsc_leaves(Path::new(&args[2]), Path::new(&args[3]));
    }
    if args.len() == 4 && ["--inventory", "--prepare-closed"].contains(&args[1].as_str()) {
        return batch::run(
            Path::new(&args[2]),
            Path::new(&args[3]),
            args[1] == "--prepare-closed",
        );
    }
    if args.len() != 3 {
        return Err("requires input-unit.rs new-output-directory".into());
    }
    let source = fs::read(&args[1])?;
    let source_hash = format!("{:x}", Sha256::digest(&source));
    if source_hash != "442f7812516f3c5157ee74e15aede23d0ac423105acb7d973733bbe9f01ad94d" {
        return Err("unreviewed input unit hash".into());
    }
    let ast = syn::parse_file(std::str::from_utf8(&source)?)?;
    let original_helpers = ast
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(f)
                if [
                    "XMM_ZERO",
                    "XMM_SCALAR",
                    "XMM_MEM",
                    "XMM_STORE",
                    "XMM_ADD",
                    "XMM_SUB",
                    "XMM_MUL",
                    "XMM_XOR",
                    "XMM_SHUFFLE",
                ]
                .iter()
                .any(|name| f.sig.ident == *name) =>
            {
                Some(f)
            }
            _ => None,
        })
        .cloned()
        .collect::<Vec<_>>();
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
    fs::write(
        output.join("oracle_helpers.rs"),
        quote!(#(#original_helpers)*).to_string(),
    )?;
    let runtime_call_functions = accepted
        .iter_mut()
        .filter_map(|(name, (function, _))| {
            let count = icall::count(function);
            (count != 0).then_some((name.clone(), count))
        })
        .collect::<BTreeMap<_, _>>();
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
    let available = accepted
        .keys()
        .map(|name| u32::from_str_radix(&name[4..], 16).unwrap())
        .collect::<Vec<_>>();
    let route_names = (0..routes.len())
        .map(|i| format_ident!("route_{i}"))
        .collect::<Vec<_>>();
    let dispatch = quote!(#(#routes)* pub fn entry_available()->bool{false} pub fn routine_available(address:u32)->bool{match address{#(#available => true,)* _ => false}} pub fn invoke(native_state:&mut crate::registers::Registers,guest_ram:&mut crate::memory::Memory,address:u32)->Result<(),i32>{if !native_state.valid(){return Err(-1);}#(match #route_names(native_state,guest_ram,address){Err(-5)=>{},result=>return result})*Err(-5)});
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
    let provenance = serde_json::json!({"schema":1,"register_record_version":5,"register_record_bytes":316,"debug_adapter_sha256":format!("{:x}",Sha256::digest(include_bytes!("../../debug.rs"))),"debug_review_sha256":format!("{:x}",Sha256::digest(include_bytes!("../reviewed_debug_service.json"))),"registers_source_sha256":format!("{:x}",Sha256::digest(include_bytes!("../../registers.rs"))),"kernel_services_source_sha256":format!("{:x}",Sha256::digest(include_bytes!("../../../service-kernel-pool/kernel_pool.rs"))),"float_arithmetic_nan_policy":"quiet-first-NaN-preserve-sign-and-payload","generator_source_sha256":format!("{:x}",Sha256::digest(include_bytes!("main.rs"))),"generator_batch_sha256":format!("{:x}",Sha256::digest(include_bytes!("batch.rs"))),"generator_icall_sha256":format!("{:x}",Sha256::digest(include_bytes!("icall.rs"))),"generator_icall_compacted_sha256":format!("{:x}",Sha256::digest(include_bytes!("icall_compacted.rs"))),"generator_icall_compacted_pattern_sha256":format!("{:x}",Sha256::digest(include_bytes!("icall_compacted_template.inc"))),"generator_icall_guarded_pattern_sha256":format!("{:x}",Sha256::digest(include_bytes!("icall_guarded_template.inc"))),"generator_icall_jump_pattern_sha256":format!("{:x}",Sha256::digest(include_bytes!("icall_jump_template.inc"))),"generator_icall_jump_compacted_pattern_sha256":format!("{:x}",Sha256::digest(include_bytes!("icall_jump_compacted_template.inc"))),"generator_icall_pattern_sha256":format!("{:x}",Sha256::digest(include_bytes!("icall_template.inc"))),"generator_inventory_sha256":format!("{:x}",Sha256::digest(include_bytes!("../reviewed_units.tsv"))),"generator_lock_sha256":format!("{:x}",Sha256::digest(include_bytes!("../Cargo.lock"))),"formatter_version":String::from_utf8(formatter.stdout)?.trim(),"game_sha256":format!("{:x}",Sha256::digest(fs::read(output.join("game.rs"))?)),"dispatch_sha256":format!("{:x}",Sha256::digest(fs::read(output.join("dispatch.rs"))?)),"input_sha256":source_hash,"source_kit_sha256":"6b37d24c0984c686c7ed6fd73cd3a5b252b5631ceefa2798e5b7a6e0adff8dc4","runtime_call_functions":runtime_call_functions,"converted":accepted.keys().collect::<Vec<_>>(),"rejected":rejected,"production_ready":false});
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
    #[test]
    fn native_thread_patch_requires_the_whole_reviewed_shadow() {
        let fixture = |name: &str, block: &syn::Block| {
            let name = format_ident!("{name}");
            lower(parse_quote!(
                pub unsafe extern "C" fn #name() {
                    let __gamepkg_tls_g_esi = unsafe { gamepkg_tls_g_esi() };
                    let __gamepkg_tls_g_esp = unsafe { gamepkg_tls_g_esp() };
                    let __gamepkg_tls_g_ebx = unsafe { gamepkg_tls_g_ebx() };
                    let mut _icall_esp_1: uint32_t = 0;
                    let mut _pv_8: uint32_t = 0;
                    enum C2Rust_Block_45 { S_633, S_635, S_638, S_644, S_646, S_648, S_658 }
                    let mut c2rust_current_block_45: C2Rust_Block_45;
                    #block
                }
            ))
        };
        let exact = thread_patch_pattern();
        let (lowered, deps) = fixture("sub_001D3F2F", &exact).unwrap();
        let text = lowered.to_token_stream().to_string();
        assert!(deps.is_empty());
        assert!(!text.contains("recomp_unimpl") && !text.contains("write_volatile"));
        assert!(text.contains("_icall_esp_1 = crate :: irq_delivery :: irq_thread_patch_current"));
        assert!(text.contains("c2rust_current_block_45 = 6u32"));
        assert!(fixture("sub_001D6AAC", &exact).is_err());
        for (from, to) in [
            ("0x1d3fd5", "0x1d3fd6"),
            ("0x28c5048d", "0x28c5048c"),
            ("S_658", "S_648"),
            ("g_ebx", "g_esi"),
            ("wrapping_sub", "wrapping_add"),
        ] {
            let altered = exact.to_token_stream().to_string().replace(from, to);
            assert!(fixture("sub_001D3F2F", &syn::parse_str(&altered).unwrap()).is_err());
        }
        let mut altered = exact.clone();
        altered.stmts.swap(5, 6);
        assert!(fixture("sub_001D3F2F", &altered).is_err());
    }
    #[test]
    fn guest_tsc_requires_exact_direct_zero_argument_call() {
        let body = "let value:uint64_t=xbox_ReadTimeStampCounter();";
        let (function, deps) = lowered(body).unwrap();
        assert!(deps.is_empty());
        assert!(function
            .to_token_stream()
            .to_string()
            .contains("crate :: kernel_pool :: kernel_tsc_read"));
        for replacement in [
            "xbox_ReadTimeStampCounter(0u32)",
            "xbox_ReadTimeStampCounter::<u64>()",
            "::xbox_ReadTimeStampCounter()",
            "other::xbox_ReadTimeStampCounter()",
            "(#[allow(unused)] xbox_ReadTimeStampCounter)()",
            "(xbox_ReadTimeStampCounter)()",
        ] {
            assert!(
                lowered(&body.replace("xbox_ReadTimeStampCounter()", replacement)).is_err(),
                "{replacement}"
            );
        }
        assert!(lowered(&format!("let xbox_ReadTimeStampCounter:uint32_t=0;{body}")).is_err());
    }
    fn load(address: &str, width: &str) -> String {
        format!("::core::ptr::read_volatile::<{width}>(::core::ptr::with_exposed_provenance_mut::<uint32_t>((({address}) as uintptr_t).wrapping_add(g_xbox_mem_offset as uintptr_t) as usize));")
    }
    #[test]
    fn debug_service_requires_unused_exact_call_and_proven_u32_arguments() {
        let body="let service:uint32_t=1;let argument:uint32_t=0x12000;recomp_debug_service(service,argument);";
        let (function, deps) = lowered(body).unwrap();
        assert!(deps.is_empty());
        assert!(function
            .to_token_stream()
            .to_string()
            .contains("crate :: debug :: service"));
        for (old, new) in [
            ("service:uint32_t", "service:uint64_t"),
            ("argument:uint32_t", "argument:uint64_t"),
            (
                "recomp_debug_service(service,argument)",
                "recomp_debug_service(service)",
            ),
            (
                "recomp_debug_service(service,argument)",
                "recomp_debug_service::<u32>(service,argument)",
            ),
            (
                "recomp_debug_service(service,argument)",
                "recomp_debug_service(service,#[allow(unused)] argument)",
            ),
            (
                "recomp_debug_service(service,argument)",
                "let result=recomp_debug_service(service,argument)",
            ),
        ] {
            assert!(lowered(&body.replace(old, new)).is_err(), "{new}");
        }
        assert!(lowered(&format!("let recomp_debug_service:uint32_t=0;{body}")).is_err());
    }
    #[test]
    fn guest_memset_requires_exact_mutable_relocation_byte_cast_and_proven_u32_length() {
        let body="let destination:uint32_t=0x10000;let value:uint32_t=0xabcdef12;let length:uint32_t=17;memset(::core::ptr::with_exposed_provenance_mut::<::core::ffi::c_void>((destination as uintptr_t).wrapping_add(g_xbox_mem_offset as uintptr_t) as usize),value as uint8_t as ::core::ffi::c_int,length as size_t);";
        let (function, deps) = lowered(body).unwrap();
        assert!(deps.is_empty());
        let text = function.to_token_stream().to_string();
        assert!(text.contains("guest_ram . fill"));
        for forbidden in ["memset", "core :: ptr", "uintptr_t", "size_t"] {
            assert!(!text.contains(forbidden), "{text}");
        }
        for (old, new) in [
            ("destination:uint32_t", "destination:uint64_t"),
            ("length:uint32_t", "length:uint64_t"),
            ("value:uint32_t", "value:uint64_t"),
            ("with_exposed_provenance_mut", "with_exposed_provenance"),
            ("::<::core::ffi::c_void>", "::<uint32_t>"),
            (
                "wrapping_add(g_xbox_mem_offset as uintptr_t)",
                "wrapping_sub(g_xbox_mem_offset as uintptr_t)",
            ),
            (
                "as uint8_t as ::core::ffi::c_int",
                "as uint16_t as ::core::ffi::c_int",
            ),
            ("length as size_t", "length as uintptr_t"),
            ("memset(", "let result=memset("),
            ("value as uint8_t", "#[allow(unused)] value as uint8_t"),
        ] {
            assert!(body.contains(old));
            assert!(lowered(&body.replace(old, new)).is_err(), "{old}");
        }
        for prefix in [
            "let memset:uint32_t=0;",
            "let g_xbox_mem_offset:uint32_t=0;",
        ] {
            assert!(lowered(&format!("{prefix}{body}")).is_err(), "{prefix}");
        }
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
    fn guest_fs_base_is_only_a_proven_u32_context_field_used_through_checked_ram() {
        let input="let __gamepkg_tls_g_fs_base=unsafe{gamepkg_tls_g_fs_base()};let mut address:uint32_t=(*__gamepkg_tls_g_fs_base).wrapping_add(4 as uint32_t);let word:uint32_t=::core::ptr::read_volatile::<uint32_t>(::core::ptr::with_exposed_provenance_mut::<uint32_t>((address as uintptr_t).wrapping_add(g_xbox_mem_offset as uintptr_t) as usize));";
        let (function, deps) = lowered(input).unwrap();
        assert!(deps.is_empty());
        let text = function.to_token_stream().to_string();
        assert!(text.contains("native_state . fs_base"));
        assert!(text.contains("guest_ram . read_u32"));
        for forbidden in ["gamepkg_tls", "ptr ::", "uintptr_t", "unsafe", "extern"] {
            assert!(!text.contains(forbidden), "{text}");
        }
        for changed in [
            input.replace("address:uint32_t", "address:uint64_t"),
            input.replace("gamepkg_tls_g_fs_base()", "gamepkg_tls_g_host_fs_base()"),
            input.replace(
                "(address as uintptr_t).wrapping_add(g_xbox_mem_offset as uintptr_t)",
                "(address as uintptr_t)",
            ),
        ] {
            assert!(lowered(&changed).is_err());
        }
    }
    #[test]
    fn native_pointer_paths_unknown_tls_and_aliases_are_not_converted() {
        for body in [
            "let p:*mut u32=0 as *mut u32;*p=1;",
            "let other=unsafe{gamepkg_tls_g_eax()};",
            "let __gamepkg_tls_g_xmm8=unsafe{gamepkg_tls_g_xmm8()};",
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
    fn address_width_proofs_follow_lexical_scope_initializers_and_shadowing() {
        assert!(lowered(&format!(
            "let a:uint64_t=0x100001234;{{let a:uint32_t=0;}}{}",
            load("a", "uint32_t")
        ))
        .is_err());
        assert!(lowered(&format!(
            "let a:uint32_t=0x1234;{{let a:uint64_t=0x100001234;}}{}",
            load("a", "uint32_t")
        ))
        .is_ok());
        assert!(lowered(&format!(
            "let a:uint32_t=0x1234;let a=0x100001234u64;{}",
            load("a", "uint32_t")
        ))
        .is_err());
        assert!(lowered(&format!(
            "let a:uint64_t=0x100001234;let a:uint32_t={{{}0}};",
            load("a", "uint32_t")
        ))
        .is_err());
        assert!(lowered(&load("(1 as uint32_t)==(2 as uint32_t)", "uint32_t")).is_err());
        assert!(lowered("let __gamepkg_tls_g_eax=unsafe{gamepkg_tls_g_eax()};{let __gamepkg_tls_g_eax:uint64_t=0;*__gamepkg_tls_g_eax=1;}").is_err());
    }
    #[test]
    fn vector_lowering_keeps_bit_views_checked_and_rejects_unknown_widths_and_addresses() {
        let prefix = "let __gamepkg_tls_g_xmm0=unsafe{gamepkg_tls_g_xmm0()};";
        assert!(lowered(&format!("{prefix}(*__gamepkg_tls_g_xmm0).f[0usize]=(*__gamepkg_tls_g_xmm0).f[1usize]+(*__gamepkg_tls_g_xmm0).f[2usize];")).is_ok());
        assert!(lowered(&format!(
            "{prefix}let value=(*__gamepkg_tls_g_xmm0).q[0usize];"
        ))
        .is_err());
        assert!(lowered(&format!("{prefix}let value=(*__gamepkg_tls_g_xmm0).f;")).is_err());
        assert!(lowered("XMM_MEM(0x100001234u64);").is_err());
        assert!(lowered("XMM_ZERO(1u32);").is_err());
        assert!(lowered("let a:RecompXmm=XMM_ZERO();let lane=a.u[0usize];").is_ok());
    }
    #[test]
    fn indexed_guest_byte_dereferences_keep_access_order_and_width() {
        let prefix = "let mut offset:uint32_t=0;let destination:*mut uint8_t=::core::ptr::null_mut::<uint8_t>();let source:*mut uint8_t=::core::ptr::null_mut::<uint8_t>();";
        let lower_thread_wrapper = |body: &str| {
            lower(
                syn::parse_str(&format!(
                    "pub unsafe extern \"C\" fn sub_001D1818(){{{prefix}{body}}}"
                ))
                .unwrap(),
            )
        };
        let copy = "*destination.offset(offset as isize)=*source.offset(offset as isize);";
        let (function, _) = lower_thread_wrapper(copy).unwrap();
        assert!(lowered(&format!("{prefix}{copy}")).is_err());
        let lowered_body = function.to_token_stream().to_string();
        assert!(lowered_body.contains("guest_store_byte (guest_ram . read_u8"));
        assert!(lowered_body.contains("guest_byte_address (source , offset)"));
        assert!(lowered_body.contains("guest_byte_address (destination , offset)"));
        assert!(!lowered_body.contains("isize") && !lowered_body.contains("* mut"));
        for rejected in [
            "*destination.offset(-1isize)=*source.offset(offset as isize);",
            "*destination.offset(0x100000000u64 as isize)=3;",
            "*destination.wrapping_offset(offset as isize)=3;",
            "*destination.offset(offset as i32)=3;",
            "let leaked=destination.offset(offset as isize);",
            "let leaked=&mut *destination.offset(offset as isize);",
            "*destination.offset(offset as isize)+=1;",
            "let loaded=*source.offset(offset as isize);",
            "*destination.offset(offset as isize)=3;",
            "*(destination as *mut u32).offset(offset as isize)=3;",
            "*destination=3;",
        ] {
            assert!(lower_thread_wrapper(rejected).is_err(), "{rejected}");
        }
    }

    #[test]
    fn guest_copy_temporaries_are_values_and_only_reviewed_uses_survive() {
        let make = |value: &str| {
            format!("::core::ptr::with_exposed_provenance_mut::<uint8_t>((({value}) as uintptr_t).wrapping_add(g_xbox_mem_offset as uintptr_t) as usize)")
        };
        let body=format!("let mut destination:*mut uint8_t=::core::ptr::null_mut::<uint8_t>();let source:*mut uint8_t={};destination={};if destination.offset(16u32 as isize)<=source || source.offset(16u32 as isize)<=destination{{memcpy(destination as *mut ::core::ffi::c_void,source as *const ::core::ffi::c_void,16u32 as size_t);}}",make("0x20000u32"),make("0x10000u32"));
        let (function, _) = lowered(&body).unwrap();
        let text = function.to_token_stream().to_string();
        assert!(text.contains("Option < u32 >"));
        assert!(text.contains("guest_offset"));
        assert!(text.contains("copy_nonoverlapping"));
        for removed in [
            "uintptr_t",
            "size_t",
            "isize",
            "core :: ptr",
            "* mut",
            "* const",
        ] {
            assert!(!text.contains(removed), "{text}");
        }
        let prefix = format!(
            "let mut destination:*mut uint8_t={};let source:*mut uint8_t={};",
            make("0x10000u32"),
            make("0x20000u32")
        );
        for tail in [
            "memcpy(destination as *mut ::core::ffi::c_void,source as *const ::core::ffi::c_void,16u64 as size_t);",
            "let escaped=destination;", "let result=memcpy(destination as *mut ::core::ffi::c_void,source as *const ::core::ffi::c_void,16u32 as size_t);",
            "destination=1u32 as *mut uint8_t;", "let n:u64=16;if destination.offset(n as isize)<=source{}",
            "if destination.offset(-1isize)<=source{}", "if destination==source{}",
            "memcpy(destination as *const ::core::ffi::c_void,source as *const ::core::ffi::c_void,16u32 as size_t);",
            "memcpy(destination as *mut ::core::ffi::c_void,source as *mut ::core::ffi::c_void,16u32 as size_t);",
            "SDL_Copy(destination);",
        ] {assert!(lowered(&(prefix.clone()+tail)).is_err(),"{tail}");}
        assert!(lowered(&format!(
            "let wide:u64=0x100001000;let destination:*mut uint8_t={};",
            make("wide")
        ))
        .is_err());
        assert!(
            lowered("let destination:*mut uint8_t=::core::ptr::null_mut::<uint16_t>();").is_err()
        );
        assert!(lowered("let destination:*const uint8_t=::core::ptr::null::<uint8_t>();").is_err());
        assert!(lowered("let destination:*mut uint8_t=::core::ptr::null_mut::<uint8_t>();let empty=destination.is_null();").is_ok());
        for binding in ["ref destination", "destination @ _"] {
            assert!(lowered(&format!(
                "let {binding}:*mut uint8_t=::core::ptr::null_mut::<uint8_t>();"
            ))
            .is_err());
        }
    }
    #[test]
    fn integer_rounding_requires_reviewed_control_words_and_fixed_store_widths() {
        let (function,_)=lowered("let control:uint16_t=0x37fu16;let result=recomp_fist(1.5f64,control,64u32);let rounded=recomp_frndint(-1.5f64,control);").unwrap();
        let text = function.to_token_stream().to_string();
        assert!(text.contains("scalar :: integer_store"));
        assert!(text.contains("scalar :: round_integer"));
        for body in [
            "recomp_fist(1.5f64,0x37fu32,64u32);",
            "recomp_fist(1.5f64,0x37fu16,8u32);",
            "let width:u32=64;recomp_fist(1.5f64,0x37fu16,width);",
            "recomp_fist(1.5f64,0x37fu16);",
            "recomp_fist::<u64>(1.5f64,0x37fu16,64u32);",
            "let recomp_fist=other;recomp_fist(1.5f64,0x37fu16,64u32);",
            "recomp_frndint(1.5f64,0x37fu64);",
            "recomp_frndint(1.5f64,0x37fu16,0u32);",
        ] {
            assert!(lowered(body).is_err(), "{body}");
        }
    }
    #[test]
    fn scalar_helpers_accept_only_proven_values_and_local_carry_borrows() {
        let (function,_)=lowered("let mut carry:int32_t=1;let result=RC_ROT(1u32,31u32,&raw mut carry,32u32,1);let parity=recomp_parity8(1u32);let fp=recomp_fp_round24(1.0f64);").unwrap();
        let text = function.to_token_stream().to_string();
        assert!(text.contains("crate :: scalar :: rotate_carry"));
        assert!(text.contains("& mut carry"));
        assert!(!text.contains("raw"));
        for body in [
            "let mut carry:int64_t=1;RC_ROT(1u32,1u32,&raw mut carry,32u32,1);",
            "let mut carry:int32_t=1;RC_ROT(1u64,1u32,&raw mut carry,32u32,1);",
            "let mut carry:int32_t=1;RC_ROT(1u32,1u64,&raw mut carry,32u32,1);",
            "let mut carry:int32_t=1;RC_ROT(1u32,1u32,&raw const carry,32u32,1);",
            "let mut carry:int32_t=1;RC_ROT(1u32,1u32,&raw mut carry,64u32,1);",
            "let mut carry:int32_t=1;RC_ROT(1u32,1u32,&raw mut carry,32u32,2);",
            "let mut carry:int32_t=1;let width:u32=32;RC_ROT(1u32,1u32,&raw mut carry,width,1);",
            "let mut carry:int32_t=1;RC_ROT(1u32,1u32,&mut carry,32u32,1);",
            "recomp_parity8(1u64);",
            "recomp_parity8::<u64>(1u32);",
            "recomp_fp_round24(1.0f64,2.0f64);",
            "let recomp_fp_round24=other;recomp_fp_round24(1.0f64);",
        ] {
            assert!(lowered(body).is_err(), "{body}");
        }
    }
    #[test]
    fn control_flow_enums_preserve_ordinal_values_and_lexical_scope() {
        let source="enum C2Rust_Block_1{S_100,S_5}let outer:C2Rust_Block_1=C2Rust_Block_1::S_5;{enum C2Rust_Block_1{S_5,S_100}let inner:C2Rust_Block_1=C2Rust_Block_1::S_5;}let again:C2Rust_Block_1=C2Rust_Block_1::S_5;";
        let (function, _) = lowered(source).unwrap();
        let text = function.to_token_stream().to_string();
        assert!(!text.contains("C2Rust_Block"));
        assert!(text.contains("outer : u32 = 1u32"));
        assert!(text.contains("inner : u32 = 0u32"));
        assert!(text.contains("again : u32 = 1u32"));
        assert!(!text.contains("enum "));
    }
    #[test]
    fn unreviewed_enums_and_macros_remain_rejected() {
        for source in [
            "#[repr(C)]enum C2Rust_Block_1{S_0}",
            "enum C2Rust_Block_1{S_0(u32)}",
            "enum C2Rust_Block_1{S_0=7}",
            "enum C2Rust_Block_1<T>{S_0}",
            "enum C2Rust_Block_1{Other}",
            "enum Other{S_0}",
            "enum C2Rust_Block_1{S_0,S_0}",
            "enum C2Rust_Block_1{#[cfg(windows)]S_0}",
            "enum C2Rust_Block_1{}",
            "unreachable!(\"other\");",
            "std::unreachable!(\"GamePKG compacted dispatch state reached\");",
            "unreachable!(\"GamePKG compacted dispatch state reached\",system());",
            "unreachable!(concat!(\"GamePKG compacted dispatch state reached\"));",
            "type C2Rust_Block_1=u32;",
        ] {
            assert!(lowered(source).is_err(), "{source}");
        }
        for source in [
            "unreachable!(\"GamePKG compacted dispatch state reached\")",
            "unreachable!(\"GamePKG compacted dispatch state reached\");",
        ] {
            let (function, _) = lowered(source).unwrap();
            let text = function.to_token_stream().to_string();
            assert!(text.contains("return Err (- 1)"), "{text}");
            assert!(!text.contains("unreachable"));
            assert!(!text.contains("return {"));
        }
    }
    #[test]
    fn sizes_are_folded_only_for_reviewed_fixed_width_primitives() {
        for (ty, size) in [
            ("uint8_t", 1),
            ("int16_t", 2),
            ("int32_t", 4),
            ("uint64_t", 8),
            ("f32", 4),
            ("f64", 8),
        ] {
            let (function, _) =
                lowered(&format!("let width=::core::mem::size_of::<{ty}>();")).unwrap();
            let text = function.to_token_stream().to_string();
            assert!(text.contains(&format!("{size}usize")));
            assert!(!text.contains("size_of"));
        }
        for ty in [
            "usize",
            "isize",
            "uintptr_t",
            "size_t",
            "*mut u32",
            "[u8;4]",
            "Unknown",
        ] {
            assert!(
                lowered(&format!("let width=::core::mem::size_of::<{ty}>();")).is_err(),
                "{ty}"
            );
        }
    }
    #[test]
    #[ignore = "compiles/runs four-target native CFG reference harness including Wine"]
    fn native_control_flow_matches_reference_on_all_four_targets() {
        let body = r#"let __gamepkg_tls_g_eax=unsafe{gamepkg_tls_g_eax()};
enum C2Rust_Block_1{S_100,S_5,S_9,S_10,S_20}
let mut c2rust_current_block_1:C2Rust_Block_1=C2Rust_Block_1::S_100;
loop{match c2rust_current_block_1{
C2Rust_Block_1::S_10=>{unreachable!("GamePKG compacted dispatch state reached")}
C2Rust_Block_1::S_100=>{if *__gamepkg_tls_g_eax==0xffffffffu32{c2rust_current_block_1=C2Rust_Block_1::S_10;}else if *__gamepkg_tls_g_eax&1==0{c2rust_current_block_1=C2Rust_Block_1::S_5;}else{c2rust_current_block_1=C2Rust_Block_1::S_9;}}
C2Rust_Block_1::S_5=>{*__gamepkg_tls_g_eax=(*__gamepkg_tls_g_eax).wrapping_mul(3);c2rust_current_block_1=C2Rust_Block_1::S_20;}
C2Rust_Block_1::S_9=>{*__gamepkg_tls_g_eax=(*__gamepkg_tls_g_eax).wrapping_sub(7);c2rust_current_block_1=C2Rust_Block_1::S_20;}
C2Rust_Block_1::S_20=>{return;}
}}"#;
        let mut bodies = vec![body.to_owned()];
        bodies.push(body.replace("C2Rust_Block_1::S_10=>{unreachable!(\"GamePKG compacted dispatch state reached\")}\n","").replace("C2Rust_Block_1::S_20=>{return;}","C2Rust_Block_1::S_20=>{return;} C2Rust_Block_1::S_10=>{unreachable!(\"GamePKG compacted dispatch state reached\")}"));
        bodies.push(body.replace("match c2rust_current_block_1{", "match c2rust_current_block_1{ C2Rust_Block_1::S_100 if {*__gamepkg_tls_g_eax=(*__gamepkg_tls_g_eax).wrapping_add(2);false}=>{return;},"));
        let root = std::env::temp_dir().join(format!("gamepkg-cfg-native-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        for (case, body) in bodies.iter().enumerate() {
            let (function, _) = lowered(body).unwrap();
            let converted = function.to_token_stream().to_string();
            assert!(!converted.contains("C2Rust_Block"));
            assert!(!converted.contains("unreachable"));
            let source = format!(
                r#"#![allow(dead_code,non_snake_case,non_camel_case_types,unused_assignments,unused_variables,unreachable_code)]
mod registers{{pub struct Registers{{pub eax:u32}}}}mod memory{{pub struct Memory;}}
mod game{{{converted}}}
mod reference{{static mut VALUE:u32=0;unsafe fn gamepkg_tls_g_eax()->*mut u32{{&raw mut VALUE}}pub unsafe fn run(input:u32)->u32{{VALUE=input;original();VALUE}}unsafe fn original(){{{body}}}}}
fn main(){{let mut ram=memory::Memory;for input in 0..4096u32{{let expected=unsafe{{reference::run(input)}};let mut state=registers::Registers{{eax:input}};assert_eq!(game::sub_100(&mut state,&mut ram),Ok(()));assert_eq!(state.eax,expected);}}let mut state=registers::Registers{{eax:u32::MAX-({case}==2) as u32*2}};assert_eq!(game::sub_100(&mut state,&mut ram),Err(-1));assert_eq!(state.eax,u32::MAX);}}
"#
            );
            let main = root.join("main.rs");
            fs::write(&main, source).unwrap();
            for triple in [
                "x86_64-unknown-linux-gnu",
                "i686-unknown-linux-gnu",
                "x86_64-pc-windows-gnu",
                "i686-pc-windows-gnu",
            ] {
                let executable = root.join(format!(
                    "case-{case}-{triple}{}",
                    if triple.contains("windows") {
                        ".exe"
                    } else {
                        ""
                    }
                ));
                let mut command = std::process::Command::new("rustc");
                command
                    .args(["--edition=2021", "-D", "warnings", "-O", "--target", triple])
                    .arg(&main)
                    .arg("-o")
                    .arg(&executable);
                if triple == "i686-unknown-linux-gnu" {
                    command.args(["-C", "linker=/usr/bin/gcc-11"]);
                }
                assert!(command.status().unwrap().success());
                let mut run = if triple.contains("windows") {
                    let mut wine = std::process::Command::new("wine");
                    wine.env("WINEDEBUG", "-all")
                        .env("WINEPREFIX", "/tmp/gamepkg-port-replace-wine")
                        .arg(&executable);
                    wine
                } else {
                    std::process::Command::new(&executable)
                };
                assert!(run.status().unwrap().success());
            }
        }
        fs::remove_dir_all(root).unwrap();
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

#[cfg(test)]
mod interrupt_state_tests {
    use super::*;
    #[test]
    fn flags_and_fp_comparison_lower_to_values_without_a_host_tls_accessor() {
        let function:ItemFn=syn::parse_str("pub unsafe extern \"C\" fn sub_00000001(){let __gamepkg_tls_g_eflags=unsafe{gamepkg_tls_g_eflags()};let __gamepkg_tls_g_fp_cmp=unsafe{gamepkg_tls_g_fp_cmp()};*__gamepkg_tls_g_eflags=(*__gamepkg_tls_g_eflags)&!0x200u32;*__gamepkg_tls_g_fp_cmp=-1i32;}").unwrap();
        let (converted, _) = lower(function).unwrap();
        let text = converted.to_token_stream().to_string();
        assert!(text.contains("native_state . eflags"));
        assert!(text.contains("native_state . fp_cmp"));
        assert!(!text.contains("unsafe"));
        assert!(!text.contains("gamepkg_tls"));
        for body in [
            "let __gamepkg_tls_g_eflags=unsafe{gamepkg_tls_g_eflags(1u32)};",
            "let forged=unsafe{gamepkg_tls_g_eflags()};",
            "let __gamepkg_tls_g_eflags=unsafe{gamepkg_tls_g_host_eflags()};",
            "let __gamepkg_tls_g_mm0=unsafe{gamepkg_tls_g_mm0()};",
            "let __gamepkg_tls_g_fp_cmp=unsafe{evil::gamepkg_tls_g_fp_cmp()};",
        ] {
            let function: ItemFn = syn::parse_str(&format!(
                "pub unsafe extern \"C\" fn sub_00000001(){{{body}}}"
            ))
            .unwrap();
            assert!(lower(function).is_err(), "{body}");
        }
    }
}
