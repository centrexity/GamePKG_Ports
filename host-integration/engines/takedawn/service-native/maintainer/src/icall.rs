//! Match the complete reviewed trace/lookup/fallback sequence, never a loose
//! function-pointer idiom. This code is maintainer tooling, not game runtime.
use super::*;
pub(super) fn temporary(name: &str, base: &str) -> bool {
    name == base
        || name
            .strip_prefix(&format!("{base}_"))
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}
fn block(statement: &Stmt) -> Option<&syn::ExprBlock> {
    match statement {
        Stmt::Expr(Expr::Block(b), None) if b.attrs.is_empty() => Some(b),
        _ => None,
    }
}
fn assignment(statement: &Stmt) -> Option<&syn::ExprAssign> {
    match statement {
        Stmt::Expr(Expr::Assign(a), Some(_)) if a.attrs.is_empty() => Some(a),
        _ => None,
    }
}
pub(super) fn tls_lhs(expr: &Expr) -> Option<String> {
    let Expr::Unary(u) = plain(expr) else {
        return None;
    };
    if !matches!(u.op, syn::UnOp::Deref(_)) {
        return None;
    }
    path(&u.expr)
}
pub(super) struct Normalize {
    pub(super) names: BTreeMap<String, String>,
    pub(super) labels: BTreeMap<String, String>,
}
impl VisitMut for Normalize {
    fn visit_ident_mut(&mut self, ident: &mut syn::Ident) {
        if let Some(name) = self.names.get(&ident.to_string()) {
            *ident = syn::Ident::new(name, ident.span());
        }
    }
    fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
        if let Some(name) = self.labels.get(&lifetime.ident.to_string()) {
            *lifetime = syn::Lifetime::new(&format!("'{name}"), lifetime.apostrophe);
        }
    }
}
pub(super) fn rewrite(statements: &[Stmt], pass: &Lower) -> Option<Stmt> {
    if statements.len() < 4 {
        return None;
    }
    let Stmt::Expr(Expr::Call(trace), Some(_)) = &statements[0] else {
        return None;
    };
    let va = path(trace.args.iter().nth(1)?)?;
    if !temporary(&va, "_va") || !pass.guest_u32(trace.args.iter().nth(1)?) {
        return None;
    }
    let outer = block(&statements[3])?;
    if outer.block.stmts.len() != 6 {
        return None;
    }
    let guard = block(&outer.block.stmts[0])?;
    let native = block(&outer.block.stmts[2])?;
    let kernel = block(&outer.block.stmts[3])?;
    let missing = block(&outer.block.stmts[4])?;
    if guard.block.stmts.len() != 5 {
        return None;
    }
    let esp = assignment(&guard.block.stmts[2])?;
    let eax = assignment(&guard.block.stmts[3])?;
    let saved = path(&esp.right)?;
    if !temporary(&saved, "_icall_esp") || !pass.guest_u32(&esp.right) {
        return None;
    }
    let esp_alias = tls_lhs(&esp.left)?;
    let eax_alias = tls_lhs(&eax.left)?;
    if pass.tls.get(&esp_alias).map(String::as_str) != Some("esp")
        || pass.tls.get(&eax_alias).map(String::as_str) != Some("eax")
    {
        return None;
    }
    let Stmt::Local(binding) = &outer.block.stmts[1] else {
        return None;
    };
    let syn::Pat::Type(ty) = &binding.pat else {
        return None;
    };
    let syn::Pat::Ident(ident) = &*ty.pat else {
        return None;
    };
    let function = ident.ident.to_string();
    if !temporary(&function, "_fn") {
        return None;
    }
    for name in [
        "g_icall_trace",
        "g_icall_trace_idx",
        "g_icall_count",
        "g_xbox_code_lo",
        "g_xbox_code_hi",
        "ICALL_TRACE_SIZE",
        "recomp_lookup_manual",
        "recomp_lookup",
        "recomp_lookup_kernel",
        "recomp_icall_not_code_log",
        "recomp_icall_fail_log",
    ] {
        if pass.locals.contains_key(name) || pass.tls.contains_key(name) {
            return None;
        }
    }
    let labels = [outer, guard, native, kernel, missing]
        .into_iter()
        .map(|b| b.label.as_ref().map(|l| l.name.ident.to_string()))
        .collect::<Option<Vec<_>>>()?;
    if labels.iter().collect::<BTreeSet<_>>().len() != 5 {
        return None;
    }
    let mut normalize = Normalize {
        names: BTreeMap::from([
            (va.clone(), "_va".into()),
            (saved.clone(), "_icall_esp".into()),
            (function, "_fn".into()),
            (esp_alias, "__gamepkg_tls_g_esp".into()),
            (eax_alias, "__gamepkg_tls_g_eax".into()),
        ]),
        labels: labels
            .into_iter()
            .zip(["s_200", "s_151", "s_159", "s_165", "s_171"].map(String::from))
            .collect(),
    };
    let mut actual = syn::Block {
        brace_token: Default::default(),
        stmts: statements[..4].to_vec(),
    };
    normalize.visit_block_mut(&mut actual);
    let expected: syn::Block =
        syn::parse_str(&format!("{{{}}}", include_str!("icall_template.inc"))).ok()?;
    if actual.to_token_stream().to_string() != expected.to_token_stream().to_string() {
        return None;
    }
    let va = format_ident!("{va}");
    let saved = format_ident!("{saved}");
    Some(parse_quote!(crate::calls_runtime::invoke_call(native_state,guest_ram,#va,#saved)?;))
}

pub(super) fn count(function: &mut ItemFn) -> u32 {
    struct Counter(u32);
    impl VisitMut for Counter {
        fn visit_expr_call_mut(&mut self, call: &mut syn::ExprCall) {
            if path(&call.func).as_deref() == Some("crate::calls_runtime::invoke_call") {
                self.0 += 1;
            }
            visit_mut::visit_expr_call_mut(self, call);
        }
    }
    let mut counter = Counter(0);
    counter.visit_item_fn_mut(function);
    counter.0
}

#[cfg(test)]
mod tests {
    use super::*;
    fn candidate(
        pattern: &str,
        prefix: &str,
    ) -> Result<(ItemFn, BTreeSet<String>), BTreeSet<String>> {
        let source=format!("pub unsafe extern \"C\" fn sub_100(){{let __gamepkg_tls_g_esp=unsafe{{gamepkg_tls_g_esp()}};let __gamepkg_tls_g_eax=unsafe{{gamepkg_tls_g_eax()}};let mut _va:uint32_t=1;let mut _icall_esp:uint32_t=2;{prefix}{pattern}}}");
        lower(syn::parse_str(&source).unwrap())
    }
    #[test]
    fn exact_sequence_becomes_one_checked_direct_call_and_accepts_scoped_numbered_names() {
        let template = include_str!("icall_template.inc");
        let (function, deps) = candidate(template, "").unwrap();
        assert!(deps.is_empty());
        let source = function.to_token_stream().to_string();
        assert!(source.contains("calls_runtime :: invoke_call"));
        for removed in [
            "recomp_lookup",
            "g_icall_",
            "g_xbox_code_",
            "expect",
            "core :: ptr",
            "recomp_func_t",
        ] {
            assert!(!source.contains(removed), "{source}");
        }
        let numbered = template
            .replace("_va", "_va_7")
            .replace("_icall_esp", "_icall_esp_3")
            .replace("_fn", "_fn_2")
            .replace("'s_", "'numbered_");
        assert!(candidate(
            &numbered,
            "let mut _va_7:uint32_t=1;let mut _icall_esp_3:uint32_t=2;"
        )
        .is_ok());
        assert!(candidate(&format!("{{{template}}}{{{template}}}"), "").is_ok());
    }
    #[test]
    fn partial_altered_shadowed_wrong_width_or_wrong_register_sequences_remain_rejected() {
        let original = include_str!("icall_template.inc");
        for (from, to) in [
            ("wrapping_add(1)", "wrapping_add(2)"),
            ("ICALL_TRACE_SIZE - 1", "ICALL_TRACE_SIZE - 2"),
            ("0xfe000000", "0xfd000000"),
            ("g_xbox_code_hi == 0", "g_xbox_code_hi != 0"),
            ("recomp_lookup_manual(_va)", "recomp_lookup(_va)"),
            (
                "*__gamepkg_tls_g_esp) = _icall_esp",
                "*__gamepkg_tls_g_eax) = _icall_esp",
            ),
            ("non-null function pointer", "different marker"),
            ("g_icall_trace_idx,", "g_icall_count,"),
            ("_fn.expect", "other.expect"),
        ] {
            assert!(original.contains(from), "missing mutation {from}");
            let changed = original.replace(from, to);
            assert!(candidate(&changed, "").is_err(), "{from}");
        }
        for prefix in [
            "let recomp_lookup:uint32_t=1;",
            "let g_icall_trace_idx:uint32_t=0;",
            "let _va:uint64_t=0;",
            "let _icall_esp:uint64_t=0;",
        ] {
            assert!(candidate(original, prefix).is_err(), "{prefix}");
        }
        let shortened = &original[..original.find("    's_200:").unwrap()];
        assert!(candidate(shortened, "").is_err());
    }
}
