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
// Normalization must be injective over the fragment's names. An uncaptured
// canonical spelling cannot silently become a captured variable or label.
pub(super) fn capture_safe(
    block: &mut syn::Block,
    names: &BTreeMap<String, String>,
    labels: &BTreeMap<String, String>,
) -> bool {
    struct Check<'a> {
        names: &'a BTreeMap<String, String>,
        labels: &'a BTreeMap<String, String>,
        bad: bool,
    }
    impl VisitMut for Check<'_> {
        fn visit_ident_mut(&mut self, i: &mut syn::Ident) {
            let name = i.to_string();
            self.bad |= self.names.values().any(|v| v == &name) && !self.names.contains_key(&name);
        }
        fn visit_lifetime_mut(&mut self, l: &mut syn::Lifetime) {
            self.bad |= !self.labels.contains_key(&l.ident.to_string());
        }
    }
    let mut check = Check {
        names,
        labels,
        bad: false,
    };
    check.visit_block_mut(block);
    !check.bad
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
    let guard = block(outer.block.stmts.first()?)?;
    let (esp, eax, binding, native, kernel, missing, template) =
        match (outer.block.stmts.len(), guard.block.stmts.len()) {
            (6, 5) => (
                assignment(&guard.block.stmts[2])?,
                assignment(&guard.block.stmts[3])?,
                &outer.block.stmts[1],
                block(&outer.block.stmts[2])?,
                block(&outer.block.stmts[3])?,
                block(&outer.block.stmts[4])?,
                include_str!("icall_template.inc"),
            ),
            (4, 7) => (
                assignment(&outer.block.stmts[2])?,
                assignment(&outer.block.stmts[3])?,
                &guard.block.stmts[1],
                block(&guard.block.stmts[2])?,
                block(&guard.block.stmts[3])?,
                block(&guard.block.stmts[4])?,
                include_str!("icall_guarded_template.inc"),
            ),
            _ => return None,
        };
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
    let Stmt::Local(binding) = binding else {
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
    if !capture_safe(&mut actual, &normalize.names, &normalize.labels) {
        return None;
    }
    normalize.visit_block_mut(&mut actual);
    let expected: syn::Block = syn::parse_str(&format!("{{{template}}}")).ok()?;
    if actual.to_token_stream().to_string() != expected.to_token_stream().to_string() {
        return None;
    }
    let va = format_ident!("{va}");
    let saved = format_ident!("{saved}");
    Some(parse_quote!(crate::calls_runtime::invoke_call(native_state,guest_ram,#va,#saved)?;))
}

// At the exact tail of the current labeled block, a fallback break and normal
// fallthrough both leave that block. Do not apply this to ancestor labels,
// loop labels, unlabeled nested blocks or sequences with trailing effects.
pub(super) fn rewrite_tail(statements: &[Stmt], pass: &Lower, label: &str) -> Option<Stmt> {
    if statements.len() != 9 {
        return None;
    }
    let lifetime = syn::Lifetime::new(&format!("'{label}"), Span::call_site());
    let tail = &statements[3..];
    let wrapper: Stmt = parse_quote!(#lifetime:{#(#tail)*});
    let mut isolated = statements[..3].to_vec();
    isolated.push(wrapper);
    rewrite(&isolated, pass)
}
pub(super) fn label_used(block: &mut syn::Block, name: &str) -> bool {
    struct Used<'a> {
        name: &'a str,
        found: bool,
    }
    impl VisitMut for Used<'_> {
        fn visit_expr_break_mut(&mut self, b: &mut syn::ExprBreak) {
            self.found |= b.label.as_ref().is_some_and(|l| l.ident == self.name);
            visit_mut::visit_expr_break_mut(self, b);
        }
        fn visit_expr_continue_mut(&mut self, c: &mut syn::ExprContinue) {
            self.found |= c.label.as_ref().is_some_and(|l| l.ident == self.name);
            visit_mut::visit_expr_continue_mut(self, c);
        }
    }
    let mut used = Used { name, found: false };
    used.visit_block_mut(block);
    used.found
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
    fn guarded_layout_is_matched_in_full_with_the_same_width_tls_and_shadowing_proofs() {
        let original = include_str!("icall_guarded_template.inc");
        let (mut function, deps) = candidate(original, "").unwrap();
        assert!(deps.is_empty());
        assert_eq!(count(&mut function), 1);
        for (from, to) in [
            ("if !(_va >=", "if (_va >="),
            ("wrapping_add(1)", "wrapping_add(2)"),
            ("ICALL_TRACE_SIZE - 1", "ICALL_TRACE_SIZE - 2"),
            ("recomp_lookup_manual(_va)", "recomp_lookup(_va)"),
            (
                "(*__gamepkg_tls_g_esp) = _icall_esp",
                "(*__gamepkg_tls_g_eax) = _icall_esp",
            ),
            ("non-null function pointer", "changed marker"),
        ] {
            assert!(original.contains(from), "{from}");
            assert!(
                candidate(&original.replace(from, to), "").is_err(),
                "{from}"
            );
        }
        for prefix in [
            "let _va:uint64_t=0;",
            "let _icall_esp:uint64_t=0;",
            "let recomp_lookup_manual:uint32_t=0;",
            "let g_xbox_code_hi:uint32_t=0;",
        ] {
            assert!(candidate(original, prefix).is_err(), "{prefix}");
        }
        let numbered = original
            .replace("_va", "_va_2")
            .replace("_icall_esp", "_icall_esp_5")
            .replace("_fn", "_fn_3")
            .replace("'s_", "'renamed_");
        assert!(candidate(
            &numbered,
            "let _va_2:uint32_t=1;let _icall_esp_5:uint32_t=2;"
        )
        .is_ok());
    }
    #[test]
    fn normalization_rejects_uncaptured_canonical_variable_and_label_spellings() {
        let original = include_str!("icall_template.inc");
        let alien = original
            .replace("_va", "_va_7")
            .replacen("if _va_7 >=", "if _va >=", 1);
        assert!(candidate(&alien, "let _va_7:uint32_t=0x155a0;").is_err());
        let alien = original.replacen("'s_200:", "'foreign:", 1);
        assert!(candidate(&alien, "").is_err());
        let mut block: syn::Block = syn::parse_str("{let _va=1;}").unwrap();
        assert!(!capture_safe(
            &mut block,
            &BTreeMap::from([("_va_7".into(), "_va".into())]),
            &BTreeMap::new()
        ));
        let mut block: syn::Block = syn::parse_str("{'outer:{break 'foreign;}}").unwrap();
        assert!(!capture_safe(
            &mut block,
            &BTreeMap::new(),
            &BTreeMap::from([("outer".into(), "s_200".into())])
        ));
    }
    fn tail_pattern() -> String {
        let mut original: syn::Block =
            syn::parse_str(&format!("{{{}}}", include_str!("icall_template.inc"))).unwrap();
        let Stmt::Expr(Expr::Block(outer), None) = original.stmts.pop().unwrap() else {
            panic!("pinned template shape");
        };
        original.stmts.extend(outer.block.stmts);
        let statements = original.stmts;
        quote!('s_200:{#(#statements)*}).to_string()
    }
    #[test]
    fn tail_sequence_requires_the_current_block_label_and_no_trailing_effects() {
        let pattern = tail_pattern();
        let (mut function, deps) = candidate(&pattern, "").unwrap();
        assert!(deps.is_empty());
        assert_eq!(count(&mut function), 1);
        let invalid = [
            pattern.replacen("'s_200 : {", "'wrong : {", 1),
            pattern.replacen("'s_200 : {", "'s_200 : loop {", 1),
            pattern.replacen("'s_200 : {", "'s_200 : { {", 1).replacen(
                "_fn . expect (\"non-null function pointer\") () ;",
                "_fn . expect (\"non-null function pointer\") () ; } let trailing:u32=0;",
                1,
            ),
        ];
        for changed in invalid {
            assert_ne!(changed, pattern);
            assert!(candidate(&changed, "").is_err());
        }
        let mut block: syn::ExprBlock = syn::parse_str(&pattern).unwrap();
        block.block.stmts.push(parse_quote!(let trailing:u32=0;));
        assert!(candidate(&block.to_token_stream().to_string(), "").is_err());
    }
    #[test]
    #[ignore = "compares both pinned guard layouts on Linux/Windows 32/64-bit with controlled handlers, including Wine"]
    fn guarded_native_reference_matches_original_layout_on_four_targets() {
        let source = |template: &str| {
            format!("pub unsafe extern \"C\" fn sub_100(){{let __gamepkg_tls_g_esp=unsafe{{gamepkg_tls_g_esp()}};let __gamepkg_tls_g_eax=unsafe{{gamepkg_tls_g_eax()}};let mut _va:uint32_t=*__gamepkg_tls_g_eax;let mut _icall_esp:uint32_t=*__gamepkg_tls_g_esp;{template}let post=(*__gamepkg_tls_g_eax).wrapping_add(0x123);*__gamepkg_tls_g_eax=post;}}")
        };
        let guarded = source(include_str!("icall_guarded_template.inc"));
        let canonical: ItemFn =
            syn::parse_str(&source(include_str!("icall_template.inc"))).unwrap();
        assert!(lower(syn::parse_str(&guarded).unwrap()).is_ok());
        crate::call_reference::compare(&guarded, &canonical, "guarded");
        let tail = source(&tail_pattern());
        assert!(lower(syn::parse_str(&tail).unwrap()).is_ok());
        crate::call_reference::compare(&tail, &canonical, "tail");
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
