//! Generation-zero assembler: flat instructions, labels, literal data and roots.
//! This module is not part of the runtime library or normal runtime binary.
use march7::{Blob, Cid, Image, Op, Primitive};
use std::collections::{BTreeMap, HashMap, HashSet};

pub fn assemble(source: &str) -> Result<Image, String> {
    let mut defs: BTreeMap<String, Vec<Vec<String>>> = BTreeMap::new();
    let mut data = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut entry = None;
    let mut root = None;
    for (i, line) in source.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
        let fail = || format!("assembly line {}: {line}", i + 1);
        if let Some(name) = &current {
            if fields == ["end"] {
                current = None;
            } else {
                defs.get_mut(name).ok_or_else(fail)?.push(fields);
            }
            continue;
        }
        match fields[0].as_str() {
            "word" if fields.len() == 2 => {
                let n = fields[1].clone();
                if defs.insert(n.clone(), Vec::new()).is_some() {
                    return Err(fail());
                }
                current = Some(n);
            }
            "data" if fields.len() == 2 || fields.len() == 3 => {
                let hex = fields.get(2).map(String::as_str).unwrap_or("");
                if !hex.len().is_multiple_of(2) || !hex.is_ascii() {
                    return Err(fail());
                }
                let bytes = (0..hex.len())
                    .step_by(2)
                    .map(|n| u8::from_str_radix(&hex[n..n + 2], 16).map_err(|_| fail()))
                    .collect::<Result<Vec<_>, _>>()?;
                if data.insert(fields[1].clone(), bytes).is_some() {
                    return Err(fail());
                }
            }
            "entry" if fields.len() == 2 && entry.is_none() => entry = Some(fields[1].clone()),
            "root" if fields.len() == 2 && root.is_none() => root = Some(fields[1].clone()),
            _ => return Err(fail()),
        }
    }
    if current.is_some() {
        return Err("unclosed word".into());
    }
    if defs.keys().any(|n| data.contains_key(n)) {
        return Err("duplicate label".into());
    }
    let mut objects = BTreeMap::new();
    let mut ids = HashMap::new();
    for (n, b) in data {
        let blob = Blob::Data(b);
        let cid = blob.cid();
        objects.insert(cid, blob);
        ids.insert(n, cid);
    }
    fn compile(
        name: &str,
        defs: &BTreeMap<String, Vec<Vec<String>>>,
        ids: &mut HashMap<String, Cid>,
        objects: &mut BTreeMap<Cid, Blob>,
        active: &mut HashSet<String>,
    ) -> Result<Cid, String> {
        if let Some(c) = ids.get(name) {
            return Ok(*c);
        }
        if active.len() >= 256 || !active.insert(name.into()) {
            return Err(format!("cyclic/too-deep assembly: {name}"));
        }
        let lines = defs
            .get(name)
            .ok_or_else(|| format!("unknown assembly label {name}"))?;
        let mut labels = HashMap::new();
        let mut pc = 0u32;
        for l in lines {
            if l.len() == 1 && l[0].ends_with(':') {
                if labels
                    .insert(l[0].trim_end_matches(':').to_owned(), pc)
                    .is_some()
                {
                    return Err("duplicate local label".into());
                }
            } else {
                pc += 1;
            }
        }
        let mut ops = Vec::new();
        for l in lines {
            if l.len() == 1 && l[0].ends_with(':') {
                continue;
            }
            let arg = || l.get(1).ok_or_else(|| format!("missing operand: {l:?}"));
            let op = match l[0].as_str() {
                "ret" if l.len() == 1 => Op::Return,
                "recur" if l.len() == 1 => Op::Recur,
                "tail-recur" if l.len() == 1 => Op::TailRecur,
                "lit" if l.len() == 2 => Op::Lit(
                    arg()?
                        .parse::<u64>()
                        .or_else(|_| arg().unwrap().parse::<i64>().map(|n| n as u64))
                        .map_err(|_| "invalid literal")?,
                ),
                "prim" if l.len() == 2 => {
                    Op::Prim(Primitive::named(arg()?).ok_or("unknown primitive")?)
                }
                "call" | "quote" | "tail" | "ref" if l.len() == 2 => {
                    let c = compile(arg()?, defs, ids, objects, active)?;
                    match l[0].as_str() {
                        "call" => Op::Call(c),
                        "quote" => Op::Quote(c),
                        "tail" => Op::Tail(c),
                        _ => Op::Data(c),
                    }
                }
                "branch" | "zero" if l.len() == 2 => {
                    let n = *labels.get(arg()?).ok_or("unknown branch label")?;
                    if l[0] == "branch" {
                        Op::Branch(n)
                    } else {
                        Op::ZeroBranch(n)
                    }
                }
                _ => return Err(format!("invalid instruction {l:?}")),
            };
            ops.push(op);
        }
        let bytes = march7::code::encode(&ops);
        march7::code::decode(&bytes).map_err(|e| e.to_string())?;
        let blob = Blob::Code(bytes);
        let cid = blob.cid();
        objects.insert(cid, blob);
        ids.insert(name.into(), cid);
        active.remove(name);
        Ok(cid)
    }
    let entry = compile(
        &entry.ok_or("missing entry")?,
        &defs,
        &mut ids,
        &mut objects,
        &mut HashSet::new(),
    )?;
    let data = *ids
        .get(&root.ok_or("missing root")?)
        .ok_or("missing root data")?;
    let image = Image {
        entry,
        data,
        blobs: objects,
    };
    image.validate().map_err(|e| e.to_string())?;
    Ok(image)
}
