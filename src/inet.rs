use anyhow::{Result, bail};
use rusqlite::Connection;

use crate::cbor::{push_array, push_map, push_text};
use crate::sexpr::{self, SExpr};
use crate::{cid, db};

#[derive(Clone, Debug)]
pub struct AgentCanon<'a> {
    pub name: &'a str,
    /// Ports in declaration order; port 0 is principal by convention.
    pub ports: &'a [&'a str],
    pub doc: Option<&'a str>,
}

#[derive(Clone, Debug)]
pub struct RuleCanon<'a> {
    /// Names of the two agent kinds participating in the active pair.
    pub lhs_a: &'a str,
    pub lhs_b: &'a str,
    /// S-expression or YAML snippet describing rewiring (opaque to the core for now).
    pub body_syntax: &'a str,
}

/// Encode an agent into canonical CBOR as a small map.
pub fn encode_agent(agent: &AgentCanon) -> Vec<u8> {
    let mut buf = Vec::new();
    let map_len = if agent.doc.is_some() { 4 } else { 3 };
    push_map(&mut buf, map_len);
    push_text(&mut buf, "kind");
    push_text(&mut buf, "agent");
    push_text(&mut buf, "name");
    push_text(&mut buf, agent.name);
    push_text(&mut buf, "ports");
    push_array(&mut buf, agent.ports.len() as u64);
    for p in agent.ports {
        push_text(&mut buf, p);
    }
    if let Some(doc) = agent.doc {
        push_text(&mut buf, "doc");
        push_text(&mut buf, doc);
    }
    buf
}

/// Encode a rule into canonical CBOR as a small map.
pub fn encode_rule(rule: &RuleCanon) -> Vec<u8> {
    let mut buf = Vec::new();
    push_map(&mut buf, 4);
    push_text(&mut buf, "kind");
    push_text(&mut buf, "rule");
    push_text(&mut buf, "lhs");
    push_array(&mut buf, 2);
    push_text(&mut buf, rule.lhs_a);
    push_text(&mut buf, rule.lhs_b);
    push_text(&mut buf, "rewire");
    push_text(&mut buf, rule.body_syntax);
    // Placeholder for future attachments
    push_text(&mut buf, "version");
    push_text(&mut buf, "0");
    buf
}

pub struct AgentStoreOutcome {
    pub cid: [u8; 32],
    pub inserted: bool,
}

pub struct RuleStoreOutcome {
    pub cid: [u8; 32],
    pub inserted: bool,
}

pub fn store_agent(conn: &Connection, agent: &AgentCanon) -> Result<AgentStoreOutcome> {
    let cbor = encode_agent(agent);
    let cid = cid::compute(&cbor);
    let inserted = db::put_object(conn, &cid, "agent", &cbor)?;
    Ok(AgentStoreOutcome { cid, inserted })
}

pub fn store_rule(conn: &Connection, rule: &RuleCanon) -> Result<RuleStoreOutcome> {
    let cbor = encode_rule(rule);
    let cid = cid::compute(&cbor);
    let inserted = db::put_object(conn, &cid, "rule", &cbor)?;
    Ok(RuleStoreOutcome { cid, inserted })
}

/// Minimal net representation (placeholder). Future: ports, wires, active pairs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AgentId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WireId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PortRef {
    pub agent: AgentId,
    pub port: usize,
}

#[derive(Clone, Debug)]
pub struct NetAgent {
    pub kind: String,
    pub ports: Vec<Option<WireId>>, // None means free
    pub port_names: Vec<String>,
    pub deleted: bool,
}

#[derive(Clone, Debug)]
pub struct NetWire(pub Option<(PortRef, PortRef)>);

#[derive(Clone, Debug)]
pub struct Net {
    pub agents: Vec<NetAgent>,
    pub wires: Vec<NetWire>,
    pub entry: Option<AgentId>,
}

