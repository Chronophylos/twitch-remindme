use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    fs::File,
    hash::Hash,
    path::PathBuf,
};

use eyre::{eyre, Context, Result};
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
}

#[derive(Debug, Clone)]
pub struct MessageStore {
    path: PathBuf,
    data: HashMap<String, HashSet<Message>>,
}

impl MessageStore {
    pub fn from_path(path: PathBuf) -> Result<Self> {
        let data = if path.exists() {
            if path.is_dir() {
                return Err(eyre!("Path points to a directory"));
            }

            let file = File::open(&path).wrap_err("Failed to open storage")?;
            ron::de::from_reader(file).wrap_err("Failed to deserialize storage")?
        } else {
            HashMap::new()
        };

        Ok(Self { path, data })
    }

    pub fn insert(&mut self, username: String, message: Message) {
        self.data
            .entry(username)
            .and_modify(|messages| {
                messages.insert(message.clone());
            })
            .or_insert_with(|| {
                let mut set = HashSet::new();
                set.insert(message);
                set
            });
    }

    pub fn pop_pending(&mut self, username: &str) -> HashSet<Message> {
        let now = OffsetDateTime::now_utc();

        self.data
            .get_mut(username)
            .map(|messages| {
                messages
                    .drain_filter(|message| match message.activation {
                        Activation::OnNextMessage => true,
                        Activation::Fixed(then) => now >= then,
                    })
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default()
    }

    pub fn remove(&mut self, username: &str, message: &Message) {
        self.data
            .get_mut(username)
            .map(|messages| messages.remove(message));
    }

    pub fn save(&self) -> Result<()> {
        let file = File::create(&self.path).wrap_err("Failed to open storage")?;
        ron::ser::to_writer(file, &self.data).wrap_err("Failed to serialize storage")?;

        Ok(())
    }
}
