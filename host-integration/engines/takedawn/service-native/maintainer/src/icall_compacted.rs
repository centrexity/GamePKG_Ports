//! Maintainer-only reversal of one complete, pinned compacted ICALL region.
//! This is not a general CFG flattener or a runtime dispatch mechanism.
use super::*;

fn spelling(node: &impl ToTokens) -> String {
    node.to_token_stream().to_string()
}
fn bare(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(p)
            if p.attrs.is_empty()
                && p.qself.is_none()
                && p.path.segments.len() == 1
                && p.path.leading_colon.is_none() =>
        {
            Some(p.path.segments[0].ident.to_string())
        }
        _ => None,
    }
}
fn body(expr: &Expr) -> Option<&syn::Block> {
    match expr {
        Expr::Block(b) if b.attrs.is_empty() && b.label.is_none() => Some(&b.block),
        _ => None,
    }
}
fn statement_body(statement: &Stmt) -> Option<&syn::Block> {
    match statement {
        Stmt::Expr(expr, _) => body(expr),
        _ => None,
    }
}
fn assignment(statement: &Stmt) -> Option<&syn::ExprAssign> {
    match statement {
        Stmt::Expr(Expr::Assign(a), Some(_)) if a.attrs.is_empty() => Some(a),
        _ => None,
    }
}
fn variant(path: &syn::Path, enumeration: &str) -> Option<String> {
    if path.leading_colon.is_some()
        || path.segments.len() != 2
        || path.segments[0].ident != enumeration
        || path
            .segments
            .iter()
            .any(|s| !matches!(s.arguments, syn::PathArguments::None))
    {
        return None;
    }
    let name = path.segments[1].ident.to_string();
    name.strip_prefix("S_")
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .map(|_| name.clone())
}
fn state(expr: &Expr, enumeration: &str) -> Option<String> {
    match expr {
        Expr::Path(p) if p.attrs.is_empty() && p.qself.is_none() => variant(&p.path, enumeration),
        _ => None,
    }
}
fn tag(arm: &syn::Arm, enumeration: &str) -> Option<String> {
    if !arm.attrs.is_empty() || arm.guard.is_some() {
        return None;
    }
    match &arm.pat {
        syn::Pat::Path(p) if p.attrs.is_empty() && p.qself.is_none() => {
            variant(&p.path, enumeration)
        }
        _ => None,
    }
}
fn destination(statement: &Stmt, selector: &str, enumeration: &str) -> Option<String> {
    let a = assignment(statement)?;
    (bare(&a.left)?.as_str() == selector)
        .then(|| state(&a.right, enumeration))
        .flatten()
}
fn branch(statement: &Stmt, selector: &str, enumeration: &str) -> Option<(String, String)> {
    let Stmt::Expr(Expr::If(node), _) = statement else {
        return None;
    };
    if !node.attrs.is_empty() || node.then_branch.stmts.len() != 1 {
        return None;
    }
    let other = body(&node.else_branch.as_ref()?.1)?;
    if other.stmts.len() != 1 {
        return None;
    }
    Some((
        destination(&node.then_branch.stmts[0], selector, enumeration)?,
        destination(&other.stmts[0], selector, enumeration)?,
    ))
}
fn paths(node: &mut impl PathNode) -> BTreeMap<String, usize> {
    struct Counter(BTreeMap<String, usize>);
    impl VisitMut for Counter {
        fn visit_pat_mut(&mut self, _: &mut syn::Pat) {}
        fn visit_expr_path_mut(&mut self, p: &mut syn::ExprPath) {
            *self.0.entry(spelling(&p.path)).or_default() += 1;
            visit_mut::visit_expr_path_mut(self, p);
        }
    }
    let mut counter = Counter(BTreeMap::new());
    node.walk(&mut counter);
    counter.0
}
trait PathNode {
    fn walk(&mut self, visitor: &mut impl VisitMut);
}
impl PathNode for ItemFn {
    fn walk(&mut self, v: &mut impl VisitMut) {
        v.visit_item_fn_mut(self);
    }
}
impl PathNode for syn::ExprMatch {
    fn walk(&mut self, v: &mut impl VisitMut) {
        v.visit_expr_match_mut(self);
    }
}

