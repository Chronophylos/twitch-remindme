use std::{fmt::Display, hash::Hash};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::format_duration;

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
pub enum Activation {
    OnNextMessage,
    Fixed(OffsetDateTime),
}

impl Default for Activation {
    fn default() -> Self {
        Activation::OnNextMessage
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    id: String,
    activation: Activation,
    author: String,
    created: OffsetDateTime,
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
            created,
            text: Default::default(),
        }
    }
}

impl Message {
    pub fn new(activation: Activation, author: String, text: String) -> Self {
        Self {
            activation,
            author,
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
}
