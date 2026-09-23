use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Result, bail};
use rusqlite::Connection;

use crate::cli::commands::util::{lookup_named_cid, require_store_path};
use march5::effect::{self, EffectCanon};
use march5::global_store::{GlobalStoreSnapshot, store_snapshot};
use march5::prim::{self, PrimCanon};
use march5::surface::{
    CatalogEntry, Definition, GuardSpec, OverloadEntry, OverloadSetSpec, PrimSpec, StackOp,
    StateSpec, WordSpec, parse_catalog_from_sexpr_str,
};
use march5::types::EffectMask;
use march5::yaml;
use march5::{TypeTag, Value, cid, get_name, open_store, put_name};

pub(crate) fn cmd_catalog(store: Option<&Path>, file: &Path, dry_run: bool) -> Result<()> {
    let catalog: Vec<CatalogEntry> = if is_sexpr_path(file) {
        let contents = fs::read_to_string(file)?;
        parse_catalog_from_sexpr_str(&contents)?
    } else {
        yaml::parse_catalog_from_file(file)?
    };
    if dry_run {
        for entry in &catalog {
            println!(
                "[dry-run] {}: {}",
                join_namespace_symbol(&entry.namespace, &entry.symbol),
                describe_definition(&entry.definition)
            );
        }
        return Ok(());
    }

    let store_path = require_store_path(store)?;
    let conn = open_store(store_path)?;
    let mut guard_specs: Vec<(String, GuardSpec)> = Vec::new();
    let mut word_specs: Vec<(String, WordSpec)> = Vec::new();
    let mut overload_sets: Vec<(String, OverloadSetSpec)> = Vec::new();
    let mut state_specs: Vec<StateSpec> = Vec::new();

    for entry in catalog {
        let namespace = entry.namespace;
        let symbol = entry.symbol;
        let full_name = join_namespace_symbol(&namespace, &symbol);
        let short_symbol = if symbol.is_empty() {
            namespace.clone()
        } else {
            symbol.clone()
        };
        match entry.definition {
            Definition::Effect(spec) => {
                let effect_spec = EffectCanon {
                    name: &spec.name,
                    doc: spec.doc.as_deref(),
                };
                let outcome = effect::store_effect(&conn, &effect_spec)?;
                put_name(&conn, "effect", &spec.name, &outcome.cid)?;
                println!(
                    "stored effect `{}` with cid {}",
                    spec.name,
                    cid::to_hex(&outcome.cid)
                );
            }
            Definition::Prim(spec) => {
                let PrimSpec {
                    name,
                    params,
                    results,
                    effects,
                    effect_mask,
                } = spec;
                let prim_spec = PrimCanon {
                    params: &params,
                    results: &results,
                    effects: effects.as_slice(),
                    effect_mask,
                };
                let outcome = prim::store_prim(&conn, &prim_spec)?;
                put_name(&conn, "prim", &name, &outcome.cid)?;
                if get_name(&conn, "prim", &short_symbol)?.is_none() {
                    put_name(&conn, "prim", &short_symbol, &outcome.cid)?;
                }
                println!(
                    "stored prim `{name}` with cid {}",
                    cid::to_hex(&outcome.cid)
                );
            }
            Definition::Guard(spec) => {
                guard_specs.push((short_symbol, spec));
            }
            Definition::Word(spec) => {
                word_specs.push((short_symbol, spec));
            }
            Definition::OverloadSet(spec) => {
                overload_sets.push((short_symbol, spec));
            }
            Definition::State(spec) => {
                state_specs.push(spec);
            }
            Definition::Namespace(spec) => {
                println!(
                    "skipping namespace `{}` (not yet supported by catalog import)",
                    spec.name
                );
            }
            Definition::Interface(_)
            | Definition::Agent(_)
            | Definition::Rule(_) => {
                bail!(
                    "catalog entry `{full_name}` uses a definition type not yet supported by the CLI"
                );
            }
        }
    }

    for (symbol, spec) in guard_specs {
        let guard_cid = apply_guard_spec(&conn, &spec)?;
        if get_name(&conn, "guard", &symbol)?.is_none() {
            put_name(&conn, "guard", &symbol, &guard_cid)?;
        }
    }

    for (symbol, spec) in word_specs {
        let word_cid = apply_word_spec(&conn, &spec)?;
        if get_name(&conn, "word", &symbol)?.is_none() {
            put_name(&conn, "word", &symbol, &word_cid)?;
        }
    }

    for (_symbol, spec) in overload_sets {
        let OverloadSetSpec { name, entries } = spec;
        let entry_count = entries.len();
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for entry in entries {
            let OverloadEntry {
                params,
                results,
                guards,
                ops,
            } = entry;
            let sig = format_signature(&params, &results);
            let counter = counts.entry(sig.clone()).or_insert(0);
            *counter += 1;
            let derived = if *counter == 1 {
                format!("{name}#{}", sig)
            } else {
                format!("{name}#{}${}", sig, *counter)
            };
            let word_spec = WordSpec {
                name: derived.clone(),
                params,
                results,
                ops,
                guards,
            };
            apply_word_spec(&conn, &word_spec)?;
        }
        println!("registered overload set `{name}` ({entry_count} entries)");
    }

    for spec in state_specs {
        let StateSpec { name, entries } = spec;
        let snapshot = GlobalStoreSnapshot::from_entries(entries);
        let outcome = store_snapshot(&conn, &snapshot)?;
        put_name(&conn, "gstate", &name, &outcome.cid)?;
        println!(
            "stored state `{}` with cid {}",
            name,
            cid::to_hex(&outcome.cid)
        );
    }

    Ok(())
}

