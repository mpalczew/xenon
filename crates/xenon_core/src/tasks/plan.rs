//! dependsOn orchestration as a pure state machine. The app launches terminals
//! for `Action::Launch` and reports back; nothing here spawns or waits.

use std::collections::BTreeMap;

use super::model::TaskSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Launch(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Succeeded,
    Failed { label: String, reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Waiting,
    Running,
    /// Finished with success, or a background task that reported ready.
    Satisfied,
}

#[derive(Debug)]
struct Node {
    /// Everything that must be satisfied before this task launches.
    needs: Vec<String>,
    state: State,
}

#[derive(Debug)]
pub struct Orchestrator {
    root: String,
    nodes: BTreeMap<String, Node>,
    failed: Option<(String, String)>,
}

impl Orchestrator {
    /// Plan `root` and its dependency closure. Errors on a missing, unsupported
    /// or cyclic dependency, before anything runs.
    pub fn new(set: &TaskSet, root: &str) -> Result<Self, String> {
        let mut nodes = BTreeMap::new();
        let mut stack = vec![root.to_string()];
        while let Some(label) = stack.pop() {
            if nodes.contains_key(&label) {
                continue;
            }
            let def = set
                .get(&label)
                .ok_or_else(|| format!("no task named {label:?}"))?;
            if let Some(reason) = &def.unsupported {
                return Err(format!("{label}: {reason}"));
            }
            let mut needs = def.depends_on.clone();
            needs.dedup();
            stack.extend(def.depends_on.iter().cloned());
            nodes.insert(
                label,
                Node {
                    needs,
                    state: State::Waiting,
                },
            );
        }
        let planned: Vec<String> = nodes.keys().cloned().collect();
        // Sequence ordering adds an edge from each dependency to its predecessor.
        for def in &set.tasks {
            if def.depends_sequence && planned.contains(&def.label) {
                for pair in def.depends_on.windows(2) {
                    if let Some(node) = nodes.get_mut(&pair[1])
                        && !node.needs.contains(&pair[0])
                    {
                        node.needs.push(pair[0].clone());
                    }
                }
            }
        }
        let this = Self {
            root: root.to_string(),
            nodes,
            failed: None,
        };
        this.check_cycles()?;
        Ok(this)
    }

    fn check_cycles(&self) -> Result<(), String> {
        // Kahn: repeatedly remove nodes with no remaining needs.
        let mut remaining: BTreeMap<&str, Vec<&str>> = self
            .nodes
            .iter()
            .map(|(k, n)| (k.as_str(), n.needs.iter().map(String::as_str).collect()))
            .collect();
        loop {
            let ready: Vec<&str> = remaining
                .iter()
                .filter(|(_, needs)| needs.is_empty())
                .map(|(k, _)| *k)
                .collect();
            if ready.is_empty() {
                break;
            }
            for r in &ready {
                remaining.remove(r);
            }
            for needs in remaining.values_mut() {
                needs.retain(|n| !ready.contains(n));
            }
        }
        match remaining.keys().next() {
            None => Ok(()),
            Some(label) => Err(format!("dependsOn cycle involving {label:?}")),
        }
    }

    /// Labels in the plan.
    pub fn labels(&self) -> Vec<String> {
        self.nodes.keys().cloned().collect()
    }

    /// Launch whatever has no unmet needs.
    pub fn start(&mut self) -> Vec<Action> {
        self.advance()
    }

    /// A task finished successfully, or a background task reported ready.
    pub fn satisfied(&mut self, label: &str) -> Vec<Action> {
        if let Some(node) = self.nodes.get_mut(label) {
            node.state = State::Satisfied;
        }
        self.advance()
    }

    /// A task failed. The chain stops: nothing further launches.
    pub fn failed(&mut self, label: &str, reason: &str) -> Vec<Action> {
        if self.failed.is_none() {
            self.failed = Some((label.to_string(), reason.to_string()));
        }
        Vec::new()
    }

    /// `Some` once the run is over: root satisfied, or a task failed.
    pub fn outcome(&self) -> Option<Outcome> {
        if let Some((label, reason)) = &self.failed {
            return Some(Outcome::Failed {
                label: label.clone(),
                reason: reason.clone(),
            });
        }
        self.nodes
            .get(&self.root)
            .is_some_and(|n| n.state == State::Satisfied)
            .then_some(Outcome::Succeeded)
    }

    fn advance(&mut self) -> Vec<Action> {
        if self.failed.is_some() {
            return Vec::new();
        }
        let satisfied: Vec<String> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.state == State::Satisfied)
            .map(|(k, _)| k.clone())
            .collect();
        let mut actions = Vec::new();
        for (label, node) in self.nodes.iter_mut() {
            if node.state == State::Waiting && node.needs.iter().all(|n| satisfied.contains(n)) {
                node.state = State::Running;
                actions.push(Action::Launch(label.clone()));
            }
        }
        actions
    }
}
