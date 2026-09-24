//! B0c: a hand-assembled image, not a host parser/interpreter.
//!
//! Every operation below constructs a graph. Source tokens are consumed only
//! by the resulting guarded families running through the ordinary reducer.
use crate::{Atom, Bindings, Cid, Clause, Node, ReduceError, Reducer, Reduction, Store};
use std::cell::RefCell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Syntax {
    NameFirst,
    Forth,
}

#[derive(Clone, Copy, Debug)]
pub struct Seed {
    pub runner: Cid,
    pub dictionary: Cid,
}

impl Seed {
    pub fn build(store: &mut Store, syntax: Syntax) -> Self {
        let g = Graph(RefCell::new(store));
        let ensure = build_ensure(&g);
        let take = build_take(&g);
        let execute = build_execute(&g, ensure, take);
        let open = build_open(&g);
        let close = build_close(&g);
        let finish = build_finish(&g);
        let colon = build_colon(&g, syntax);
        let semicolon = if syntax == Syntax::NameFirst {
            finish
        } else {
            let s = g.param(0);
            g.family(1, &[(g.yes(), g.call(finish, &[g.call(close, &[s])]))])
        };
        let quote = {
            let s = g.param(0);
            g.family(
                1,
                &[
                    (g.mode(s, "eval"), g.put(s, "mode", g.text("quoted-name"))),
                    (
                        g.yes(),
                        g.error(s, "quote is only supported in evaluation mode"),
                    ),
                ],
            )
        };
        let mut words = vec![
            (":", g.cell("handler", colon, g.int(0), true)),
            (";", g.cell("handler", semicolon, g.int(0), false)),
            ("(", g.cell("handler", open, g.int(0), false)),
            (")", g.cell("handler", close, g.int(0), false)),
            ("quote", g.cell("handler", quote, g.int(0), false)),
        ];
        for operation in ["dup", "drop", "swap", "+", "*"] {
            let handler = build_primitive(&g, ensure, operation);
            words.push((operation, g.cell("handler", handler, g.int(0), false)));
        }
        let dictionary = g.record(&words);
        let step = build_step(&g, execute, syntax);
        let runner = build_runner(&g, step);
        Self { runner, dictionary }
    }

    /// Source may itself be an unresolved graph; it need not be a literal Text.
    pub fn state(&self, store: &mut Store, source: Cid) -> Cid {
        let g = Graph(RefCell::new(store));
        g.record(&[
            ("text", source),
            ("position", g.int(0)),
            ("dictionary", self.dictionary),
            ("stack", g.unit()),
            ("mode", g.text("eval")),
            ("name", g.unit()),
            ("saved", g.unit()),
            ("quote-saved", g.unit()),
            ("inputs", g.int(0)),
            ("error", g.unit()),
            ("token", g.text("")),
        ])
    }
}

/// Resume using image roots alone. Does not build a seed or inspect source.
pub fn resume(
    store: &mut Store,
    runner: Cid,
    state: Cid,
    tokens: u32,
    work: usize,
) -> Result<Reduction, ReduceError> {
    let tokens = store.intern(Node::Const(Atom::Int(i64::from(tokens))));
    let call = store.intern(Node::Dispatch {
        family: runner,
        arguments: vec![state, tokens],
    });
    Reducer::with_budget(store, &Bindings::new(), work).run(call)
}

