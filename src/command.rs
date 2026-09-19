use std::ops::Range;

#[derive(Debug)]
pub enum MaeveCommand {
    Add(String),
    Pause,
    Play,
    Jump(usize),
    Remove(Range<usize>),
    Help,
    Next,
    Past,
    List,
    Clear,
    QueueClear,
    Shuffle,
    Sort,
    Loop,
}

impl MaeveCommand {
    pub fn help() -> &'static str {
        "
Maeve commands:

!add <song-query> -- use like music/fadaei*
!jump <index>
!remove <index> | <start>..<end>
!play -- toggle playing
!pause
!next
!past
!list
!clear -- this will clear the list
!queue-clear this will clear the queue
!shuffle
!sort
!loop -- toggle between playlist loop, song loop, and no loop

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
            "remove" => {
                let mut it = it.next()?.splitn(2, "..");
                let start = it.next()?.parse::<usize>().ok()?;
                let end = it.next().and_then(|v| v.parse::<usize>().ok());
                let range = if let Some(end) = end {
                    start.min(end)..start.max(end) + 1
                } else {
                    start..start + 1
                };

                Self::Remove(range)
            }
            "help" => Self::Help,
            "next" => Self::Next,
            "past" => Self::Past,
            "list" => Self::List,
            "clear" => Self::Clear,
            "queue-clear" => Self::QueueClear,
            "shuffle" => Self::Shuffle,
            "sort" => Self::Sort,
            "loop" => Self::Loop,
            _ => return None,
        })
    }
}
