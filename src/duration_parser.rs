use std::str::FromStr;

use pest::Parser;
use pest_derive::Parser;
use time::Duration;

macro_rules! handle_rule {
    ($duration:ident, $pair:ident, $( $rule:ident ),+) => {
        match dbg!($pair.as_rule()) {
            $(
                Rule::$rule => {
                    $duration.$rule += dbg!($pair.into_inner()).next().unwrap().as_str().parse::<u32>()?;
                }
            ,)+
            Rule::EOI => {
                let s = $pair.as_span().as_str();
                if !s.is_empty() {
                    return Err(Error::DanglingChars(s.to_string()));
                }
            }
            _ => {}
        }
    };
}

#[derive(Parser)]
#[grammar = "duration.pest"]
struct DurationParser;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to parse string as {rule:?}")]
    ParseRule {
        rule: Rule,
        source: pest::error::Error<Rule>,
    },

    #[error("String contains unparsed chars: {0:?}")]
    DanglingChars(String),

    #[error("Failed to parse integer: {0}")]
    ParseInt(#[from] std::num::ParseIntError),
}

#[derive(Debug, Default)]
pub struct IntermediateDuration {
    years: u32,
    months: u32,
    weeks: u32,
    days: u32,
    hours: u32,
    minutes: u32,
    seconds: u32,
}

impl FromStr for IntermediateDuration {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let duration_pair = DurationParser::parse(Rule::duration, s)
            .map_err(|source| Error::ParseRule {
                rule: Rule::duration,
                source,
            })?
            .next()
            .unwrap();

        let mut duration = IntermediateDuration::default();

        for pair in duration_pair.into_inner() {
            handle_rule!(duration, pair, years, months, weeks, days, hours, minutes, seconds);
        }

        Ok(duration)
    }
}

impl From<IntermediateDuration> for Duration {
    fn from(d: IntermediateDuration) -> Self {
        Duration::seconds(
            (d.years * 30_779_352
                + d.months * 2_564_946
                + d.weeks * 604_800
                + d.days * 86_400
                + d.hours * 3_600
                + d.minutes * 60
                + d.seconds) as i64,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use time::Duration;

    #[test]
    fn test_parser1() {
        let duration: Duration = "1y 123d 111d 1d 2s"
            .parse::<IntermediateDuration>()
            .unwrap()
            .into();

        assert_eq!(356 + 123 + 111 + 1, duration.whole_days());
    }

    #[test]
    fn test_parser2() {
        let duration: Duration = "1231234s".parse::<IntermediateDuration>().unwrap().into();

        assert_eq!(1231234, duration.whole_seconds());
    }
}
