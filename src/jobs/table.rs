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
pub struct ProcessEntry {
    pub pid: u32,
    pub state: ProcessState,
    pub exit_status: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobEntry {
    pub id: JobId,
    pub pids: Vec<u32>,
    pub processes: BTreeMap<u32, ProcessEntry>,
    pub command: String,
    pub state: ProcessState,
    pub exit_status: Option<i32>,
    pub background: bool,
    pub foreground: bool,
    pub notified: bool,
    pub coproc_endpoints: Vec<u32>,
}

#[derive(Debug, Default, Clone)]
pub struct JobTable {
    pub jobs: BTreeMap<JobId, JobEntry>,
    pub pid_to_job: HashMap<u32, JobId>,
    pub completed_statuses: HashMap<u32, i32>,
    current_job: Option<JobId>,
    previous_job: Option<JobId>,
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
        let processes = pids
            .iter()
            .copied()
            .map(|pid| {
                (
                    pid,
                    ProcessEntry {
                        pid,
                        state: ProcessState::Running,
                        exit_status: None,
                    },
                )
            })
            .collect();
        let entry = JobEntry {
            id,
            pids: pids.clone(),
            processes,
            command: command.into(),
            state: ProcessState::Running,
            exit_status: None,
            background,
            foreground: !background,
            notified: false,
            coproc_endpoints: Vec::new(),
        };
        for pid in pids {
            self.pid_to_job.insert(pid, id);
        }
        self.jobs.insert(id, entry);
        self.set_current_job(id);
        id
    }

    pub fn resolve_jobspec(&self, spec: &str) -> Option<JobId> {
        let body = spec.strip_prefix('%').unwrap_or(spec);
        if body.is_empty() || matches!(body, "%" | "+") {
            return self
                .current_job
                .or_else(|| self.jobs.keys().next_back().copied());
        }
        if body == "-" {
            return self.previous_job;
        }
        if let Some(id) = body.strip_prefix('?') {
            return self
                .jobs
                .iter()
                .rev()
                .find(|(_, job)| job.command.contains(id))
                .map(|(id, _)| *id);
        }
        if let Ok(id) = body.parse::<JobId>() {
            return self.jobs.contains_key(&id).then_some(id);
        }
        self.jobs
            .iter()
            .rev()
            .find(|(_, job)| job.command.starts_with(body))
            .map(|(id, _)| *id)
    }

    pub fn current_job(&self) -> Option<JobId> {
        self.current_job
    }

    pub fn previous_job(&self) -> Option<JobId> {
        self.previous_job
    }

    pub fn set_current_job(&mut self, id: JobId) {
        if self.current_job != Some(id) {
            self.previous_job = self.current_job;
            self.current_job = Some(id);
        }
    }

    pub fn mark_completed(&mut self, pid: u32, status: i32) {
        self.completed_statuses.insert(pid, status);
        let Some(job_id) = self.pid_to_job.get(&pid).copied() else {
            return;
        };
        let Some(job) = self.jobs.get_mut(&job_id) else {
            return;
        };
        if let Some(process) = job.processes.get_mut(&pid) {
            process.state = ProcessState::Completed;
            process.exit_status = Some(status);
        }
        self.recompute_job(job_id);
    }

    pub fn mark_stopped(&mut self, pid: u32) {
        let Some(job_id) = self.pid_to_job.get(&pid).copied() else {
            return;
        };
        if let Some(job) = self.jobs.get_mut(&job_id) {
            if let Some(process) = job.processes.get_mut(&pid) {
                process.state = ProcessState::Stopped;
            }
        }
        self.recompute_job(job_id);
    }

    pub fn mark_running(&mut self, pid: u32) {
        let Some(job_id) = self.pid_to_job.get(&pid).copied() else {
            return;
        };
        if let Some(job) = self.jobs.get_mut(&job_id) {
            if let Some(process) = job.processes.get_mut(&pid) {
                process.state = ProcessState::Running;
            }
            job.foreground = true;
        }
        self.set_current_job(job_id);
        self.recompute_job(job_id);
    }

