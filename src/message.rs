use std::{fmt::Display, hash::Hash};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{format_duration, message_parser::Schedule};

#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum Activation {
    OnNextMessage,
    Fixed(OffsetDateTime),
}

impl Default for Activation {
    fn default() -> Self {
        Activation::OnNextMessage
    }
}

impl From<Schedule> for Activation {
    fn from(schedule: Schedule) -> Self {
        match schedule {
            Schedule::None => Activation::OnNextMessage,
            Schedule::Relative(duration) => Activation::Fixed(OffsetDateTime::now_utc() + duration),
            Schedule::Fixed(datetime) => Activation::Fixed(datetime),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    id: String,
    activation: Activation,
    author: String,
    recipient: String,
    created: OffsetDateTime,
    channel: String,
    text: String,
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let now = OffsetDateTime::now_utc();

        write!(
            f,
            "{} ({}): {}",
            self.author,
            format_duration((now - self.created).abs()),
            self.text
        )
    }
}

impl PartialEq for Message {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Message {}

impl Hash for Message {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Default for Message {
    fn default() -> Self {
        let created = OffsetDateTime::now_utc();
        Self {
            id: cuid::slug().unwrap_or_else(|_| created.to_string()),
            activation: Default::default(),
            author: Default::default(),
            recipient: Default::default(),
            created,
            channel: Default::default(),
            text: Default::default(),
        }
    }
}

impl Message {
    pub fn new(
        activation: Activation,
        author: String,
        channel: String,
        recipient: String,
        text: String,
    ) -> Self {
        Self {
            activation,
            author,
            channel,
            recipient,
            text,
            ..Default::default()
        }
    }

    pub fn from_id(id: String) -> Self {
        Self {
            id,
            ..Default::default()
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn activation(&self) -> &Activation {
        &self.activation
    }

    pub fn recipient(&self) -> &str {
        &self.recipient
    }

    pub fn channel(&self) -> &str {
        &self.channel
    }
}
