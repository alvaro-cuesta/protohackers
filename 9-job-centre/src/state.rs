use priority_queue::PriorityQueue;
use tokio::sync::oneshot::{self, Receiver, Sender};

use crate::job::{FullJob, JobId, JobPriority, JobValue};
use std::{
    collections::{hash_map::Entry, HashMap},
    net::SocketAddr,
};

pub type QueueName = String;

type Queue = PriorityQueue<JobId, JobPriority>;

type WorkingJob = (JobPriority, QueueName);

#[derive(Debug, Default)]
pub struct State {
    last_job_id: u64,
    /// Notice that a job can be missing here (due to deletion) and be present on a queue or client working jobs.
    /// This is considered a deleted job and should be ignored. We do not clear it from queues or working jobs
    /// to avoid iterating over them to
    jobs_by_id: HashMap<JobId, (JobValue, QueueName)>,
    queues: HashMap<QueueName, Queue>,
    // TODO: Clear these ones on delete?
    working_jobs_by_client: HashMap<SocketAddr, HashMap<JobId, WorkingJob>>,
    // TODO: Clear these ones on disconnect?
    waiting: Vec<(Vec<QueueName>, Sender<FullJob>)>,
}

impl State {
    pub fn add_job(
        &mut self,
        queue_name: QueueName,
        job_value: JobValue,
        priority: JobPriority,
    ) -> JobId {
        let job_id = self.get_job_id();

        // TODO: Clone
        self.jobs_by_id
            .insert(job_id, (job_value.clone(), queue_name.clone()));

        self.add_existing_job_to_queue(job_id, queue_name, job_value, priority);

        job_id
    }

    pub fn get_job(&mut self, client: SocketAddr, queue_names: Vec<QueueName>) -> Option<FullJob> {
        // TODO: This is bugged because it can get a None here but next one might have lower prio
        let max_peeked = queue_names
            .iter()
            .filter_map(|queue_name| {
                self.queues
                    .get(queue_name)
                    .and_then(|queue| queue.peek().map(|job| (queue_name, job)))
            })
            .max_by_key(|(_, (_, job_priority))| *job_priority);

        let Some((max_queue_name, _)) = max_peeked else {
            return None;
        };

        let Some(queue) = self.queues.get_mut(max_queue_name) else {
            return None;
        };

        let Some((job_id, job_priority)) = queue.pop() else {
            return None
        };

        let Some((job_value, queue_name)) = self
            .jobs_by_id
            .get(&job_id) else {
                // Job was deleted
                return None;
            };

        self.working_jobs_by_client
            .entry(client)
            .or_default()
            .insert(job_id, (job_priority, max_queue_name.to_owned()));

        return Some(FullJob::new(
            job_id,
            job_value.clone(),
            job_priority,
            max_queue_name.to_owned(),
        ));
    }

    pub fn wait_job(&mut self, client: SocketAddr, queue_names: Vec<QueueName>) -> WaitResponse {
        // TODO: Dont' clone
        match self.get_job(client, queue_names.clone()) {
            Some(full_job) => WaitResponse::Job(full_job),
            None => {
                // TODO: What if the client dropped?
                let (sender, receiver) = oneshot::channel::<FullJob>();

                self.waiting.push((queue_names, sender));

                WaitResponse::Wait(receiver)
            }
        }
    }

    pub fn delete_job(&mut self, job_id: JobId) -> bool {
        let Entry::Occupied(entry) = self.jobs_by_id.entry(job_id) else {
            return false
        };

        let (_, (_, queue_name)) = entry.remove_entry();

        if let Some(queue) = self.queues.get_mut(&queue_name) {
            queue.remove(&job_id);
        };

        true
    }

    pub fn abort_job(&mut self, client: SocketAddr, job_id: JobId) -> bool {
        let Some((job_value, queue_name)) = self.jobs_by_id.get(&job_id) else {
            return false;
        };

        let Some(working_jobs) = self.working_jobs_by_client.get_mut(&client) else {
            return false;
        };

        let Entry::Occupied(entry) = working_jobs.entry(job_id) else {
            return false;
        };

        let (job_priority, queue_name) = entry.remove();

        // TODO: Clone
        self.add_existing_job_to_queue(job_id, queue_name, job_value.clone(), job_priority);

        true
    }

    pub fn abort_client_jobs(&mut self, client: SocketAddr) {
        let Entry::Occupied(working_jobs) = self.working_jobs_by_client.entry(client) else {
            return;
        };

        for (job_id, (job_priority, queue_name)) in working_jobs.remove().drain() {
            let Some((job_value, queue_name_two)) = self.jobs_by_id.get(&job_id) else {
                continue;
            };

            // TODO: Clone
            self.add_existing_job_to_queue(job_id, queue_name, job_value.clone(), job_priority);
        }
    }

    fn get_job_id(&mut self) -> u64 {
        let job_id = self.last_job_id;
        self.last_job_id += 1;

        job_id
    }

    fn add_existing_job_to_queue(
        &mut self,
        job_id: JobId,
        queue_name: QueueName,
        job_value: JobValue,
        priority: JobPriority,
    ) {
        // TODO: Make map-ish
        if let Some(index) = self
            .waiting
            .iter()
            .position(|(queue_names, _)| queue_names.contains(&queue_name))
        {
            let (_, sender) = self.waiting.swap_remove(index);

            sender
                .send(FullJob::new(job_id, job_value, priority, queue_name))
                // TODO: Panic
                .unwrap()
        } else {
            self.queues
                .entry(queue_name)
                .or_default()
                .push(job_id, priority);
        }
    }
}

pub enum WaitResponse {
    Job(FullJob),
    Wait(Receiver<FullJob>),
}