fn is_sexpr_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let lower = ext.to_ascii_lowercase();
            matches!(lower.as_str(), "sexpr" | "sxp" | "scm" | "lisp")
        })
        .unwrap_or(false)
}

fn join_namespace_symbol(namespace: &str, symbol: &str) -> String {
    if namespace.is_empty() {
        symbol.to_string()
    } else if symbol.is_empty() {
        namespace.to_string()
    } else {
        format!("{namespace}/{symbol}")
    }
}

fn describe_definition(definition: &Definition) -> &'static str {
    match definition {
        Definition::Effect(_) => "effect",
        Definition::Prim(_) => "prim",
        Definition::Guard(_) => "guard",
        Definition::Word(_) => "word",
        Definition::OverloadSet(_) => "overloads",
        Definition::State(_) => "state",
        Definition::Namespace(_) => "namespace",
        Definition::Interface(_) => "interface",
        Definition::Agent(_) => "agent",
        Definition::Rule(_) => "rule",
    }
}

fn apply_word_spec(conn: &Connection, spec: &WordSpec) -> Result<[u8; 32]> {
    let mut builder = march5::GraphBuilder::new(conn);
    builder.begin_word(&spec.params)?;
    for guard_name in &spec.guards {
        let cid = lookup_named_cid(conn, "guard", guard_name)?;
        builder.attach_guard(cid);
    }
    apply_stack_ops(&mut builder, conn, &spec.name, &spec.ops)?;
    let word_cid = builder.finish_word(&spec.params, &spec.results, Some(&spec.name))?;
    put_name(conn, "word", &spec.name, &word_cid)?;
    println!(
        "stored word `{}` with cid {}",
        spec.name,
        cid::to_hex(&word_cid)
    );
    Ok(word_cid)
}

fn format_signature(params: &[TypeTag], results: &[TypeTag]) -> String {
    let left = params
        .iter()
        .map(|t| t.as_atom())
        .collect::<Vec<_>>()
        .join(",");
    let right = results
        .iter()
        .map(|t| t.as_atom())
        .collect::<Vec<_>>()
        .join(",");
    format!("{left}->{right}")
}

fn apply_guard_spec(conn: &Connection, spec: &GuardSpec) -> Result<[u8; 32]> {
    let mut builder = march5::GraphBuilder::new(conn);
    builder.begin_guard(&spec.params)?;
    apply_stack_ops(&mut builder, conn, &spec.name, &spec.ops)?;
    let guard_cid = builder.finish_guard(&spec.params, &spec.results, Some(&spec.name))?;
    put_name(conn, "guard", &spec.name, &guard_cid)?;
    println!(
        "stored guard `{}` with cid {}",
        spec.name,
        cid::to_hex(&guard_cid)
    );
    Ok(guard_cid)
}

fn apply_stack_ops(
    builder: &mut march5::GraphBuilder<'_>,
    conn: &Connection,
    full_name: &str,
    ops: &[StackOp],
) -> Result<()> {
    for op in ops {
        match op {
            StackOp::Prim(name) => {
                let cid = lookup_named_cid(conn, "prim", name)?;
                builder.apply_prim(cid)?;
            }
            StackOp::Word(name) => match lookup_named_cid(conn, "word", name) {
                Ok(cid) => {
                    builder.apply_word(cid)?;
                }
                Err(_) => {
                    apply_overloaded_symbol(builder, conn, name)?;
                }
            },
            StackOp::Dup => builder.dup()?,
            StackOp::Swap => builder.swap()?,
            StackOp::Over => builder.over()?,
            StackOp::Lit(value) => match value {
                Value::I64(n) => {
                    builder.push_lit_i64(*n)?;
                }
                other => bail!("unsupported literal in `{full_name}`: {:?}", other),
            },
            StackOp::Quote(cid_bytes) => {
                builder.quote(*cid_bytes)?;
            }
        }
    }
    Ok(())
}

fn apply_overloaded_symbol(
    builder: &mut march5::GraphBuilder<'_>,
    conn: &Connection,
    base_name: &str,
) -> Result<()> {
    let mut candidates: Vec<([u8; 32], Vec<TypeTag>, Vec<TypeTag>)> = Vec::new();
    let prefix = format!("{base_name}#");
    for entry in march5::db::list_names(conn, "word", Some(&prefix))? {
        let cid = entry.cid;
        let info = march5::word::load_word_info(conn, &cid)?;
        candidates.push((cid, info.params.clone(), info.results.clone()));
    }
    if candidates.is_empty() {
        bail!("word `{base_name}` not found and no overloads registered");
    }

    let mut matches: Vec<(
        [u8; 32],
        Vec<TypeTag>,
        Vec<TypeTag>,
        Vec<[u8; 32]>,
        EffectMask,
    )> = Vec::new();
    for (cid, params, results) in &candidates {
        let arity = params.len();
        let top_types = builder.peek_top_types(arity)?;
        if *params == top_types {
            let info = march5::word::load_word_info(conn, cid)?;
            matches.push((
                *cid,
                params.clone(),
                results.clone(),
                info.guards.clone(),
                info.effect_mask,
            ));
        }
    }
    if matches.is_empty() {
        bail!("no overload of `{base_name}` matches top-of-stack types");
    }
    if matches.len() == 1 && matches[0].3.is_empty() {
        builder.apply_word(matches[0].0)?;
        return Ok(());
    }
    let mut specs: Vec<march5::builder::DispatchSpec<'_>> = Vec::with_capacity(matches.len());
    for (word, params, results, guards, effect_mask) in &matches {
        specs.push(march5::builder::DispatchSpec {
            word: *word,
            params,
            results,
            guards,
            effect_mask: *effect_mask,
        });
    }
    builder.apply_dispatch(&specs)?;
    Ok(())
}
