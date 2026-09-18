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
}

impl MaeveCommand {
    pub fn help() -> &'static str {
        "
Maeve commands:

!add <song-name>
!jump <index>
!remove <index>
!play
!pause
!next
!past
!list

made by ostad 007
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
            _ => return None,
        })
    }
}