// This assembler uses interior mutability only to make nested graph
// construction readable. It never reads a token or dispatches source code.
struct Graph<'a>(RefCell<&'a mut Store>);
impl Graph<'_> {
    fn node(&self, n: Node) -> Cid {
        self.0.borrow_mut().intern(n)
    }
    fn int(&self, n: i64) -> Cid {
        self.node(Node::Const(Atom::Int(n)))
    }
    fn text(&self, s: &str) -> Cid {
        self.node(Node::Const(Atom::Text(s.into())))
    }
    fn unit(&self) -> Cid {
        self.node(Node::Const(Atom::Unit))
    }
    fn yes(&self) -> Cid {
        self.node(Node::Const(Atom::Bool(true)))
    }
    fn no(&self) -> Cid {
        self.node(Node::Const(Atom::Bool(false)))
    }
    fn param(&self, n: u16) -> Cid {
        self.node(Node::Param(n))
    }
    fn pair(&self, a: Cid, b: Cid) -> Cid {
        self.node(Node::Pair(a, b))
    }
    fn first(&self, p: Cid) -> Cid {
        self.node(Node::First(p))
    }
    fn second(&self, p: Cid) -> Cid {
        self.node(Node::Second(p))
    }
    fn eq(&self, a: Cid, b: Cid) -> Cid {
        self.node(Node::Eq(a, b))
    }
    fn add(&self, a: Cid, b: Cid) -> Cid {
        self.node(Node::Add(a, b))
    }
    fn is(&self, a: Cid, b: &str) -> Cid {
        self.eq(a, self.text(b))
    }
    fn not(&self, a: Cid) -> Cid {
        self.eq(a, self.no())
    }
    fn choose(&self, condition: Cid, yes: Cid, no: Cid) -> Cid {
        self.node(Node::If {
            condition,
            when_true: yes,
            when_false: no,
        })
    }
    fn record(&self, fields: &[(&str, Cid)]) -> Cid {
        self.node(Node::Record(
            fields.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        ))
    }
    fn get(&self, record: Cid, field: &str) -> Cid {
        self.node(Node::Get {
            record,
            field: field.into(),
        })
    }
    fn put(&self, record: Cid, field: &str, value: Cid) -> Cid {
        self.node(Node::Put {
            record,
            field: field.into(),
            value,
        })
    }
    fn update(&self, mut state: Cid, fields: &[(&str, Cid)]) -> Cid {
        for (field, value) in fields {
            state = self.put(state, field, *value);
        }
        state
    }
    fn family(&self, parameters: u16, clauses: &[(Cid, Cid)]) -> Cid {
        self.node(Node::Family {
            parameters,
            clauses: clauses
                .iter()
                .map(|(guard, body)| Clause {
                    guard: *guard,
                    body: *body,
                })
                .collect(),
        })
    }
    fn call(&self, family: Cid, arguments: &[Cid]) -> Cid {
        self.node(Node::Dispatch {
            family,
            arguments: arguments.to_vec(),
        })
    }
    fn recur(&self, arguments: &[Cid]) -> Cid {
        self.node(Node::Recur(arguments.to_vec()))
    }
    fn mode(&self, state: Cid, mode: &str) -> Cid {
        self.is(self.get(state, "mode"), mode)
    }
    fn error(&self, state: Cid, error: &str) -> Cid {
        self.put(state, "error", self.text(error))
    }
    fn failed(&self, state: Cid) -> Cid {
        self.not(self.eq(self.get(state, "error"), self.unit()))
    }
    fn lookup(&self, record: Cid, key: Cid) -> Cid {
        self.node(Node::Lookup { record, key })
    }
    fn next(&self, state: Cid) -> Cid {
        self.node(Node::NextToken {
            text: self.get(state, "text"),
            position: self.get(state, "position"),
        })
    }
    fn desc(&self, op: &str, fields: &[(&str, Cid)]) -> Cid {
        let mut fields = fields.to_vec();
        fields.push(("op", self.text(op)));
        self.record(&fields)
    }
    fn cell(&self, kind: &str, value: Cid, inputs: Cid, binder: bool) -> Cid {
        self.record(&[
            ("kind", self.text(kind)),
            ("value", value),
            ("inputs", inputs),
            ("binder", if binder { self.yes() } else { self.no() }),
        ])
    }
    fn expression(&self, description: Cid) -> Cid {
        self.cell("expr", description, self.int(0), false)
    }
    fn integer(&self, value: Cid) -> Cid {
        self.cell("int", value, self.int(0), false)
    }
    fn push(&self, state: Cid, value: Cid) -> Cid {
        self.put(state, "stack", self.pair(value, self.get(state, "stack")))
    }
    fn closed(&self, inputs: Cid, body: Cid) -> Cid {
        self.node(Node::Intern(
            self.desc("quote", &[("parameters", inputs), ("body", body)]),
        ))
    }
}

