use serde::Serialize;

use crate::state::QueueName;

pub type JobId = u64;

pub type JobValue = serde_json::Value;

pub type JobPriority = u64;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct FullJob {
    id: JobId,
    #[serde(rename = "job")]
    value: JobValue,
    #[serde(rename = "pri")]
    priority: JobPriority,
    #[serde(rename = "queue")]
    queue_name: QueueName,
}

impl FullJob {
    pub fn new(id: JobId, value: JobValue, priority: JobPriority, queue_name: QueueName) -> Self {
        Self {
            id,
            value,
            priority,
            queue_name,
        }
    }
}
