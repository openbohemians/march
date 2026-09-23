//! Shared surface-level AST for March definitions.
//!
//! This module provides a structured representation of user-authored catalog
//! items (effects, primitives, words, guards, state snapshots, inet rules,
//! etc.).  Frontends such as the YAML loader or a future S-expression parser
//! can decode their input into these specs, and a lowering layer can convert
//! specs into the canonical CBOR objects stored in the March database.

use std::collections::BTreeMap;

use anyhow::{Result, anyhow, bail};

use crate::cid;
use crate::interp::Value;
use crate::sexpr::{SExpr, parse_sequence as parse_sexpr_sequence};
use crate::types::{EffectMask, TypeTag, effect_mask};

/// Unified definition enum covering all user-declarable artifacts.
#[derive(Clone, Debug, PartialEq)]
pub enum Definition {
    Effect(EffectSpec),
    Prim(PrimSpec),
    Word(WordSpec),
    Guard(GuardSpec),
    OverloadSet(OverloadSetSpec),
    State(StateSpec),
    Namespace(NamespaceSpec),
    Interface(InterfaceSpec),
    Agent(AgentSpec),
    Rule(RuleSpec),
}

/// Catalog entry ties a definition to its namespace/symbol.
#[derive(Clone, Debug)]
pub struct CatalogEntry {
    pub namespace: String,
    pub symbol: String,
    pub definition: Definition,
}

impl CatalogEntry {
    pub fn full_name(&self) -> String {
        if self.namespace.is_empty() {
            self.symbol.clone()
        } else if self.symbol.is_empty() {
            self.namespace.clone()
        } else {
            format!("{}/{}", self.namespace, self.symbol)
        }
    }
}

/// Effect catalogue entry.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectSpec {
    pub name: String,
    pub doc: Option<String>,
}

/// Primitive descriptor.
#[derive(Clone, Debug, PartialEq)]
pub struct PrimSpec {
    pub name: String,
    pub params: Vec<TypeTag>,
    pub results: Vec<TypeTag>,
    pub effects: Vec<[u8; 32]>,
    pub effect_mask: EffectMask,
}

/// Guard definition (lowered to a word internally but tracked separately for clarity).
#[derive(Clone, Debug, PartialEq)]
pub struct GuardSpec {
    pub name: String,
    pub params: Vec<TypeTag>,
    pub results: Vec<TypeTag>,
    pub ops: Vec<StackOp>,
}

/// Word definition.
#[derive(Clone, Debug, PartialEq)]
pub struct WordSpec {
    pub name: String,
    pub params: Vec<TypeTag>,
    pub results: Vec<TypeTag>,
    pub ops: Vec<StackOp>,
    pub guards: Vec<String>,
}

