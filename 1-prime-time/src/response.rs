use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum Response {
    IsPrime { prime: bool },
    Error,
}

impl Response {
    pub fn is_prime(is_prime: bool) -> Self {
        Self::IsPrime { prime: is_prime }
    }

    pub fn error() -> Self {
        Self::Error
    }
}

impl AsRef<Response> for Response {
    fn as_ref(&self) -> &Response {
        self
    }
}
