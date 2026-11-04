use crate::{
    error::Result,
    presence,
    state::{AppState, EVENT_CHANNEL, Envelope},
};
use futures_util::StreamExt;
use nm_protocol::ServerEvent;
use std::time::Duration;
use tokio::task::JoinHandle;

pub struct Workers(Vec<JoinHandle<()>>);
impl Drop for Workers {
    fn drop(&mut self) {
        for task in &self.0 {
            task.abort();
        }
    }
}

fn resync(state: &AppState) {
    let _ = state.events.send(Envelope {
        recipients: Vec::new(),
        event: ServerEvent::ResyncRequired,
    });
}

pub async fn start(state: &AppState) -> Result<Workers> {
    let mut subscription = state.redis_client.get_async_pubsub().await?;
    subscription.subscribe(EVENT_CHANNEL).await?;
    let state = state.clone();
    let sweep_state = state.clone();
    let outbox_state = state.clone();
    let push_state = state.clone();
    let subscriber = tokio::spawn(async move {
        loop {
            {
                let mut messages = subscription.on_message();
                while let Some(message) = messages.next().await {
                    match serde_json::from_slice::<Envelope>(message.get_payload_bytes()) {
                        Ok(envelope) => {
                            let _ = state.events.send(envelope);
                        }
                        Err(_) => resync(&state),
                    }
                }
            }
            resync(&state);
            // Pub/sub has no replay: every reconnection explicitly requests resync.
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                if let Ok(mut next) = state.redis_client.get_async_pubsub().await
                    && next.subscribe(EVENT_CHANNEL).await.is_ok()
                {
                    subscription = next;
                    resync(&state);
                    break;
                }
            }
        }
    });
    let sweeper = tokio::spawn(async move {
        let mut timer = tokio::time::interval(Duration::from_secs(15));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            timer.tick().await;
            if presence::sweep(&sweep_state).await.is_err() {
                resync(&sweep_state);
            }
        }
    });
    let outbox = tokio::spawn(crate::outbox::run(outbox_state));
    let push = tokio::spawn(crate::push::run(push_state));
    Ok(Workers(vec![subscriber, sweeper, outbox, push]))
}
