use std::{
    collections::{HashMap, HashSet},
    fs::File,
    path::PathBuf,
};

use eyre::{eyre, Context, Result};
use time::OffsetDateTime;

use crate::{
    message::{Activation, Message},
    message_parser::MessageDefinition,
};

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

    pub fn insert_from_definition(&mut self, author: String, def: MessageDefinition) {
        let activation = def.schedule.into();
        for recipient in def.recipients {
            self.insert(
                recipient,
                Message::new(activation, author.clone(), def.text.clone()),
            )
        }
    }

    pub fn pop_pending(&mut self, username: &str) -> HashSet<Message> {
        let now = OffsetDateTime::now_utc();

        self.data
            .get_mut(username)
            .map(|messages| {
                messages
                    .drain_filter(|message| match message.activation() {
                        Activation::OnNextMessage => true,
                        Activation::Fixed(then) => &now >= then,
                    })
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default()
    }

    pub fn remove(&mut self, username: &str, message: &Message) -> bool {
        self.data
            .get_mut(username)
            .map(|messages| messages.remove(message))
            .unwrap_or(false)
    }

    pub fn save(&self) -> Result<()> {
        let file = File::create(&self.path).wrap_err("Failed to open storage")?;
        ron::ser::to_writer(file, &self.data).wrap_err("Failed to serialize storage")?;

        Ok(())
    }
}