// The selector is a private local value, used only as a match selector or the
// LHS of assignments from literal variants. No borrow, escape or observation of
// intermediate selector writes may survive outside the reconstructed region.
fn private_selector(function: &ItemFn, selector: &str, enumeration: &str) -> bool {
    struct Check<'a> {
        selector: &'a str,
        enumeration: &'a str,
        declarations: usize,
        typed: usize,
        bad: bool,
    }
    impl VisitMut for Check<'_> {
        fn visit_local_mut(&mut self, local: &mut syn::Local) {
            if let syn::Pat::Type(t) = &local.pat {
                if let syn::Pat::Ident(i) = &*t.pat {
                    if i.ident == self.selector {
                        self.typed += 1;
                        self.bad |= !local.attrs.is_empty()
                            || !i.attrs.is_empty()
                            || i.mutability.is_none()
                            || i.by_ref.is_some()
                            || i.subpat.is_some()
                            || spelling(&t.ty) != self.enumeration
                            || local.init.as_ref().is_some_and(|init| {
                                init.diverge.is_some()
                                    || state(&init.expr, self.enumeration).is_none()
                            });
                    }
                }
            }
            visit_mut::visit_local_mut(self, local);
        }
        fn visit_expr_mut(&mut self, expr: &mut Expr) {
            match expr {
                Expr::Assign(a) if bare(&a.left).as_deref() == Some(self.selector) => {
                    self.bad |= !a.attrs.is_empty() || state(&a.right, self.enumeration).is_none();
                    self.visit_expr_mut(&mut a.right);
                    return;
                }
                Expr::Match(m) if bare(&m.expr).as_deref() == Some(self.selector) => {
                    for arm in &mut m.arms {
                        self.visit_arm_mut(arm);
                    }
                    return;
                }
                Expr::Path(_) if bare(expr).as_deref() == Some(self.selector) => self.bad = true,
                _ => {}
            }
            visit_mut::visit_expr_mut(self, expr);
        }
        fn visit_pat_ident_mut(&mut self, p: &mut syn::PatIdent) {
            // Every declaration, including shadowing in an untyped pattern, is
            // independently counted against the one typed local above.
            if p.ident == self.selector {
                self.declarations += 1;
            }
            visit_mut::visit_pat_ident_mut(self, p);
        }
    }
    let mut check = Check {
        selector,
        enumeration,
        declarations: 0,
        typed: 0,
        bad: false,
    };
    check.visit_item_fn_mut(&mut function.clone());
    !check.bad && check.declarations == 1 && check.typed == 1
}

