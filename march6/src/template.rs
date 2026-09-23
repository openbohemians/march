//! Per-invocation resource-plan templates.
//!
//! A static allocation site is not a runtime object.  This module instantiates
//! a verified closed memory plan repeatedly, assigning every dynamic object an
//! `(invocation, site)` identity while reusing the plan's physical slots.

use crate::memory::{
    InputIdentities, Instr, MemoryPlan, PlanError, ReferenceCountReport, Resource, VerifyError,
    plan, simulate_reference_counting, verify_plan,
};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    body: Vec<Instr>,
    plan: MemoryPlan,
    reference_counting: ReferenceCountReport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instance {
    pub invocation: usize,
    pub site: Resource,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TemplateRun {
    pub invocations: usize,
    pub dynamic_allocations: usize,
    pub exact_frees: usize,
    pub allocator_slots: usize,
    pub static_reuses: usize,
    pub frame_arena_slots: usize,
    pub iteration_arena_slots: usize,
    pub replayed_plan_steps: usize,
    pub rc_increments: usize,
    pub rc_decrements: usize,
    pub rc_zero_tests: usize,
    pub rc_frees: usize,
    pub unreclaimed_instances: usize,
    pub double_frees: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemplateError {
    Plan(PlanError),
    Verify(VerifyError),
    ReturnedValues(usize),
    EscapingAllocation(Resource),
    OpenInput,
    OracleDisagreement,
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plan(error) => error.fmt(f),
            Self::Verify(error) => error.fmt(f),
            Self::ReturnedValues(count) => {
                write!(f, "closed iteration template returns {count} stack values")
            }
            Self::EscapingAllocation(resource) => {
                write!(f, "allocation {resource:?} escapes the iteration template")
            }
            Self::OpenInput => f.write_str("closed iteration template refers to an open input"),
            Self::OracleDisagreement => {
                f.write_str("compile-time plan disagrees with the verification oracle")
            }
        }
    }
}

impl std::error::Error for TemplateError {}

impl From<PlanError> for TemplateError {
    fn from(value: PlanError) -> Self {
        Self::Plan(value)
    }
}

impl From<VerifyError> for TemplateError {
    fn from(value: VerifyError) -> Self {
        Self::Verify(value)
    }
}

impl Template {
    pub fn compile(body: &[Instr]) -> Result<Self, TemplateError> {
        if body
            .iter()
            .any(|instruction| matches!(instruction, Instr::InputRef { .. }))
        {
            return Err(TemplateError::OpenInput);
        }
        let plan = plan(body)?;
        if plan.returned != 0 {
            return Err(TemplateError::ReturnedValues(plan.returned));
        }
        if let Some(placement) = plan
            .placements
            .iter()
            .find(|placement| placement.freed_at.is_none())
        {
            return Err(TemplateError::EscapingAllocation(placement.resource));
        }
        let oracle = verify_plan(body, &plan, &InputIdentities::new())?;
        if !oracle.premature_frees.is_empty() {
            return Err(TemplateError::OracleDisagreement);
        }
        if let Some(resource) = oracle.missed_frees.first() {
            return Err(TemplateError::EscapingAllocation(*resource));
        }
        let reference_counting = simulate_reference_counting(body, &InputIdentities::new())?;
        Ok(Self {
            body: body.to_vec(),
            plan,
            reference_counting,
        })
    }

    pub fn body(&self) -> &[Instr] {
        &self.body
    }

    pub fn plan(&self) -> &MemoryPlan {
        &self.plan
    }

    pub fn run(&self, invocations: usize) -> TemplateRun {
        let allocations_per_invocation = self.plan.metrics.local_allocations;
        let frees_per_invocation = self.plan.metrics.exact_frees;
        let dynamic_allocations = allocations_per_invocation * invocations;
        let exact_frees = frees_per_invocation * invocations;

        // Verify instance-level identity, independently of allocation-site
        // identity.  A site may be live once per invocation but never aliases
        // an instance from another invocation.
        let mut live = std::collections::BTreeSet::<Instance>::new();
        let mut unreclaimed_instances = 0_usize;
        let mut double_frees = 0_usize;
        for invocation in 0..invocations {
            for step in &self.plan.steps {
                if let Some(site @ Resource::Local(_)) = step.allocate {
                    live.insert(Instance { invocation, site });
                }
                for site in &step.free {
                    if !live.remove(&Instance {
                        invocation,
                        site: *site,
                    }) {
                        double_frees += 1;
                    }
                }
            }
            if live
                .iter()
                .any(|instance| instance.invocation == invocation)
            {
                unreclaimed_instances += live
                    .iter()
                    .filter(|instance| instance.invocation == invocation)
                    .count();
                live.retain(|instance| instance.invocation != invocation);
            }
        }

        let allocator_slots = if invocations == 0 {
            0
        } else {
            self.plan.metrics.physical_slots
        };
        TemplateRun {
            invocations,
            dynamic_allocations,
            exact_frees,
            allocator_slots,
            static_reuses: dynamic_allocations.saturating_sub(allocator_slots),
            frame_arena_slots: dynamic_allocations,
            iteration_arena_slots: allocator_slots,
            replayed_plan_steps: self.plan.steps.len() * invocations,
            rc_increments: self.reference_counting.increments * invocations,
            rc_decrements: self.reference_counting.decrements * invocations,
            rc_zero_tests: self.reference_counting.zero_tests * invocations,
            rc_frees: self.reference_counting.frees * invocations,
            unreclaimed_instances,
            double_frees,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_loop_count_uses_one_verified_plan_template() {
        let body = [Instr::Int(1), Instr::Int(2), Instr::Pair, Instr::Drop];
        let template = Template::compile(&body).unwrap();
        let run = template.run(10_000);
        assert_eq!(run.dynamic_allocations, 10_000);
        assert_eq!(run.exact_frees, 10_000);
        assert_eq!(run.allocator_slots, 1);
        assert_eq!(run.static_reuses, 9_999);
        assert_eq!(run.frame_arena_slots, 10_000);
        assert_eq!(run.iteration_arena_slots, 1);
        assert_eq!(run.replayed_plan_steps, 40_000);
        assert_eq!(run.rc_decrements, 10_000);
        assert_eq!(run.rc_zero_tests, 10_000);
        assert_eq!(run.unreclaimed_instances, 0);
        assert_eq!(run.double_frees, 0);
    }

    #[test]
    fn zero_iterations_allocate_nothing() {
        let body = [Instr::Int(1), Instr::Int(2), Instr::Pair, Instr::Drop];
        let template = Template::compile(&body).unwrap();
        let run = template.run(0);
        assert_eq!(run.dynamic_allocations, 0);
        assert_eq!(run.allocator_slots, 0);
    }

    #[test]
    fn escaping_body_is_not_a_closed_reusable_template() {
        let body = [Instr::Int(1), Instr::Int(2), Instr::Pair];
        assert!(matches!(
            Template::compile(&body),
            Err(TemplateError::ReturnedValues(1))
        ));
    }

    #[test]
    fn open_inputs_require_a_real_invocation_contract() {
        let body = [
            Instr::InputRef {
                index: 0,
                ownership: crate::memory::Ownership::Unique,
            },
            Instr::Drop,
        ];
        assert_eq!(Template::compile(&body), Err(TemplateError::OpenInput));
    }
}
