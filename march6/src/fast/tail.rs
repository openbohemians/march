//! Conservative tail-loop execution, derived from immutable code.
//!
//! Only one-input/one-output families qualify. Their first guard must demand
//! the input before doing anything fallible. Consequently evaluating the next
//! argument at the back edge preserves demand and error order. All guards,
//! terminal bodies, and next-argument expressions must be scalar fast plans.
//! This is not eager calling in general, nor reclamation of arbitrary thunks.
use super::*;

#[derive(Clone, Debug)]
pub(super) struct TailPlan {
    entry: FastPlan,
    clauses: Vec<Clause>,
    direct_family: bool,
    registers: usize,
}

#[derive(Clone, Debug)]
struct Clause {
    guard: FastPlan,
    body: FastPlan,
    recur: bool,
    unchanged_argument: bool,
}

pub(super) fn compile(
    program: &Program,
    inputs: usize,
    ops: &[Op],
    outputs: &[Slot],
) -> Option<TailPlan> {
    let &[output] = outputs else { return None };
    let Op::Project { call, output: 0 } = ops.get(output)? else {
        return None;
    };
    match ops.get(*call)? {
        Op::Dispatch { clauses, arguments } => {
            if inputs != 1
                || arguments.len() != 1
                || !matches!(ops[arguments[0]], Op::Arg(0))
                || clauses.len() > 1024
            {
                return None;
            }
            let first = program.word(clauses.first()?.0).ok()?.fast.as_ref()?;
            // A simple, deliberately conservative proof of strict first demand.
            // Constants before Arg could also be safe, but need not qualify yet.
            if !matches!(first.ops.first(), Some(FastOp::Arg(_, 0))) {
                return None;
            }
            let mut compiled = Vec::new();
            let mut has_recur = false;
            let mut registers = 1;
            let mut size = 1usize;
            for &(guard, body) in clauses {
                let guard = program.word(guard).ok()?.fast.clone()?;
                let body = program.word(body).ok()?;
                if body.inputs != 1 || body.outputs.len() != 1 {
                    return None;
                }
                let (body_plan, recur, unchanged_argument) = if let Some(plan) = &body.fast {
                    (plan.clone(), false, false)
                } else {
                    let &[out] = body.outputs.as_slice() else {
                        return None;
                    };
                    let Op::Project { call, output: 0 } = body.ops[out] else {
                        return None;
                    };
                    let Op::Recur { arguments } = &body.ops[call] else {
                        return None;
                    };
                    let &[arg] = arguments.as_slice() else {
                        return None;
                    };
                    let plan = fast_plan(program, &body.ops, &[arg])?;
                    (plan, true, matches!(body.ops[arg], Op::Arg(0)))
                };
                has_recur |= recur;
                size = size
                    .checked_add(guard.ops.len())?
                    .checked_add(body_plan.ops.len())?;
                if size > 16_384 {
                    return None;
                }
                registers = registers.max(guard.ops.len()).max(body_plan.ops.len());
                compiled.push(Clause {
                    guard,
                    body: body_plan,
                    recur,
                    unchanged_argument,
                });
            }
            if !has_recur {
                return None;
            }
            Some(TailPlan {
                entry: FastPlan {
                    ops: vec![FastOp::Arg(0, 0)],
                    outputs: vec![0],
                },
                clauses: compiled,
                direct_family: true,
                registers,
            })
        }
        Op::Call { word, arguments } => {
            // A source entry such as `100 countdown`. Only wrap a direct family;
            // do not silently discard another wrapper's input transformation.
            let target = program.word(*word).ok()?.tail.as_ref()?;
            if !target.direct_family || arguments.len() != 1 {
                return None;
            }
            let entry = fast_plan(program, ops, arguments)?;
            let size: usize = target
                .clauses
                .iter()
                .map(|c| c.guard.ops.len() + c.body.ops.len())
                .sum();
            if size.checked_add(entry.ops.len())? > 16_384 {
                return None;
            }
            Some(TailPlan {
                registers: target.registers.max(entry.ops.len()),
                entry,
                clauses: target.clauses.clone(),
                direct_family: false,
            })
        }
        _ => None,
    }
}

impl Executor<'_> {
    fn tail_scalar(&mut self, plan: &FastPlan, args: &[Datum]) -> Result<Datum, Error> {
        for op in &plan.ops {
            if self.remaining == 0 {
                return Err(Error::Budget);
            }
            self.remaining -= 1;
            self.stats.steps += 1;
            match *op {
                FastOp::Arg(dst, arg) => self.registers[dst] = args[arg],
                FastOp::Const(dst, v) => self.registers[dst] = v.into(),
                FastOp::Binary(dst, b, a, c) => {
                    self.stats.primitive_ops += 1;
                    self.registers[dst] = binary(b, self.registers[a], self.registers[c])?;
                }
            }
        }
        Ok(self.registers[plan.outputs[0]])
    }

    pub(super) fn run_tail(&mut self, plan: &TailPlan, args: &[Literal]) -> Result<Datum, Error> {
        if plan.registers > self.cell_limit || args.len().max(1) > self.argument_limit {
            return Err(Error::StorageLimit);
        }
        self.registers.resize(plan.registers, Datum::Unit);
        self.stats.peak_registers = plan.registers;
        self.stats.peak_arguments = args.len().max(1);
        let args: Vec<_> = args.iter().copied().map(Datum::from).collect();
        let mut argument = self.tail_scalar(&plan.entry, &args)?;
        loop {
            // Even trivial constant plans cannot form an unbudgeted loop.
            if self.remaining == 0 {
                return Err(Error::Budget);
            }
            self.remaining -= 1;
            self.stats.steps += 1;
            self.stats.tail_iterations += 1;
            self.stats.calls += 1;
            let mut chosen = None;
            for clause in &plan.clauses {
                self.stats.calls += 1;
                let Datum::Bool(selected) = self.tail_scalar(&clause.guard, &[argument])? else {
                    return Err(Error::Type("guard must be Boolean"));
                };
                if selected {
                    chosen = Some(clause);
                    break;
                }
            }
            let clause = chosen.ok_or(Error::NoClause)?;
            self.stats.calls += 1;
            let value = self.tail_scalar(&clause.body, &[argument])?;
            if !clause.recur {
                return Ok(value);
            }
            // A direct parameter alias preserves the argument-cell identity in
            // the generic evaluator, so revisiting this family's output cycles.
            // Equal VALUES computed by a new expression must not trigger this.
            if clause.unchanged_argument {
                return Err(Error::Cycle);
            }
            argument = value;
        }
    }
}