/// Ensure N stack cells; compilation invents explicit input wires at the
/// bottom, in top-first caller order. Runtime reports underflow instead.
fn build_ensure(g: &Graph<'_>) -> Cid {
    let stack = g.param(0);
    let needed = g.param(1);
    let inputs = g.param(2);
    let compile = g.param(3);
    let rest = g.recur(&[g.second(stack), g.add(needed, g.int(-1)), inputs, compile]);
    let extended = g.record(&[
        ("stack", g.pair(g.first(stack), g.get(rest, "stack"))),
        ("inputs", g.get(rest, "inputs")),
        ("ok", g.get(rest, "ok")),
        ("error", g.get(rest, "error")),
    ]);
    let parameter = g.expression(g.desc("param", &[("index", inputs)]));
    let missing = g.recur(&[
        g.unit(),
        g.add(needed, g.int(-1)),
        g.add(inputs, g.int(1)),
        compile,
    ]);
    let generated = g.record(&[
        ("stack", g.pair(parameter, g.get(missing, "stack"))),
        ("inputs", g.get(missing, "inputs")),
        ("ok", g.get(missing, "ok")),
        ("error", g.get(missing, "error")),
    ]);
    let fail = g.record(&[
        ("stack", stack),
        ("inputs", inputs),
        ("ok", g.no()),
        ("error", g.text("stack underflow")),
    ]);
    // Valid compiler states allocate one input at a time, starting at zero.
    // Stop before creating input 65535, which needs an arity of 65536.
    let limit = g.put(fail, "error", g.text("quotation input arity exceeds 65535"));
    let generated = g.choose(g.eq(inputs, g.int(65535)), limit, generated);
    g.family(
        4,
        &[
            (
                g.eq(needed, g.int(0)),
                g.record(&[
                    ("stack", stack),
                    ("inputs", inputs),
                    ("ok", g.yes()),
                    ("error", g.unit()),
                ]),
            ),
            (g.eq(stack, g.unit()), g.choose(compile, generated, fail)),
            (g.yes(), extended),
        ],
    )
}

/// Convert N cells into reflection arguments while preserving caller order.
fn build_take(g: &Graph<'_>) -> Cid {
    let stack = g.param(0);
    let needed = g.param(1);
    let compile = g.param(2);
    let head = g.first(stack);
    let tail = g.recur(&[g.second(stack), g.add(needed, g.int(-1)), compile]);
    let description = g.choose(
        compile,
        g.get(head, "value"),
        g.desc("int", &[("value", g.get(head, "value"))]),
    );
    let valid = g.choose(compile, g.yes(), g.is(g.get(head, "kind"), "int"));
    g.family(
        3,
        &[
            (
                g.eq(needed, g.int(0)),
                g.record(&[("arguments", g.unit()), ("rest", stack), ("ok", g.yes())]),
            ),
            (
                g.yes(),
                g.record(&[
                    ("arguments", g.pair(description, g.get(tail, "arguments"))),
                    ("rest", g.get(tail, "rest")),
                    ("ok", g.choose(valid, g.get(tail, "ok"), g.no())),
                ]),
            ),
        ],
    )
}

fn build_execute(g: &Graph<'_>, ensure: Cid, take: Cid) -> Cid {
    let s = g.param(0);
    let entry = g.param(1);
    let compile = g.mode(s, "compile");
    let prepared = g.call(
        ensure,
        &[
            g.get(s, "stack"),
            g.get(entry, "inputs"),
            g.get(s, "inputs"),
            compile,
        ],
    );
    let taken = g.call(
        take,
        &[g.get(prepared, "stack"), g.get(entry, "inputs"), compile],
    );
    let application = g.desc(
        "apply",
        &[
            (
                "function",
                g.desc("embed", &[("value", g.get(entry, "value"))]),
            ),
            ("arguments", g.get(taken, "arguments")),
        ],
    );
    let value = g.node(Node::Apply {
        function: g.closed(g.int(0), application),
        arguments: vec![],
    });
    let cell = g.choose(compile, g.expression(application), g.integer(value));
    let applied = g.update(
        s,
        &[
            ("stack", g.pair(cell, g.get(taken, "rest"))),
            ("inputs", g.get(prepared, "inputs")),
        ],
    );
    let applied = g.choose(
        g.get(taken, "ok"),
        applied,
        g.error(s, "quotation arguments must be integers"),
    );
    let applied = g.choose(
        g.get(prepared, "ok"),
        applied,
        g.put(s, "error", g.get(prepared, "error")),
    );
    let constant = g.choose(
        compile,
        g.expression(g.desc("int", &[("value", g.get(entry, "value"))])),
        entry,
    );
    g.family(
        2,
        &[
            (
                g.is(g.get(entry, "kind"), "handler"),
                g.call(g.get(entry, "value"), &[s]),
            ),
            (g.is(g.get(entry, "kind"), "int"), g.push(s, constant)),
            (g.is(g.get(entry, "kind"), "code"), applied),
            (g.yes(), g.error(s, "invalid dictionary entry")),
        ],
    )
}