impl Net {
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
            wires: Vec::new(),
            entry: None,
        }
    }

    pub fn add_agent(&mut self, kind: &str, port_names: &[&str]) -> AgentId {
        let id = AgentId(self.agents.len());
        let ports = vec![None; port_names.len()];
        let names = port_names.iter().map(|s| s.to_string()).collect();
        self.agents.push(NetAgent {
            kind: kind.to_string(),
            ports,
            port_names: names,
            deleted: false,
        });
        id
    }

    pub fn connect(&mut self, a: PortRef, b: PortRef) -> Result<WireId> {
        if self.port_wire(a).is_some() || self.port_wire(b).is_some() {
            bail!("port already connected");
        }
        let w = WireId(self.wires.len());
        self.wires.push(NetWire(Some((a, b))));
        self.set_port_wire(a, Some(w));
        self.set_port_wire(b, Some(w));
        Ok(w)
    }

    pub fn disconnect(&mut self, p: PortRef) {
        if let Some(w) = self.port_wire(p) {
            let idx = w.0;
            if let Some((x, y)) = self.wires[idx].0.take() {
                self.set_port_wire(x, None);
                self.set_port_wire(y, None);
            }
        }
    }

    fn port_wire(&self, p: PortRef) -> Option<WireId> {
        self.agents[p.agent.0].ports[p.port]
    }

    fn set_port_wire(&mut self, p: PortRef, w: Option<WireId>) {
        self.agents[p.agent.0].ports[p.port] = w;
    }

    fn other_end(&self, w: WireId, p: PortRef) -> Option<PortRef> {
        match self.wires[w.0].0 {
            Some((x, y)) if x == p => Some(y),
            Some((x, y)) if y == p => Some(x),
            _ => None,
        }
    }

    pub fn find_active_pair(&self) -> Option<(PortRef, PortRef)> {
        for w in &self.wires {
            if let Some((a, b)) = w.0 {
                if a.port == 0 && b.port == 0 {
                    let aa = &self.agents[a.agent.0];
                    let bb = &self.agents[b.agent.0];
                    if !aa.deleted && !bb.deleted {
                        return Some((a, b));
                    }
                }
            }
        }
        None
    }
}

/// Reduce one active pair using a built-in example rule: (pair, unpair).
/// Returns true if a rule was applied.
pub fn reduce_step(net: &mut Net) -> Result<bool> {
    let Some((a, b)) = net.find_active_pair() else {
        return Ok(false);
    };
    let kind_a = net.agents[a.agent.0].kind.clone();
    let kind_b = net.agents[b.agent.0].kind.clone();

    let (pair, unpair) = if (kind_a.as_str(), kind_b.as_str()) == ("pair", "unpair") {
        (a, b)
    } else if (kind_a.as_str(), kind_b.as_str()) == ("unpair", "pair") {
        (b, a)
    } else {
        return Ok(false);
    };

    // Map: pair(head)->unpair(left), pair(tail)->unpair(right)
    let head = PortRef {
        agent: pair.agent,
        port: 1,
    };
    let tail = PortRef {
        agent: pair.agent,
        port: 2,
    };
    let left = PortRef {
        agent: unpair.agent,
        port: 1,
    };
    let right = PortRef {
        agent: unpair.agent,
        port: 2,
    };

    // Capture external endpoints
    let head_w = net.port_wire(head);
    let tail_w = net.port_wire(tail);
    let left_w = net.port_wire(left);
    let right_w = net.port_wire(right);

    let head_ext = head_w.and_then(|w| net.other_end(w, head));
    let tail_ext = tail_w.and_then(|w| net.other_end(w, tail));
    let left_ext = left_w.and_then(|w| net.other_end(w, left));
    let right_ext = right_w.and_then(|w| net.other_end(w, right));

    // Disconnect principal and aux wires on the two agents
    net.disconnect(pair);
    net.disconnect(unpair);
    net.disconnect(head);
    net.disconnect(tail);
    net.disconnect(left);
    net.disconnect(right);

    // Reconnect external endpoints
    if let (Some(p), Some(q)) = (head_ext, left_ext) {
        net.connect(p, q)?;
    }
    if let (Some(p), Some(q)) = (tail_ext, right_ext) {
        net.connect(p, q)?;
    }

    // Mark agents deleted
    net.agents[pair.agent.0].deleted = true;
    net.agents[unpair.agent.0].deleted = true;

    Ok(true)
}

