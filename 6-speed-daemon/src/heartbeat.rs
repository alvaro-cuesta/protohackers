use std::time::Duration;
use thiserror::Error;
use tokio::{sync::mpsc, task::JoinHandle, time::MissedTickBehavior};

use crate::message::MessageToClient;

pub struct Heartbeat(Option<JoinHandle<()>>);

impl Heartbeat {
    pub fn new() -> Self {
        Self(None)
    }

    pub fn start(
        &mut self,
        write: mpsc::Sender<MessageToClient>,
        interval: Duration,
    ) -> Result<(), HeartbeatExistsError> {
        if self.0.is_some() {
            return Err(HeartbeatExistsError);
        }

        self.0 = Some(tokio::spawn(async move {
            if interval.is_zero() {
                return;
            }

            let mut interval_f = tokio::time::interval(interval);
            interval_f.set_missed_tick_behavior(MissedTickBehavior::Delay);

            interval_f.tick().await;

            loop {
                interval_f.tick().await;
                if let Err(_) = write.send(MessageToClient::Heartbeat).await {
                    return;
                }
            }
        }));

        Ok(())
    }
}

impl Drop for Heartbeat {
    fn drop(&mut self) {
        if let Some(task) = &self.0 {
            task.abort();
        }
    }
}

#[derive(Debug, Error)]
#[error("heartbeat already exists")]
pub struct HeartbeatExistsError;
