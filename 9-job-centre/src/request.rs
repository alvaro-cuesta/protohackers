use crate::{
    job::{JobId, JobPriority, JobValue},
    state::QueueName,
};
use serde::Deserialize;

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "request", rename_all = "kebab-case")]
pub enum Request {
    Put {
        queue: QueueName,
        job: JobValue,
        pri: JobPriority,
    },
    Get {
        queues: Vec<QueueName>,
        #[serde(default)]
        wait: bool,
    },
    Delete {
        id: JobId,
    },
    Abort {
        id: JobId,
    },
    #[cfg(debug_assertions)]
    Debug,
}
