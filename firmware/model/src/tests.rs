use super::*;
use alloc::string::ToString;
use alloc::vec::Vec;

fn art(id: &str, version: &str) -> ArtifactDescriptor {
    ArtifactDescriptor { id: id.to_string(), version: version.to_string(), digest: [7; 32], size: 1024 }
}

fn agent_req(version: &str, provides: RuntimeApi) -> UpdateRequest {
    UpdateRequest::agent(art("embewi-agent", version), provides)
}

fn workload_req(version: &str, requires: RuntimeApi) -> UpdateRequest {
    UpdateRequest::workload(art("pod", version), requires)
}

fn installed_agent(version: &str, api: RuntimeApi) -> InstalledAgent {
    InstalledAgent { artifact: art("embewi-agent", version), provides: api }
}

fn installed_workload(version: &str, api: RuntimeApi) -> InstalledWorkload {
    InstalledWorkload { artifact: art("pod", version), requires: api }
}

const API3: RuntimeApi = RuntimeApi::new(1, 3);
const API4: RuntimeApi = RuntimeApi::new(1, 4);

#[derive(Default)]
struct Boot(Vec<&'static str>);
impl BootAuthority for Boot {
    fn schedule_boot(&mut self, _side: Side) {
        self.0.push("boot:schedule");
    }
    fn restore_boot(&mut self, _side: Side) {
        self.0.push("boot:restore");
    }
}

#[derive(Default)]
struct Supervisor(Vec<&'static str>);
impl WorkloadSupervisor for Supervisor {
    fn switch_to(&mut self, _side: Side) {
        self.0.push("supervisor:switch");
    }
    fn restore(&mut self, _side: Side) {
        self.0.push("supervisor:restore");
    }
}

struct System {
    agent: AgentOta,
    workload: WorkloadOta,
    boot: Boot,
    supervisor: Supervisor,
}

fn system() -> System {
    System {
        agent: AgentOta::new(AbSlots::with_active(installed_agent("a1", API4))),
        workload: WorkloadOta::new(AbSlots::with_active(installed_workload("w1", API3))),
        boot: Boot::default(),
        supervisor: Supervisor::default(),
    }
}

#[test]
fn runtime_api_rule_is_same_major_and_at_least_the_required_minor() {
    assert!(API4.satisfies(API3));
    assert!(API4.satisfies(API4));
    assert!(!API3.satisfies(API4));
    assert!(!RuntimeApi::new(2, 0).satisfies(RuntimeApi::new(1, 0)));
    assert!(!RuntimeApi::new(1, 9).satisfies(RuntimeApi::new(2, 0)));
}

#[test]
fn a_request_cannot_disagree_with_its_own_target_and_names_no_slot() {
    let a = agent_req("a2", API4);
    let w = workload_req("w2", API3);
    assert_eq!(a.target(), UpdateTarget::Agent);
    assert_eq!(w.target(), UpdateTarget::Workload);
    // The agent policy refuses a workload order and vice versa.
    let mut s = system();
    assert_eq!(s.agent.stage(&w), Err(Refusal::WrongTarget));
    assert_eq!(s.workload.stage(&a), Err(Refusal::WrongTarget));
}

#[test]
fn agent_update_does_not_alter_workload_selection() {
    let mut s = system();
    let before = s.workload.slots().clone();
    s.agent.stage(&agent_req("a2", API4)).unwrap();
    assert_eq!(s.agent.activate(&mut s.boot), Ok(Activation::RebootRequired));
    assert_eq!(s.agent.confirm(&mut s.boot, true, s.workload.active()), Confirmation::Confirmed);
    assert_eq!(*s.workload.slots(), before);
    assert!(s.supervisor.0.is_empty());
}

#[test]
fn workload_update_does_not_alter_agent_selection() {
    let mut s = system();
    let before = s.agent.slots().clone();
    s.workload.stage(&workload_req("w2", API3)).unwrap();
    let api = s.agent.provided_api().unwrap();
    assert_eq!(s.workload.activate(&mut s.supervisor, api), Ok(Activation::Switched));
    assert_eq!(s.workload.confirm(&mut s.supervisor, true), Confirmation::Confirmed);
    assert_eq!(*s.agent.slots(), before);
    assert!(s.boot.0.is_empty(), "no Agent reboot / boot change for a Workload update");
}

#[test]
fn incompatible_workload_is_rejected_before_it_becomes_active() {
    let mut s = system();
    s.workload.stage(&workload_req("w-new", RuntimeApi::new(1, 9))).unwrap(); // needs 1.9, Agent has 1.4
    let api = s.agent.provided_api().unwrap();
    assert_eq!(s.workload.activate(&mut s.supervisor, api), Err(Refusal::IncompatibleRuntimeApi));
    assert!(s.supervisor.0.is_empty(), "the supervisor must never be called");
    assert_eq!(s.workload.active().unwrap().artifact.version, "w1");
}

#[test]
fn agent_rollback_preserves_workload() {
    let mut s = system();
    let before = s.workload.slots().clone();
    s.agent.stage(&agent_req("a2", API4)).unwrap();
    s.agent.activate(&mut s.boot).unwrap();
    assert_eq!(
        s.agent.confirm(&mut s.boot, false, s.workload.active()),
        Confirmation::RolledBack(Reason::Unhealthy)
    );
    assert_eq!(s.agent.slots().active().unwrap().artifact.version, "a1");
    assert_eq!(*s.workload.slots(), before);
    assert_eq!(s.boot.0, ["boot:schedule", "boot:restore"]);
}

#[test]
fn workload_rollback_preserves_agent() {
    let mut s = system();
    let before = s.agent.slots().clone();
    s.workload.stage(&workload_req("w2", API3)).unwrap();
    let api = s.agent.provided_api().unwrap();
    s.workload.activate(&mut s.supervisor, api).unwrap();
    assert_eq!(s.workload.confirm(&mut s.supervisor, false), Confirmation::RolledBack(Reason::Unhealthy));
    assert_eq!(s.workload.active().unwrap().artifact.version, "w1");
    assert_eq!(*s.agent.slots(), before);
    assert_eq!(s.supervisor.0, ["supervisor:switch", "supervisor:restore"]);
    assert!(s.boot.0.is_empty());
}

#[test]
fn a_new_agent_that_cannot_run_the_active_workload_is_not_confirmed() {
    let mut s = system();
    // Active Workload needs 1.3; the candidate Agent only provides 1.2.
    s.agent.stage(&agent_req("a-old-api", RuntimeApi::new(1, 2))).unwrap();
    s.agent.activate(&mut s.boot).unwrap();
    assert_eq!(
        s.agent.confirm(&mut s.boot, true, s.workload.active()),
        Confirmation::RolledBack(Reason::IncompatibleWorkload)
    );
    assert_eq!(s.agent.slots().active().unwrap().artifact.version, "a1");
}

#[test]
fn activation_authorities_cannot_be_crossed() {
    let mut s = system();
    s.agent.stage(&agent_req("a2", API4)).unwrap();
    s.agent.activate(&mut s.boot).unwrap();
    assert_eq!(s.boot.0, ["boot:schedule"]);
    assert!(s.supervisor.0.is_empty());

    let mut s = system();
    s.workload.stage(&workload_req("w2", API3)).unwrap();
    s.workload.activate(&mut s.supervisor, API4).unwrap();
    assert_eq!(s.supervisor.0, ["supervisor:switch"]);
    assert!(s.boot.0.is_empty());
}

#[test]
fn a_pending_candidate_cannot_be_superseded_and_nothing_staged_cannot_activate() {
    let mut s = system();
    assert_eq!(s.agent.activate(&mut s.boot), Err(Refusal::NothingStaged));
    s.agent.stage(&agent_req("a2", API4)).unwrap();
    s.agent.activate(&mut s.boot).unwrap();
    assert_eq!(s.agent.stage(&agent_req("a3", API4)), Err(Refusal::Busy));
    s.agent.confirm(&mut s.boot, true, None);
    assert!(s.agent.stage(&agent_req("a3", API4)).is_ok());
}

#[test]
fn either_level_can_change_in_either_order() {
    // Agent first, then Workload.
    let mut s = system();
    s.agent.stage(&agent_req("a2", RuntimeApi::new(1, 5))).unwrap();
    s.agent.activate(&mut s.boot).unwrap();
    s.agent.confirm(&mut s.boot, true, s.workload.active());
    s.workload.stage(&workload_req("w2", RuntimeApi::new(1, 5))).unwrap();
    let api = s.agent.provided_api().unwrap();
    assert_eq!(s.workload.activate(&mut s.supervisor, api), Ok(Activation::Switched));
    // Workload first (needs 1.3, Agent 1.4), then Agent.
    let mut s = system();
    s.workload.stage(&workload_req("w2", API3)).unwrap();
    s.workload.activate(&mut s.supervisor, API4).unwrap();
    s.workload.confirm(&mut s.supervisor, true);
    s.agent.stage(&agent_req("a2", API4)).unwrap();
    s.agent.activate(&mut s.boot).unwrap();
    assert_eq!(s.agent.confirm(&mut s.boot, true, s.workload.active()), Confirmation::Confirmed);
}
