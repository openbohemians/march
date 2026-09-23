use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ownership {
    /// The caller retains responsibility; this word must not reclaim it.
    Borrowed,
    /// The invocation owns this object and promises it does not alias another
    /// unique input.
    Unique,
    /// Reclamation needs runtime reference accounting or another fallback.
    Shared,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Instr {
    Int(i64),
    InputRef { index: u16, ownership: Ownership },
    Pair,
    Dup,
    Drop,
    Swap,
    First,
    Second,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Resource {
    Input(u16),
    Local(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Symbol {
    Scalar,
    Ref(Resource),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Object {
    fields: Vec<Symbol>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanStep {
    pub instruction: Instr,
    /// Static allocation-site identity, distinct from a dynamic object
    /// instance.  Straight-line E0 executes each site once; control-flow
    /// plans may execute a site zero or one times on a selected path.
    pub allocate: Option<Resource>,
    pub free: Vec<Resource>,
    pub fallback: Vec<FallbackOp>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FallbackOp {
    /// Release one shared input handle with runtime reference accounting.
    ReleaseShared(Resource),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    pub resource: Resource,
    /// A reusable physical slot for equal-sized E0 pair headers.
    pub slot: usize,
    pub allocated_at: usize,
    pub freed_at: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanMetrics {
    pub local_allocations: usize,
    pub unique_inputs: usize,
    pub exact_frees: usize,
    pub fallback_inputs: usize,
    pub fallback_operations: usize,
    pub runtime_liveness_checks: usize,
    /// Physical pair slots needed with the statically derived reuse schedule.
    pub physical_slots: usize,
    pub reused_allocations: usize,
    /// Managed resources simultaneously reachable at the planner boundary.
    pub peak_managed: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryPlan {
    pub steps: Vec<PlanStep>,
    pub placements: Vec<Placement>,
    pub returned: usize,
    pub metrics: PlanMetrics,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError {
    StackUnderflow(&'static str),
    ExpectedReference(&'static str),
    UnknownFields(Resource),
    ConflictingInputContract(u16),
    UseAfterFree(Resource),
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StackUnderflow(op) => write!(f, "stack underflow in {op}"),
            Self::ExpectedReference(op) => write!(f, "{op} expected a pair reference"),
            Self::UnknownFields(resource) => {
                write!(f, "fields of open input {resource:?} are not summarized")
            }
            Self::ConflictingInputContract(index) => {
                write!(f, "input {index} has conflicting ownership contracts")
            }
            Self::UseAfterFree(resource) => write!(f, "planned use after free of {resource:?}"),
        }
    }
}

impl std::error::Error for PlanError {}

pub fn plan(program: &[Instr]) -> Result<MemoryPlan, PlanError> {
    let mut stack = Vec::<Symbol>::new();
    let mut objects = BTreeMap::<Resource, Object>::new();
    let mut contracts = BTreeMap::<u16, Ownership>::new();
    let mut managed = BTreeSet::<Resource>::new();
    let mut fallback_managed = BTreeSet::<Resource>::new();
    let mut freed = BTreeSet::<Resource>::new();
    let mut fallback_released = BTreeSet::<Resource>::new();
    let mut next_local = 0_u32;
    let mut metrics = PlanMetrics::default();
    let mut steps = Vec::new();
    let mut peak_managed = 0_usize;

    for instruction in program {
        let allocate = (instruction == &Instr::Pair).then_some(Resource::Local(next_local));
        execute_symbolic(
            instruction,
            &mut stack,
            &mut objects,
            &mut contracts,
            &mut managed,
            &mut fallback_managed,
            &freed,
            &fallback_released,
            &mut next_local,
            &mut metrics,
        )?;
        let reachable = reachable_from(&stack, &objects);
        let dead = managed
            .difference(&reachable)
            .filter(|resource| !freed.contains(resource))
            .copied()
            .collect::<BTreeSet<_>>();
        let mut free = Vec::new();
        let mut visited = BTreeSet::new();
        for resource in dead.iter().copied() {
            postorder_dead(resource, &dead, &objects, &mut visited, &mut free);
        }
        peak_managed = peak_managed.max(managed.len() - freed.len());
        for resource in &free {
            freed.insert(*resource);
        }
        let fallback = fallback_managed
            .difference(&reachable)
            .filter(|resource| !fallback_released.contains(resource))
            .copied()
            .map(FallbackOp::ReleaseShared)
            .collect::<Vec<_>>();
        for operation in &fallback {
            let FallbackOp::ReleaseShared(resource) = operation;
            fallback_released.insert(*resource);
        }
        metrics.exact_frees += free.len();
        metrics.fallback_operations += fallback.len();
        metrics.runtime_liveness_checks += fallback.len();
        steps.push(PlanStep {
            instruction: instruction.clone(),
            allocate,
            free,
            fallback,
        });
    }

    let (placements, physical_slots, reused_allocations) = assign_slots(&steps);
    metrics.physical_slots = physical_slots;
    metrics.reused_allocations = reused_allocations;
    metrics.peak_managed = peak_managed;

    Ok(MemoryPlan {
        steps,
        placements,
        returned: stack.len(),
        metrics,
    })
}

fn assign_slots(steps: &[PlanStep]) -> (Vec<Placement>, usize, usize) {
    let mut next_slot = 0_usize;
    let mut free_slots = BTreeSet::<usize>::new();
    let mut resource_slots = BTreeMap::<Resource, usize>::new();
    let mut placements = Vec::<Placement>::new();
    let mut placement_index = BTreeMap::<Resource, usize>::new();
    let mut reused = 0_usize;

    for (step_index, step) in steps.iter().enumerate() {
        if let Some(resource @ Resource::Local(_)) = step.allocate {
            let slot = match free_slots.pop_first() {
                Some(slot) => {
                    reused += 1;
                    slot
                }
                None => {
                    let slot = next_slot;
                    next_slot += 1;
                    slot
                }
            };
            resource_slots.insert(resource, slot);
            placement_index.insert(resource, placements.len());
            placements.push(Placement {
                resource,
                slot,
                allocated_at: step_index,
                freed_at: None,
            });
        }
        for resource in &step.free {
            if let Some(slot) = resource_slots.get(resource).copied() {
                free_slots.insert(slot);
                if let Some(index) = placement_index.get(resource).copied() {
                    placements[index].freed_at = Some(step_index);
                }
            }
        }
    }

    (placements, next_slot, reused)
}

#[allow(clippy::too_many_arguments)]
fn execute_symbolic(
    instruction: &Instr,
    stack: &mut Vec<Symbol>,
    objects: &mut BTreeMap<Resource, Object>,
    contracts: &mut BTreeMap<u16, Ownership>,
    managed: &mut BTreeSet<Resource>,
    fallback_managed: &mut BTreeSet<Resource>,
    freed: &BTreeSet<Resource>,
    fallback_released: &BTreeSet<Resource>,
    next_local: &mut u32,
    metrics: &mut PlanMetrics,
) -> Result<(), PlanError> {
    match instruction {
        Instr::Int(_) => stack.push(Symbol::Scalar),
        Instr::InputRef { index, ownership } => {
            let resource = Resource::Input(*index);
            if freed.contains(&resource) || fallback_released.contains(&resource) {
                return Err(PlanError::UseAfterFree(resource));
            }
            if let Some(previous) = contracts.insert(*index, *ownership) {
                if previous != *ownership {
                    return Err(PlanError::ConflictingInputContract(*index));
                }
            } else {
                match ownership {
                    Ownership::Borrowed => {}
                    Ownership::Unique => {
                        managed.insert(Resource::Input(*index));
                        metrics.unique_inputs += 1;
                    }
                    Ownership::Shared => {
                        fallback_managed.insert(resource);
                        metrics.fallback_inputs += 1;
                    }
                }
            }
            stack.push(Symbol::Ref(resource));
        }
        Instr::Pair => {
            let right = pop(stack, "pair")?;
            let left = pop(stack, "pair")?;
            ensure_live(&left, freed)?;
            ensure_live(&right, freed)?;
            let resource = Resource::Local(*next_local);
            *next_local += 1;
            objects.insert(
                resource,
                Object {
                    fields: vec![left, right],
                },
            );
            managed.insert(resource);
            metrics.local_allocations += 1;
            stack.push(Symbol::Ref(resource));
        }
        Instr::Dup => {
            let value = stack
                .last()
                .cloned()
                .ok_or(PlanError::StackUnderflow("dup"))?;
            ensure_live(&value, freed)?;
            stack.push(value);
        }
        Instr::Drop => {
            let value = pop(stack, "drop")?;
            ensure_live(&value, freed)?;
        }
        Instr::Swap => {
            if stack.len() < 2 {
                return Err(PlanError::StackUnderflow("swap"));
            }
            let len = stack.len();
            stack.swap(len - 1, len - 2);
        }
        Instr::First => project(stack, objects, freed, 0, "first")?,
        Instr::Second => project(stack, objects, freed, 1, "second")?,
    }
    Ok(())
}

fn pop(stack: &mut Vec<Symbol>, op: &'static str) -> Result<Symbol, PlanError> {
    stack.pop().ok_or(PlanError::StackUnderflow(op))
}

fn ensure_live(value: &Symbol, freed: &BTreeSet<Resource>) -> Result<(), PlanError> {
    if let Symbol::Ref(resource) = value
        && freed.contains(resource)
    {
        return Err(PlanError::UseAfterFree(*resource));
    }
    Ok(())
}

fn project(
    stack: &mut Vec<Symbol>,
    objects: &BTreeMap<Resource, Object>,
    freed: &BTreeSet<Resource>,
    field: usize,
    op: &'static str,
) -> Result<(), PlanError> {
    let pair = pop(stack, op)?;
    ensure_live(&pair, freed)?;
    let Symbol::Ref(resource) = pair else {
        return Err(PlanError::ExpectedReference(op));
    };
    let object = objects
        .get(&resource)
        .ok_or(PlanError::UnknownFields(resource))?;
    let value = object
        .fields
        .get(field)
        .cloned()
        .ok_or(PlanError::ExpectedReference(op))?;
    stack.push(value);
    Ok(())
}

fn reachable_from(stack: &[Symbol], objects: &BTreeMap<Resource, Object>) -> BTreeSet<Resource> {
    let mut reachable = BTreeSet::new();
    let mut pending = stack
        .iter()
        .filter_map(|value| match value {
            Symbol::Ref(resource) => Some(*resource),
            Symbol::Scalar => None,
        })
        .collect::<Vec<_>>();
    while let Some(resource) = pending.pop() {
        if !reachable.insert(resource) {
            continue;
        }
        if let Some(object) = objects.get(&resource) {
            for field in &object.fields {
                if let Symbol::Ref(child) = field {
                    pending.push(*child);
                }
            }
        }
    }
    reachable
}

fn postorder_dead(
    resource: Resource,
    dead: &BTreeSet<Resource>,
    objects: &BTreeMap<Resource, Object>,
    visited: &mut BTreeSet<Resource>,
    output: &mut Vec<Resource>,
) {
    if !dead.contains(&resource) || !visited.insert(resource) {
        return;
    }
    if let Some(object) = objects.get(&resource) {
        for field in &object.fields {
            if let Symbol::Ref(child) = field {
                postorder_dead(*child, dead, objects, visited, output);
            }
        }
    }
    output.push(resource);
}

pub type InputIdentities = BTreeMap<u16, u64>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OracleReport {
    pub oracle_dead_at: BTreeMap<Resource, usize>,
    pub planned_free_at: BTreeMap<Resource, usize>,
    pub premature_frees: Vec<(usize, Resource)>,
    pub missed_frees: Vec<Resource>,
    pub max_reclamation_delay: usize,
    pub peak_reachable_managed: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReferenceCountReport {
    pub increments: usize,
    pub decrements: usize,
    pub zero_tests: usize,
    pub frees: usize,
    pub peak_live_locals: usize,
    /// Dynamic instruction at which ownership-transfer reference counting
    /// reaches zero.  This is computed without graph-reachability walks and
    /// therefore serves as a second, algorithmically independent lifetime
    /// oracle for locals and unique inputs.
    pub freed_at: BTreeMap<Resource, usize>,
}

/// A simple region baseline that resets whenever the operand stack becomes
/// empty.  It is stronger than one frame-wide arena, but retains every local
/// allocation while any value keeps the current scope open.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScopedRegionReport {
    pub allocations: usize,
    pub resets: usize,
    pub physical_slots: usize,
    pub peak_retained: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TracingReport {
    pub collection_interval: usize,
    pub collections: usize,
    pub roots_scanned: usize,
    pub objects_traced: usize,
    pub edges_scanned: usize,
    pub objects_swept: usize,
    pub frees: usize,
    pub peak_resident_locals: usize,
}

pub const TRACING_ALLOCATION_INTERVAL: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FreeTimeMismatch {
    pub resource: Resource,
    pub planned: Option<usize>,
    pub reference_counted: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryComparison {
    pub plan: MemoryPlan,
    pub oracle: OracleReport,
    pub reference_counting: ReferenceCountReport,
    pub scoped_regions: ScopedRegionReport,
    pub tracing: TracingReport,
    pub rc_oracle_mismatches: Vec<FreeTimeMismatch>,
    /// Equal-sized slots retained until a frame/arena boundary.
    pub frame_arena_slots: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifyError {
    Plan(PlanError),
    PlanShape,
    UniqueAlias {
        unique: u16,
        other: u16,
        identity: u64,
    },
    MissingResource(Resource),
    RuntimeUseAfterFree(u64),
    DoubleFree(Resource),
    CounterUnderflow(u64),
    InputIdentityOutOfRange {
        index: u16,
        identity: u64,
    },
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plan(error) => error.fmt(f),
            Self::PlanShape => f.write_str("memory plan does not match the program"),
            Self::UniqueAlias {
                unique,
                other,
                identity,
            } => write!(
                f,
                "unique input {unique} aliases input {other} at runtime identity {identity}"
            ),
            Self::MissingResource(resource) => {
                write!(f, "plan names unavailable resource {resource:?}")
            }
            Self::RuntimeUseAfterFree(identity) => {
                write!(f, "runtime use after free of identity {identity}")
            }
            Self::DoubleFree(resource) => write!(f, "plan frees {resource:?} twice"),
            Self::CounterUnderflow(identity) => {
                write!(f, "reference count underflow for identity {identity}")
            }
            Self::InputIdentityOutOfRange { index, identity } => write!(
                f,
                "input {index} identity {identity} uses the reserved local-identity bit"
            ),
        }
    }
}

impl std::error::Error for VerifyError {}

impl From<PlanError> for VerifyError {
    fn from(value: PlanError) -> Self {
        Self::Plan(value)
    }
}

#[derive(Clone, Debug)]
enum RuntimeValue {
    Scalar,
    Ref(u64),
}

#[derive(Clone, Debug)]
struct RuntimeObject {
    fields: Vec<RuntimeValue>,
}

pub fn compare(
    program: &[Instr],
    input_identities: &InputIdentities,
) -> Result<MemoryComparison, VerifyError> {
    let plan = plan(program)?;
    let oracle = verify_plan(program, &plan, input_identities)?;
    let reference_counting = simulate_reference_counting(program, input_identities)?;
    let scoped_regions = simulate_scoped_regions(program)?;
    let tracing = simulate_tracing(
        program,
        &plan,
        input_identities,
        TRACING_ALLOCATION_INTERVAL,
    )?;
    let rc_oracle_mismatches = compare_free_times(&plan, &reference_counting);
    let frame_arena_slots = plan.metrics.local_allocations;
    Ok(MemoryComparison {
        plan,
        oracle,
        reference_counting,
        scoped_regions,
        tracing,
        rc_oracle_mismatches,
        frame_arena_slots,
    })
}

pub fn simulate_tracing(
    program: &[Instr],
    plan: &MemoryPlan,
    input_identities: &InputIdentities,
    collection_interval: usize,
) -> Result<TracingReport, VerifyError> {
    if collection_interval == 0
        || plan.steps.len() != program.len()
        || plan
            .steps
            .iter()
            .zip(program)
            .any(|(step, instruction)| &step.instruction != instruction)
    {
        return Err(VerifyError::PlanShape);
    }
    validate_input_aliases(program, input_identities)?;
    let mut stack = Vec::<RuntimeValue>::new();
    let mut objects = BTreeMap::<u64, RuntimeObject>::new();
    let mut identities = BTreeMap::<Resource, u64>::new();
    let mut allocated = BTreeSet::<Resource>::new();
    let mut next_local = 0_u32;
    let never_freed = BTreeSet::<u64>::new();
    let mut allocations_since_collection = 0_usize;
    let mut last_collection_step = None;
    let mut report = TracingReport {
        collection_interval,
        ..TracingReport::default()
    };

    for (step_index, (instruction, step)) in program.iter().zip(&plan.steps).enumerate() {
        execute_runtime(
            instruction,
            step.allocate,
            &mut stack,
            &mut objects,
            &mut identities,
            &mut allocated,
            &mut next_local,
            input_identities,
            &never_freed,
        )?;
        if instruction == &Instr::Pair {
            allocations_since_collection += 1;
        }
        report.peak_resident_locals = report.peak_resident_locals.max(objects.len());
        if allocations_since_collection == collection_interval {
            trace_collect(&stack, &mut objects, &mut report);
            allocations_since_collection = 0;
            last_collection_step = Some(step_index);
        }
    }
    if !objects.is_empty() && last_collection_step != program.len().checked_sub(1) {
        trace_collect(&stack, &mut objects, &mut report);
    }
    Ok(report)
}

fn trace_collect(
    stack: &[RuntimeValue],
    objects: &mut BTreeMap<u64, RuntimeObject>,
    report: &mut TracingReport,
) {
    report.collections += 1;
    report.roots_scanned += stack.len();
    let mut reachable = BTreeSet::new();
    let mut pending = stack
        .iter()
        .filter_map(|value| match value {
            RuntimeValue::Scalar => None,
            RuntimeValue::Ref(identity) => Some(*identity),
        })
        .collect::<Vec<_>>();
    while let Some(identity) = pending.pop() {
        if !reachable.insert(identity) {
            continue;
        }
        if let Some(object) = objects.get(&identity) {
            report.objects_traced += 1;
            report.edges_scanned += object.fields.len();
            for field in &object.fields {
                if let RuntimeValue::Ref(child) = field {
                    pending.push(*child);
                }
            }
        }
    }
    let before = objects.len();
    report.objects_swept += before;
    objects.retain(|identity, _| reachable.contains(identity));
    report.frees += before - objects.len();
}

pub fn simulate_scoped_regions(program: &[Instr]) -> Result<ScopedRegionReport, PlanError> {
    let mut depth = 0_usize;
    let mut retained = 0_usize;
    let mut report = ScopedRegionReport::default();
    for instruction in program {
        match instruction {
            Instr::Int(_) | Instr::InputRef { .. } => depth += 1,
            Instr::Pair => {
                if depth < 2 {
                    return Err(PlanError::StackUnderflow("pair"));
                }
                depth -= 1;
                retained += 1;
                report.allocations += 1;
                report.peak_retained = report.peak_retained.max(retained);
            }
            Instr::Dup => {
                if depth == 0 {
                    return Err(PlanError::StackUnderflow("dup"));
                }
                depth += 1;
            }
            Instr::Drop => {
                if depth == 0 {
                    return Err(PlanError::StackUnderflow("drop"));
                }
                depth -= 1;
            }
            Instr::Swap => {
                if depth < 2 {
                    return Err(PlanError::StackUnderflow("swap"));
                }
            }
            Instr::First | Instr::Second => {
                if depth == 0 {
                    let op = if instruction == &Instr::First {
                        "first"
                    } else {
                        "second"
                    };
                    return Err(PlanError::StackUnderflow(op));
                }
            }
        }
        if depth == 0 && retained > 0 {
            report.physical_slots = report.physical_slots.max(retained);
            report.resets += 1;
            retained = 0;
        }
    }
    report.physical_slots = report.physical_slots.max(retained);
    Ok(report)
}

fn compare_free_times(
    plan: &MemoryPlan,
    reference_counting: &ReferenceCountReport,
) -> Vec<FreeTimeMismatch> {
    let planned = plan
        .steps
        .iter()
        .enumerate()
        .flat_map(|(step, plan_step)| {
            plan_step
                .free
                .iter()
                .copied()
                .map(move |resource| (resource, step))
        })
        .collect::<BTreeMap<_, _>>();
    let resources = planned
        .keys()
        .chain(reference_counting.freed_at.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    resources
        .into_iter()
        .filter_map(|resource| {
            let planned = planned.get(&resource).copied();
            let reference_counted = reference_counting.freed_at.get(&resource).copied();
            (planned != reference_counted).then_some(FreeTimeMismatch {
                resource,
                planned,
                reference_counted,
            })
        })
        .collect()
}

/// Execute the abstract program while independently recomputing reachability
/// after every instruction.  This deliberately slow oracle validates safety
/// and promptness of the compile-time plan; the planner never calls it.
pub fn verify_plan(
    program: &[Instr],
    plan: &MemoryPlan,
    input_identities: &InputIdentities,
) -> Result<OracleReport, VerifyError> {
    if plan.steps.len() != program.len()
        || plan
            .steps
            .iter()
            .zip(program)
            .any(|(step, instruction)| &step.instruction != instruction)
    {
        return Err(VerifyError::PlanShape);
    }
    let contracts = validate_input_aliases(program, input_identities)?;
    let mut stack = Vec::<RuntimeValue>::new();
    let mut objects = BTreeMap::<u64, RuntimeObject>::new();
    let mut identities = BTreeMap::<Resource, u64>::new();
    let mut allocated = BTreeSet::<Resource>::new();
    let mut freed_ids = BTreeSet::<u64>::new();
    let mut next_local = 0_u32;
    let mut report = OracleReport::default();

    for (step_index, (instruction, step)) in program.iter().zip(&plan.steps).enumerate() {
        execute_runtime(
            instruction,
            step.allocate,
            &mut stack,
            &mut objects,
            &mut identities,
            &mut allocated,
            &mut next_local,
            input_identities,
            &freed_ids,
        )?;

        let reachable_ids = runtime_reachable(&stack, &objects);
        let reachable_managed = allocated
            .iter()
            .filter(|resource| {
                is_managed(**resource, &contracts)
                    && identities
                        .get(resource)
                        .is_some_and(|identity| reachable_ids.contains(identity))
            })
            .count();
        report.peak_reachable_managed = report.peak_reachable_managed.max(reachable_managed);

        for resource in allocated.iter().copied() {
            if !is_managed(resource, &contracts) || report.oracle_dead_at.contains_key(&resource) {
                continue;
            }
            let identity = identities
                .get(&resource)
                .ok_or(VerifyError::MissingResource(resource))?;
            if !reachable_ids.contains(identity) {
                report.oracle_dead_at.insert(resource, step_index);
            }
        }

        for resource in &step.free {
            let identity = identities
                .get(resource)
                .copied()
                .ok_or(VerifyError::MissingResource(*resource))?;
            if report
                .planned_free_at
                .insert(*resource, step_index)
                .is_some()
            {
                return Err(VerifyError::DoubleFree(*resource));
            }
            if reachable_ids.contains(&identity) {
                report.premature_frees.push((step_index, *resource));
            }
            freed_ids.insert(identity);
        }
    }

    report.missed_frees = report
        .oracle_dead_at
        .keys()
        .filter(|resource| !report.planned_free_at.contains_key(resource))
        .copied()
        .collect();
    report.max_reclamation_delay = report
        .planned_free_at
        .iter()
        .filter_map(|(resource, planned)| {
            report
                .oracle_dead_at
                .get(resource)
                .map(|dead| planned.saturating_sub(*dead))
        })
        .max()
        .unwrap_or(0);
    Ok(report)
}

#[allow(clippy::too_many_arguments)]
fn execute_runtime(
    instruction: &Instr,
    allocation: Option<Resource>,
    stack: &mut Vec<RuntimeValue>,
    objects: &mut BTreeMap<u64, RuntimeObject>,
    identities: &mut BTreeMap<Resource, u64>,
    allocated: &mut BTreeSet<Resource>,
    next_local: &mut u32,
    input_identities: &InputIdentities,
    freed: &BTreeSet<u64>,
) -> Result<(), VerifyError> {
    match instruction {
        Instr::Int(_) => stack.push(RuntimeValue::Scalar),
        Instr::InputRef { index, .. } => {
            let resource = Resource::Input(*index);
            let identity = input_identity(*index, input_identities);
            if freed.contains(&identity) {
                return Err(VerifyError::RuntimeUseAfterFree(identity));
            }
            identities.insert(resource, identity);
            allocated.insert(resource);
            stack.push(RuntimeValue::Ref(identity));
        }
        Instr::Pair => {
            let right = runtime_pop(stack, "pair")?;
            let left = runtime_pop(stack, "pair")?;
            runtime_ensure_live(&left, freed)?;
            runtime_ensure_live(&right, freed)?;
            let resource = allocation.ok_or(VerifyError::PlanShape)?;
            let Resource::Local(index) = resource else {
                return Err(VerifyError::PlanShape);
            };
            *next_local = (*next_local).max(index + 1);
            let identity = local_identity(resource);
            objects.insert(
                identity,
                RuntimeObject {
                    fields: vec![left, right],
                },
            );
            identities.insert(resource, identity);
            allocated.insert(resource);
            stack.push(RuntimeValue::Ref(identity));
        }
        Instr::Dup => {
            let value = stack
                .last()
                .cloned()
                .ok_or(PlanError::StackUnderflow("dup"))?;
            runtime_ensure_live(&value, freed)?;
            stack.push(value);
        }
        Instr::Drop => {
            let value = runtime_pop(stack, "drop")?;
            runtime_ensure_live(&value, freed)?;
        }
        Instr::Swap => {
            if stack.len() < 2 {
                return Err(PlanError::StackUnderflow("swap").into());
            }
            let len = stack.len();
            stack.swap(len - 1, len - 2);
        }
        Instr::First => runtime_project(stack, objects, freed, 0, "first")?,
        Instr::Second => runtime_project(stack, objects, freed, 1, "second")?,
    }
    Ok(())
}

fn runtime_pop(
    stack: &mut Vec<RuntimeValue>,
    op: &'static str,
) -> Result<RuntimeValue, VerifyError> {
    stack
        .pop()
        .ok_or_else(|| PlanError::StackUnderflow(op).into())
}

fn runtime_ensure_live(value: &RuntimeValue, freed: &BTreeSet<u64>) -> Result<(), VerifyError> {
    if let RuntimeValue::Ref(identity) = value
        && freed.contains(identity)
    {
        return Err(VerifyError::RuntimeUseAfterFree(*identity));
    }
    Ok(())
}

fn runtime_project(
    stack: &mut Vec<RuntimeValue>,
    objects: &BTreeMap<u64, RuntimeObject>,
    freed: &BTreeSet<u64>,
    field: usize,
    op: &'static str,
) -> Result<(), VerifyError> {
    let pair = runtime_pop(stack, op)?;
    runtime_ensure_live(&pair, freed)?;
    let RuntimeValue::Ref(identity) = pair else {
        return Err(PlanError::ExpectedReference(op).into());
    };
    let object = objects
        .get(&identity)
        .ok_or(PlanError::UnknownFields(Resource::Input(identity as u16)))?;
    let value = object
        .fields
        .get(field)
        .cloned()
        .ok_or(PlanError::ExpectedReference(op))?;
    runtime_ensure_live(&value, freed)?;
    stack.push(value);
    Ok(())
}

fn runtime_reachable(
    stack: &[RuntimeValue],
    objects: &BTreeMap<u64, RuntimeObject>,
) -> BTreeSet<u64> {
    let mut reachable = BTreeSet::new();
    let mut pending = stack
        .iter()
        .filter_map(|value| match value {
            RuntimeValue::Scalar => None,
            RuntimeValue::Ref(identity) => Some(*identity),
        })
        .collect::<Vec<_>>();
    while let Some(identity) = pending.pop() {
        if !reachable.insert(identity) {
            continue;
        }
        if let Some(object) = objects.get(&identity) {
            for field in &object.fields {
                if let RuntimeValue::Ref(child) = field {
                    pending.push(*child);
                }
            }
        }
    }
    reachable
}

fn validate_input_aliases(
    program: &[Instr],
    input_identities: &InputIdentities,
) -> Result<BTreeMap<u16, Ownership>, VerifyError> {
    let mut contracts = BTreeMap::<u16, Ownership>::new();
    for instruction in program {
        if let Instr::InputRef { index, ownership } = instruction
            && let Some(previous) = contracts.insert(*index, *ownership)
            && previous != *ownership
        {
            return Err(PlanError::ConflictingInputContract(*index).into());
        }
    }
    for (&index, &ownership) in &contracts {
        let identity = input_identity(index, input_identities);
        if identity & (1_u64 << 63) != 0 {
            return Err(VerifyError::InputIdentityOutOfRange { index, identity });
        }
        if ownership != Ownership::Unique {
            continue;
        }
        for &other in contracts.keys() {
            if other != index && input_identity(other, input_identities) == identity {
                return Err(VerifyError::UniqueAlias {
                    unique: index,
                    other,
                    identity,
                });
            }
        }
    }
    Ok(contracts)
}

fn is_managed(resource: Resource, contracts: &BTreeMap<u16, Ownership>) -> bool {
    match resource {
        Resource::Local(_) => true,
        Resource::Input(index) => contracts.get(&index) == Some(&Ownership::Unique),
    }
}

fn input_identity(index: u16, identities: &InputIdentities) -> u64 {
    identities.get(&index).copied().unwrap_or(u64::from(index))
}

fn local_identity(resource: Resource) -> u64 {
    let Resource::Local(index) = resource else {
        unreachable!();
    };
    (1_u64 << 63) | u64::from(index)
}

#[derive(Clone, Debug)]
enum RcValue {
    Scalar,
    Ref { identity: u64, counted: bool },
}

#[derive(Clone, Debug)]
struct RcObject {
    fields: Vec<RcValue>,
    count: usize,
    local: bool,
    /// Shared inputs retain an external reference and are deliberately not an
    /// exact-free oracle target.  Locals and unique inputs have one resource.
    resource: Option<Resource>,
}

pub fn simulate_reference_counting(
    program: &[Instr],
    input_identities: &InputIdentities,
) -> Result<ReferenceCountReport, VerifyError> {
    let contracts = validate_input_aliases(program, input_identities)?;
    let mut stack = Vec::<RcValue>::new();
    let mut objects = BTreeMap::<u64, RcObject>::new();
    let mut next_local = 0_u32;
    let mut seen_inputs = BTreeSet::<u16>::new();
    let mut report = ReferenceCountReport::default();

    for (step_index, instruction) in program.iter().enumerate() {
        match instruction {
            Instr::Int(_) => stack.push(RcValue::Scalar),
            Instr::InputRef { index, ownership } => {
                let identity = input_identity(*index, input_identities);
                let counted = *ownership != Ownership::Borrowed;
                if counted {
                    let initial = if *ownership == Ownership::Shared {
                        1
                    } else {
                        0
                    };
                    let object = objects.entry(identity).or_insert(RcObject {
                        fields: Vec::new(),
                        count: initial,
                        local: false,
                        resource: (*ownership == Ownership::Unique)
                            .then_some(Resource::Input(*index)),
                    });
                    if seen_inputs.insert(*index) && *ownership == Ownership::Unique {
                        object.count = 1;
                    } else {
                        object.count += 1;
                        report.increments += 1;
                    }
                }
                stack.push(RcValue::Ref { identity, counted });
            }
            Instr::Pair => {
                let right = rc_pop(&mut stack, "pair")?;
                let left = rc_pop(&mut stack, "pair")?;
                let resource = Resource::Local(next_local);
                next_local += 1;
                let identity = local_identity(resource);
                objects.insert(
                    identity,
                    RcObject {
                        fields: vec![left, right],
                        count: 1,
                        local: true,
                        resource: Some(resource),
                    },
                );
                stack.push(RcValue::Ref {
                    identity,
                    counted: true,
                });
            }
            Instr::Dup => {
                let value = stack
                    .last()
                    .cloned()
                    .ok_or(PlanError::StackUnderflow("dup"))?;
                rc_retain(&value, &mut objects, &mut report)?;
                stack.push(value);
            }
            Instr::Drop => {
                let value = rc_pop(&mut stack, "drop")?;
                rc_release(value, step_index, &mut objects, &mut report)?;
            }
            Instr::Swap => {
                if stack.len() < 2 {
                    return Err(PlanError::StackUnderflow("swap").into());
                }
                let len = stack.len();
                stack.swap(len - 1, len - 2);
            }
            Instr::First | Instr::Second => {
                let field = usize::from(*instruction == Instr::Second);
                let op = if field == 0 { "first" } else { "second" };
                let pair = rc_pop(&mut stack, op)?;
                let RcValue::Ref { identity, .. } = pair else {
                    return Err(PlanError::ExpectedReference(op).into());
                };
                let selected = objects
                    .get(&identity)
                    .and_then(|object| object.fields.get(field))
                    .cloned()
                    .ok_or(PlanError::UnknownFields(Resource::Input(identity as u16)))?;
                rc_retain(&selected, &mut objects, &mut report)?;
                rc_release(pair, step_index, &mut objects, &mut report)?;
                stack.push(selected);
            }
        }
        let live_locals = objects
            .values()
            .filter(|object| object.local && object.count > 0)
            .count();
        report.peak_live_locals = report.peak_live_locals.max(live_locals);
    }

    let _ = contracts;
    Ok(report)
}

fn rc_pop(stack: &mut Vec<RcValue>, op: &'static str) -> Result<RcValue, VerifyError> {
    stack
        .pop()
        .ok_or_else(|| PlanError::StackUnderflow(op).into())
}

fn rc_retain(
    value: &RcValue,
    objects: &mut BTreeMap<u64, RcObject>,
    report: &mut ReferenceCountReport,
) -> Result<(), VerifyError> {
    if let RcValue::Ref {
        identity,
        counted: true,
    } = value
    {
        let object = objects
            .get_mut(identity)
            .ok_or(VerifyError::RuntimeUseAfterFree(*identity))?;
        if object.count == 0 {
            return Err(VerifyError::RuntimeUseAfterFree(*identity));
        }
        object.count += 1;
        report.increments += 1;
    }
    Ok(())
}

fn rc_release(
    value: RcValue,
    step_index: usize,
    objects: &mut BTreeMap<u64, RcObject>,
    report: &mut ReferenceCountReport,
) -> Result<(), VerifyError> {
    let RcValue::Ref {
        identity,
        counted: true,
    } = value
    else {
        return Ok(());
    };
    report.decrements += 1;
    report.zero_tests += 1;
    let (fields, resource) = {
        let object = objects
            .get_mut(&identity)
            .ok_or(VerifyError::RuntimeUseAfterFree(identity))?;
        if object.count == 0 {
            return Err(VerifyError::CounterUnderflow(identity));
        }
        object.count -= 1;
        if object.count == 0 {
            (Some(object.fields.clone()), object.resource)
        } else {
            (None, None)
        }
    };
    if let Some(fields) = fields {
        report.frees += 1;
        if let Some(resource) = resource {
            report.freed_at.insert(resource, step_index);
        }
        for field in fields {
            rc_release(field, step_index, objects, report)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_extraction_frees_only_the_dead_subgraph() {
        let program = vec![
            Instr::Int(1),
            Instr::Int(2),
            Instr::Pair, // local 0
            Instr::Int(3),
            Instr::Int(4),
            Instr::Pair, // local 1
            Instr::Pair, // local 2 = (local 0, local 1)
            Instr::First,
            Instr::Drop,
        ];
        let plan = plan(&program).unwrap();
        assert_eq!(
            plan.steps[7].free,
            vec![Resource::Local(1), Resource::Local(2)]
        );
        assert_eq!(plan.steps[8].free, vec![Resource::Local(0)]);
        assert_eq!(plan.metrics.local_allocations, 3);
        assert_eq!(plan.metrics.exact_frees, 3);
        assert_eq!(plan.metrics.runtime_liveness_checks, 0);
    }

    #[test]
    fn explicit_duplication_is_known_aliasing_and_frees_once() {
        let program = vec![
            Instr::Int(1),
            Instr::Int(2),
            Instr::Pair,
            Instr::Dup,
            Instr::Drop,
            Instr::Drop,
        ];
        let plan = plan(&program).unwrap();
        assert!(plan.steps[4].free.is_empty());
        assert_eq!(plan.steps[5].free, vec![Resource::Local(0)]);
    }

    #[test]
    fn input_contract_exposes_the_static_boundary() {
        let program = vec![
            Instr::InputRef {
                index: 0,
                ownership: Ownership::Borrowed,
            },
            Instr::Drop,
            Instr::InputRef {
                index: 1,
                ownership: Ownership::Unique,
            },
            Instr::Drop,
            Instr::InputRef {
                index: 2,
                ownership: Ownership::Shared,
            },
            Instr::Drop,
        ];
        let plan = plan(&program).unwrap();
        assert!(plan.steps[1].free.is_empty());
        assert_eq!(plan.steps[3].free, vec![Resource::Input(1)]);
        assert!(plan.steps[5].free.is_empty());
        assert_eq!(
            plan.steps[5].fallback,
            vec![FallbackOp::ReleaseShared(Resource::Input(2))]
        );
        assert_eq!(plan.metrics.unique_inputs, 1);
        assert_eq!(plan.metrics.fallback_inputs, 1);
        assert_eq!(plan.metrics.fallback_operations, 1);
        assert_eq!(plan.metrics.runtime_liveness_checks, 1);
    }

    #[test]
    fn shared_child_emits_a_residual_release_when_container_dies() {
        let program = vec![
            Instr::InputRef {
                index: 0,
                ownership: Ownership::Shared,
            },
            Instr::Int(1),
            Instr::Pair,
            Instr::Drop,
        ];
        let plan = plan(&program).unwrap();
        assert_eq!(plan.steps[3].free, vec![Resource::Local(0)]);
        assert_eq!(
            plan.steps[3].fallback,
            vec![FallbackOp::ReleaseShared(Resource::Input(0))]
        );
        assert_eq!(plan.metrics.runtime_liveness_checks, 1);
    }

    #[test]
    fn a_shared_child_is_freed_once_before_its_parent() {
        let program = vec![
            Instr::Int(1),
            Instr::Int(2),
            Instr::Pair, // local 0
            Instr::Dup,
            Instr::Pair, // local 1 contains local 0 twice
            Instr::Drop,
        ];
        let plan = plan(&program).unwrap();
        assert_eq!(
            plan.steps[5].free,
            vec![Resource::Local(0), Resource::Local(1)]
        );
    }

    #[test]
    fn reachability_oracle_confirms_exact_and_prompt_reclamation() {
        let program = vec![
            Instr::Int(1),
            Instr::Int(2),
            Instr::Pair,
            Instr::Dup,
            Instr::First,
            Instr::Drop,
            Instr::Drop,
        ];
        let comparison = compare(&program, &InputIdentities::new()).unwrap();
        assert!(comparison.oracle.premature_frees.is_empty());
        assert!(comparison.oracle.missed_frees.is_empty());
        assert_eq!(comparison.oracle.max_reclamation_delay, 0);
        assert!(comparison.rc_oracle_mismatches.is_empty());
        assert_eq!(comparison.plan.metrics.exact_frees, 1);
    }

    #[test]
    fn runtime_rejects_a_violated_unique_input_contract() {
        let program = vec![
            Instr::InputRef {
                index: 0,
                ownership: Ownership::Unique,
            },
            Instr::InputRef {
                index: 1,
                ownership: Ownership::Borrowed,
            },
            Instr::Drop,
            Instr::Drop,
        ];
        let identities = BTreeMap::from([(0, 7), (1, 7)]);
        assert!(matches!(
            compare(&program, &identities),
            Err(VerifyError::UniqueAlias {
                unique: 0,
                other: 1,
                identity: 7,
            })
        ));
    }

    #[test]
    fn runtime_input_identity_cannot_collide_with_local_namespace() {
        let program = vec![
            Instr::InputRef {
                index: 0,
                ownership: Ownership::Borrowed,
            },
            Instr::Drop,
        ];
        let identity = (1_u64 << 63) | 7;
        assert_eq!(
            compare(&program, &BTreeMap::from([(0, identity)])),
            Err(VerifyError::InputIdentityOutOfRange { index: 0, identity })
        );
    }

    #[test]
    fn planner_rejects_reintroducing_a_consumed_unique_input() {
        let program = vec![
            Instr::InputRef {
                index: 0,
                ownership: Ownership::Unique,
            },
            Instr::Drop,
            Instr::InputRef {
                index: 0,
                ownership: Ownership::Unique,
            },
        ];
        assert_eq!(
            plan(&program),
            Err(PlanError::UseAfterFree(Resource::Input(0)))
        );
    }

    #[test]
    fn static_slots_reuse_storage_without_arena_retention_or_rc_checks() {
        let mut program = Vec::new();
        for index in 0..100 {
            program.extend([
                Instr::Int(index),
                Instr::Int(index + 1),
                Instr::Pair,
                Instr::Drop,
            ]);
        }
        let comparison = compare(&program, &InputIdentities::new()).unwrap();
        assert_eq!(comparison.plan.metrics.local_allocations, 100);
        assert_eq!(comparison.plan.metrics.physical_slots, 1);
        assert_eq!(comparison.plan.metrics.reused_allocations, 99);
        assert_eq!(comparison.frame_arena_slots, 100);
        assert_eq!(comparison.scoped_regions.physical_slots, 1);
        assert_eq!(comparison.scoped_regions.resets, 100);
        assert_eq!(comparison.tracing.collection_interval, 32);
        assert_eq!(comparison.tracing.peak_resident_locals, 33);
        assert_eq!(comparison.tracing.collections, 4);
        assert_eq!(comparison.tracing.frees, 100);
        assert_eq!(comparison.reference_counting.decrements, 100);
        assert_eq!(comparison.reference_counting.zero_tests, 100);
        assert_eq!(comparison.reference_counting.frees, 100);
        assert!(comparison.oracle.premature_frees.is_empty());
        assert!(comparison.oracle.missed_frees.is_empty());
        assert!(comparison.rc_oracle_mismatches.is_empty());
    }

    #[test]
    fn long_lived_anchor_exposes_scope_region_retention() {
        let mut program = vec![Instr::Int(-1), Instr::Int(-2), Instr::Pair];
        for index in 0..100 {
            program.extend([
                Instr::Int(index),
                Instr::Int(index + 1),
                Instr::Pair,
                Instr::Drop,
            ]);
        }
        program.push(Instr::Drop);

        let comparison = compare(&program, &InputIdentities::new()).unwrap();
        assert_eq!(comparison.plan.metrics.local_allocations, 101);
        assert_eq!(comparison.plan.metrics.physical_slots, 2);
        assert_eq!(comparison.plan.metrics.reused_allocations, 99);
        assert_eq!(comparison.scoped_regions.physical_slots, 101);
        assert_eq!(comparison.scoped_regions.peak_retained, 101);
        assert_eq!(comparison.scoped_regions.resets, 1);
        assert_eq!(comparison.frame_arena_slots, 101);
        assert_eq!(comparison.tracing.peak_resident_locals, 34);
        assert_eq!(comparison.tracing.frees, 101);
        assert!(comparison.tracing.objects_traced >= 3);
        assert!(comparison.rc_oracle_mismatches.is_empty());
    }

    #[derive(Clone, Debug)]
    enum TestType {
        Scalar,
        Pair(Box<TestType>, Box<TestType>),
    }

    #[test]
    fn exhaustive_small_well_typed_programs_match_the_oracle() {
        let mut checked = 0_usize;
        enumerate_programs(&mut Vec::new(), &[], 6, &mut checked);
        assert!(checked > 500, "only checked {checked} programs");
    }

    #[test]
    fn input_ownership_and_top_level_alias_partitions_are_exercised() {
        let ownerships = [Ownership::Borrowed, Ownership::Unique, Ownership::Shared];
        let tails = [
            vec![Instr::Drop],
            vec![Instr::Dup, Instr::First, Instr::Drop, Instr::Drop],
        ];
        for left in ownerships {
            for right in ownerships {
                for tail in &tails {
                    let mut program = vec![
                        Instr::InputRef {
                            index: 0,
                            ownership: left,
                        },
                        Instr::InputRef {
                            index: 1,
                            ownership: right,
                        },
                        Instr::Pair,
                    ];
                    program.extend(tail.clone());
                    for identities in [
                        BTreeMap::from([(0, 10), (1, 11)]),
                        BTreeMap::from([(0, 10), (1, 10)]),
                    ] {
                        let aliased = identities[&0] == identities[&1];
                        let violates_unique =
                            aliased && (left == Ownership::Unique || right == Ownership::Unique);
                        let result = compare(&program, &identities);
                        if violates_unique {
                            assert!(matches!(result, Err(VerifyError::UniqueAlias { .. })));
                        } else {
                            let result = result.unwrap();
                            assert!(result.oracle.premature_frees.is_empty());
                            assert!(result.oracle.missed_frees.is_empty());
                            assert!(result.rc_oracle_mismatches.is_empty());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn randomized_deep_alias_shapes_match_two_lifetime_oracles() {
        let mut random = 0x4d41_5243_4836_cafe_u64;
        for case in 0..2_048 {
            let mut program = Vec::new();
            let mut stack = Vec::<TestType>::new();
            for _ in 0..64 {
                let choices = typed_choices(&stack, next_random(&mut random) as i64, 10);
                let choice = (next_random(&mut random) as usize) % choices.len();
                let (instruction, next_stack) = choices[choice].clone();
                program.push(instruction);
                stack = next_stack;
            }
            let comparison = compare(&program, &InputIdentities::new()).unwrap();
            assert!(
                comparison.oracle.premature_frees.is_empty(),
                "case {case}: {program:?}"
            );
            assert!(
                comparison.oracle.missed_frees.is_empty(),
                "case {case}: {program:?}"
            );
            assert_eq!(
                comparison.oracle.max_reclamation_delay, 0,
                "case {case}: {program:?}"
            );
            assert!(
                comparison.rc_oracle_mismatches.is_empty(),
                "case {case}: {:?}: {program:?}",
                comparison.rc_oracle_mismatches
            );
        }
    }

    fn next_random(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state
    }

    fn enumerate_programs(
        program: &mut Vec<Instr>,
        stack: &[TestType],
        remaining: usize,
        checked: &mut usize,
    ) {
        if !program.is_empty() {
            let plan = plan(program).unwrap();
            let oracle = verify_plan(program, &plan, &InputIdentities::new()).unwrap();
            let reference_counting =
                simulate_reference_counting(program, &InputIdentities::new()).unwrap();
            let rc_mismatches = compare_free_times(&plan, &reference_counting);
            assert!(oracle.premature_frees.is_empty(), "program: {program:?}");
            assert!(oracle.missed_frees.is_empty(), "program: {program:?}");
            assert_eq!(oracle.max_reclamation_delay, 0, "program: {program:?}");
            assert!(
                rc_mismatches.is_empty(),
                "mismatches: {rc_mismatches:?}; program: {program:?}"
            );
            *checked += 1;
        }
        if remaining == 0 {
            return;
        }

        for (instruction, next_stack) in typed_choices(stack, 0, 4) {
            program.push(instruction);
            enumerate_programs(program, &next_stack, remaining - 1, checked);
            program.pop();
        }
    }

    fn typed_choices(
        stack: &[TestType],
        scalar: i64,
        max_stack: usize,
    ) -> Vec<(Instr, Vec<TestType>)> {
        let mut choices = Vec::<(Instr, Vec<TestType>)>::new();
        if stack.len() < max_stack {
            let mut next = stack.to_vec();
            next.push(TestType::Scalar);
            choices.push((Instr::Int(scalar), next));
        }
        if let Some(top) = stack.last() {
            if stack.len() < max_stack {
                let mut next = stack.to_vec();
                next.push(top.clone());
                choices.push((Instr::Dup, next));
            }
            let mut next = stack.to_vec();
            next.pop();
            choices.push((Instr::Drop, next));
            if let TestType::Pair(left, right) = top {
                let mut first = stack.to_vec();
                first.pop();
                first.push((**left).clone());
                choices.push((Instr::First, first));
                let mut second = stack.to_vec();
                second.pop();
                second.push((**right).clone());
                choices.push((Instr::Second, second));
            }
        }
        if stack.len() >= 2 {
            let mut swapped = stack.to_vec();
            let len = swapped.len();
            swapped.swap(len - 1, len - 2);
            choices.push((Instr::Swap, swapped));

            let mut paired = stack.to_vec();
            let right = paired.pop().unwrap();
            let left = paired.pop().unwrap();
            paired.push(TestType::Pair(Box::new(left), Box::new(right)));
            choices.push((Instr::Pair, paired));
        }
        choices
    }
}
