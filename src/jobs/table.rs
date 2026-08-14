//! Bash-observable job identity and completion state.
//!
//! Windows process handles and waiting primitives stay in the executor/backend.

use std::collections::{BTreeMap, HashMap};

pub type JobId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    Stopped,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobEntry {
    pub id: JobId,
    pub pids: Vec<u32>,
    pub command: String,
    pub state: ProcessState,
    pub exit_status: Option<i32>,
    pub background: bool,
    pub foreground: bool,
    pub coproc_endpoints: Vec<u32>,
}

#[derive(Debug, Default, Clone)]
pub struct JobTable {
    pub jobs: BTreeMap<JobId, JobEntry>,
    pub pid_to_job: HashMap<u32, JobId>,
    pub completed_statuses: HashMap<u32, i32>,
    next_job_id: JobId,
}

impl JobTable {
    pub fn register_process(
        &mut self,
        pid: u32,
        command: impl Into<String>,
        background: bool,
    ) -> JobId {
        self.register_pipeline(vec![pid], command, background)
    }

    pub fn register_pipeline(
        &mut self,
        pids: Vec<u32>,
        command: impl Into<String>,
        background: bool,
    ) -> JobId {
        let id = self.next_job_id.max(1);
        self.next_job_id = id.saturating_add(1);
        let entry = JobEntry {
            id,
            pids: pids.clone(),
            command: command.into(),
            state: ProcessState::Running,
            exit_status: None,
            background,
            foreground: !background,
            coproc_endpoints: Vec::new(),
        };
        for pid in pids {
            self.pid_to_job.insert(pid, id);
        }
        self.jobs.insert(id, entry);
        id
    }

    pub fn resolve_jobspec(&self, spec: &str) -> Option<JobId> {
        if spec == "%" || spec == "%%" || spec == "%+" {
            return self.jobs.keys().next_back().copied();
        }
        if spec == "%-" {
            return self.jobs.keys().rev().nth(1).copied();
        }
        spec.strip_prefix('%')
            .and_then(|id| id.parse::<JobId>().ok())
            .filter(|id| self.jobs.contains_key(id))
    }

    pub fn mark_completed(&mut self, pid: u32, status: i32) {
        self.completed_statuses.insert(pid, status);
        let Some(job_id) = self.pid_to_job.get(&pid).copied() else {
            return;
        };
        let Some(job) = self.jobs.get_mut(&job_id) else {
            return;
        };
        job.state = ProcessState::Completed;
        job.exit_status = Some(status);
    }

    pub fn reap_finished<I>(&mut self, statuses: I)
    where
        I: IntoIterator<Item = (u32, i32)>,
    {
        for (pid, status) in statuses {
            self.mark_completed(pid, status);
        }
    }

    pub fn wait_pid(&mut self, pid: u32) -> Option<i32> {
        self.completed_statuses.remove(&pid).or_else(|| {
            let job_id = self.pid_to_job.get(&pid).copied()?;
            self.jobs.get(&job_id)?.exit_status
        })
    }

    pub fn wait_any(&mut self) -> Option<(u32, i32)> {
        let pid = self.completed_statuses.keys().next().copied()?;
        let status = self.completed_statuses.remove(&pid)?;
        Some((pid, status))
    }

    pub fn wait_all(&mut self) -> Vec<(u32, i32)> {
        let mut result: Vec<_> = self.completed_statuses.drain().collect();
        result.sort_by_key(|(pid, _)| *pid);
        result
    }

    pub fn attach_coproc_endpoint(&mut self, job_id: JobId, fd: u32) {
        if let Some(job) = self.jobs.get_mut(&job_id) {
            job.coproc_endpoints.push(fd);
        }
    }

    pub fn take_coproc_endpoint(&mut self, job_id: JobId, fd: u32) -> bool {
        self.jobs.get_mut(&job_id).map_or(false, |job| {
            let before = job.coproc_endpoints.len();
            job.coproc_endpoints.retain(|candidate| *candidate != fd);
            before != job.coproc_endpoints.len()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_can_be_waited_explicitly_after_reaping() {
        let mut table = JobTable::default();
        table.register_process(42, "sleep", true);
        table.reap_finished([(42, 7)]);
        assert_eq!(table.wait_pid(42), Some(7));
    }

    #[test]
    fn jobspecs_resolve_in_job_order() {
        let mut table = JobTable::default();
        let first = table.register_process(1, "one", true);
        let second = table.register_process(2, "two", true);
        assert_eq!(table.resolve_jobspec("%+"), Some(second));
        assert_eq!(table.resolve_jobspec("%-"), Some(first));
        assert_eq!(table.resolve_jobspec("%1"), Some(first));
    }
}
