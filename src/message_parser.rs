use std::{collections::HashSet, str::FromStr};

use pest::Parser;
use pest_derive::Parser;
use time::{Duration, OffsetDateTime};

use crate::{duration_parser::IntermediateDuration, message::Message};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Schedule {
    None,
    Relative(Duration),
    Fixed(OffsetDateTime),
}

#[derive(Debug, Clone)]
pub struct MessageDefinition {
    pub text: String,
    pub created: OffsetDateTime,
    pub schedule: Schedule,
    pub recipients: HashSet<String>,
}

impl FromStr for MessageDefinition {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let message_pair = MessageDefinitionParser::parse(Rule::message, s)
            .map_err(|source| Error::ParseRule {
                rule: Rule::message,
                source,
            })?
            .next()
            .unwrap();

        let mut def = MessageDefinition {
            text: String::new(),
            created: OffsetDateTime::now_utc(),
            schedule: Schedule::None,
            recipients: HashSet::new(),
        };

        for pair in message_pair.into_inner() {
            match pair.as_rule() {
                Rule::attributes => {
                    for pair in pair.into_inner() {
                        let mut inner = pair.into_inner();
                        let key = inner.next().unwrap().as_str();
                        let value_pair = inner.next().unwrap().into_inner().next().unwrap();
                        let value = match value_pair.as_rule() {
                            Rule::quoted_string => value_pair
                                .as_str()
                                .strip_prefix('\"')
                                .unwrap()
                                .strip_suffix('\"')
                                .unwrap(),
                            Rule::unquoted_string => value_pair.as_str(),
                            _ => {
                                unreachable!()
                            }
                        };

                        match key {
                            "cc" => {
                                def.recipients.insert(value.to_lowercase());
                            }
                            "in" => {
                                def.schedule = Schedule::Relative(
                                    value.to_lowercase().parse::<IntermediateDuration>()?.into(),
                                )
                            }
                            _ => return Err(Error::UnknownAttributeKey(key.to_string())),
                        }
                    }
                }
                Rule::recipient => {
                    def.recipients
                        .insert(pair.as_span().as_str().to_lowercase());
                }
                Rule::text => def.text = pair.as_span().as_str().to_string(),
                Rule::EOI => {
                    let s = pair.as_span().as_str();
                    if !s.is_empty() {
                        return Err(Error::DanglingChars(s.to_string()));
                    }
                }
                _ => {}
            }
        }

        Ok(def)
    }
}

impl MessageDefinition {
    pub fn into_messages(self, author: String) -> Vec<Message> {
        let activation = self.schedule.into();
        self.recipients
            .into_iter()
            .map(|recipient| Message::new(activation, author.clone(), recipient, self.text.clone()))
            .collect()
    }
}

#[derive(Parser)]
#[grammar = "message.pest"]
struct MessageDefinitionParser;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to parse string as {rule:?}")]
    ParseRule {
        rule: Rule,
        source: pest::error::Error<Rule>,
    },

    #[error("String contains unparsed chars: {0:?}")]
    DanglingChars(String),

    #[error("Unknown attribute key: {0:?}")]
    UnknownAttributeKey(String),

    #[error("Failed to parse duration: {0}")]
    ParseDuration(#[from] crate::duration_parser::Error),
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use time::OffsetDateTime;

    use crate::message_parser::{MessageDefinition, Schedule};

    #[test]
    fn parse_empty() {
        assert!("".parse::<MessageDefinition>().is_err())
    }

    #[test]
    fn parse_simple() {
        let def = "recipient actual message"
            .parse::<MessageDefinition>()
            .unwrap();

        assert_eq!(HashSet::from([String::from("recipient")]), def.recipients);
        assert_eq!("actual message", &def.text);
        assert_eq!(Schedule::None, def.schedule);
    }

    #[test]
    fn parse_with_cc_attribute() {
        let def = "cc:\"other\" cc:foo recipient actual message"
            .parse::<MessageDefinition>()
            .unwrap();

        assert_eq!(
            ["other", "foo", "recipient"]
                .into_iter()
                .map(|s| s.to_string())
                .collect::<HashSet<_>>(),
            def.recipients
        );
        assert_eq!("actual message", &def.text);
        assert_eq!(Schedule::None, def.schedule);
    }

    #[test]
    fn test_uppercase() {
        let def = "cc:\"other\" cc:Foo recIpient actual message"
            .parse::<MessageDefinition>()
            .unwrap();

        assert_eq!(
            ["other", "foo", "recipient"]
                .into_iter()
                .map(|s| s.to_string())
                .collect::<HashSet<_>>(),
            def.recipients
        );
        assert_eq!("actual message", &def.text);
        assert_eq!(Schedule::None, def.schedule);
    }

    #[test]
    fn message_definition_into_messages() {
        let def = MessageDefinition {
            text: "this is text".to_string(),
            created: OffsetDateTime::now_utc(),
            schedule: Schedule::None,
            recipients: ["foo".to_string(), "bar".to_string()].into(),
        };

        assert_eq!(
            vec!["foo", "bar"],
            def.into_messages("me".to_string())
                .into_iter()
                .map(|message| message.recipient().to_string())
                .collect::<Vec<_>>()
        );
    }
}
