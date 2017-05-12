extern crate discord;
extern crate regex;

use std::collections::{HashSet, HashMap};
use std::env;
use std::time::{Duration, Instant};

use discord::Discord;
use discord::model::{Event, ChannelId, Message, UserId};

use regex::Regex;

const TIMEOUT: u64 = 5 * 60; // 5 minutes

fn handle_message(message: Message,
                  greps: &mut Vec<(Regex, UserId)>,
                  timeouts: &mut HashMap<(UserId, ChannelId), Instant>)
                  -> Option<String> {
    let channel = message.channel_id;
    let content = message.content;
    let author = message.author;

    if author.bot {
        return None;
    }

    if content == "!grephelp" {
        Some(include_str!("help.md").into())
    } else if content == "!mygreps" {
        Some(greps.iter()
            .filter(|&&(_, id)| id == author.id)
            .map(|&(ref regex, _)| regex)
            .fold(String::new(),
                  |string, regex| format!("{}\n{}", string, regex)))
    } else if content.starts_with("!grep ") {
        content.splitn(2, ' ').nth(1).map(|pattern| match Regex::new(pattern) {
            Ok(regex) => {
                if greps.iter()
                    .any(|&(ref regex, id)| id == author.id && regex.as_str() == pattern) {
                    "Regex already exists".into()
                } else {
                    greps.push((regex, author.id));
                    "Regex added".into()
                }
            }
            Err(error) => format!("Invalid regex. {}", error),
        })
    } else if content.starts_with("!ungrep ") {
        content.splitn(2, ' ').nth(1).map(|pattern| {
            let mut removals = false;
            greps.retain(|&(ref regex, id)| {
                if id == author.id && regex.as_str() == pattern {
                    removals = true;
                    false
                } else {
                    true
                }
            });
            if removals {
                format!("Regex {} removed", pattern)
            } else {
                format!("Regex {} was not found", pattern)
            }
        })
    } else {
        let users: HashSet<_> = greps.iter()
            .filter(|&&(ref regex, _)| regex.is_match(&content))
            .map(|&(_, id)| id)
            .filter(|&id| id != author.id)
            .filter(|&id| match timeouts.get(&(id, channel)) {
                Some(instant) => instant.elapsed() > Duration::from_secs(TIMEOUT),
                None => true,
            })
            .collect();
        if !users.is_empty() {
            Some(users.into_iter()
                .inspect(|&id| {
                    timeouts.insert((id, channel), Instant::now());
                })
                .fold("Hey!".into(),
                      |string, id| format!("{} {}", string, id.mention())))
        } else {
            None
        }
    }
}

fn main() {
    // state
    let mut greps = Vec::new();
    let mut timeouts = HashMap::new();
    // api
    let discord = Discord::from_bot_token(&env::var("DISCORD_BOT_TOKEN")
            .expect("DISCORD_BOT_TOKEN not set"))
        .expect("Login Failed");
    let mut connection = match discord.connect() {
        Ok((connection, _)) => connection,
        Err(e) => panic!("Unable to connect to discord API: {}", e),
    };
    // generic fun stuff
    connection.set_game_name("Talk to me with !grephelp".to_string());
    // main loop time
    while let Ok(event) = connection.recv_event() {
        if let Event::MessageCreate(message) = event {
            let channel = message.channel_id;
            if let Some(content) = handle_message(message, &mut greps, &mut timeouts) {
                let _ = discord.send_message(channel, &content, "", false);
            }
        }
    }
}