    fn recompute_job(&mut self, job_id: JobId) {
        let Some(job) = self.jobs.get_mut(&job_id) else {
            return;
        };
        let any_running = job
            .processes
            .values()
            .any(|p| p.state == ProcessState::Running);
        let any_stopped = job
            .processes
            .values()
            .any(|p| p.state == ProcessState::Stopped);
        job.state = if any_running {
            ProcessState::Running
        } else if any_stopped {
            ProcessState::Stopped
        } else {
            ProcessState::Completed
        };
        job.exit_status = job
            .pids
            .last()
            .and_then(|pid| job.processes.get(pid))
            .and_then(|process| process.exit_status);
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

    pub fn remove_job(&mut self, job_id: JobId) -> bool {
        let Some(job) = self.jobs.remove(&job_id) else {
            return false;
        };
        for pid in &job.pids {
            self.pid_to_job.remove(pid);
            self.completed_statuses.remove(pid);
        }
        if self.current_job == Some(job_id) {
            self.current_job = self.previous_job;
        }
        if self.previous_job == Some(job_id) {
            self.previous_job = None;
        }
        true
    }

    pub fn remove_job_by_pid(&mut self, pid: u32) -> bool {
        self.pid_to_job
            .get(&pid)
            .copied()
            .is_some_and(|job_id| self.remove_job(job_id))
    }

    pub fn remove_job_by_pid_preserve_status(&mut self, pid: u32) -> bool {
        let Some(job_id) = self.pid_to_job.get(&pid).copied() else {
            return false;
        };
        let Some(job) = self.jobs.remove(&job_id) else {
            return false;
        };
        for candidate in &job.pids {
            self.pid_to_job.remove(candidate);
        }
        if self.current_job == Some(job_id) {
            self.current_job = self.previous_job;
        }
        if self.previous_job == Some(job_id) {
            self.previous_job = None;
        }
        true
    }

    pub fn clear_jobs(&mut self) {
        self.jobs.clear();
        self.pid_to_job.clear();
        self.completed_statuses.clear();
        self.current_job = None;
        self.previous_job = None;
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

    #[test]
    fn pipeline_state_waits_for_all_processes_and_uses_last_status() {
        let mut table = JobTable::default();
        let job = table.register_pipeline(vec![10, 11], "producer | consumer", true);
        assert_eq!(table.jobs[&job].state, ProcessState::Running);
        table.mark_completed(10, 3);
        assert_eq!(table.jobs[&job].state, ProcessState::Running);
        table.mark_completed(11, 7);
        assert_eq!(table.jobs[&job].state, ProcessState::Completed);
        assert_eq!(table.jobs[&job].exit_status, Some(7));
    }

    #[test]
    fn stopped_and_continued_processes_update_job_state() {
        let mut table = JobTable::default();
        let job = table.register_process(20, "sleep", true);
        table.mark_stopped(20);
        assert_eq!(table.jobs[&job].state, ProcessState::Stopped);
        table.mark_running(20);
        assert_eq!(table.jobs[&job].state, ProcessState::Running);
    }

    #[test]
    fn removing_a_job_clears_pid_and_current_previous_indexes() {
        let mut table = JobTable::default();
        let first = table.register_process(30, "first", true);
        table.register_process(31, "second", true);
        assert!(table.remove_job_by_pid(31));
        assert!(!table.pid_to_job.contains_key(&31));
        assert_eq!(table.resolve_jobspec("%+"), Some(first));
        assert_eq!(table.resolve_jobspec("%1"), Some(first));
        assert!(table.remove_job(first));
        assert!(table.jobs.is_empty());
    }

    #[test]
    fn wait_minus_removes_job_but_retains_explicit_pid_status() {
        let mut table = JobTable::default();
        let job = table.register_process(40, "false", true);
        table.mark_completed(40, 1);
        assert!(table.remove_job_by_pid_preserve_status(40));
        assert!(!table.jobs.contains_key(&job));
        assert_eq!(table.completed_statuses.get(&40), Some(&1));
        assert_eq!(table.pid_to_job.get(&40), None);
        assert_eq!(table.wait_pid(40), Some(1));
    }
}