/// In-memory rule table loaded from the object store.
pub struct Reducer {
    /// Map from (lhs_a, lhs_b) -> body_syntax
    rules: std::collections::HashMap<(String, String), String>,
}

impl Reducer {
    pub fn new(conn: &Connection) -> Result<Self> {
        let mut rules = std::collections::HashMap::new();
        for cbor in db::load_all_cbor_for_kind(conn, "rule")? {
            // Decode minimal fields: kind, lhs (array of two), rewire
            let value: serde_cbor::Value = serde_cbor::from_slice(&cbor)?;
            let map = match value {
                serde_cbor::Value::Map(m) => m,
                _ => continue,
            };
            let mut lhs_a = None;
            let mut lhs_b = None;
            let mut rewire = None;
            for (k, v) in map {
                match k {
                    serde_cbor::Value::Text(ref s) if s == "lhs" => match v {
                        serde_cbor::Value::Array(items) if items.len() == 2 => {
                            lhs_a = items.get(0).and_then(|x| match x {
                                serde_cbor::Value::Text(s) => Some(s.clone()),
                                _ => None,
                            });
                            lhs_b = items.get(1).and_then(|x| match x {
                                serde_cbor::Value::Text(s) => Some(s.clone()),
                                _ => None,
                            });
                        }
                        _ => {}
                    },
                    serde_cbor::Value::Text(ref s) if s == "rewire" => {
                        if let serde_cbor::Value::Text(body) = v {
                            rewire = Some(body.clone());
                        }
                    }
                    _ => {}
                }
            }
            if let (Some(a), Some(b), Some(body)) = (lhs_a, lhs_b, rewire) {
                rules.insert((a, b), body);
            }
        }
        Ok(Self { rules })
    }

    /// Apply one rule step if possible. Returns true if a rule was applied.
    pub fn step(&self, net: &mut Net) -> Result<bool> {
        let Some((a, b)) = net.find_active_pair() else {
            return Ok(false);
        };
        let kind_a = net.agents[a.agent.0].kind.clone();
        let kind_b = net.agents[b.agent.0].kind.clone();
        // Prefer exact (a,b), then symmetric (b,a)
        if let Some(body) = self.rules.get(&(kind_a.clone(), kind_b.clone())) {
            return self.apply_rewire(body, net, a, b);
        }
        if let Some(body) = self.rules.get(&(kind_b.clone(), kind_a.clone())) {
            return self.apply_rewire(body, net, b, a);
        }
        Ok(false)
    }

