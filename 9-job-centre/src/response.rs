use crate::job::{FullJob, JobId};
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum Response {
    Ok(OkResponse),
    NoJob,
    Error {
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

impl Response {
    pub fn ok_get(full_job: FullJob) -> Self {
        Self::Ok(OkResponse::Get(full_job))
    }

    pub fn ok_put(id: JobId) -> Self {
        Self::Ok(OkResponse::Put { id })
    }

    pub fn ok_delete() -> Self {
        Self::Ok(OkResponse::Delete)
    }

    pub fn ok_abort() -> Self {
        Self::Ok(OkResponse::Abort)
    }

    #[cfg(debug_assertions)]
    pub fn ok_debug() -> Self {
        Self::Ok(OkResponse::Debug)
    }

    pub fn no_job() -> Self {
        Self::NoJob
    }

    pub fn error(error: String) -> Self {
        Self::Error { error: Some(error) }
    }

    #[allow(dead_code)]
    pub fn error_anon() -> Self {
        Self::Error { error: None }
    }
}

impl AsRef<Response> for Response {
    fn as_ref(&self) -> &Response {
        self
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum OkResponse {
    Put {
        id: JobId,
    },
    Get(FullJob),
    Delete,
    Abort,
    #[cfg(debug_assertions)]
    Debug,
}
