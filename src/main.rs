#![feature(hash_drain_filter, iter_intersperse)]
#![warn(clippy::dbg_macro)]

mod duration_parser;
mod message;
mod message_parser;
mod message_store;

use std::{env, path::PathBuf, str::SplitWhitespace};

use eyre::{eyre, Context, Result};
use time::{Duration, OffsetDateTime};
use tokio::time::sleep;
use tracing::{debug, error, info, trace};
use twitch_irc::{
    login::StaticLoginCredentials,
    message::{PrivmsgMessage, ServerMessage},
    ClientConfig, SecureTCPTransport, TwitchIRCClient,
};

use crate::{
    message::{Activation, Message},
    message_parser::MessageDefinition,
    message_store::MessageStore,
};

type Client = TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>;

const PREFIX: char = '~';

async fn handle_cancel_command(
    store: &mut MessageStore,
    client: &Client,
    privmsg: &PrivmsgMessage,
    parts: &mut SplitWhitespace<'_>,
) -> Result<()> {
    if let Some(id) = parts.next() {
        info!("Removing message with id {}", id);

        if store.remove(&privmsg.sender.login, &Message::from_id(id.to_string())) {
            store.save().wrap_err("Error saving store")?;
            client
                .say_in_response(
                    privmsg.channel_login.clone(),
                    "Removed messsage".to_string(),
                    Some(privmsg.channel_id.clone()),
                )
                .await
                .wrap_err("Failed to send reply")?;
        } else {
            client
                .say_in_response(
                    privmsg.channel_login.clone(),
                    "You do not have a reminder to yourself with that id".to_string(),
                    Some(privmsg.channel_id.clone()),
                )
                .await
                .wrap_err("Failed to send reply")?;
        }
    } else {
        client
            .say_in_response(
                privmsg.channel_login.clone(),
                "Error: Missing id".to_string(),
                Some(privmsg.channel_id.clone()),
            )
            .await
            .wrap_err("Failed to send reply")?;
    }

    Ok(())
}
async fn handle_tell_command(
    store: &mut MessageStore,
    client: &Client,
    privmsg: &PrivmsgMessage,
    parts: &mut SplitWhitespace<'_>,
) -> Result<()> {
    let text = parts.intersperse(" ").collect::<String>();

    if text.is_empty() {
        return client
            .say_in_response(
                privmsg.channel_login.clone(),
                "Error: Message is empty".to_string(),
                Some(privmsg.channel_id.clone()),
            )
            .await
            .wrap_err("Failed to send reply");
    } else if text.len() > 300 {
        return client
            .say_in_response(
                privmsg.channel_login.clone(),
                "Error: Message is too long (max 300)".to_string(),
                Some(privmsg.channel_id.clone()),
            )
            .await
            .wrap_err("Failed to send reply");
    }

    let mut def = text
        .parse::<MessageDefinition>()
        .wrap_err("Failed to parse message")?;

    if def.recipients.remove("me") {
        def.recipients.insert(privmsg.sender.login.clone());
    }

    let messages = def.into_messages(&privmsg.sender.login, &privmsg.channel_login);

    let response;

    // TODO: adapt for scheduled messages
    if messages.len() == 1 {
        let message = messages.first().unwrap();

        if message.recipient() == privmsg.sender.login {
            response = format!(
                "I'll remind you the next time you type in chat [{}]",
                message.id()
            )
        } else {
            response = format!(
                "I'll remind {} when they next type in chat [{}]",
                message.recipient(),
                message.id()
            )
        }
    } else {
        response = format!(
            "I'll remind {} next time they type in chat",
            messages
                .iter()
                .map(|message| format!("{} [{}]", message.recipient(), message.id()))
                .intersperse(", ".to_string())
                .collect::<String>()
        )
    }

    let ids = messages
        .iter()
        .map(|message| message.id())
        .intersperse(", ")
        .collect::<String>();
    info!("Inserting messages with ids: {}", ids);

    for message in messages {
        if message.activation() != &Activation::OnNextMessage {
            // queue scheduled messages
            queue_message(store.clone(), client.clone(), message.clone()).await;
        }
        store.insert(message);
    }

    store.save().wrap_err("Failed to save store")?;

    client
        .say_in_response(
            privmsg.channel_login.clone(),
            response,
            Some(privmsg.channel_id.clone()),
        )
        .await
        .wrap_err("Failed to send reply")
}

async fn handle_commands(
    store: &mut MessageStore,
    client: &Client,
    privmsg: &PrivmsgMessage,
) -> Result<()> {
    let mut parts = privmsg.message_text.split_whitespace();

    match parts.next() {
        Some(word) if word.starts_with(PREFIX) => {
            let command = word
                .strip_prefix(PREFIX)
                .ok_or_else(|| eyre!("Failed to remove prefix"))?;

            match command {
                "tell" => handle_tell_command(store, client, privmsg, &mut parts)
                    .await
                    .wrap_err("Failed to handle tell command"),
                "cancel" => handle_cancel_command(store, client, privmsg, &mut parts)
                    .await
                    .wrap_err("Failed to handle tell command"),
                "bot" => client
                    .say_in_response(
                        privmsg.channel_login.clone(),
                        format!("I let you leave messages for others. Written by @Chronophylos in Rust. Version {}", env!("CARGO_PKG_VERSION")),
                        Some(privmsg.channel_id.clone()),
                    )
                    .await
                    .wrap_err("Failed to send reply"),
                _ => {
                    Err(eyre!("Unknown command"))
                    // error unknown command
                }
            }?
        }
        _ => {
            // message does not start with the prefix
        }
    }

    Ok(())
}

