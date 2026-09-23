//! Path-sensitive control-flow experiment for the static resource planner.
//!
//! This first version deliberately enumerates branch paths.  That gives a
//! correctness oracle and exposes specialization growth; it is not proposed as
//! the final representation.  A later structured plan should share prefixes,
//! suffixes, and compatible joins while preserving these per-path results.

use crate::memory::{
    InputIdentities, Instr, MemoryPlan, OracleReport, PlanError, Resource, VerifyError, plan,
    verify_plan,
};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Flow {
    Op(Instr),
    If {
        label: String,
        when_true: Vec<Flow>,
        when_false: Vec<Flow>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AnnotatedFlow {
    Op {
        instruction: Instr,
        allocation: Option<Resource>,
    },
    If {
        label: String,
        when_true: Vec<AnnotatedFlow>,
        when_false: Vec<AnnotatedFlow>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExpandedPath {
    decisions: Vec<(String, bool)>,
    instructions: Vec<Instr>,
    allocations: Vec<Option<Resource>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathPlan {
    pub decisions: Vec<(String, bool)>,
    pub program: Vec<Instr>,
    pub plan: MemoryPlan,
}

impl PathPlan {
    pub fn verify(&self, inputs: &InputIdentities) -> Result<OracleReport, VerifyError> {
        verify_plan(&self.program, &self.plan, inputs)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowPlanSet {
    pub paths: Vec<PathPlan>,
    pub allocation_sites: usize,
    pub expanded_steps: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FlowError {
    Plan(PlanError),
    AllocationShape,
}

impl fmt::Display for FlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plan(error) => error.fmt(f),
            Self::AllocationShape => {
                f.write_str("expanded branch allocations do not match their static sites")
            }
        }
    }
}

impl std::error::Error for FlowError {}

impl From<PlanError> for FlowError {
    fn from(value: PlanError) -> Self {
        Self::Plan(value)
    }
}

pub fn plan_paths(program: &[Flow]) -> Result<FlowPlanSet, FlowError> {
    let mut next_site = 0_u32;
    let annotated = annotate(program, &mut next_site);
    let expanded = expand_sequence(
        &annotated,
        vec![ExpandedPath {
            decisions: Vec::new(),
            instructions: Vec::new(),
            allocations: Vec::new(),
        }],
    );
    let mut paths = Vec::with_capacity(expanded.len());
    let mut expanded_steps = 0_usize;
    for path in expanded {
        let dynamic = plan(&path.instructions)?;
        let plan = relabel_allocations(dynamic, &path.allocations)?;
        expanded_steps += plan.steps.len();
        paths.push(PathPlan {
            decisions: path.decisions,
            program: path.instructions,
            plan,
        });
    }
    Ok(FlowPlanSet {
        paths,
        allocation_sites: next_site as usize,
        expanded_steps,
    })
}

fn annotate(program: &[Flow], next_site: &mut u32) -> Vec<AnnotatedFlow> {
    program
        .iter()
        .map(|flow| match flow {
            Flow::Op(instruction) => {
                let allocation = if instruction == &Instr::Pair {
                    let resource = Resource::Local(*next_site);
                    *next_site += 1;
                    Some(resource)
                } else {
                    None
                };
                AnnotatedFlow::Op {
                    instruction: instruction.clone(),
                    allocation,
                }
            }
            Flow::If {
                label,
                when_true,
                when_false,
            } => AnnotatedFlow::If {
                label: label.clone(),
                when_true: annotate(when_true, next_site),
                when_false: annotate(when_false, next_site),
            },
        })
        .collect()
}

fn expand_sequence(program: &[AnnotatedFlow], mut paths: Vec<ExpandedPath>) -> Vec<ExpandedPath> {
    for flow in program {
        match flow {
            AnnotatedFlow::Op {
                instruction,
                allocation,
            } => {
                for path in &mut paths {
                    path.instructions.push(instruction.clone());
                    path.allocations.push(*allocation);
                }
            }
            AnnotatedFlow::If {
                label,
                when_true,
                when_false,
            } => {
                let mut branched = Vec::new();
                for path in paths {
                    let mut true_path = path.clone();
                    true_path.decisions.push((label.clone(), true));
                    branched.extend(expand_sequence(when_true, vec![true_path]));

                    let mut false_path = path;
                    false_path.decisions.push((label.clone(), false));
                    branched.extend(expand_sequence(when_false, vec![false_path]));
                }
                paths = branched;
            }
        }
    }
    paths
}

fn relabel_allocations(
    mut plan: MemoryPlan,
    static_allocations: &[Option<Resource>],
) -> Result<MemoryPlan, FlowError> {
    if plan.steps.len() != static_allocations.len() {
        return Err(FlowError::AllocationShape);
    }
    let mut mapping = BTreeMap::<Resource, Resource>::new();
    for (step, static_resource) in plan.steps.iter().zip(static_allocations) {
        match (step.allocate, static_resource) {
            (Some(dynamic), Some(static_resource)) => {
                mapping.insert(dynamic, *static_resource);
            }
            (None, None) => {}
            _ => return Err(FlowError::AllocationShape),
        }
    }
    let relabel = |resource: Resource| match resource {
        Resource::Input(_) => resource,
        Resource::Local(_) => mapping.get(&resource).copied().unwrap_or(resource),
    };
    for (step, static_resource) in plan.steps.iter_mut().zip(static_allocations) {
        step.allocate = *static_resource;
        for resource in &mut step.free {
            *resource = relabel(*resource);
        }
    }
    for placement in &mut plan.placements {
        placement.resource = relabel(placement.resource);
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(instruction: Instr) -> Flow {
        Flow::Op(instruction)
    }

    #[test]
    fn both_dynamic_paths_reclaim_at_their_actual_last_use() {
        let program = vec![
            op(Instr::Int(1)),
            op(Instr::Int(2)),
            op(Instr::Pair),
            Flow::If {
                label: "condition".into(),
                when_true: vec![op(Instr::Drop)],
                when_false: vec![op(Instr::Dup), op(Instr::Drop), op(Instr::Drop)],
            },
        ];
        let plans = plan_paths(&program).unwrap();
        assert_eq!(plans.paths.len(), 2);
        for path in &plans.paths {
            let oracle = path.verify(&InputIdentities::new()).unwrap();
            assert!(oracle.premature_frees.is_empty());
            assert!(oracle.missed_frees.is_empty());
            assert_eq!(oracle.max_reclamation_delay, 0);
        }
        let true_path = plans
            .paths
            .iter()
            .find(|path| path.decisions == [("condition".into(), true)])
            .unwrap();
        let false_path = plans
            .paths
            .iter()
            .find(|path| path.decisions == [("condition".into(), false)])
            .unwrap();
        assert_eq!(true_path.plan.steps[3].free, [Resource::Local(0)]);
        assert_eq!(false_path.plan.steps[5].free, [Resource::Local(0)]);
    }

    #[test]
    fn allocation_sites_in_opposite_branches_keep_distinct_identities() {
        let make_and_drop = || {
            vec![
                op(Instr::Int(1)),
                op(Instr::Int(2)),
                op(Instr::Pair),
                op(Instr::Drop),
            ]
        };
        let program = vec![Flow::If {
            label: "side".into(),
            when_true: make_and_drop(),
            when_false: make_and_drop(),
        }];
        let plans = plan_paths(&program).unwrap();
        assert_eq!(plans.allocation_sites, 2);
        assert_eq!(
            plans.paths[0].plan.steps[2].allocate,
            Some(Resource::Local(0))
        );
        assert_eq!(
            plans.paths[1].plan.steps[2].allocate,
            Some(Resource::Local(1))
        );
        for path in &plans.paths {
            let oracle = path.verify(&InputIdentities::new()).unwrap();
            assert!(oracle.premature_frees.is_empty());
            assert!(oracle.missed_frees.is_empty());
        }
    }

    #[test]
    fn naive_path_specialization_exposes_exponential_growth() {
        let program = (0..8)
            .map(|index| Flow::If {
                label: format!("b{index}"),
                when_true: vec![],
                when_false: vec![],
            })
            .collect::<Vec<_>>();
        let plans = plan_paths(&program).unwrap();
        assert_eq!(plans.paths.len(), 256);
        assert_eq!(plans.expanded_steps, 0);
    }
}
