use serde::Deserialize;

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum Request {
    IsPrime { number: serde_json::Number },
}