fn build_primitive(g: &Graph<'_>, ensure: Cid, operation: &str) -> Cid {
    let s = g.param(0);
    let compile = g.mode(s, "compile");
    let needed = if matches!(operation, "dup" | "drop") {
        1
    } else {
        2
    };
    let prepared = g.call(
        ensure,
        &[
            g.get(s, "stack"),
            g.int(needed),
            g.get(s, "inputs"),
            compile,
        ],
    );
    let stack = g.get(prepared, "stack");
    let a = g.first(stack);
    let rest = g.second(stack);
    let b = g.first(rest);
    let tail = g.second(rest);
    let (stack, valid) = match operation {
        "dup" => (g.pair(a, stack), g.yes()),
        "drop" => (rest, g.yes()),
        "swap" => (g.pair(b, g.pair(a, tail)), g.yes()),
        "+" | "*" => {
            let av = g.get(a, "value");
            let bv = g.get(b, "value");
            let description = g.desc(
                if operation == "+" { "add" } else { "multiply" },
                &[("left", bv), ("right", av)],
            );
            let result = g.node(if operation == "+" {
                Node::Add(bv, av)
            } else {
                Node::Mul(bv, av)
            });
            let cell = g.choose(compile, g.expression(description), g.integer(result));
            let integers = g.choose(
                g.is(g.get(a, "kind"), "int"),
                g.is(g.get(b, "kind"), "int"),
                g.no(),
            );
            (g.pair(cell, tail), g.choose(compile, g.yes(), integers))
        }
        _ => unreachable!("assembler primitive list"),
    };
    let result = g.update(
        s,
        &[("stack", stack), ("inputs", g.get(prepared, "inputs"))],
    );
    let result = g.choose(valid, result, g.error(s, "arithmetic expects integers"));
    let result = g.choose(
        g.get(prepared, "ok"),
        result,
        g.put(s, "error", g.get(prepared, "error")),
    );
    g.family(
        1,
        &[
            (g.mode(s, "eval"), result),
            (g.mode(s, "compile"), result),
            (
                g.yes(),
                g.error(s, "word is not enabled in this reader mode"),
            ),
        ],
    )
}

fn build_open(g: &Graph<'_>) -> Cid {
    let s = g.param(0);
    let opened = g.update(
        s,
        &[
            ("quote-saved", g.get(s, "stack")),
            ("stack", g.unit()),
            ("inputs", g.int(0)),
            ("mode", g.text("compile")),
        ],
    );
    g.family(
        1,
        &[
            (g.mode(s, "eval"), opened),
            (
                g.yes(),
                g.error(s, "nested quotations are not supported by this seed"),
            ),
        ],
    )
}

fn build_close(g: &Graph<'_>) -> Cid {
    let s = g.param(0);
    let stack = g.get(s, "stack");
    let cell = g.first(stack);
    let quote = g.closed(g.get(s, "inputs"), g.get(cell, "value"));
    let code = g.cell("code", quote, g.get(s, "inputs"), false);
    let closed = g.update(
        s,
        &[
            ("stack", g.pair(code, g.get(s, "quote-saved"))),
            ("quote-saved", g.unit()),
            ("inputs", g.int(0)),
            ("mode", g.text("eval")),
        ],
    );
    let checked = g.choose(
        g.eq(g.second(stack), g.unit()),
        closed,
        g.error(s, "quotation must produce exactly one result"),
    );
    let checked = g.choose(
        g.eq(stack, g.unit()),
        g.error(s, "quotation must produce exactly one result"),
        checked,
    );
    g.family(
        1,
        &[
            (g.failed(s), s),
            (g.mode(s, "compile"), checked),
            (g.yes(), g.error(s, "no quotation is open")),
        ],
    )
}

fn build_finish(g: &Graph<'_>) -> Cid {
    let s = g.param(0);
    let stack = g.get(s, "stack");
    let dictionary = g.node(Node::PutKey {
        record: g.get(s, "dictionary"),
        key: g.get(s, "name"),
        value: g.first(stack),
    });
    let finished = g.update(
        s,
        &[
            ("dictionary", dictionary),
            ("stack", g.get(s, "saved")),
            ("saved", g.unit()),
            ("name", g.unit()),
        ],
    );
    let checked = g.choose(
        g.eq(g.second(stack), g.unit()),
        finished,
        g.error(s, "definition must produce exactly one value"),
    );
    let checked = g.choose(
        g.eq(stack, g.unit()),
        g.error(s, "definition must produce exactly one value"),
        checked,
    );
    g.family(
        1,
        &[
            (g.failed(s), s),
            (
                g.not(g.mode(s, "eval")),
                g.error(s, "definition ended with an open quotation"),
            ),
            (
                g.eq(g.get(s, "name"), g.unit()),
                g.error(s, "no definition is open"),
            ),
            (g.yes(), checked),
        ],
    )
}

