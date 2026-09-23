use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Result, anyhow, bail};

use crate::interp::Value;
use crate::surface::{
    CatalogEntry, Definition, EffectSpec, GuardSpec, OverloadEntry, OverloadSetSpec, PrimSpec,
    StackOp, StateSpec, WordSpec,
};
use crate::types::{EffectMask, TypeTag, effect_mask};

#[derive(Clone, Debug)]
pub enum Node {
    Scalar(String),
    Sequence(Vec<Node>),
    Mapping(BTreeMap<String, Node>),
    Tagged { tag: String, value: Box<Node> },
}

struct Line<'a> {
    indent: usize,
    content: &'a str,
}

fn preprocess(input: &str) -> Vec<Line<'_>> {
    input
        .lines()
        .filter_map(|raw| {
            let stripped = raw.split('#').next().unwrap_or("").trim_end();
            let trimmed = stripped.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('%') || trimmed == "---" {
                return None;
            }
            let indent = stripped.len() - trimmed.len();
            Some(Line {
                indent,
                content: trimmed,
            })
        })
        .collect()
}

fn parse_node(lines: &[Line<'_>], idx: &mut usize, indent: usize) -> Result<Node> {
    if *idx >= lines.len() {
        bail!("unexpected end of document");
    }
    let line = &lines[*idx];
    if line.indent < indent {
        bail!("invalid indentation at line {}", *idx + 1);
    }
    if line.content.starts_with("- ") {
        parse_sequence(lines, idx, indent)
    } else if let Some(pos) = line.content.find(':') {
        if pos == line.content.len() - 1 {
            parse_mapping(lines, idx, indent)
        } else {
            // check if colon is part of scalar (e.g., "http://")
            let (_key, remainder) = line.content.split_at(pos);
            if remainder.starts_with("://") {
                parse_scalar_node(lines, idx, indent)
            } else {
                parse_mapping(lines, idx, indent)
            }
        }
    } else {
        parse_scalar_node(lines, idx, indent)
    }
}

fn parse_sequence(lines: &[Line<'_>], idx: &mut usize, indent: usize) -> Result<Node> {
    let mut items = Vec::new();
    while *idx < lines.len() {
        let line = &lines[*idx];
        if line.indent < indent {
            break;
        }
        if !line.content.starts_with("- ") || line.indent != indent {
            break;
        }
        let remainder = &line.content[2..].trim_start();
        *idx += 1;
        let item = if remainder.is_empty() {
            parse_node(lines, idx, indent + 2)?
        } else {
            parse_tag_or_scalar(lines, idx, indent + 2, remainder)?
        };
        items.push(item);
    }
    Ok(Node::Sequence(items))
}

fn parse_mapping(lines: &[Line<'_>], idx: &mut usize, indent: usize) -> Result<Node> {
    let mut map = BTreeMap::new();
    while *idx < lines.len() {
        let line = &lines[*idx];
        if line.indent < indent {
            break;
        }
        if line.indent != indent {
            break;
        }
        let mut parts = line.content.splitn(2, ':');
        let key = parts.next().unwrap().trim();
        let rest = parts.next().unwrap_or("").trim_start();
        *idx += 1;
        let value = if rest.is_empty() {
            parse_node(lines, idx, indent + 2)?
        } else {
            parse_tag_or_scalar(lines, idx, indent + 2, rest)?
        };
        if map.insert(key.to_string(), value).is_some() {
            bail!("duplicate key `{key}` in mapping");
        }
    }
    Ok(Node::Mapping(map))
}

fn parse_tag_or_scalar(
    lines: &[Line<'_>],
    idx: &mut usize,
    indent: usize,
    text: &str,
) -> Result<Node> {
    if let Some(rest) = text.strip_prefix('!') {
        let mut parts = rest.splitn(2, char::is_whitespace);
        let tag = parts
            .next()
            .ok_or_else(|| anyhow!("missing tag after `!`"))?
            .to_string();
        let remainder = parts
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("");
        let inner = if remainder.is_empty() {
            if *idx >= lines.len() || lines[*idx].indent < indent {
                Node::Scalar(String::new())
            } else {
                parse_node(lines, idx, indent)?
            }
        } else {
            parse_tag_or_scalar(lines, idx, indent, remainder)?
        };
        Ok(Node::Tagged {
            tag,
            value: Box::new(inner),
        })
    } else {
        if let Some(seq) = parse_inline_sequence(text) {
            Ok(Node::Sequence(seq))
        } else {
            Ok(Node::Scalar(text.to_string()))
        }
    }
}

fn parse_inline_sequence(text: &str) -> Option<Vec<Node>> {
    let trimmed = text.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return None;
    }
    let mut items = Vec::new();
    let inner = &trimmed[1..trimmed.len() - 1];
    if inner.trim().is_empty() {
        return Some(items);
    }
    for entry in inner.split(',') {
        let value = entry.trim();
        if value.is_empty() {
            continue;
        }
        if let Some(seq) = parse_inline_sequence(value) {
            items.push(Node::Sequence(seq));
        } else if let Some(rest) = value.strip_prefix('!') {
            // treat inline tag without payload as scalar tagged later
            let mut parts = rest.splitn(2, char::is_whitespace);
            let tag = parts.next().unwrap().to_string();
            let remainder = parts
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("{}");
            let inner = if remainder == "{}" {
                Node::Scalar(String::new())
            } else if let Some(seq) = parse_inline_sequence(remainder) {
                Node::Sequence(seq)
            } else {
                Node::Scalar(remainder.to_string())
            };
            items.push(Node::Tagged {
                tag,
                value: Box::new(inner),
            });
        } else {
            items.push(Node::Scalar(value.to_string()));
        }
    }
    Some(items)
}

fn parse_scalar_node(lines: &[Line<'_>], idx: &mut usize, _indent: usize) -> Result<Node> {
    let content = lines[*idx].content;
    *idx += 1;
    parse_tag_or_scalar(lines, idx, _indent, content)
}

fn decode_hex(input: &str) -> Result<Vec<u8>> {
    let trimmed = input.trim();
    if trimmed.len() % 2 != 0 {
        bail!("hex string must have even length");
    }
    let mut bytes = Vec::with_capacity(trimmed.len() / 2);
    let mut chars = trimmed.chars();
    while let Some(high) = chars.next() {
        let low = chars.next().unwrap();
        let hi = high
            .to_digit(16)
            .ok_or_else(|| anyhow!("invalid hex char `{high}`"))?;
        let lo = low
            .to_digit(16)
            .ok_or_else(|| anyhow!("invalid hex char `{low}`"))?;
        bytes.push(((hi << 4) | lo) as u8);
    }
    Ok(bytes)
}

fn decode_string(raw: &str) -> Result<String> {
    if raw.starts_with('"') {
        if raw.len() < 2 || !raw.ends_with('"') {
            bail!("unterminated string literal `{raw}`");
        }
        let mut result = String::new();
        let mut chars = raw[1..raw.len() - 1].chars();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                let next = chars.next().ok_or_else(|| anyhow!("incomplete escape"))?;
                match next {
                    'n' => result.push('\n'),
                    'r' => result.push('\r'),
                    't' => result.push('\t'),
                    '"' => result.push('"'),
                    '\\' => result.push('\\'),
                    other => bail!("unsupported escape `\\{other}`"),
                }
            } else {
                result.push(ch);
            }
        }
        Ok(result)
    } else {
        Ok(raw.to_string())
    }
}

/// Parse a YAML string into a sequence of interpreter values (used by CLI `run`).
pub fn parse_values_from_str(input: &str) -> Result<Vec<Value>> {
    let lines = preprocess(input);
    if lines.is_empty() {
        return Ok(Vec::new());
    }
    let mut idx = 0;
    match parse_node(&lines, &mut idx, 0)? {
        Node::Sequence(items) => items.into_iter().map(decode_value).collect(),
        other => bail!("expected YAML sequence at root, found {:?}", other),
    }
}

pub fn parse_values_from_file(path: &Path) -> Result<Vec<Value>> {
    let contents = fs::read_to_string(path)?;
    parse_values_from_str(&contents)
}

fn decode_value(node: Node) -> Result<Value> {
    match node {
        Node::Scalar(text) => parse_scalar_value(&text),
        Node::Tagged { tag, value } => match tag.as_str() {
            "i64" => {
                let scalar = as_scalar(&value)?;
                let number: i64 = scalar.parse()?;
                Ok(Value::I64(number))
            }
            "f64" => {
                let scalar = as_scalar(&value)?;
                let number: f64 = scalar.parse()?;
                Ok(Value::F64(number))
            }
            "text" => {
                let scalar = as_scalar(&value)?;
                Ok(Value::Text(scalar))
            }
            "tuple" => match *value {
                Node::Sequence(items) => {
                    let mut elements = Vec::with_capacity(items.len());
                    for item in items {
                        elements.push(decode_value(item)?);
                    }
                    Ok(Value::Tuple(elements))
                }
                other => bail!("tuple payload must be sequence, found {:?}", other),
            },
            "unit" => Ok(Value::Unit),
            "quote" => {
                let scalar = as_scalar(&value)?;
                let bytes = decode_hex(&scalar)?;
                if bytes.len() != 32 {
                    bail!("quote requires 32-byte hex, found {} bytes", bytes.len());
                }
                let mut cid = [0u8; 32];
                cid.copy_from_slice(&bytes);
                Ok(Value::Quote(cid))
            }
            other => bail!("unsupported value tag `{other}`"),
        },
        Node::Sequence(items) => {
            let mut values = Vec::with_capacity(items.len());
            for item in items {
                values.push(decode_value(item)?);
            }
            Ok(Value::Tuple(values))
        }
        Node::Mapping(_) => bail!("cannot decode mapping into Value"),
    }
}

fn parse_scalar_value(text: &str) -> Result<Value> {
    if text == "~" || text.eq_ignore_ascii_case("null") {
        return Ok(Value::Unit);
    }
    if let Ok(number) = text.parse::<i64>() {
        return Ok(Value::I64(number));
    }
    if let Ok(number) = text.parse::<f64>() {
        return Ok(Value::F64(number));
    }
    Ok(Value::Text(decode_string(text)?))
}

fn as_scalar(node: &Node) -> Result<String> {
    match node {
        Node::Scalar(text) => decode_string(text),
        Node::Tagged { value, .. } => as_scalar(value),
        other => bail!("expected scalar node, found {:?}", other),
    }
}

pub fn parse_catalog_from_str(input: &str) -> Result<Vec<CatalogEntry>> {
    let lines = preprocess(input);
    if lines.is_empty() {
        return Ok(Vec::new());
    }
    let mut idx = 0;
    let root = parse_node(&lines, &mut idx, 0)?;
    match root {
        Node::Mapping(namespaces) => {
            let mut entries_out = Vec::new();
            for (namespace, entries) in namespaces {
                let mapping = match entries {
                    Node::Mapping(items) => items,
                    other => bail!("namespace `{namespace}` must be mapping, found {:?}", other),
                };
                for (symbol, node) in mapping {
                    let definition = decode_catalog_entry(&namespace, &symbol, node)?;
                    entries_out.push(CatalogEntry {
                        namespace: namespace.clone(),
                        symbol,
                        definition,
                    });
                }
            }
            Ok(entries_out)
        }
        other => bail!("catalog root must be mapping, found {:?}", other),
    }
}

pub fn parse_catalog_from_file(path: &Path) -> Result<Vec<CatalogEntry>> {
    let contents = fs::read_to_string(path)?;
    parse_catalog_from_str(&contents)
}

fn decode_catalog_entry(namespace: &str, symbol: &str, node: Node) -> Result<Definition> {
    let full_name = format!("{namespace}/{symbol}");
    match node {
        Node::Tagged { tag, value } => match tag.as_str() {
            "effect" => decode_effect_entry(&full_name, *value),
            "prim" => decode_prim_entry(&full_name, *value),
            "word" => decode_word_entry(&full_name, symbol, *value),
            "guard" => decode_guard_entry(&full_name, symbol, *value),
            "overloads" => decode_overloads_entry(&full_name, *value),
            "snapshot" | "state" => decode_snapshot_entry(&full_name, *value),
            other => bail!("unsupported catalog tag `{other}`"),
        },
        other => bail!(
            "catalog entry `{symbol}` must use tagged form, found {:?}",
            other
        ),
    }
}

fn decode_overloads_entry(full_name: &str, node: Node) -> Result<Definition> {
    // Require a sequence of overload entries; labels are not supported to avoid ambiguity.
    let list = match node {
        Node::Sequence(items) => items,
        other => bail!("overloads entry must be sequence, found {:?}", other),
    };
    let mut entries: Vec<OverloadEntry> = Vec::with_capacity(list.len());
    for item in list {
        let map = match item {
            Node::Mapping(map) => map,
            Node::Tagged { tag, value } if tag == "word" => match *value {
                Node::Mapping(map) => map,
                other => bail!("!word payload must be mapping, found {:?}", other),
            },
            other => bail!("overload spec must be mapping or !word, found {:?}", other),
        };
        let params = parse_type_list(map.get("params"))?;
        let results = parse_type_list(map.get("results"))?;
        let guards = parse_string_list(map.get("guards"))?;
        let stack_node = map
            .get("stack")
            .ok_or_else(|| anyhow!("overload entry missing `stack` field"))?;
        let stack = decode_word_ops(stack_node)?;
        entries.push(OverloadEntry {
            params,
            results,
            guards,
            ops: stack,
        });
    }
    Ok(Definition::OverloadSet(OverloadSetSpec {
        name: full_name.to_string(),
        entries,
    }))
}

fn decode_effect_entry(full_name: &str, node: Node) -> Result<Definition> {
    match node {
        Node::Mapping(map) => {
            let doc = match map.get("doc") {
                Some(node) => Some(as_scalar(node)?),
                None => None,
            };
            Ok(Definition::Effect(EffectSpec {
                name: full_name.to_string(),
                doc,
            }))
        }
        Node::Scalar(text) if text.is_empty() => Ok(Definition::Effect(EffectSpec {
            name: full_name.to_string(),
            doc: None,
        })),
        other => bail!("effect entry must be mapping or empty, found {:?}", other),
    }
}

fn decode_prim_entry(full_name: &str, node: Node) -> Result<Definition> {
    let map = match node {
        Node::Mapping(map) => map,
        other => bail!("prim entry must be mapping, found {:?}", other),
    };
    let params = parse_type_list(map.get("params"))?;
    let results = parse_type_list(map.get("results"))?;
    let effects = parse_hex_list(map.get("effects"))?;
    let effect_mask = parse_effect_mask_node(map.get("emask"))?;
    Ok(Definition::Prim(PrimSpec {
        name: full_name.to_string(),
        params,
        results,
        effects,
        effect_mask,
    }))
}

fn decode_word_entry(full_name: &str, symbol: &str, node: Node) -> Result<Definition> {
    let map = match node {
        Node::Mapping(map) => map,
        other => bail!("word entry `{symbol}` must be mapping, found {:?}", other),
    };
    let params = parse_type_list(map.get("params"))?;
    let results = parse_type_list(map.get("results"))?;
    let stack_node = map
        .get("stack")
        .ok_or_else(|| anyhow!("word `{symbol}` missing `stack` field"))?;
    let ops = decode_word_ops(stack_node)?;
    let guards = parse_string_list(map.get("guards"))?;
    Ok(Definition::Word(WordSpec {
        name: full_name.to_string(),
        params,
        results,
        ops,
        guards,
    }))
}

fn decode_guard_entry(full_name: &str, symbol: &str, node: Node) -> Result<Definition> {
    let map = match node {
        Node::Mapping(map) => map,
        other => bail!("guard entry `{symbol}` must be mapping, found {:?}", other),
    };
    let params = parse_type_list(map.get("params"))?;
    let mut results = parse_type_list(map.get("results"))?;
    if results.is_empty() {
        results.push(TypeTag::I64);
    }
    let stack_node = map
        .get("stack")
        .ok_or_else(|| anyhow!("guard `{symbol}` missing `stack` field"))?;
    let ops = decode_word_ops(stack_node)?;
    Ok(Definition::Guard(GuardSpec {
        name: full_name.to_string(),
        params,
        results,
        ops,
    }))
}

fn decode_snapshot_entry(full_name: &str, node: Node) -> Result<Definition> {
    let map = match node {
        Node::Mapping(map) => map,
        other => bail!("snapshot entry must be mapping, found {:?}", other),
    };
    let mut values = BTreeMap::new();
    for (key, node) in map {
        let value = decode_value(node)?;
        values.insert(key, value);
    }
    Ok(Definition::State(StateSpec {
        name: full_name.to_string(),
        entries: values,
    }))
}

fn parse_type_list(node: Option<&Node>) -> Result<Vec<TypeTag>> {
    match node {
        None => Ok(Vec::new()),
        Some(Node::Sequence(items)) => items
            .iter()
            .map(|item| TypeTag::from_atom(&as_scalar(item)?))
            .collect(),
        Some(other) => bail!("type list must be sequence, found {:?}", other),
    }
}

fn parse_hex_list(node: Option<&Node>) -> Result<Vec<[u8; 32]>> {
    match node {
        None => Ok(Vec::new()),
        Some(Node::Sequence(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                let scalar = as_scalar(item)?;
                if scalar.is_empty() {
                    continue;
                }
                let bytes = decode_hex(&scalar)?;
                if bytes.len() != 32 {
                    bail!("effect CID must be 32 bytes, found {}", bytes.len());
                }
                let mut cid = [0u8; 32];
                cid.copy_from_slice(&bytes);
                out.push(cid);
            }
            Ok(out)
        }
        Some(other) => bail!("effects list must be sequence, found {:?}", other),
    }
}

fn parse_string_list(node: Option<&Node>) -> Result<Vec<String>> {
    match node {
        None => Ok(Vec::new()),
        Some(Node::Sequence(items)) => items
            .iter()
            .map(|item| as_scalar(item))
            .collect::<Result<Vec<_>>>(),
        Some(other) => bail!("string list must be sequence, found {:?}", other),
    }
}

fn parse_effect_mask_node(node: Option<&Node>) -> Result<EffectMask> {
    let mut mask = effect_mask::NONE;
    match node {
        None => Ok(mask),
        Some(Node::Sequence(items)) => {
            for item in items {
                let flag = as_scalar(item)?.trim().to_ascii_lowercase();
                if flag.is_empty() {
                    continue;
                }
                match flag.as_str() {
                    "io" => mask |= effect_mask::IO,
                    "state" => mask |= effect_mask::STATE_READ | effect_mask::STATE_WRITE,
                    "state.read" | "state_read" | "state-read" => mask |= effect_mask::STATE_READ,
                    "state.write" | "state_write" | "state-write" => {
                        mask |= effect_mask::STATE_WRITE
                    }
                    "test" => mask |= effect_mask::TEST,
                    "metric" => mask |= effect_mask::METRIC,
                    other => bail!("unknown effect mask flag `{other}`"),
                }
            }
            Ok(mask)
        }
        Some(other) => bail!("emask must be sequence, found {:?}", other),
    }
}

fn decode_word_ops(node: &Node) -> Result<Vec<StackOp>> {
    match node {
        Node::Sequence(items) => {
            let mut ops = Vec::with_capacity(items.len());
            for item in items {
                ops.push(decode_word_op(item)?);
            }
            Ok(ops)
        }
        other => bail!("word stack must be sequence, found {:?}", other),
    }
}

fn decode_word_op(node: &Node) -> Result<StackOp> {
    match node {
        Node::Tagged { tag, value } => match tag.as_str() {
            "prim" => Ok(StackOp::Prim(as_scalar(value)?)),
            "word" => Ok(StackOp::Word(as_scalar(value)?)),
            "dup" => Ok(StackOp::Dup),
            "swap" => Ok(StackOp::Swap),
            "over" => Ok(StackOp::Over),
            "quote" => {
                let scalar = as_scalar(value)?;
                let bytes = decode_hex(&scalar)?;
                if bytes.len() != 32 {
                    bail!("quote op requires 32-byte hex, found {}", bytes.len());
                }
                let mut cid = [0u8; 32];
                cid.copy_from_slice(&bytes);
                Ok(StackOp::Quote(cid))
            }
            "lit" => {
                let value_node = match &**value {
                    Node::Sequence(seq) if !seq.is_empty() => seq[0].clone(),
                    other => other.clone(),
                };
                let lit = decode_value(value_node)?;
                Ok(StackOp::Lit(lit))
            }
            other => bail!("unsupported word stack tag `{other}`"),
        },
        other => {
            let scalar = as_scalar(other)?;
            Ok(StackOp::Prim(scalar))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_values() -> Result<()> {
        let doc = r#"
- !i64 42
- !f64 3.14
- !text "hello world"
- !tuple
  - !i64 1
  - !text "nested"
- !unit
- !quote 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f
"#;
        let values = parse_values_from_str(doc)?;
        assert_eq!(values.len(), 6);
        Ok(())
    }

    #[test]
    fn parse_catalog_structures() -> Result<()> {
        let doc = r#"
core:
  io: !effect
    doc: "performs IO"
  add_i64: !prim
    params: [i64, i64]
    results: [i64]
demo:
  always_true: !guard
    params: []
    results: [i64]
    stack:
      - !lit -1
  add: !overloads
    - !word
      params: [i64, i64]
      results: [i64]
      stack:
        - !prim core/add_i64
    - !word
      params: [text, text]
      results: [text]
      stack:
        - !word text/concat
  counter: !state
    demo.counter: !i64 0
  square: !word
    params: [i64]
    results: [i64]
    stack:
      - !dup
      - !prim core/add_i64
"#;
        let entries = parse_catalog_from_str(doc)?;
        assert_eq!(entries.len(), 6);
        let guard_entry = entries
            .iter()
            .find(|e| e.namespace == "demo" && e.symbol == "always_true")
            .expect("guard entry parsed");
        assert!(matches!(
            guard_entry.definition,
            Definition::Guard(GuardSpec { .. })
        ));
        let overload_entry = entries
            .iter()
            .find(|e| e.namespace == "demo" && e.symbol == "add")
            .expect("overload entry parsed");
        assert!(matches!(
            overload_entry.definition,
            Definition::OverloadSet(OverloadSetSpec { .. })
        ));
        let state_entry = entries
            .iter()
            .find(|e| e.namespace == "demo" && e.symbol == "counter")
            .expect("state entry parsed");
        assert!(matches!(
            state_entry.definition,
            Definition::State(StateSpec { .. })
        ));
        let word_entry = entries
            .iter()
            .find(|e| e.namespace == "demo" && e.symbol == "square")
            .expect("word entry parsed");
        match &word_entry.definition {
            Definition::Word(spec) => {
                assert_eq!(spec.params, vec![TypeTag::I64]);
                assert_eq!(spec.results, vec![TypeTag::I64]);
                assert_eq!(spec.ops.len(), 2);
            }
            other => panic!("expected word definition, found {:?}", other),
        }
        Ok(())
    }
}