struct Normalize<'a> {
    names: &'a BTreeMap<String, String>,
    enumeration: &'a str,
    states: BTreeMap<String, String>,
}
impl VisitMut for Normalize<'_> {
    fn visit_path_mut(&mut self, p: &mut syn::Path) {
        if let Some(name) = variant(p, self.enumeration) {
            let next = format!("S_{}", self.states.len());
            let normalized = self.states.entry(name).or_insert(next);
            p.segments[1].ident = syn::Ident::new(normalized, Span::call_site());
        }
        visit_mut::visit_path_mut(self, p);
    }
    fn visit_ident_mut(&mut self, i: &mut syn::Ident) {
        if let Some(name) = self.names.get(&i.to_string()) {
            *i = syn::Ident::new(name, i.span());
        }
    }
}
fn normalized(
    mut fragment: syn::ExprMatch,
    names: &BTreeMap<String, String>,
    enumeration: &str,
) -> Option<String> {
    let mut wrapped: syn::Block = parse_quote!({#fragment});
    if !icall::capture_safe(&mut wrapped, names, &BTreeMap::new()) {
        return None;
    }
    Normalize {
        names,
        enumeration,
        states: BTreeMap::new(),
    }
    .visit_expr_match_mut(&mut fragment);
    Some(spelling(&fragment))
}
struct Region {
    entry: usize,
    removed: BTreeSet<usize>,
    tail: usize,
    names: BTreeMap<String, String>,
    common: String,
    selector: String,
    pointer: String,
    untraced: bool,
}
fn region(
    function: &ItemFn,
    references: &BTreeMap<String, usize>,
    selectors: &mut BTreeMap<(String, String), bool>,
    m: &syn::ExprMatch,
    entry: usize,
) -> Option<Region> {
    if !m.attrs.is_empty() {
        return None;
    }
    let selector = bare(&m.expr)?;
    if !selector.starts_with("c2rust_current_block_") {
        return None;
    }
    let syn::Pat::Path(p) = &m.arms[entry].pat else {
        return None;
    };
    let enumeration = p.path.segments.first()?.ident.to_string();
    let entry_body = body(&m.arms[entry].body)?;
    let tail = entry_body.stmts.len().checked_sub(4)?;
    let fragments = entry_body.stmts[tail..].to_vec();
    let first = statement_body(&fragments[0])?;
    let fourth = statement_body(&fragments[3])?;
    let Stmt::Expr(Expr::Call(trace), Some(_)) = first.stmts.first()? else {
        return None;
    };
    if path(&trace.func).as_deref() != Some("core::ptr::write_volatile")
        || !spelling(trace.args.first()?).contains("g_icall_trace")
    {
        return None;
    }
    let va = bare(trace.args.iter().nth(1)?)?;
    let (noncode, manual) = branch(fourth.stmts.first()?, &selector, &enumeration)?;
    if !enumeration.starts_with("C2Rust_Block_")
        || !*selectors
            .entry((selector.clone(), enumeration.clone()))
            .or_insert_with(|| private_selector(function, &selector, &enumeration))
    {
        return None;
    }
    let mut indices = BTreeMap::new();
    for (index, arm) in m.arms.iter().enumerate() {
        if let Some(tag) = tag(arm, &enumeration) {
            if indices.insert(tag, index).is_some() {
                return None;
            }
        } else if !arm.attrs.is_empty()
            || arm.guard.is_some()
            || !compacted_trap_body(&arm.body)
            || !matches!(&arm.pat, syn::Pat::Or(p) if p.attrs.is_empty() && p.cases.iter().all(|case| matches!(case,syn::Pat::Path(p) if p.attrs.is_empty() && p.qself.is_none() && variant(&p.path,&enumeration).is_some())))
        {
            return None;
        }
    }
    let get = |s: &String| indices.get(s).copied();
    let nc = get(&noncode)?;
    let ma = get(&manual)?;
    let ncb = body(&m.arms[nc].body)?;
    let mab = body(&m.arms[ma].body)?;
    let saved_assignment = assignment(statement_body(ncb.stmts.get(1)?)?.stmts.first()?)?;
    let saved = bare(&saved_assignment.right)?;
    let esp = icall::tls_lhs(&saved_assignment.left)?;
    let eax_assignment = assignment(statement_body(ncb.stmts.get(2)?)?.stmts.first()?)?;
    let eax = icall::tls_lhs(&eax_assignment.left)?;
    let common = destination(
        statement_body(ncb.stmts.last()?)?.stmts.last()?,
        &selector,
        &enumeration,
    )?;
    let manual_assign = assignment(statement_body(mab.stmts.first()?)?.stmts.first()?)?;
    let pointer = bare(&manual_assign.left)?;
    if !icall::temporary(&va, "_va")
        || !icall::temporary(&saved, "_icall_esp")
        || !icall::temporary(&pointer, "_fn")
    {
        return None;
    }
    let (generated, kernel_guard) = branch(
        statement_body(mab.stmts.get(1)?)?.stmts.first()?,
        &selector,
        &enumeration,
    )?;
    let ge = get(&generated)?;
    let kg = get(&kernel_guard)?;
    let (kernel, result_guard) = branch(
        body(&m.arms[kg].body)?.stmts.first()?,
        &selector,
        &enumeration,
    )?;
    let ke = get(&kernel)?;
    let rg = get(&result_guard)?;
    let (invoke, missing) = branch(
        body(&m.arms[rg].body)?.stmts.first()?,
        &selector,
        &enumeration,
    )?;
    let iv = get(&invoke)?;
    let mi = get(&missing)?;
    let removed = BTreeSet::from([nc, ma, ge, kg, ke, rg, iv, mi]);
    if removed.len() != 8 || removed.contains(&entry) || removed.contains(&get(&common)?) {
        return None;
    }
    let mut fragment = m.clone();
    fragment.arms.clear();
    let mut head = m.arms[entry].clone();
    head.body = Box::new(parse_quote!({#(#fragments)*}));
    fragment.arms.push(head);
    for index in [nc, ma, ge, kg, ke, rg, iv, mi] {
        fragment.arms.push(m.arms[index].clone());
    }
    let names = BTreeMap::from([
        (selector.clone(), "c2rust_current_block_54".into()),
        (enumeration.clone(), "C2Rust_Block_54".into()),
        (va, "_va".into()),
        (saved, "_icall_esp".into()),
        (esp, "__gamepkg_tls_g_esp".into()),
        (eax, "__gamepkg_tls_g_eax".into()),
        (pointer.clone(), "_fn".into()),
    ]);
    if names.len() != 7 {
        return None;
    }
    let expected: syn::ExprMatch =
        syn::parse_str(include_str!("icall_compacted_template.inc")).ok()?;
    if normalized(fragment.clone(), &names, &enumeration)?
        != normalized(expected.clone(), &BTreeMap::new(), "C2Rust_Block_54")?
    {
        return None;
    }
    let inside = paths(&mut fragment);
    let entry_tag = tag(&m.arms[entry], &enumeration)?;
    for (qualified, count) in &inside {
        if let Ok(p) = syn::parse_str::<syn::Path>(qualified) {
            if let Some(name) = variant(&p, &enumeration) {
                if name != entry_tag && name != common && references.get(qualified) != Some(count) {
                    return None;
                }
            }
        }
    }
    Some(Region {
        entry,
        removed,
        tail,
        names,
        common,
        selector,
        pointer,
        untraced: false,
    })
}

// Exact second contract from the formatter: manual/generated/kernel lookup
// without trace/range logic, then a one-word stack pop only on true absence.
fn untraced_region(
    function: &ItemFn,
    references: &BTreeMap<String, usize>,
    selectors: &mut BTreeMap<(String, String), bool>,
    m: &syn::ExprMatch,
    entry: usize,
) -> Option<Region> {
    if !m.attrs.is_empty() {
        return None;
    }
    let selector = bare(&m.expr)?;
    let syn::Pat::Path(p) = &m.arms[entry].pat else {
        return None;
    };
    let enumeration = p.path.segments.first()?.ident.to_string();
    let entry_body = body(&m.arms[entry].body)?;
    let tail = entry_body.stmts.len().checked_sub(2)?;
    let fragments = entry_body.stmts[tail..].to_vec();
    let manual = assignment(statement_body(&fragments[0])?.stmts.first()?)?;
    let Expr::Call(lookup) = &*manual.right else {
        return None;
    };
    if path(&lookup.func).as_deref() != Some("recomp_lookup_manual") {
        return None;
    }
    let pointer = bare(&manual.left)?;
    let va = bare(lookup.args.first()?)?;
    if !icall::temporary(&va, "_va")
        || !icall::temporary(&pointer, "_fn")
        || !selector.starts_with("c2rust_current_block_")
        || !enumeration.starts_with("C2Rust_Block_")
        || !*selectors
            .entry((selector.clone(), enumeration.clone()))
            .or_insert_with(|| private_selector(function, &selector, &enumeration))
    {
        return None;
    }
    let mut indices = BTreeMap::new();
    for (index, arm) in m.arms.iter().enumerate() {
        if let Some(tag) = tag(arm, &enumeration) {
            if indices.insert(tag, index).is_some() {
                return None;
            }
        } else if !arm.attrs.is_empty()
            || arm.guard.is_some()
            || !compacted_trap_body(&arm.body)
            || !matches!(&arm.pat,syn::Pat::Or(p) if p.attrs.is_empty() && p.cases.iter().all(|case|matches!(case,syn::Pat::Path(p) if p.attrs.is_empty() && p.qself.is_none() && variant(&p.path,&enumeration).is_some())))
        {
            return None;
        }
    }
    let get = |s: &String| indices.get(s).copied();
    let (generated, kernel_guard) = branch(
        statement_body(&fragments[1])?.stmts.first()?,
        &selector,
        &enumeration,
    )?;
    let ge = get(&generated)?;
    let kg = get(&kernel_guard)?;
    let (kernel, result_guard) = branch(
        body(&m.arms[kg].body)?.stmts.first()?,
        &selector,
        &enumeration,
    )?;
    let ke = get(&kernel)?;
    let rg = get(&result_guard)?;
    let (invoke, missing) = branch(
        body(&m.arms[rg].body)?.stmts.first()?,
        &selector,
        &enumeration,
    )?;
    let iv = get(&invoke)?;
    let mi = get(&missing)?;
    let missing_body = body(&m.arms[mi].body)?;
    let esp = icall::tls_lhs(
        &assignment(statement_body(missing_body.stmts.get(1)?)?.stmts.first()?)?.left,
    )?;
    let eax = icall::tls_lhs(
        &assignment(statement_body(missing_body.stmts.get(2)?)?.stmts.first()?)?.left,
    )?;
    let common = destination(
        body(&m.arms[iv].body)?.stmts.last()?,
        &selector,
        &enumeration,
    )?;
    let removed = BTreeSet::from([ge, kg, ke, rg, iv, mi]);
    if removed.len() != 6 || removed.contains(&entry) || removed.contains(&get(&common)?) {
        return None;
    }
    let mut fragment = m.clone();
    fragment.arms.clear();
    let mut head = m.arms[entry].clone();
    head.body = Box::new(parse_quote!({#(#fragments)*}));
    fragment.arms.push(head);
    for index in [ge, kg, ke, rg, iv, mi] {
        fragment.arms.push(m.arms[index].clone());
    }
    let names = BTreeMap::from([
        (selector.clone(), "c2rust_current_block_30".into()),
        (enumeration.clone(), "C2Rust_Block_30".into()),
        (va, "_va".into()),
        (pointer.clone(), "_fn".into()),
        (esp, "__gamepkg_tls_g_esp".into()),
        (eax, "__gamepkg_tls_g_eax".into()),
    ]);
    if names.len() != 6 {
        return None;
    }
    let expected: syn::ExprMatch =
        syn::parse_str(include_str!("icall_jump_compacted_template.inc")).ok()?;
    if normalized(fragment.clone(), &names, &enumeration)?
        != normalized(expected, &BTreeMap::new(), "C2Rust_Block_30")?
    {
        return None;
    }
    let inside = paths(&mut fragment);
    let entry_tag = tag(&m.arms[entry], &enumeration)?;
    for (qualified, count) in &inside {
        if let Ok(p) = syn::parse_str::<syn::Path>(qualified) {
            if let Some(name) = variant(&p, &enumeration) {
                if name != entry_tag && name != common && references.get(qualified) != Some(count) {
                    return None;
                }
            }
        }
    }
    Some(Region {
        entry,
        removed,
        tail,
        names,
        common,
        selector,
        pointer,
        untraced: true,
    })
}

pub(super) fn rewrite(function: &mut ItemFn) {
    struct Rewrite {
        original: ItemFn,
        references: BTreeMap<String, usize>,
        selectors: BTreeMap<(String, String), bool>,
        occupied: BTreeSet<String>,
        pointers: BTreeSet<String>,
        serial: u32,
    }
    impl VisitMut for Rewrite {
        fn visit_expr_match_mut(&mut self, m: &mut syn::ExprMatch) {
            loop {
                let candidate = (0..m.arms.len()).find_map(|entry| {
                    region(
                        &self.original,
                        &self.references,
                        &mut self.selectors,
                        m,
                        entry,
                    )
                    .or_else(|| {
                        untraced_region(
                            &self.original,
                            &self.references,
                            &mut self.selectors,
                            m,
                            entry,
                        )
                    })
                });
                let Some(r) = candidate else {
                    break;
                };
                let pointer = loop {
                    let name = format!("_fn_{}", self.serial);
                    self.serial += 1;
                    if self.occupied.insert(name.clone()) {
                        break name;
                    }
                };
                let template = if r.untraced {
                    include_str!("icall_jump_template.inc")
                } else {
                    include_str!("icall_template.inc")
                };
                let mut canonical: syn::Block = syn::parse_str(&format!("{{{template}}}")).unwrap();
                let mut names = r
                    .names
                    .iter()
                    .map(|(original, normal)| (normal.clone(), original.clone()))
                    .collect::<BTreeMap<_, _>>();
                names.insert("_fn".into(), pointer);
                let mut normal = icall::Normalize {
                    names,
                    labels: BTreeMap::new(),
                };
                // Canonical labels are local and cannot be branch destinations
                // from the original compacted state machine.
                normal.visit_block_mut(&mut canonical);
                let selected = format_ident!("{}", r.selector);
                let enumeration = format_ident!(
                    "{}",
                    r.names
                        .iter()
                        .find(|(_, n)| n.as_str()
                            == if r.untraced {
                                "C2Rust_Block_30"
                            } else {
                                "C2Rust_Block_54"
                            })
                        .unwrap()
                        .0
                );
                let common = format_ident!("{}", r.common);
                let Expr::Block(entry_body) = &mut *m.arms[r.entry].body else {
                    unreachable!()
                };
                entry_body.block.stmts.truncate(r.tail);
                entry_body.block.stmts.extend(canonical.stmts);
                entry_body
                    .block
                    .stmts
                    .push(parse_quote!(#selected=#enumeration::#common;));
                // These tags have no incoming paths after reconstruction. Keep
                // explicit impossible arms so the pre-lowering enum remains
                // exhaustive; ordinary lowering turns the reviewed trap into Err.
                for index in r.removed {
                    m.arms[index].body = Box::new(parse_quote!({
                        unreachable!("GamePKG compacted dispatch state reached")
                    }));
                }
                self.pointers.insert(r.pointer);
            }
            visit_mut::visit_expr_match_mut(self, m);
        }
    }
    fn identifiers(tokens: proc_macro2::TokenStream, counts: &mut BTreeMap<String, usize>) {
        for token in tokens {
            match token {
                proc_macro2::TokenTree::Ident(i) => {
                    *counts.entry(i.to_string()).or_default() += 1;
                }
                proc_macro2::TokenTree::Group(g) => identifiers(g.stream(), counts),
                _ => {}
            }
        }
    }
    let mut counts = BTreeMap::new();
    identifiers(function.to_token_stream(), &mut counts);
    if !counts.contains_key("recomp_lookup_manual") {
        return;
    }
    let references = paths(function);
    let mut pass = Rewrite {
        original: function.clone(),
        references,
        selectors: BTreeMap::new(),
        occupied: counts.keys().cloned().collect(),
        pointers: BTreeSet::new(),
        serial: 0,
    };
    pass.visit_item_fn_mut(function);
    counts.clear();
    identifiers(function.to_token_stream(), &mut counts);
    struct Remove {
        names: BTreeSet<String>,
    }
    impl VisitMut for Remove {
        fn visit_block_mut(&mut self, b: &mut syn::Block) {
            b.stmts.retain(|s| {
                !self.names.iter().any(|name| {
                    let i = format_ident!("{name}");
                    {
                        let expected: Stmt = parse_quote!(let mut #i:recomp_func_t=None;);
                        spelling(s) == spelling(&expected)
                    }
                })
            });
            visit_mut::visit_block_mut(self, b);
        }
    }
    let names = pass
        .pointers
        .into_iter()
        .filter(|n| counts.get(n) == Some(&1))
        .collect();
    Remove { names }.visit_item_fn_mut(function);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn original_untraced() -> ItemFn {
        let mut m: syn::ExprMatch =
            syn::parse_str(include_str!("icall_jump_compacted_template.inc")).unwrap();
        let mut variants = paths(&mut m)
            .keys()
            .filter_map(|s| {
                syn::parse_str::<syn::Path>(s)
                    .ok()
                    .and_then(|p| variant(&p, "C2Rust_Block_30"))
            })
            .collect::<BTreeSet<_>>();
        variants.insert("S_350".into());
        let tags = variants
            .iter()
            .map(|s| format_ident!("{s}"))
            .collect::<Vec<_>>();
        let dispatched = m
            .arms
            .iter()
            .filter_map(|a| tag(a, "C2Rust_Block_30"))
            .collect::<BTreeSet<_>>();
        let traps = variants
            .iter()
            .filter(|s| !dispatched.contains(*s) && s.as_str() != "S_399")
            .map(|s| format_ident!("{s}"))
            .collect::<Vec<_>>();
        m.arms.push(parse_quote!(C2Rust_Block_30::S_399=>{*__gamepkg_tls_g_eax=(*__gamepkg_tls_g_eax).wrapping_add(0x123 as uint32_t);return;}));
        m.arms.push(parse_quote!(#(C2Rust_Block_30::#traps)|* =>{unreachable!("GamePKG compacted dispatch state reached")}));
        parse_quote! {pub unsafe extern "C" fn sub_100(){
            let __gamepkg_tls_g_esp=unsafe{gamepkg_tls_g_esp()};
            let __gamepkg_tls_g_eax=unsafe{gamepkg_tls_g_eax()};
            let mut _va:uint32_t=(*__gamepkg_tls_g_eax);
            let mut _fn:recomp_func_t=None;
            enum C2Rust_Block_30{#(#tags),*}
            let mut c2rust_current_block_30:C2Rust_Block_30=C2Rust_Block_30::S_350;
            loop{#m}
        }}
    }
    #[test]
    fn untraced_region_preserves_the_common_exit_and_never_injects_trace_logic() {
        let (mut lowered, deps) = lower(original_untraced()).unwrap();
        assert!(deps.is_empty());
        assert_eq!(icall::count(&mut lowered), 1);
        let text = spelling(&lowered);
        assert!(text.contains("calls_runtime :: invoke_jump"));
        assert!(text.contains("wrapping_add (0x123"));
        for forbidden in [
            "invoke_call",
            "recomp_func_t",
            "recomp_lookup",
            "g_icall_",
            "expect",
        ] {
            assert!(!text.contains(forbidden), "{text}");
        }
        let source = spelling(&original_untraced());
        for (old, new) in [
            (
                "wrapping_add (4 as uint32_t)",
                "wrapping_add (8 as uint32_t)",
            ),
            ("recomp_lookup_manual", "recomp_lookup_kernel"),
            ("_fn . is_some ()", "_fn . is_none ()"),
            ("S_360", "S_366"),
            ("let mut _va : uint32_t", "let mut _va : uint64_t"),
            ("loop {", "let observer=c2rust_current_block_30;loop {"),
            ("loop {", "let observer=&mut _fn;loop {"),
            ("loop {", "let recomp_lookup_manual:uint32_t=1;loop {"),
            (
                "S_399 => {",
                "S_399 => {c2rust_current_block_30=C2Rust_Block_30::S_362;",
            ),
        ] {
            assert!(source.contains(old), "{old}");
            assert!(
                lower(syn::parse_str(&source.replace(old, new)).unwrap()).is_err(),
                "{old}"
            );
        }
    }
    fn original() -> ItemFn {
        let mut m: syn::ExprMatch =
            syn::parse_str(include_str!("icall_compacted_template.inc")).unwrap();
        let mut variants = paths(&mut m)
            .keys()
            .filter_map(|s| {
                syn::parse_str::<syn::Path>(s)
                    .ok()
                    .and_then(|p| variant(&p, "C2Rust_Block_54"))
            })
            .collect::<BTreeSet<_>>();
        variants.insert("S_124".into());
        let tags = variants
            .iter()
            .map(|s| format_ident!("{s}"))
            .collect::<Vec<_>>();
        let dispatched = m
            .arms
            .iter()
            .filter_map(|a| tag(a, "C2Rust_Block_54"))
            .collect::<BTreeSet<_>>();
        let traps = variants
            .iter()
            .filter(|s| !dispatched.contains(*s) && s.as_str() != "S_224")
            .map(|s| format_ident!("{s}"))
            .collect::<Vec<_>>();
        m.arms.push(parse_quote!(C2Rust_Block_54::S_224=>{return;}));
        m.arms.push(parse_quote!(#(C2Rust_Block_54::#traps)|* =>{unreachable!("GamePKG compacted dispatch state reached")}));
        parse_quote! {pub unsafe extern "C" fn sub_100(){
            let __gamepkg_tls_g_esp=unsafe{gamepkg_tls_g_esp()};
            let __gamepkg_tls_g_eax=unsafe{gamepkg_tls_g_eax()};
            let mut _va:uint32_t=0x155a0;
            let mut _icall_esp:uint32_t=0x12345678;
            let mut _fn:recomp_func_t=None;
            enum C2Rust_Block_54{#(#tags),*}
            let mut c2rust_current_block_54:C2Rust_Block_54=C2Rust_Block_54::S_124;
            loop{#m}
        }}
    }
    #[test]
    fn complete_pinned_region_reconstructs_then_uses_the_existing_typed_icall_proof() {
        assert!(private_selector(
            &original(),
            "c2rust_current_block_54",
            "C2Rust_Block_54"
        ));
        let (mut converted, deps) = lower(original()).unwrap();
        assert!(deps.is_empty());
        assert_eq!(icall::count(&mut converted), 1);
        let text = spelling(&converted);
        for forbidden in [
            "recomp_func_t",
            "recomp_lookup",
            "g_icall_",
            "g_xbox_code_",
            "expect",
            "write_volatile",
        ] {
            assert!(!text.contains(forbidden), "{forbidden}: {text}");
        }
        let separate = spelling(&original()).replace(
            "let mut c2rust_current_block_54 : C2Rust_Block_54 =",
            "let mut c2rust_current_block_54 : C2Rust_Block_54 ; c2rust_current_block_54 =",
        );
        assert!(lower(syn::parse_str(&separate).unwrap()).is_ok());
        let numbered = spelling(&original())
            .replace("C2Rust_Block_54", "C2Rust_Block_812")
            .replace("c2rust_current_block_54", "c2rust_current_block_812")
            .replace("_va", "_va_7")
            .replace("_icall_esp", "_icall_esp_4")
            .replace("_fn", "_fn_2");
        assert!(lower(syn::parse_str(&numbered).unwrap()).is_ok());
    }
    #[test]
    fn selector_proof_requires_one_explicit_typed_binding_without_untyped_shadowing() {
        for source in [
            "fn x(){let mut selector=E::S_1;{let mut selector=E::S_1;}}",
            "fn x(){let mut selector:E=E::S_1;{let mut selector=E::S_1;}}",
            "fn x(){let mut selector:E=E::S_1;let observer=selector;}",
            "fn x(){let mut selector:E=E::S_1;let observer=&mut selector;}",
        ] {
            assert!(!private_selector(
                &syn::parse_str(source).unwrap(),
                "selector",
                "E"
            ));
        }
        for source in [
            "fn x(){let mut selector:E=E::S_1;}",
            "fn x(){let mut selector:E;selector=E::S_1;}",
        ] {
            assert!(private_selector(
                &syn::parse_str(source).unwrap(),
                "selector",
                "E"
            ));
        }
    }
    #[test]
    #[ignore = "compiles and executes the original/reconstructed region with controlled handlers on four native targets, including Wine"]
    fn compacted_native_reference_matches_original_graph_on_four_targets() {
        let source = spelling(&original())
            .replace(
                "let mut _va : uint32_t = 0x155a0",
                "let mut _va : uint32_t = (*__gamepkg_tls_g_eax)",
            )
            .replace(
                "let mut _icall_esp : uint32_t = 0x12345678",
                "let mut _icall_esp : uint32_t = (*__gamepkg_tls_g_esp)",
            );
        let original: ItemFn = syn::parse_str(&source).unwrap();
        let mut reconstructed = original.clone();
        rewrite(&mut reconstructed);
        assert!(spelling(&reconstructed).contains("'s_200"));
        assert!(lower(original.clone()).is_ok());
        crate::call_reference::compare(&source, &reconstructed, "compacted");
        let original = original_untraced();
        let source = spelling(&original);
        let mut reconstructed = original.clone();
        rewrite(&mut reconstructed);
        assert!(spelling(&reconstructed).contains("'s_jump_0"));
        assert!(lower(original).is_ok());
        crate::call_reference::compare(&source, &reconstructed, "untraced");
    }
    #[test]
    fn altered_effects_guards_transitions_aliases_incoming_edges_or_observed_state_remain_rejected()
    {
        let source = spelling(&original());
        for (old, new) in [
            ("wrapping_add (1)", "wrapping_add (2)"),
            ("0xfe000000", "0xfd000000"),
            ("recomp_lookup_manual", "recomp_lookup_kernel"),
            ("_fn . is_some ()", "_fn . is_none ()"),
            ("S_156", "S_158"),
            (
                "_fn . expect (\"non-null function pointer\") ()",
                "_fn . expect (\"non-null function pointer\") () ; sub_200 ()",
            ),
            (
                "S_224 => { return ; }",
                "S_224 => { c2rust_current_block_54 = C2Rust_Block_54 :: S_163 ; return ; }",
            ),
            ("let mut _va : uint32_t", "let mut _va : uint64_t"),
            ("loop {", "let observed = c2rust_current_block_54 ; loop {"),
            (
                "loop {",
                "let observed = & mut c2rust_current_block_54 ; loop {",
            ),
            ("loop {", "let g_xbox_code_hi : uint32_t = 0 ; loop {"),
        ] {
            assert!(source.contains(old), "missing mutation: {old}");
            let changed: ItemFn = syn::parse_str(&source.replace(old, new)).unwrap();
            assert!(lower(changed).is_err(), "accepted mutation: {old}");
        }
    }
}