fn build_colon(g: &Graph<'_>, syntax: Syntax) -> Cid {
    let s = g.param(0);
    let opened = g.update(
        s,
        &[
            ("saved", g.get(s, "stack")),
            ("stack", g.unit()),
            ("mode", g.text("eval")),
        ],
    );
    if syntax == Syntax::NameFirst {
        g.family(
            1,
            &[
                (g.mode(s, "await-binding"), opened),
                (g.yes(), g.error(s, "binding requires a preceding name")),
            ],
        )
    } else {
        let opened = g.put(opened, "mode", g.text("await-name"));
        g.family(
            1,
            &[
                (
                    g.not(g.eq(g.get(s, "name"), g.unit())),
                    g.error(s, "nested definitions are not supported"),
                ),
                (g.mode(s, "eval"), opened),
                (
                    g.yes(),
                    g.error(s, "definition is not enabled in this mode"),
                ),
            ],
        )
    }
}

fn build_step(g: &Graph<'_>, execute: Cid, syntax: Syntax) -> Cid {
    let s = g.param(0);
    let token = g.get(s, "token");
    let lookup = g.lookup(g.get(s, "dictionary"), token);
    let entry = g.get(lookup, "value");
    let number = g.node(Node::ParseInt(token));
    let integer = g.integer(g.get(number, "value"));
    let normal = g.choose(
        g.get(number, "found"),
        g.call(execute, &[s, integer]),
        g.choose(
            g.get(lookup, "found"),
            g.call(execute, &[s, entry]),
            g.error(s, "unknown word"),
        ),
    );
    let quoted = g.choose(
        g.get(lookup, "found"),
        g.push(g.put(s, "mode", g.text("eval")), entry),
        g.error(s, "quoted name does not exist"),
    );
    let binding = g.choose(
        g.get(lookup, "found"),
        g.choose(
            g.get(entry, "binder"),
            g.call(execute, &[s, entry]),
            g.error(s, "expected a binding word"),
        ),
        g.error(s, "expected a binding word"),
    );
    let normal = if syntax == Syntax::NameFirst {
        let next = g.next(s);
        let following = g.lookup(g.get(s, "dictionary"), g.get(next, "token"));
        let binder = g.choose(
            g.get(next, "found"),
            g.choose(
                g.get(following, "found"),
                g.get(g.get(following, "value"), "binder"),
                g.no(),
            ),
            g.no(),
        );
        // Existing handler words retain precedence, so `quote :` can retrieve
        // a parser word. Ordinary values can be rebound using lookahead.
        let handler = g.choose(
            g.get(lookup, "found"),
            g.is(g.get(entry, "kind"), "handler"),
            g.no(),
        );
        let candidate = g.choose(handler, g.no(), binder);
        let candidate = g.choose(g.eq(g.get(s, "name"), g.unit()), candidate, g.no());
        let candidate = g.choose(g.mode(s, "eval"), candidate, g.no());
        let pending = g.update(s, &[("mode", g.text("await-binding")), ("name", token)]);
        let pending = g.choose(
            g.get(number, "found"),
            g.error(s, "integer literals cannot be definition names"),
            pending,
        );
        g.choose(candidate, pending, normal)
    } else {
        normal
    };
    let named = g.update(
        s,
        &[
            ("name", token),
            ("mode", g.text("compile")),
            ("inputs", g.int(0)),
            ("quote-saved", g.unit()),
        ],
    );
    let named = g.choose(
        g.get(number, "found"),
        g.error(s, "integer literals cannot be definition names"),
        named,
    );
    g.family(
        1,
        &[
            (g.mode(s, "quoted-name"), quoted),
            (g.mode(s, "await-binding"), binding),
            (g.mode(s, "await-name"), named),
            (g.yes(), normal),
        ],
    )
}

fn build_runner(g: &Graph<'_>, step: Cid) -> Cid {
    let s = g.param(0);
    let quota = g.param(1);
    let next = g.next(s);
    let advanced = g.update(
        s,
        &[
            ("position", g.get(next, "position")),
            ("token", g.get(next, "token")),
        ],
    );
    let progressed = g.call(step, &[advanced]);
    let again = g.recur(&[progressed, g.add(quota, g.int(-1))]);
    let eof = g.put(s, "position", g.get(next, "position"));
    let eof = g.choose(
        g.eq(g.get(s, "name"), g.unit()),
        eof,
        g.error(eof, "unfinished definition"),
    );
    let eof = g.choose(
        g.mode(s, "eval"),
        eof,
        g.error(eof, "unfinished reader mode"),
    );
    g.family(
        2,
        &[
            (g.failed(s), s),
            (g.eq(quota, g.int(0)), s),
            (g.not(g.get(next, "found")), eof),
            (g.yes(), again),
        ],
    )
}