async fn queue_message(mut store: MessageStore, client: Client, message: Message) {
    tokio::spawn(async move {
        if let Activation::Fixed(deadline) = message.activation() {
            let now = OffsetDateTime::now_utc();
            let duration = *deadline - now;

            if duration.is_positive() {
                debug!("Queuing message {}", message.id());

                sleep(duration.try_into().unwrap()).await;
            }

            info!("Replaying timed message: {}", message.id());

            client
                .say(
                    message.channel().to_string(),
                    format!(
                        "@{} one timed message for you {}",
                        message.recipient(),
                        message
                    ),
                )
                .await
                .expect("Failed to replay message in chat");

            store.remove(message.recipient(), &message);
            store.save().expect("Failed to save store")
        }
    });
}

async fn handle_privmsg(
    store: &mut MessageStore,
    client: &Client,
    privmsg: &PrivmsgMessage,
) -> Result<()> {
    let messages = store.pop_pending(&privmsg.sender.login);
    store.save().wrap_err("Error saving store")?;

    handle_commands(store, client, privmsg)
        .await
        .wrap_err("Failed to handle commands")?;

    // process pending messages
    if !messages.is_empty() {
        info!(
            "Replaying messages: {}",
            messages
                .iter()
                .map(|m| m.id())
                .intersperse(",")
                .collect::<String>()
        );

        let text = messages
            .iter()
            .map(|message| message.to_string())
            .intersperse(" - ".to_string())
            .collect::<String>();

        let reply = format!(
            "@{} {}: {}",
            privmsg.sender.name,
            format_num(messages.len(), "reminder", "reminders"),
            text
        );

        for chunk in reply
            .chars()
            .collect::<Vec<char>>()
            .chunks(450)
            .map(|c| c.iter().collect::<String>())
        {
            client
                .say_in_response(
                    privmsg.channel_login.clone(),
                    chunk,
                    Some(privmsg.channel_id.clone()),
                )
                .await
                .wrap_err("Failed to send reply")?;
        }
    }

    Ok(())
}

async fn handle_server_message(
    store: &mut MessageStore,
    client: &Client,
    login: &str,
    message: ServerMessage,
) -> Result<()> {
    trace!("Received message: {:?}", message);

    match message {
        ServerMessage::Privmsg(privmsg) => handle_privmsg(store, client, &privmsg)
            .await
            .wrap_err("Failed to handle privmsg")?,
        ServerMessage::Join(join) => {
            if join.user_login == login {
                info!("Joined channel {}", join.channel_login);
            }
        }
        ServerMessage::Notice(notice) => {
            if notice.message_text == "Login authentication failed" {
                error!("{}", notice.message_text);
                return Err(eyre!("Failed to authenticate"));
            }
        }
        ServerMessage::Reconnect(_) => {
            todo!()
        }
        _ => {}
    }

    Ok(())
}

#[tokio::main]
pub async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let login = env::var("TWITCH_LOGIN").wrap_err("Failed to get TWITCH_LOGIN")?;
    let token = env::var("TWITCH_TOKEN").wrap_err("Failed to get TWITCH_TOKEN")?;

    // default configuration is to join chat as anonymous.
    let config = ClientConfig::new_simple(StaticLoginCredentials::new(login.clone(), Some(token)));
    let (mut incoming_messages, client) = Client::new(config);

    let store = MessageStore::from_path(PathBuf::from("messages.ron"))
        .wrap_err("Failed to open storage")?;

    // first thing you should do: start consuming incoming messages,
    // otherwise they will back up.
    let handle = tokio::spawn({
        let client = client.clone();
        let mut store = store.clone();
        async move {
            while let Some(message) = incoming_messages.recv().await {
                if let Err(err) = handle_server_message(&mut store, &client, &login, message)
                    .await
                    .wrap_err("Failed to handle server message")
                {
                    error!("{:?}", err)
                }
            }

            Ok(())
        }
    });

    // join channels
    for channel in env::var("TWITCH_CHANNELS")
        .unwrap_or_else(|_| "colnahuacatl".to_string())
        .split(',')
    {
        info!("Joining {}", channel);
        client.join(channel.to_string());
    }

    // queue messages
    for message in store.get_all() {
        queue_message(store.clone(), client.clone(), message.to_owned()).await;
    }

    handle.await.wrap_err("Failed to run bot")?
}

pub(crate) fn format_duration(duration: Duration) -> String {
    let days = duration.whole_days();
    let years = days / 356;
    let hours = duration.whole_hours() - days * 24;
    let minutes = duration.whole_minutes() - hours * 60 - days * 24 * 60;
    let seconds = duration.whole_seconds() - minutes * 60 - hours * 60 * 60 - days * 24 * 60 * 60;

    vec![
        format_short_num(years, "y"),
        format_short_num(days, "d"),
        format_short_num(hours, "h"),
        format_short_num(minutes, "m"),
        format_short_num(seconds, "s"),
        "ago".to_string(),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .intersperse(" ".to_string())
    .collect()
}

fn format_short_num(num: i64, text: &str) -> String {
    match num {
        0 => String::new(),
        x => format!("{}{}", x, text),
    }
}

fn format_num(num: usize, singular: &str, plural: &str) -> String {
    match num {
        0 => String::new(),
        1 => format!("1 {}", singular),
        x => format!("{} {}", x, plural),
    }
}
