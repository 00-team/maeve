#[derive(Debug)]
pub enum MaeveCommand {
    Add(String),
    Pause,
    Play,
    Jump(usize),
    Remove(usize),
    Help,
    Next,
    Past,
    List,
    Clear,
    QueueClear,
}

impl MaeveCommand {
    pub fn help() -> &'static str {
        "
Maeve commands:

!add <song-query> -- use like music/fadaei*
!jump <index>
!remove <index>
!play
!pause
!next
!past
!list
!clear -- this will clear the list
!queue-clear this will clear the queue

[COLOR=#FFD700]made by ostad 007[/COLOR]
        "
    }

    pub fn parse_str(value: &str) -> Option<Self> {
        if !value.starts_with('!') {
            return None;
        }

        let mut it = value[1..].splitn(2, ' ');
        let cmd = it.next()?;

        Some(match cmd {
            "add" => Self::Add(it.next()?.to_string()),
            "play" => Self::Play,
            "pause" => Self::Pause,
            "jump" => Self::Jump(it.next()?.parse().ok()?),
            "remove" => Self::Remove(it.next()?.parse().ok()?),
            "help" => Self::Help,
            "next" => Self::Next,
            "past" => Self::Past,
            "list" => Self::List,
            "clear" => Self::Clear,
            "queue-clear" => Self::QueueClear,
            _ => return None,
        })
    }
}