    fn apply_rewire(
        &self,
        body: &str,
        net: &mut Net,
        lhs_a: PortRef,
        lhs_b: PortRef,
    ) -> Result<bool> {
        // First builtin: (pair-unpair)
        if body.trim() == "(pair-unpair)" {
            // Use the same logic as reduce_step's hard-coded pair/unpair rule, assuming lhs_a is pair and lhs_b is unpair
            let head = PortRef {
                agent: lhs_a.agent,
                port: 1,
            };
            let tail = PortRef {
                agent: lhs_a.agent,
                port: 2,
            };
            let left = PortRef {
                agent: lhs_b.agent,
                port: 1,
            };
            let right = PortRef {
                agent: lhs_b.agent,
                port: 2,
            };

            let head_ext = net.port_wire(head).and_then(|w| net.other_end(w, head));
            let tail_ext = net.port_wire(tail).and_then(|w| net.other_end(w, tail));
            let left_ext = net.port_wire(left).and_then(|w| net.other_end(w, left));
            let right_ext = net.port_wire(right).and_then(|w| net.other_end(w, right));

            net.disconnect(lhs_a);
            net.disconnect(lhs_b);
            net.disconnect(head);
            net.disconnect(tail);
            net.disconnect(left);
            net.disconnect(right);

            if let (Some(p), Some(q)) = (head_ext, left_ext) {
                net.connect(p, q)?;
            }
            if let (Some(p), Some(q)) = (tail_ext, right_ext) {
                net.connect(p, q)?;
            }
            net.agents[lhs_a.agent.0].deleted = true;
            net.agents[lhs_b.agent.0].deleted = true;
            return Ok(true);
        }
        // Minimal S-expr DSL: (seq (connect (A port) (B port))* (delete A B)?)
        let forms = sexpr::parse_sequence(body)?;
        // Collect connect and disconnect operations
        let mut connects: Vec<(PortRef, PortRef)> = Vec::new();
        let mut to_disconnect: Vec<PortRef> = Vec::new();
        // Aliases created via (new KIND alias (ports...))
        let mut aliases: std::collections::HashMap<String, AgentId> =
            std::collections::HashMap::new();
        fn resolve_alias(
            net: &Net,
            aliases: &std::collections::HashMap<String, AgentId>,
            lhs_a: PortRef,
            lhs_b: PortRef,
            sym: &str,
            port: &str,
        ) -> Result<PortRef> {
            let ar = if sym == "A" {
                lhs_a.agent
            } else if sym == "B" {
                lhs_b.agent
            } else if let Some(id) = aliases.get(sym) {
                *id
            } else {
                bail!("unknown agent symbol `{sym}`");
            };
            let idx = net.agents[ar.0]
                .port_names
                .iter()
                .position(|n| n == port)
                .ok_or_else(|| anyhow::anyhow!("unknown port `{port}` on {sym}`"))?;
            Ok(PortRef {
                agent: ar,
                port: idx,
            })
        }
        for form in &forms {
            if let SExpr::List(items) = form {
                if !items.is_empty() {
                    if let SExpr::Sym(op) = &items[0] {
                        if op == "connect" && items.len() == 3 {
                            let (s1, p1) = match &items[1] {
                                SExpr::List(v) if v.len() == 2 => match (&v[0], &v[1]) {
                                    (SExpr::Sym(a), SExpr::Sym(b)) => (a.as_str(), b.as_str()),
                                    _ => continue,
                                },
                                _ => continue,
                            };
                            let (s2, p2) = match &items[2] {
                                SExpr::List(v) if v.len() == 2 => match (&v[0], &v[1]) {
                                    (SExpr::Sym(a), SExpr::Sym(b)) => (a.as_str(), b.as_str()),
                                    _ => continue,
                                },
                                _ => continue,
                            };
                            let pr1 = resolve_alias(net, &aliases, lhs_a, lhs_b, s1, p1)?;
                            let pr2 = resolve_alias(net, &aliases, lhs_a, lhs_b, s2, p2)?;
                            connects.push((pr1, pr2));
                        } else if op == "disconnect" {
                            for arg in items.iter().skip(1) {
                                if let SExpr::List(v) = arg {
                                    if v.len() == 2 {
                                        if let (SExpr::Sym(a), SExpr::Sym(p)) = (&v[0], &v[1]) {
                                            if let Ok(pr) =
                                                resolve_alias(net, &aliases, lhs_a, lhs_b, a, p)
                                            {
                                                to_disconnect.push(pr);
                                            }
                                        }
                                    }
                                }
                            }
                        } else if op == "new" && items.len() >= 3 {
                            // (new KIND alias (port port ...))
                            let kind = if let SExpr::Sym(k) = &items[1] {
                                k
                            } else {
                                continue;
                            };
                            let alias = if let SExpr::Sym(a) = &items[2] {
                                a
                            } else {
                                continue;
                            };
                            let mut plist: Vec<&str> = Vec::new();
                            if items.len() >= 4 {
                                if let SExpr::List(l) = &items[3] {
                                    for it in l {
                                        if let SExpr::Sym(s) = it {
                                            plist.push(s)
                                        }
                                    }
                                }
                            }
                            let id = net.add_agent(kind, &plist);
                            aliases.insert(alias.clone(), id);
                        }
                    }
                }
            }
        }
        // For each connect, rewire. If ports are connected, reattach external endpoints. Otherwise connect ports directly.
        for (p1, p2) in &connects {
            let e1 = net.port_wire(*p1).and_then(|w| net.other_end(w, *p1));
            let e2 = net.port_wire(*p2).and_then(|w| net.other_end(w, *p2));
            // Disconnect both sides so endpoints/ports are free
            net.disconnect(*p1);
            net.disconnect(*p2);
            let a = e1.unwrap_or(*p1);
            let b = e2.unwrap_or(*p2);
            let _ = net.connect(a, b);
        }
        // Apply any remaining disconnect actions
        for pr in to_disconnect {
            net.disconnect(pr);
        }
        // Handle delete if present
        for form in &forms {
            if let SExpr::List(items) = form {
                if items.len() >= 2 {
                    if let SExpr::Sym(op) = &items[0] {
                        if op == "delete" {
                            for sym in items.iter().skip(1) {
                                if let SExpr::Sym(s) = sym {
                                    let ag = if s == "A" {
                                        lhs_a.agent
                                    } else if s == "B" {
                                        lhs_b.agent
                                    } else {
                                        continue;
                                    };
                                    // Disconnect all ports
                                    let nports = net.agents[ag.0].ports.len();
                                    for i in 0..nports {
                                        net.disconnect(PortRef { agent: ag, port: i });
                                    }
                                    net.agents[ag.0].deleted = true;
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn encode_store_agent() -> Result<()> {
        let agent = AgentCanon {
            name: "pair",
            ports: &["principal", "head", "tail"],
            doc: None,
        };
        let bytes = encode_agent(&agent);
        assert!(!bytes.is_empty());
        let conn = Connection::open_in_memory()?;
        crate::db::install_schema(&conn)?;
        let out = store_agent(&conn, &agent)?;
        assert!(!out.cid.iter().all(|b| *b == 0));
        Ok(())
    }

    #[test]
    fn encode_store_rule() -> Result<()> {
        let rule = RuleCanon {
            lhs_a: "dispatch",
            lhs_b: "apply",
            body_syntax: "(connect ...)",
        };
        let bytes = encode_rule(&rule);
        assert!(!bytes.is_empty());
        let conn = Connection::open_in_memory()?;
        crate::db::install_schema(&conn)?;
        let out = store_rule(&conn, &rule)?;
        assert!(!out.cid.iter().all(|b| *b == 0));
        Ok(())
    }

    #[test]
    fn reduce_pair_unpair_rewires() -> Result<()> {
        // Build a small net: pair — unpair as active pair, each with two aux ports
        // External endpoints: h<->A, t<->C, l<->B, r<->D. After reduce: A<->B and C<->D.
        let mut net = Net::new();
        let a = net.add_agent("A", &["p"]);
        let b = net.add_agent("B", &["p"]);
        let c = net.add_agent("C", &["p"]);
        let d = net.add_agent("D", &["p"]);
        let pair = net.add_agent("pair", &["principal", "head", "tail"]);
        let unpair = net.add_agent("unpair", &["principal", "left", "right"]);

        // Connect principal-principal
        net.connect(
            PortRef {
                agent: pair,
                port: 0,
            },
            PortRef {
                agent: unpair,
                port: 0,
            },
        )?;
        // Aux connections
        net.connect(
            PortRef {
                agent: pair,
                port: 1,
            },
            PortRef { agent: a, port: 0 },
        )?; // head-A
        net.connect(
            PortRef {
                agent: pair,
                port: 2,
            },
            PortRef { agent: c, port: 0 },
        )?; // tail-C
        net.connect(
            PortRef {
                agent: unpair,
                port: 1,
            },
            PortRef { agent: b, port: 0 },
        )?; // left-B
        net.connect(
            PortRef {
                agent: unpair,
                port: 2,
            },
            PortRef { agent: d, port: 0 },
        )?; // right-D

        assert!(reduce_step(&mut net)?);
        // After rewrite, pair/unpair are deleted; A<->B and C<->D should be connected.
        let wire_ab = net.agents[a.0].ports[0];
        let wire_cd = net.agents[c.0].ports[0];
        assert!(wire_ab.is_some());
        assert!(wire_cd.is_some());
        let other_a = net
            .other_end(wire_ab.unwrap(), PortRef { agent: a, port: 0 })
            .unwrap();
        let other_c = net
            .other_end(wire_cd.unwrap(), PortRef { agent: c, port: 0 })
            .unwrap();
        assert_eq!(other_a.agent.0, b.0);
        assert_eq!(other_c.agent.0, d.0);
        Ok(())
    }

    #[test]
    fn reducer_applies_stored_pair_unpair_rule() -> Result<()> {
        let conn = Connection::open_in_memory()?;
        crate::db::install_schema(&conn)?;
        let rule = RuleCanon {
            lhs_a: "pair",
            lhs_b: "unpair",
            body_syntax: "(seq (connect (A head) (B left)) (connect (A tail) (B right)) (delete A B))",
        };
        store_rule(&conn, &rule)?;
        let reducer = Reducer::new(&conn)?;

        let mut net = Net::new();
        let a = net.add_agent("A", &["p"]);
        let b = net.add_agent("B", &["p"]);
        let c = net.add_agent("C", &["p"]);
        let d = net.add_agent("D", &["p"]);
        let pair = net.add_agent("pair", &["principal", "head", "tail"]);
        let unpair = net.add_agent("unpair", &["principal", "left", "right"]);

        net.connect(
            PortRef {
                agent: pair,
                port: 0,
            },
            PortRef {
                agent: unpair,
                port: 0,
            },
        )?;
        net.connect(
            PortRef {
                agent: pair,
                port: 1,
            },
            PortRef { agent: a, port: 0 },
        )?;
        net.connect(
            PortRef {
                agent: pair,
                port: 2,
            },
            PortRef { agent: c, port: 0 },
        )?;
        net.connect(
            PortRef {
                agent: unpair,
                port: 1,
            },
            PortRef { agent: b, port: 0 },
        )?;
        net.connect(
            PortRef {
                agent: unpair,
                port: 2,
            },
            PortRef { agent: d, port: 0 },
        )?;

        assert!(reducer.step(&mut net)?);
        let other_a = net
            .other_end(
                net.agents[a.0].ports[0].unwrap(),
                PortRef { agent: a, port: 0 },
            )
            .unwrap();
        let other_c = net
            .other_end(
                net.agents[c.0].ports[0].unwrap(),
                PortRef { agent: c, port: 0 },
            )
            .unwrap();
        assert_eq!(other_a.agent.0, b.0);
        assert_eq!(other_c.agent.0, d.0);
        Ok(())
    }

    #[test]
    fn dsl_disconnect_then_connect() -> Result<()> {
        let conn = Connection::open_in_memory()?;
        crate::db::install_schema(&conn)?;
        // Rule: disconnect A.p then connect A.p to B.q and delete A/B
        let rule = RuleCanon {
            lhs_a: "X",
            lhs_b: "Y",
            body_syntax: "(seq (disconnect (A p)) (connect (A p) (B q)) (delete A B))",
        };
        store_rule(&conn, &rule)?;
        let reducer = Reducer::new(&conn)?;

        let mut net = Net::new();
        let left = net.add_agent("left", &["x"]);
        let right = net.add_agent("right", &["y"]);
        let a = net.add_agent("X", &["principal", "p"]);
        let b = net.add_agent("Y", &["principal", "q"]);

        // A.p <-> left.x, B.q <-> right.y, and A <-> B principal
        net.connect(
            PortRef { agent: a, port: 1 },
            PortRef {
                agent: left,
                port: 0,
            },
        )?;
        net.connect(
            PortRef { agent: b, port: 1 },
            PortRef {
                agent: right,
                port: 0,
            },
        )?;
        net.connect(PortRef { agent: a, port: 0 }, PortRef { agent: b, port: 0 })?;

        assert!(reducer.step(&mut net)?);
        // After rewrite and delete, left.x should connect to right.y
        let w_left = net.agents[left.0].ports[0];
        assert!(w_left.is_some());
        let other = net
            .other_end(
                w_left.unwrap(),
                PortRef {
                    agent: left,
                    port: 0,
                },
            )
            .unwrap();
        assert_eq!(other.agent.0, right.0);
        Ok(())
    }

    #[test]
    fn dsl_guard_type_if_rewire() -> Result<()> {
        let conn = Connection::open_in_memory()?;
        crate::db::install_schema(&conn)?;
        // Rule: when (gtype, if) meet, connect match->true and else->false, then delete both
        let rule = RuleCanon {
            lhs_a: "gtype",
            lhs_b: "if",
            body_syntax: "(seq (connect (A match) (B true)) (connect (A else) (B false)) (delete A B))",
        };
        store_rule(&conn, &rule)?;
        let reducer = Reducer::new(&conn)?;

        let mut net = Net::new();
        // External endpoints
        let t_out = net.add_agent("T", &["o"]);
        let f_out = net.add_agent("F", &["o"]);
        // Agents under test
        let g = net.add_agent("gtype", &["principal", "input", "match", "else"]);
        let i = net.add_agent("if", &["principal", "true", "false"]);
        // Principal pair
        net.connect(PortRef { agent: g, port: 0 }, PortRef { agent: i, port: 0 })?;
        // Connect g.match -> t_out.o and g.else -> f_out.o (will be rewired to B.true/B.false external endpoints)
        net.connect(
            PortRef { agent: g, port: 2 },
            PortRef {
                agent: t_out,
                port: 0,
            },
        )?;
        net.connect(
            PortRef { agent: g, port: 3 },
            PortRef {
                agent: f_out,
                port: 0,
            },
        )?;
        // Connect B.true -> A side external X and B.false -> Y (simulate pre-existing external consumers)
        let x = net.add_agent("X", &["p"]);
        let y = net.add_agent("Y", &["p"]);
        net.connect(PortRef { agent: i, port: 1 }, PortRef { agent: x, port: 0 })?;
        net.connect(PortRef { agent: i, port: 2 }, PortRef { agent: y, port: 0 })?;

        assert!(reducer.step(&mut net)?);
        // Expect T connected to X, and F connected to Y
        let w_tx = net.agents[t_out.0].ports[0];
        let w_fy = net.agents[f_out.0].ports[0];
        assert!(w_tx.is_some());
        assert!(w_fy.is_some());
        let other_t = net
            .other_end(
                w_tx.unwrap(),
                PortRef {
                    agent: t_out,
                    port: 0,
                },
            )
            .unwrap();
        let other_f = net
            .other_end(
                w_fy.unwrap(),
                PortRef {
                    agent: f_out,
                    port: 0,
                },
            )
            .unwrap();
        assert_eq!(other_t.agent.0, x.0);
        assert_eq!(other_f.agent.0, y.0);
        Ok(())
    }

    #[test]
    fn dsl_deopt_delete() -> Result<()> {
        let conn = Connection::open_in_memory()?;
        crate::db::install_schema(&conn)?;
        // Rule: when (foo, deopt) meet, delete both (disconnect all ports)
        let rule = RuleCanon {
            lhs_a: "foo",
            lhs_b: "deopt",
            body_syntax: "(delete A B)",
        };
        store_rule(&conn, &rule)?;
        let reducer = Reducer::new(&conn)?;

        let mut net = Net::new();
        let ext = net.add_agent("ext", &["p"]);
        let foo = net.add_agent("foo", &["principal", "data"]);
        let deopt = net.add_agent("deopt", &["principal"]);
        net.connect(
            PortRef {
                agent: foo,
                port: 0,
            },
            PortRef {
                agent: deopt,
                port: 0,
            },
        )?;
        net.connect(
            PortRef {
                agent: foo,
                port: 1,
            },
            PortRef {
                agent: ext,
                port: 0,
            },
        )?;

        assert!(reducer.step(&mut net)?);
        // After delete, ext port should be free (disconnected)
        assert!(net.agents[ext.0].ports[0].is_none());
        Ok(())
    }
}
