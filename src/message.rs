use std::{str::FromStr, time::Duration};

#[derive(Debug)]
pub enum JoinMessage {
    NodeJoin,
    Client,
}

impl FromStr for JoinMessage {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        tracing::debug!(s);
        let open = s.split_once(' ');

        let (action, rest) = match open {
            Some(s) => s,
            None => return Err(()),
        };
        tracing::debug!(action);
        tracing::debug!(rest);

        if action.eq_ignore_ascii_case("JOINNODE") {
            return Ok(JoinMessage::NodeJoin);
        }
        if action.eq_ignore_ascii_case("CLIENT") {
            return Ok(JoinMessage::Client);
        }
        Err(())
    }
}

#[derive(Debug)]
pub enum ClientMessage {
    SetKey { key: String, dur: Duration }, // SET KEY_NAME DURATION
    SetValue { key: String, value: String },
    GetValue { key: String }, // GET KEY_NAME
}

const ONE_HOUR: u64 = 10;

impl FromStr for ClientMessage {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let mut s = input.split_whitespace();

        match s.next().ok_or(())? {
            "GET" => Ok(ClientMessage::GetValue {
                key: s.next().ok_or(())?.to_string(),
            }),
            "SET" => {
                let key = s.next().ok_or(())?.to_string();

                let dur = {
                    let s = s.next();
                    if let Some(s) = s {
                        let sec = s.parse::<u64>();

                        if sec.is_err() {
                            return Err(());
                        }

                        Duration::from_secs(sec.expect("unreachable"))
                    } else {
                        Duration::from_secs(ONE_HOUR)
                    }
                };

                Ok(ClientMessage::SetKey { key, dur })
            }
            _ => {
                // parse "Hello:jhsjdh"
                // where "Hello" is the key and "jhsjdh" is the value
                let (key, value) = input
                    .split_once(':')
                    .ok_or(())
                    .map(|(f, u)| (f.to_string(), u.to_string()))?;
                Ok(ClientMessage::SetValue { key, value })
            }
        }
    }
}
