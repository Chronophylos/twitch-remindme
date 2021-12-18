use std::{
    collections::{HashMap, HashSet},
    fs::File,
    path::PathBuf,
};

use eyre::{eyre, Context, Result};

use crate::message::{Activation, Message};

#[derive(Debug, Clone)]
pub struct MessageStore {
    path: PathBuf,
    data: HashMap<String, HashSet<Message>>,
}

impl MessageStore {
    pub fn from_path(path: PathBuf) -> Result<Self> {
        let raw_data = if path.exists() {
            if path.is_dir() {
                return Err(eyre!("Path points to a directory"));
            }

            let file = File::open(&path).wrap_err("Failed to open storage")?;
            ron::de::from_reader(file).wrap_err("Failed to deserialize storage")?
        } else {
            HashSet::<Message>::new()
        };

        let data = raw_data.into_iter().fold(
            HashMap::<String, HashSet<Message>>::new(),
            |mut acc, message| {
                acc.entry(message.recipient().to_string())
                    .and_modify(|messages| {
                        messages.insert(message.clone());
                    })
                    .or_insert_with(|| {
                        let mut set = HashSet::new();
                        set.insert(message);
                        set
                    });
                acc
            },
        );

        Ok(Self { path, data })
    }

    pub fn insert(&mut self, message: Message) {
        self.data
            .entry(message.recipient().to_string())
            .and_modify(|messages| {
                messages.insert(message.clone());
            })
            .or_insert_with(|| {
                let mut set = HashSet::new();
                set.insert(message);
                set
            });
    }

    /// Get all message that have not been sent yet. This does not include timedout scheduled
    /// messages.
    pub fn pop_pending(&mut self, username: &str) -> HashSet<Message> {
        self.data
            .get_mut(username)
            .map(|messages| {
                messages
                    .drain_filter(|message| {
                        matches!(message.activation(), Activation::OnNextMessage)
                    })
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default()
    }

    pub fn get_all(&self) -> HashSet<&Message> {
        self.data.values().flatten().collect()
    }

    pub fn remove(&mut self, message: &Message) -> bool {
        self.data
            .values_mut()
            .map(|messages| messages.remove(message))
            .any(|x| x)
    }

    pub fn save(&self) -> Result<()> {
        let file = File::create(&self.path).wrap_err("Failed to open storage")?;
        let data = self
            .data
            .values()
            .flat_map(|set| set.iter())
            .collect::<Vec<&Message>>();

        write_store(file, &data).wrap_err("Failed to write storeage")
    }
}

#[cfg(not(feature = "pretty_store"))]
fn write_store(file: File, data: &[&Message]) -> Result<(), ron::Error> {
    ron::ser::to_writer(file, &data)
}

#[cfg(feature = "pretty_store")]
fn write_store(file: File, data: &[&Message]) -> Result<(), ron::Error> {
    ron::ser::to_writer_pretty(file, &data, ron::ser::PrettyConfig::default())
}
