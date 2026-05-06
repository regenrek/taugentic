use std::collections::VecDeque;
use std::sync::Arc;
use std::time::SystemTime;

use futures_util::future::BoxFuture;
use ta_provider_llm::client::StreamMessage;

use crate::ExecutionError;
use crate::session::Session;

pub type MessageFetcher =
    Arc<dyn Fn() -> BoxFuture<'static, Vec<StreamMessage>> + Send + Sync + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueMode {
    All,
    OneAtATime,
}

impl Default for QueueMode {
    fn default() -> Self {
        Self::All
    }
}

#[derive(Debug, Clone)]
pub struct Queued {
    pub seq: u64,
    pub enqueued_at: SystemTime,
    pub message: StreamMessage,
}

#[derive(Clone)]
pub struct MessageQueue {
    steering: VecDeque<Queued>,
    follow_up: VecDeque<Queued>,
    steering_mode: QueueMode,
    follow_up_mode: QueueMode,
    next_seq: u64,
    get_steering_messages: Option<MessageFetcher>,
    get_follow_up_messages: Option<MessageFetcher>,
}

impl Default for MessageQueue {
    fn default() -> Self {
        Self {
            steering: VecDeque::new(),
            follow_up: VecDeque::new(),
            steering_mode: QueueMode::All,
            follow_up_mode: QueueMode::All,
            next_seq: 0,
            get_steering_messages: None,
            get_follow_up_messages: None,
        }
    }
}

impl MessageQueue {
    pub fn with_fetchers(
        get_steering_messages: Option<MessageFetcher>,
        get_follow_up_messages: Option<MessageFetcher>,
    ) -> Self {
        Self {
            get_steering_messages,
            get_follow_up_messages,
            ..Self::default()
        }
    }

    pub fn set_modes(&mut self, steering_mode: QueueMode, follow_up_mode: QueueMode) {
        self.steering_mode = steering_mode;
        self.follow_up_mode = follow_up_mode;
    }

    pub fn push_steering(&mut self, message: StreamMessage) {
        let queued = self.queued(message);
        self.steering.push_back(queued);
    }

    pub fn push_follow_up(&mut self, message: StreamMessage) {
        let queued = self.queued(message);
        self.follow_up.push_back(queued);
    }

    pub fn steering_len(&self) -> usize {
        self.steering.len()
    }

    pub fn follow_up_len(&self) -> usize {
        self.follow_up.len()
    }

    pub async fn drain_steering_into(
        &mut self,
        session: &Session,
    ) -> Result<Vec<StreamMessage>, ExecutionError> {
        if let Some(fetcher) = &self.get_steering_messages {
            let fetched = fetcher().await;
            for message in fetched {
                self.push_steering(message);
            }
        }
        let messages = drain(&mut self.steering, self.steering_mode);
        session.append_messages(messages.clone())?;
        Ok(messages)
    }

    pub async fn drain_follow_up_into(
        &mut self,
        session: &Session,
    ) -> Result<Vec<StreamMessage>, ExecutionError> {
        if let Some(fetcher) = &self.get_follow_up_messages {
            let fetched = fetcher().await;
            for message in fetched {
                self.push_follow_up(message);
            }
        }
        let messages = drain(&mut self.follow_up, self.follow_up_mode);
        session.append_messages(messages.clone())?;
        Ok(messages)
    }

    fn queued(&mut self, message: StreamMessage) -> Queued {
        let queued = Queued {
            seq: self.next_seq,
            enqueued_at: SystemTime::now(),
            message,
        };
        self.next_seq = self.next_seq.saturating_add(1);
        queued
    }
}

fn drain(queue: &mut VecDeque<Queued>, mode: QueueMode) -> Vec<StreamMessage> {
    match mode {
        QueueMode::All => queue.drain(..).map(|queued| queued.message).collect(),
        QueueMode::OneAtATime => queue
            .pop_front()
            .into_iter()
            .map(|queued| queued.message)
            .collect(),
    }
}