/// Alternative implementations for an overload set. Each entry becomes a synthetic word.
#[derive(Clone, Debug, PartialEq)]
pub struct OverloadSetSpec {
    pub name: String,
    pub entries: Vec<OverloadEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OverloadEntry {
    pub params: Vec<TypeTag>,
    pub results: Vec<TypeTag>,
    pub guards: Vec<String>,
    pub ops: Vec<StackOp>,
}

/// Persistent state snapshot stored in the global store.
#[derive(Clone, Debug, PartialEq)]
pub struct StateSpec {
    pub name: String,
    /// Fully-qualified keys mapped to values.
    pub entries: BTreeMap<String, Value>,
}

/// Namespace export/import manifest.
#[derive(Clone, Debug, PartialEq)]
pub struct NamespaceSpec {
    pub name: String,
    pub iface: Option<String>,
    pub imports: Vec<String>,
    pub exports: Vec<NamespaceExportSpec>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NamespaceExportSpec {
    pub alias: String,
    pub target: String,
}

/// Interface definition (collection of symbols + types).
#[derive(Clone, Debug, PartialEq)]
pub struct InterfaceSpec {
    pub name: String,
    pub doc: Option<String>,
    pub symbols: Vec<InterfaceSymbolSpec>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InterfaceSymbolSpec {
    pub name: String,
    pub params: Vec<TypeTag>,
    pub results: Vec<TypeTag>,
    pub effects: Vec<[u8; 32]>,
}

/// Interaction-net agent descriptor.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentSpec {
    pub name: String,
    pub ports: Vec<String>,
    pub doc: Option<String>,
}

/// Interaction-net rewrite rule descriptor.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleSpec {
    pub name: String,
    pub lhs_a: String,
    pub lhs_b: String,
    /// Rule body expressed in the S-expression DSL (stored verbatim).
    pub body: String,
}

/// Stack operation used by words and guards.
#[derive(Clone, Debug, PartialEq)]
pub enum StackOp {
    Prim(String),
    Word(String),
    Dup,
    Swap,
    Over,
    Lit(Value),
    Quote([u8; 32]),
}

/// Parse a series of S-expression definitions into typed specs.
pub fn parse_definitions_from_sexpr(input: &str) -> Result<Vec<Definition>> {
    let forms = parse_sexpr_sequence(input)?;
    let mut defs = Vec::new();
    for form in forms {
        parse_definition_recursive(form, None, &mut defs)?;
    }
    Ok(defs)
}

/// Parse an S-expression catalog document into catalog entries.
pub fn parse_catalog_from_sexpr_str(input: &str) -> Result<Vec<CatalogEntry>> {
    let defs = parse_definitions_from_sexpr(input)?;
    let mut entries = Vec::with_capacity(defs.len());
    for definition in defs {
        let name = definition_name(&definition);
        let (namespace, symbol) = split_full_name(name)?;
        entries.push(CatalogEntry {
            namespace,
            symbol,
            definition,
        });
    }
    Ok(entries)
}

fn parse_definition_recursive(
    form: SExpr,
    scope: Option<&str>,
    acc: &mut Vec<Definition>,
) -> Result<()> {
    let items = match form {
        SExpr::List(items) => items,
        other => bail!("definition must be an S-expression list, found {:?}", other),
    };
    let (head, rest) = items
        .split_first()
        .ok_or_else(|| anyhow!("definition cannot be empty"))?;
    let head_sym = expect_symbol(head)?;
    match head_sym.as_str() {
        "namespace" => parse_namespace_definition(rest, scope, acc),
        "effect" => {
            let def = parse_effect_definition(rest, scope)?;
            acc.push(def);
            Ok(())
        }
        "prim" => {
            let def = parse_prim_definition(rest, scope)?;
            acc.push(def);
            Ok(())
        }
        "guard" => {
            let def = parse_guard_definition(rest, scope)?;
            acc.push(def);
            Ok(())
        }
        "word" => {
            let def = parse_word_definition(rest, scope)?;
            acc.push(def);
            Ok(())
        }
        "overloads" => {
            let def = parse_overloads_definition(rest, scope)?;
            acc.push(def);
            Ok(())
        }
        "state" => {
            let def = parse_state_definition(rest, scope)?;
            acc.push(def);
            Ok(())
        }
        other => bail!("unsupported definition kind `{other}`"),
    }
}

fn parse_effect_definition(rest: &[SExpr], scope: Option<&str>) -> Result<Definition> {
    let (name_expr, clauses) = rest
        .split_first()
        .ok_or_else(|| anyhow!("effect definition requires a name"))?;
    let raw_name = expect_name(name_expr)?;
    let name = normalize_item_name(&raw_name, scope)?;
    let mut doc = None;
    for clause in clauses {
        let (key, values) = parse_property(clause)?;
        match key.as_str() {
            "doc" => {
                ensure_single(&key, doc.is_none())?;
                doc = Some(expect_single_string(&values)?);
            }
            other => bail!("unknown `(effect {name})` property `{other}`"),
        }
    }
    Ok(Definition::Effect(EffectSpec { name, doc }))
}

fn parse_prim_definition(rest: &[SExpr], scope: Option<&str>) -> Result<Definition> {
    let (name_expr, clauses) = rest
        .split_first()
        .ok_or_else(|| anyhow!("prim definition requires a name"))?;
    let raw_name = expect_name(name_expr)?;
    let name = normalize_item_name(&raw_name, scope)?;
    let mut params: Option<Vec<TypeTag>> = None;
    let mut results: Option<Vec<TypeTag>> = None;
    let mut effects: Vec<[u8; 32]> = Vec::new();
    let mut effect_mask = effect_mask::NONE;
    for clause in clauses {
        let (key, values) = parse_property(clause)?;
        match key.as_str() {
            "params" => {
                ensure_single(&key, params.is_none())?;
                params = Some(parse_type_list(&values)?);
            }
            "results" => {
                ensure_single(&key, results.is_none())?;
                results = Some(parse_type_list(&values)?);
            }
            "effects" => {
                effects = parse_effect_list(&values)?;
            }
            "emask" => {
                effect_mask = parse_effect_mask(&values)?;
            }
            other => bail!("unknown `(prim {name})` property `{other}`"),
        }
    }
    let params = params.ok_or_else(|| anyhow!("prim `{name}` missing `params` clause"))?;
    let results = results.ok_or_else(|| anyhow!("prim `{name}` missing `results` clause"))?;
    Ok(Definition::Prim(PrimSpec {
        name,
        params,
        results,
        effects,
        effect_mask,
    }))
}

fn parse_guard_definition(rest: &[SExpr], scope: Option<&str>) -> Result<Definition> {
    let (name_expr, clauses) = rest
        .split_first()
        .ok_or_else(|| anyhow!("guard definition requires a name"))?;
    let raw_name = expect_name(name_expr)?;
    let name = normalize_item_name(&raw_name, scope)?;
    let current_scope = parent_scope(&name);
    let mut params: Option<Vec<TypeTag>> = None;
    let mut results: Option<Vec<TypeTag>> = None;
    let mut ops: Option<Vec<StackOp>> = None;
    for clause in clauses {
        let (key, values) = parse_property(clause)?;
        match key.as_str() {
            "params" => {
                ensure_single(&key, params.is_none())?;
                params = Some(parse_type_list(&values)?);
            }
            "results" => {
                ensure_single(&key, results.is_none())?;
                results = Some(parse_type_list(&values)?);
            }
            "stack" => {
                ensure_single(&key, ops.is_none())?;
                ops = Some(parse_stack_ops(&values, current_scope)?);
            }
            other => bail!("unknown `(guard {name})` property `{other}`"),
        }
    }
    let params = params.unwrap_or_default();
    let results = results.unwrap_or_else(|| vec![TypeTag::I64]);
    let ops = ops.ok_or_else(|| anyhow!("guard `{name}` missing `stack` clause"))?;
    Ok(Definition::Guard(GuardSpec {
        name,
        params,
        results,
        ops,
    }))
}

fn parse_word_definition(rest: &[SExpr], scope: Option<&str>) -> Result<Definition> {
    let (name_expr, clauses) = rest
        .split_first()
        .ok_or_else(|| anyhow!("word definition requires a name"))?;
    let raw_name = expect_name(name_expr)?;
    let name = normalize_item_name(&raw_name, scope)?;
    let current_scope = parent_scope(&name);
    let mut params: Option<Vec<TypeTag>> = None;
    let mut results: Option<Vec<TypeTag>> = None;
    let mut ops: Option<Vec<StackOp>> = None;
    let mut guards: Vec<String> = Vec::new();
    for clause in clauses {
        let (key, values) = parse_property(clause)?;
        match key.as_str() {
            "params" => {
                ensure_single(&key, params.is_none())?;
                params = Some(parse_type_list(&values)?);
            }
            "results" => {
                ensure_single(&key, results.is_none())?;
                results = Some(parse_type_list(&values)?);
            }
            "stack" => {
                ensure_single(&key, ops.is_none())?;
                ops = Some(parse_stack_ops(&values, current_scope)?);
            }
            "guards" => {
                guards = parse_name_list(&values)?
                    .into_iter()
                    .map(|name| normalize_reference(&name, current_scope))
                    .collect();
            }
            other => bail!("unknown `(word {name})` property `{other}`"),
        }
    }
    let params = params.ok_or_else(|| anyhow!("word `{name}` missing `params` clause"))?;
    let results = results.ok_or_else(|| anyhow!("word `{name}` missing `results` clause"))?;
    let ops = ops.ok_or_else(|| anyhow!("word `{name}` missing `stack` clause"))?;
    Ok(Definition::Word(WordSpec {
        name,
        params,
        results,
        ops,
        guards,
    }))
}

fn parse_overloads_definition(rest: &[SExpr], scope: Option<&str>) -> Result<Definition> {
    let (name_expr, clauses) = rest
        .split_first()
        .ok_or_else(|| anyhow!("overloads definition requires a name"))?;
    let raw_name = expect_name(name_expr)?;
    let name = normalize_item_name(&raw_name, scope)?;
    let current_scope = parent_scope(&name);
    let mut entries = Vec::new();
    for clause in clauses {
        let items = match clause {
            SExpr::List(items) => items,
            other => bail!("overload entry must be a list, found {:?}", other),
        };
        let body = if let Some(SExpr::Sym(sym)) = items.first() {
            if sym == "entry" { &items[1..] } else { items }
        } else {
            items
        };
        entries.push(parse_overload_entry(body, current_scope)?);
    }
    if entries.is_empty() {
        bail!("overloads `{name}` must contain at least one entry");
    }
    Ok(Definition::OverloadSet(OverloadSetSpec { name, entries }))
}

fn parse_state_definition(rest: &[SExpr], scope: Option<&str>) -> Result<Definition> {
    let (name_expr, clauses) = rest
        .split_first()
        .ok_or_else(|| anyhow!("state definition requires a name"))?;
    let raw_name = expect_name(name_expr)?;
    let name = normalize_item_name(&raw_name, scope)?;
    let mut entries = BTreeMap::new();
    for clause in clauses {
        let pair = match clause {
            SExpr::List(items) if items.len() == 2 => items,
            other => bail!("state entry must be `(key value)`, found {:?}", other),
        };
        let key = expect_name(&pair[0])?;
        let value = parse_value_expr(&pair[1])?;
        entries.insert(key, value);
    }
    Ok(Definition::State(StateSpec { name, entries }))
}

fn parse_namespace_definition(
    rest: &[SExpr],
    scope: Option<&str>,
    acc: &mut Vec<Definition>,
) -> Result<()> {
    let (name_expr, clauses) = rest
        .split_first()
        .ok_or_else(|| anyhow!("namespace definition requires a name"))?;
    let raw_name = expect_name(name_expr)?;
    let name = normalize_namespace_name(&raw_name, scope)?;
    let mut iface: Option<String> = None;
    let mut imports: Vec<String> = Vec::new();
    let mut exports: Vec<NamespaceExportSpec> = Vec::new();

    for clause in clauses {
        let handled = match clause {
            SExpr::List(items) if !items.is_empty() => {
                if let SExpr::Sym(keyword) = &items[0] {
                    match keyword.as_str() {
                        "iface" => {
                            ensure_single("iface", iface.is_none())?;
                            let iface_name = expect_single_name(&items[1..])?;
                            iface = Some(normalize_reference(&iface_name, Some(&name)));
                            true
                        }
                        "imports" => {
                            let list = parse_name_list(&items[1..])?;
                            for entry in list {
                                imports.push(normalize_reference(&entry, None));
                            }
                            true
                        }
                        "exports" => {
                            let specs = parse_namespace_exports(&items[1..], &name)?;
                            exports.extend(specs);
                            true
                        }
                        _ => false,
                    }
                } else {
                    false
                }
            }
            _ => false,
        };

        if !handled {
            parse_definition_recursive(clause.clone(), Some(&name), acc)?;
        }
    }

    acc.push(Definition::Namespace(NamespaceSpec {
        name,
        iface,
        imports,
        exports,
    }));
    Ok(())
}

fn parse_overload_entry(items: &[SExpr], scope: Option<&str>) -> Result<OverloadEntry> {
    let mut params: Option<Vec<TypeTag>> = None;
    let mut results: Option<Vec<TypeTag>> = None;
    let mut ops: Option<Vec<StackOp>> = None;
    let mut guards: Vec<String> = Vec::new();
    for clause in items {
        let (key, values) = parse_property(clause)?;
        match key.as_str() {
            "params" => {
                ensure_single(&key, params.is_none())?;
                params = Some(parse_type_list(&values)?);
            }
            "results" => {
                ensure_single(&key, results.is_none())?;
                results = Some(parse_type_list(&values)?);
            }
            "stack" => {
                ensure_single(&key, ops.is_none())?;
                ops = Some(parse_stack_ops(&values, scope)?);
            }
            "guards" => {
                guards = parse_name_list(&values)?
                    .into_iter()
                    .map(|name| normalize_reference(&name, scope))
                    .collect();
            }
            other => bail!("unknown overload entry property `{other}`"),
        }
    }
    let params = params.ok_or_else(|| anyhow!("overload entry missing `params` clause"))?;
    let results = results.ok_or_else(|| anyhow!("overload entry missing `results` clause"))?;
    let ops = ops.ok_or_else(|| anyhow!("overload entry missing `stack` clause"))?;
    Ok(OverloadEntry {
        params,
        results,
        guards,
        ops,
    })
}

fn parse_namespace_exports(values: &[SExpr], scope: &str) -> Result<Vec<NamespaceExportSpec>> {
    let mut exports = Vec::new();
    for entry in flatten_args(values)? {
        match entry {
            SExpr::Sym(alias) => {
                let alias_str = alias.clone();
                let target = normalize_reference(&alias_str, Some(scope));
                exports.push(NamespaceExportSpec {
                    alias: alias_str,
                    target,
                });
            }
            SExpr::Str(alias) => {
                let alias_str = alias.clone();
                let target = normalize_reference(&alias_str, Some(scope));
                exports.push(NamespaceExportSpec {
                    alias: alias_str,
                    target,
                });
            }
            SExpr::List(items) => {
                if items.is_empty() {
                    bail!("export entry cannot be empty");
                }
                let alias = expect_name(&items[0])?;
                let target_raw = if items.len() > 1 {
                    expect_name(&items[1])?
                } else {
                    alias.clone()
                };
                let target = normalize_reference(&target_raw, Some(scope));
                exports.push(NamespaceExportSpec { alias, target });
            }
        }
    }
    Ok(exports)
}

fn parse_property(clause: &SExpr) -> Result<(String, Vec<SExpr>)> {
    match clause {
        SExpr::List(items) if !items.is_empty() => {
            let key = expect_symbol(&items[0])?;
            Ok((key, items[1..].to_vec()))
        }
        other => bail!("property clause must be a list, found {:?}", other),
    }
}

fn parse_type_list(values: &[SExpr]) -> Result<Vec<TypeTag>> {
    flatten_args(values)?
        .into_iter()
        .map(|expr| {
            let atom = expect_symbol(expr)?;
            TypeTag::from_atom(&atom)
        })
        .collect()
}

fn parse_effect_list(values: &[SExpr]) -> Result<Vec<[u8; 32]>> {
    let mut out = Vec::new();
    for expr in flatten_args(values)? {
        let text = expect_name(expr)?;
        let bytes = cid::from_hex(&text)?;
        out.push(bytes);
    }
    Ok(out)
}

fn parse_effect_mask(values: &[SExpr]) -> Result<EffectMask> {
    let mut mask = effect_mask::NONE;
    for expr in flatten_args(values)? {
        let flag = expect_name(expr)?.trim().to_ascii_lowercase();
        if flag.is_empty() {
            continue;
        }
        match flag.as_str() {
            "io" => mask |= effect_mask::IO,
            "state" => mask |= effect_mask::STATE_READ | effect_mask::STATE_WRITE,
            "state.read" | "state_read" | "state-read" => mask |= effect_mask::STATE_READ,
            "state.write" | "state_write" | "state-write" => mask |= effect_mask::STATE_WRITE,
            "test" => mask |= effect_mask::TEST,
            "metric" => mask |= effect_mask::METRIC,
            other => bail!("unknown effect mask flag `{other}`"),
        }
    }
    Ok(mask)
}

fn parse_name_list(values: &[SExpr]) -> Result<Vec<String>> {
    flatten_args(values)?
        .into_iter()
        .map(|expr| expect_name(expr))
        .collect()
}

fn parse_stack_ops(values: &[SExpr], scope: Option<&str>) -> Result<Vec<StackOp>> {
    let operands: Vec<&SExpr> = if values.len() == 1 {
        match &values[0] {
            SExpr::List(inner)
                if inner
                    .first()
                    .map(|expr| matches!(expr, SExpr::List(_)))
                    .unwrap_or(false) =>
            {
                inner.iter().collect()
            }
            _ => values.iter().collect(),
        }
    } else {
        values.iter().collect()
    };
    operands
        .into_iter()
        .map(|expr| parse_stack_op(expr, scope))
        .collect()
}

fn parse_stack_op(expr: &SExpr, scope: Option<&str>) -> Result<StackOp> {
    match expr {
        SExpr::Sym(sym) => match sym.as_str() {
            "dup" => Ok(StackOp::Dup),
            "swap" => Ok(StackOp::Swap),
            "over" => Ok(StackOp::Over),
            other => bail!("unexpected bare symbol `{other}` in stack"),
        },
        SExpr::List(items) if !items.is_empty() => {
            let op = expect_symbol(&items[0])?;
            match op.as_str() {
                "prim" => {
                    let name = expect_single_name(&items[1..])?;
                    Ok(StackOp::Prim(normalize_reference(&name, scope)))
                }
                "word" => {
                    let name = expect_single_name(&items[1..])?;
                    Ok(StackOp::Word(normalize_reference(&name, scope)))
                }
                "lit" => {
                    let value_expr = items
                        .get(1)
                        .ok_or_else(|| anyhow!("`(lit ...)` requires a value"))?;
                    let value = parse_value_expr(value_expr)?;
                    Ok(StackOp::Lit(value))
                }
                "quote" => {
                    let name = expect_single_name(&items[1..])?;
                    let bytes = cid::from_hex(&name)?;
                    Ok(StackOp::Quote(bytes))
                }
                "dup" => Ok(StackOp::Dup),
                "swap" => Ok(StackOp::Swap),
                "over" => Ok(StackOp::Over),
                other => bail!("unknown stack operation `{other}`"),
            }
        }
        other => bail!("invalid stack operation expression {:?}", other),
    }
}

fn parse_value_expr(expr: &SExpr) -> Result<Value> {
    match expr {
        SExpr::Sym(sym) => {
            if sym.eq_ignore_ascii_case("unit") || sym.eq_ignore_ascii_case("null") {
                Ok(Value::Unit)
            } else if let Ok(i) = sym.parse::<i64>() {
                Ok(Value::I64(i))
            } else if let Ok(f) = sym.parse::<f64>() {
                Ok(Value::F64(f))
            } else {
                Ok(Value::Text(sym.clone()))
            }
        }
        SExpr::Str(text) => Ok(Value::Text(text.clone())),
        SExpr::List(items) if !items.is_empty() => {
            let head = expect_symbol(&items[0])?;
            match head.as_str() {
                "tuple" => {
                    let mut values = Vec::new();
                    for item in &items[1..] {
                        values.push(parse_value_expr(item)?);
                    }
                    Ok(Value::Tuple(values))
                }
                "quote" => {
                    let name = expect_single_name(&items[1..])?;
                    let bytes = cid::from_hex(&name)?;
                    Ok(Value::Quote(bytes))
                }
                other => bail!("unsupported value form `{other}`"),
            }
        }
        SExpr::List(_) => Ok(Value::Tuple(Vec::new())),
    }
}

fn flatten_args(values: &[SExpr]) -> Result<Vec<&SExpr>> {
    if values.len() == 1 {
        if let SExpr::List(inner) = &values[0] {
            return Ok(inner.iter().collect());
        }
    }
    Ok(values.iter().collect())
}

fn ensure_single(key: &str, cond: bool) -> Result<()> {
    if cond {
        Ok(())
    } else {
        bail!("property `{key}` specified more than once")
    }
}

fn expect_symbol(expr: &SExpr) -> Result<String> {
    match expr {
        SExpr::Sym(sym) => Ok(sym.clone()),
        other => bail!("expected symbol, found {:?}", other),
    }
}

fn expect_name(expr: &SExpr) -> Result<String> {
    match expr {
        SExpr::Sym(sym) => Ok(sym.clone()),
        SExpr::Str(text) => Ok(text.clone()),
        other => bail!("expected name, found {:?}", other),
    }
}

fn expect_single_string(values: &[SExpr]) -> Result<String> {
    if values.len() != 1 {
        bail!("property expects exactly one string argument");
    }
    match &values[0] {
        SExpr::Str(text) => Ok(text.clone()),
        SExpr::Sym(sym) => Ok(sym.clone()),
        other => bail!("expected string, found {:?}", other),
    }
}

fn expect_single_name(values: &[SExpr]) -> Result<String> {
    if values.len() != 1 {
        bail!("property expects exactly one name argument");
    }
    expect_name(&values[0])
}

fn normalize_reference(raw: &str, scope: Option<&str>) -> String {
    let trimmed = raw.trim().trim_start_matches('/');
    let replaced = trimmed.replace('.', "/");
    if replaced.contains('/') {
        collapse_slashes(&replaced)
    } else if let Some(scope) = scope {
        if scope.is_empty() {
            replaced
        } else {
            collapse_slashes(&format!("{scope}/{}", replaced))
        }
    } else {
        replaced
    }
}

fn normalize_item_name(raw: &str, scope: Option<&str>) -> Result<String> {
    let candidate = normalize_reference(raw, scope);
    if !candidate.contains('/') {
        bail!(
            "name `{raw}` must include a namespace; use `<namespace>/<name>` or declare it inside a `(namespace ...)` block"
        );
    }
    validate_path(&candidate)?;
    Ok(candidate)
}

fn normalize_namespace_name(raw: &str, scope: Option<&str>) -> Result<String> {
    let candidate = normalize_reference(raw, scope);
    validate_path(&candidate)?;
    Ok(candidate)
}

fn parent_scope(name: &str) -> Option<&str> {
    name.rfind('/').map(|idx| &name[..idx])
}

fn validate_path(path: &str) -> Result<()> {
    if path.split('/').any(|segment| segment.is_empty()) {
        bail!("invalid name `{path}`; empty path segment")
    }
    Ok(())
}

fn collapse_slashes(path: &str) -> String {
    let mut result = String::new();
    let mut prev_slash = false;
    for ch in path.chars() {
        if ch == '/' {
            if !prev_slash {
                result.push(ch);
            }
            prev_slash = true;
        } else {
            result.push(ch);
            prev_slash = false;
        }
    }
    if result.ends_with('/') {
        result.pop();
    }
    result
}

fn definition_name(definition: &Definition) -> &str {
    match definition {
        Definition::Effect(spec) => &spec.name,
        Definition::Prim(spec) => &spec.name,
        Definition::Word(spec) => &spec.name,
        Definition::Guard(spec) => &spec.name,
        Definition::OverloadSet(spec) => &spec.name,
        Definition::State(spec) => &spec.name,
        Definition::Namespace(spec) => &spec.name,
        Definition::Interface(spec) => &spec.name,
        Definition::Agent(spec) => &spec.name,
        Definition::Rule(spec) => &spec.name,
    }
}

fn split_full_name(name: &str) -> Result<(String, String)> {
    if let Some(idx) = name.rfind('/') {
        let namespace = &name[..idx];
        let symbol = &name[idx + 1..];
        if symbol.is_empty() {
            bail!("definition name `{name}` missing symbol segment");
        }
        if namespace.is_empty() {
            Ok((String::new(), symbol.to_string()))
        } else {
            Ok((namespace.to_string(), symbol.to_string()))
        }
    } else {
        Ok((String::new(), name.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TypeTag;

    #[test]
    fn parse_catalog_sexpr_basic() -> Result<()> {
        let doc = r#"
(seq
  (namespace core
    (effect io
      (doc "performs IO")))
  (namespace demo
    (imports core)
    (iface demo/api)
    (exports
      (add add)
      (add-twice add-twice))
    (prim add
      (params i64 i64)
      (results i64)
      (emask io))
    (guard is-positive
      (params i64)
      (stack (lit 1)))
    (word add-twice
      (params i64 i64)
      (results i64)
      (stack (dup) (prim add) (prim add))
      (guards is-positive))
    (overloads over-add
      (entry
        (params i64 i64)
        (results i64)
        (stack (prim add))))
    (state counter
      (demo.counter 0)))
)
"#;
        let entries = parse_catalog_from_sexpr_str(doc)?;
        assert_eq!(entries.len(), 8);

        let effect = entries
            .iter()
            .find(|e| e.namespace == "core" && e.symbol == "io")
            .expect("effect parsed");
        match &effect.definition {
            Definition::Effect(spec) => {
                assert_eq!(spec.name, "core/io");
                assert_eq!(spec.doc.as_deref(), Some("performs IO"));
            }
            other => panic!("expected effect, found {:?}", other),
        }

        let core_namespace = entries
            .iter()
            .find(|e| e.namespace.is_empty() && e.symbol == "core")
            .expect("core namespace parsed");
        match &core_namespace.definition {
            Definition::Namespace(spec) => {
                assert_eq!(spec.name, "core");
            }
            other => panic!("expected namespace, found {:?}", other),
        }

        let prim = entries
            .iter()
            .find(|e| e.namespace == "demo" && e.symbol == "add")
            .expect("prim parsed");
        match &prim.definition {
            Definition::Prim(spec) => {
                assert_eq!(spec.name, "demo/add");
                assert_eq!(spec.params, vec![TypeTag::I64, TypeTag::I64]);
                assert_eq!(spec.results, vec![TypeTag::I64]);
                assert_eq!(spec.effect_mask & effect_mask::IO, effect_mask::IO);
            }
            other => panic!("expected prim, found {:?}", other),
        }

        let namespace = entries
            .iter()
            .find(|e| e.namespace.is_empty() && e.symbol == "demo")
            .expect("demo namespace parsed");
        match &namespace.definition {
            Definition::Namespace(spec) => {
                assert_eq!(spec.name, "demo");
                assert_eq!(spec.imports, vec!["core"]);
                assert_eq!(spec.iface.as_deref(), Some("demo/api"));
                assert_eq!(spec.exports.len(), 2);
            }
            other => panic!("expected namespace, found {:?}", other),
        }

        let guard = entries
            .iter()
            .find(|e| e.namespace == "demo" && e.symbol == "is-positive")
            .expect("guard parsed");
        match &guard.definition {
            Definition::Guard(spec) => {
                assert_eq!(spec.results, vec![TypeTag::I64]);
                assert_eq!(spec.ops.len(), 1);
            }
            other => panic!("expected guard, found {:?}", other),
        }

        let word = entries
            .iter()
            .find(|e| e.namespace == "demo" && e.symbol == "add-twice")
            .expect("word parsed");
        match &word.definition {
            Definition::Word(spec) => {
                assert_eq!(spec.params, vec![TypeTag::I64, TypeTag::I64]);
                assert_eq!(spec.results, vec![TypeTag::I64]);
                assert_eq!(spec.guards, vec!["demo/is-positive".to_string()]);
                assert_eq!(spec.ops.len(), 3);
            }
            other => panic!("expected word, found {:?}", other),
        }

        let overload = entries
            .iter()
            .find(|e| e.namespace == "demo" && e.symbol == "over-add")
            .expect("overload parsed");
        match &overload.definition {
            Definition::OverloadSet(spec) => {
                assert_eq!(spec.entries.len(), 1);
                assert_eq!(spec.entries[0].params, vec![TypeTag::I64, TypeTag::I64]);
            }
            other => panic!("expected overload set, found {:?}", other),
        }

        let state = entries
            .iter()
            .find(|e| e.namespace == "demo" && e.symbol == "counter")
            .expect("state parsed");
        match &state.definition {
            Definition::State(spec) => {
                assert_eq!(spec.entries.len(), 1);
                assert_eq!(spec.entries.get("demo.counter"), Some(&Value::I64(0)));
            }
            other => panic!("expected state, found {:?}", other),
        }

        Ok(())
    }
}
