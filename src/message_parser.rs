use std::str::FromStr;

use pest::{Parser, RuleType};
use pest_derive::Parser;
use thiserror::Error;
use time::{Duration, OffsetDateTime};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Schedule {
    None,
    Relative(Duration),
    Fixed(OffsetDateTime),
}

#[derive(Debug, Clone)]
pub struct MessageDefinition {
    text: String,
    created: OffsetDateTime,
    schedule: Schedule,
    recipients: Vec<String>,
}

impl FromStr for MessageDefinition {
    type Err = Error<Rule>;

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
            recipients: Vec::new(),
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
                            "cc" => def.recipients.push(value.to_string()),
                            _ => return Err(Error::UnknownAttributeKey(key.to_string())),
                        }
                    }
                }
                Rule::recipient => def.recipients.push(pair.as_span().as_str().to_string()),
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

#[derive(Parser)]
#[grammar = "message.pest"]
struct MessageDefinitionParser;

#[derive(Debug, Error)]
pub enum Error<R>
where
    R: RuleType,
{
    #[error("Failed to parse string as {rule}")]
    ParseRule {
        rule: R,
        source: pest::error::Error<R>,
    },

    #[error("String contains unparsed chars: {0:?}")]
    DanglingChars(String),

    #[error("Unknown attribute key: {0:?}")]
    UnknownAttributeKey(String),
}

#[cfg(test)]
mod test {
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

        assert_eq!(vec![String::from("recipient")], def.recipients);
        assert_eq!("actual message", &def.text);
        assert_eq!(Schedule::None, def.schedule);
    }

    #[test]
    fn parse_with_cc_attribute() {
        let def = "cc:\"other\" cc:foo recipient actual message"
            .parse::<MessageDefinition>()
            .unwrap();

        assert_eq!(
            vec!["other", "foo", "recipient"]
                .into_iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
            def.recipients
        );
        assert_eq!("actual message", &def.text);
        assert_eq!(Schedule::None, def.schedule);
    }
}
