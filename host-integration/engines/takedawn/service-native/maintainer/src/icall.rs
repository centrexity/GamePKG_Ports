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

// Generic argument lists permit a trailing comma without changing arguments.
// Canonicalize only that punctuation on the comparison copies, on both sides.
// Types, argument count/order, calls and every other token remain significant.
fn generic_punctuation(block: &mut syn::Block) {
    struct Canonical;
    impl VisitMut for Canonical {
        fn visit_angle_bracketed_generic_arguments_mut(
            &mut self,
            arguments: &mut syn::AngleBracketedGenericArguments,
        ) {
            arguments.args.pop_punct();
            visit_mut::visit_angle_bracketed_generic_arguments_mut(self, arguments);
        }
    }
    Canonical.visit_block_mut(block);
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
    let mut expected: syn::Block = syn::parse_str(&format!("{{{template}}}")).ok()?;
    generic_punctuation(&mut actual);
    generic_punctuation(&mut expected);
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
// At the exact tail of this loop, both fallback continues and successful
// fallthrough start its next iteration. On comparison copies only, express
// those two continues as exits from an isolated block and reuse the full proof.
pub(super) fn rewrite_loop_tail(statements: &[Stmt], pass: &Lower, label: &str) -> Option<Stmt> {
    if statements.len() != 9 {
        return None;
    }
    struct Continues<'a> {
        label: &'a str,
        count: u32,
    }
    impl VisitMut for Continues<'_> {
        fn visit_expr_mut(&mut self, expr: &mut Expr) {
            if let Expr::Continue(node) = expr {
                if node.attrs.is_empty()
                    && node.label.as_ref().is_some_and(|l| l.ident == self.label)
                {
                    let label = node.label.clone().unwrap();
                    *expr = parse_quote!(break #label);
                    self.count += 1;
                    return;
                }
            }
            visit_mut::visit_expr_mut(self, expr);
        }
    }
    let mut isolated = syn::Block {
        brace_token: Default::default(),
        stmts: statements.to_vec(),
    };
    let mut continues = Continues { label, count: 0 };
    continues.visit_block_mut(&mut isolated);
    if continues.count != 2 {
        return None;
    }
    rewrite_tail(&isolated.stmts, pass, label)
}
// A common ancestor exit is safe only when normal completion explicitly takes
// that same exit. Keep the original break in its original lexical scope: the
// checked helper replaces the call machinery, not the caller's control flow.
pub(super) fn rewrite_leaving(statements: &[Stmt], pass: &Lower) -> Option<Vec<Stmt>> {
    if statements.len() != 10 {
        return None;
    }
    let Stmt::Expr(Expr::Break(exit), Some(_)) = &statements[9] else {
        return None;
    };
    if !exit.attrs.is_empty() || exit.expr.is_some() {
        return None;
    }
    let label = exit.label.as_ref()?.ident.to_string();
    let call = rewrite_tail(&statements[..9], pass, &label)?;
    Some(vec![call, statements[9].clone()])
}
// Separate original lookup contract: no trace or code-range check, and one
// wrapping guest stack pop on absence. Never route this through invoke_call.
pub(super) fn rewrite_jump(statements: &[Stmt], pass: &Lower) -> Option<Stmt> {
    let outer = block(statements.first()?)?;
    if outer.block.stmts.len() != 5 {
        return None;
    }
    let Stmt::Local(binding) = &outer.block.stmts[0] else {
        return None;
    };
    let syn::Pat::Type(ty) = &binding.pat else {
        return None;
    };
    let syn::Pat::Ident(function) = &*ty.pat else {
        return None;
    };
    let Expr::Call(lookup) = plain(&binding.init.as_ref()?.expr) else {
        return None;
    };
    let target = lookup.args.first()?;
    let va = path(target)?;
    if !temporary(&va, "_va") || !pass.guest_u32(target) {
        return None;
    }
    let pointer = function.ident.to_string();
    if !temporary(&pointer, "_fn") {
        return None;
    }
    let native = block(&outer.block.stmts[1])?;
    let kernel = block(&outer.block.stmts[2])?;
    let missing = block(&outer.block.stmts[3])?;
    if missing.block.stmts.len() != 5 {
        return None;
    }
    let esp = tls_lhs(&assignment(&missing.block.stmts[2])?.left)?;
    let eax = tls_lhs(&assignment(&missing.block.stmts[3])?.left)?;
    if pass.tls.get(&esp).map(String::as_str) != Some("esp")
        || pass.tls.get(&eax).map(String::as_str) != Some("eax")
    {
        return None;
    }
    for name in [
        "recomp_lookup_manual",
        "recomp_lookup",
        "recomp_lookup_kernel",
        "recomp_icall_fail_log",
    ] {
        if pass.locals.contains_key(name) || pass.tls.contains_key(name) {
            return None;
        }
    }
    let labels = [outer, native, kernel, missing]
        .into_iter()
        .map(|b| b.label.as_ref().map(|l| l.name.ident.to_string()))
        .collect::<Option<Vec<_>>>()?;
    if labels.iter().collect::<BTreeSet<_>>().len() != 4 {
        return None;
    }
    let mut normalize = Normalize {
        names: BTreeMap::from([
            (va.clone(), "_va".into()),
            (pointer, "_fn".into()),
            (esp, "__gamepkg_tls_g_esp".into()),
            (eax, "__gamepkg_tls_g_eax".into()),
        ]),
        labels: labels
            .into_iter()
            .zip(["s_jump_0", "s_jump_1", "s_jump_2", "s_jump_3"].map(String::from))
            .collect(),
    };
    if normalize.names.len() != 4 {
        return None;
    }
    let mut actual: syn::Block = parse_quote!({#outer});
    if !capture_safe(&mut actual, &normalize.names, &normalize.labels) {
        return None;
    }
    normalize.visit_block_mut(&mut actual);
    let mut expected: syn::Block =
        syn::parse_str(&format!("{{{}}}", include_str!("icall_jump_template.inc"))).ok()?;
    generic_punctuation(&mut actual);
    generic_punctuation(&mut expected);
    if actual.to_token_stream().to_string() != expected.to_token_stream().to_string() {
        return None;
    }
    let va = format_ident!("{va}");
    Some(parse_quote!(crate::calls_runtime::invoke_jump(native_state,guest_ram,#va)?;))
}
// Some pinned tail wrappers put the three lookup statements just before
// the labeled outcome block. Reconstruct the existing checked shape.
pub(super) fn rewrite_flat_jump(statements: &[Stmt], pass: &Lower) -> Option<Stmt> {
    let [binding, native, kernel, Stmt::Expr(Expr::Block(outcome), Some(_))] = statements else {
        return None;
    };
    if !matches!(binding, Stmt::Local(_)) || !outcome.attrs.is_empty() {
        return None;
    }
    let [missing, invoke] = outcome.block.stmts.as_slice() else {
        return None;
    };
    let mut outer = outcome.clone();
    outer.block.stmts = vec![
        binding.clone(),
        native.clone(),
        kernel.clone(),
        missing.clone(),
        invoke.clone(),
    ];
    rewrite_jump(&[Stmt::Expr(Expr::Block(outer), None)], pass)
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
            if matches!(
                path(&call.func).as_deref(),
                Some("crate::calls_runtime::invoke_call" | "crate::calls_runtime::invoke_jump")
            ) {
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
    fn tail_statements() -> Vec<Stmt> {
        let mut original: syn::Block =
            syn::parse_str(&format!("{{{}}}", include_str!("icall_template.inc"))).unwrap();
        let Stmt::Expr(Expr::Block(outer), None) = original.stmts.pop().unwrap() else {
            panic!("pinned template shape");
        };
        original.stmts.extend(outer.block.stmts);
        original.stmts
    }
    fn tail_pattern() -> String {
        let statements = tail_statements();
        quote!('s_200:{#(#statements)*}).to_string()
    }
    fn alternate_commas(pattern: &str) -> String {
        struct Toggle;
        impl VisitMut for Toggle {
            fn visit_angle_bracketed_generic_arguments_mut(
                &mut self,
                arguments: &mut syn::AngleBracketedGenericArguments,
            ) {
                if !arguments.args.is_empty() {
                    if arguments.args.trailing_punct() {
                        arguments.args.pop_punct();
                    } else {
                        arguments.args.push_punct(Default::default());
                    }
                }
                visit_mut::visit_angle_bracketed_generic_arguments_mut(self, arguments);
            }
        }
        let mut block: syn::Block = syn::parse_str(&format!("{{{pattern}}}")).unwrap();
        Toggle.visit_block_mut(&mut block);
        let statements = block.stmts;
        quote!(#(#statements)*).to_string()
    }
    #[test]
    fn optional_generic_commas_preserve_the_exact_types_and_argument_list() {
        for pattern in [
            include_str!("icall_template.inc"),
            include_str!("icall_guarded_template.inc"),
        ] {
            let changed = alternate_commas(pattern);
            let (mut function, _) = candidate(&changed, "").unwrap();
            assert_eq!(count(&mut function), 1);
            for alteration in [
                changed.replacen("< uint32_t >", "< uint32_t , uint32_t >", 1),
                changed.replacen("< uint32_t >", "< uint64_t >", 1),
            ] {
                assert_ne!(alteration, changed);
                assert!(candidate(&alteration, "").is_err());
            }
        }
    }
    #[test]
    fn untraced_lookup_requires_the_entire_distinct_lookup_and_stack_contract() {
        let template = include_str!("icall_jump_template.inc");
        let (mut function, deps) = candidate(template, "").unwrap();
        assert!(deps.is_empty());
        assert_eq!(count(&mut function), 1);
        let text = function.to_token_stream().to_string();
        assert!(text.contains("calls_runtime :: invoke_jump"));
        assert!(!text.contains("invoke_call"));
        for (old, new) in [
            ("recomp_lookup_manual(_va)", "recomp_lookup(_va)"),
            ("wrapping_add(4 as uint32_t)", "wrapping_add(8 as uint32_t)"),
            ("if _fn.is_some()", "if _fn.is_none()"),
            ("0 as uint32_t", "1 as uint32_t"),
            ("break 's_jump_0", "break 'elsewhere"),
            ("non-null function pointer", "different marker"),
        ] {
            assert!(template.contains(old));
            assert!(candidate(&template.replace(old, new), "").is_err(), "{old}");
        }
        for prefix in [
            "let _va:uint64_t=1;",
            "let recomp_lookup_manual:uint32_t=1;",
            "let recomp_icall_fail_log:uint32_t=1;",
        ] {
            assert!(candidate(template, prefix).is_err(), "{prefix}");
        }
        let changed = template
            .replace("_va", "_va_7")
            .replace("_fn", "_fn_2")
            .replace("'s_jump_", "'renamed_");
        assert!(candidate(&changed, "let _va_7:uint32_t=1;").is_ok());
    }
    #[test]
    fn flattened_tail_uses_the_same_untraced_lookup_proof() {
        let canonical: syn::ExprBlock =
            syn::parse_str(include_str!("icall_jump_template.inc")).unwrap();
        let label = canonical.label.unwrap();
        let parts = canonical.block.stmts;
        let (prefix, exit) = parts.split_at(3);
        let flattened = quote!(#(#prefix)* #label { #(#exit)* };).to_string();
        let convert = |name: &str, pattern: &str, bindings: &str| {
            let source = format!(
                "pub unsafe extern \"C\" fn {name}() {{
                    let __gamepkg_tls_g_esp=unsafe{{gamepkg_tls_g_esp()}};
                    let __gamepkg_tls_g_eax=unsafe{{gamepkg_tls_g_eax()}};
                    let _va:uint32_t=1;
                    {bindings}
                    {pattern}
                }}"
            );
            lower(syn::parse_str(&source).unwrap())
        };
        for name in ["sub_001D7012", "sub_001D7024", "sub_001D9290", "sub_100"] {
            let (mut lowered, deps) = convert(name, &flattened, "").unwrap();
            assert!(deps.is_empty());
            assert_eq!(count(&mut lowered), 1);
            let text = lowered.to_token_stream().to_string();
            assert!(
                text.contains("calls_runtime :: invoke_jump") && !text.contains("recomp_func_t")
            );
        }
        let subject = "sub_001D7024";
        assert!(convert(subject, &format!("{flattened} _va.wrapping_add(1);"), "").is_err());
        assert!(convert(subject, &flattened, "let recomp_lookup:uint32_t=0;").is_err());
        assert!(convert(subject, &flattened, "let _va:uint64_t=0;").is_err());
        for (from, to) in [
            ("recomp_lookup_kernel", "recomp_lookup"),
            (
                "wrapping_add (4 as uint32_t)",
                "wrapping_add (8 as uint32_t)",
            ),
            ("non-null function pointer", "unexpected diagnostic"),
            ("break 's_jump_0", "break 's_jump_2"),
            ("recomp_lookup_manual (_va)", "recomp_lookup (_va)"),
        ] {
            assert!(flattened.contains(from));
            assert!(convert(subject, &flattened.replace(from, to), "").is_err());
        }
    }
    fn leaving_pattern(body: &str) -> String {
        format!(
            "'s_200:{{'inner:{{if _va==0{{break 'inner;}}{body}}}*__gamepkg_tls_g_eax^=0x1111;}}"
        )
    }
    fn loop_pattern(body: &str) -> String {
        format!("let mut iteration:uint32_t=0;'s_200:loop{{iteration=iteration.wrapping_add(1);if iteration>3u32{{break;}}*__gamepkg_tls_g_eax^=iteration;{body}}}")
    }
    fn loop_tail() -> String {
        let statements = tail_statements();
        quote!(#(#statements)*)
            .to_string()
            .replace("break 's_200 ;", "continue 's_200 ;")
    }
    #[test]
    fn loop_tail_requires_two_current_loop_continues_and_no_trailing_effects() {
        let body = loop_tail();
        let pattern = loop_pattern(&body);
        let (mut function, deps) = candidate(&pattern, "").unwrap();
        assert!(deps.is_empty());
        assert_eq!(count(&mut function), 1);
        let text = function.to_token_stream().to_string();
        assert!(text.contains("loop {"));
        assert!(text.contains("iteration"));
        assert!(!text.contains("recomp_lookup"));
        for changed in [
            loop_pattern(&body.replacen("continue 's_200 ;", "break 's_200 ;", 1)),
            loop_pattern(&body.replace("continue 's_200 ;", "break 's_200 ;")),
            loop_pattern(&body.replacen("continue 's_200 ;", "continue 'ancestor ;", 1)),
            loop_pattern(&format!("{{{body}}}")),
            loop_pattern(&format!("{body}*__gamepkg_tls_g_eax^=123u32;")),
            pattern.replace("'s_200:loop", "'different:loop"),
            pattern.replace("'s_200:loop", "'s_200:while true"),
            loop_pattern(&body.replace("wrapping_add (1)", "wrapping_add (2)")),
        ] {
            assert_ne!(changed, pattern);
            assert!(candidate(&changed, "").is_err(), "{changed}");
        }
        assert!(candidate(&pattern, "let g_xbox_code_hi:uint32_t=0;").is_err());
    }
    #[test]
    fn common_ancestor_exit_is_preserved_and_requires_an_exact_unconditional_jump() {
        let statements = tail_statements();
        let body = quote!(#(#statements)* break 's_200;).to_string();
        let pattern = leaving_pattern(&body);
        let (mut function, deps) = candidate(&pattern, "").unwrap();
        assert!(deps.is_empty());
        assert_eq!(count(&mut function), 1);
        let lowered = function.to_token_stream().to_string();
        assert!(lowered.contains("break 's_200 ;"), "{lowered}");
        assert!(lowered.contains("break 'inner ;"), "{lowered}");
        assert!(lowered.contains("^= 0x1111"), "{lowered}");
        // A loop exit is also retained, never converted to plain fallthrough.
        let loop_pattern = pattern.replacen("'s_200:{", "'s_200:loop{", 1);
        assert!(candidate(&loop_pattern, "").is_ok());
        for changed in [
            body.replacen("break 's_200", "break 'other", 1),
            format!("{} break 'other;", quote!(#(#statements)*)),
            format!("{} break;", quote!(#(#statements)*)),
            format!("{} break 's_200 7;", quote!(#(#statements)*)),
            format!("{body}let trailing:u32=1;"),
            body.replacen("_va >=", "_icall_esp >=", 1),
        ] {
            assert_ne!(changed, body);
            assert!(candidate(&leaving_pattern(&changed), "").is_err());
        }
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
        let statements = tail_statements();
        let leaving = source(&leaving_pattern(&alternate_commas(
            &quote!(#(#statements)* break 's_200;).to_string(),
        )));
        let canonical: ItemFn = syn::parse_str(&source(&leaving_pattern(&format!(
            "{} break 's_200;",
            include_str!("icall_template.inc").replace("'s_", "'owned_")
        ))))
        .unwrap();
        assert!(lower(syn::parse_str(&leaving).unwrap()).is_ok());
        crate::call_reference::compare(&leaving, &canonical, "leaving");
        let repeated = source(&loop_pattern(&loop_tail()));
        let owned = loop_pattern(&include_str!("icall_template.inc").replace("'s_", "'owned_"))
            .replace("'s_200:loop", "loop");
        let canonical: ItemFn = syn::parse_str(&source(&owned)).unwrap();
        assert!(lower(syn::parse_str(&repeated).unwrap()).is_ok());
        crate::call_reference::compare(&repeated, &canonical, "loop-tail");
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
