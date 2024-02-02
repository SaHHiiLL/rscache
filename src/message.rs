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
enum ClientMessage {
    SetMessage { key: String, dur: Duration },
    GetMessage { key: String },
}
