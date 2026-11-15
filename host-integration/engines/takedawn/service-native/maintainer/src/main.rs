//! Maintainer-only AST converter, not a runtime or game dependency.
mod batch;
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
    enums: BTreeMap<String, Arc<BTreeMap<String, u32>>>,
    dependencies: BTreeSet<String>,
    errors: BTreeSet<String>,
}
impl Lower {
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
            "eax", "ebx", "ecx", "edx", "esi", "edi", "ebp", "esp", "seh_ebp", "df", "fp_stack",
            "fp_top", "fp_cc", "xmm0", "xmm1", "xmm2", "xmm3", "xmm4", "xmm5", "xmm6", "xmm7",
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
        let outer_locals = self.locals.clone();
        let outer_tls = self.tls.clone();
        let outer_enums = self.enums.clone();
        let mut statements = Vec::new();
        for mut statement in std::mem::take(&mut block.stmts) {
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
    let provenance = serde_json::json!({"schema":1,"register_record_version":2,"register_record_bytes":240,"float_arithmetic_nan_policy":"quiet-first-NaN-preserve-sign-and-payload","generator_source_sha256":format!("{:x}",Sha256::digest(include_bytes!("main.rs"))),"generator_batch_sha256":format!("{:x}",Sha256::digest(include_bytes!("batch.rs"))),"generator_inventory_sha256":format!("{:x}",Sha256::digest(include_bytes!("../reviewed_units.tsv"))),"generator_lock_sha256":format!("{:x}",Sha256::digest(include_bytes!("../Cargo.lock"))),"formatter_version":String::from_utf8(formatter.stdout)?.trim(),"game_sha256":format!("{:x}",Sha256::digest(fs::read(output.join("game.rs"))?)),"dispatch_sha256":format!("{:x}",Sha256::digest(fs::read(output.join("dispatch.rs"))?)),"input_sha256":source_hash,"source_kit_sha256":"6b37d24c0984c686c7ed6fd73cd3a5b252b5631ceefa2798e5b7a6e0adff8dc4","converted":accepted.keys().collect::<Vec<_>>(),"rejected":rejected,"production_ready":false});
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
