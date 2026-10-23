use std::ops::Range;

#[derive(Debug, Clone)]
pub enum ListRange {
    Page(usize),
    Range(Range<usize>),
    Current,
    All,
}

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
    List(ListRange),
    Queue,
    Info,
    Seek(u64),
    Find(String),
    Dedup,
    Clear,
    QueueClear,
    Shuffle,
    Sort,
    Loop,
    Search(String),
}

impl MaeveCommand {
    pub fn help() -> &'static str {
        concat!("
Maeve commands:

a | add <song-query> [color=#000]#[/color] use like `a fadaei*` or `add classic/hayed*`
j | jump <index>
r | remove <index> | <start>..<end>
c | play -- toggle playing
= | pause
n | next
p | past
l | list <page> | <start>..<end> | a | all
q | queue
i | info
s | seek <minutes>m <secs>
f | find <name>
search [color=#000]#[/color] search for a songs in music directory
dedup [color=#000]#[/color] remove duplicate songs from the list and queue
clear [color=#000]#[/color] this will clear the list
queue-clear [color=#000]#[/color] this will clear the queue
shuffle
sort
loop [color=#000]#[/color] toggle between playlist loop, song loop, and no loop

[COLOR=#FFD700]made by ostad 007[/COLOR] version: 
        ", env!("CARGO_PKG_VERSION"))
    }

    pub fn parse_str(value: &str) -> Option<Self> {
        let mut it = value.trim().splitn(2, ' ');
        let cmd = it.next()?;

        Some(match cmd {
            "add" | "a" => Self::Add(it.next()?.to_string()),
            "play" | "c" => Self::Play,
            "pause" | "=" => Self::Pause,
            "jump" | "j" => Self::Jump(it.next()?.parse().ok()?),
            "remove" | "r" => {
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
            "help" | "h" => Self::Help,
            "search" => Self::Search(it.next()?.to_string()),
            "find" | "f" => Self::Find(it.next()?.to_string()),
            "next" | "n" => Self::Next,
            "past" | "p" => Self::Past,
            "queue" | "q" => Self::Queue,
            "la" => Self::List(ListRange::All),
            "list" | "l" => {
                let Some(args) = it.next() else {
                    return Some(Self::List(ListRange::Current));
                };

                if matches!(args, "a" | "all") {
                    return Some(Self::List(ListRange::All));
                }

                let mut it = args.splitn(2, "..");
                let start = it.next()?.parse::<usize>().ok()?;
                let end = it.next().and_then(|v| v.parse::<usize>().ok());
                let range = if let Some(end) = end {
                    ListRange::Range(start.min(end)..start.max(end) + 1)
                } else {
                    ListRange::Page(start)
                };

                Self::List(range)
            }
            "info" | "i" => Self::Info,
            "seek" | "s" => {
                let Some(args) = it.next() else {
                    return Some(Self::Seek(0));
                };

                let mut total_secs = 0u64;
                for s in args.splitn(2, ' ') {
                    total_secs += if let Some(m) = s.strip_suffix('m') {
                        m.parse::<u64>().ok()? * 60
                    } else {
                        s.parse::<u64>().ok()?
                    };
                }

                Self::Seek(total_secs)
            }
            "dedup" => Self::Dedup,
            "clear" => Self::Clear,
            "queue-clear" => Self::QueueClear,
            "shuffle" => Self::Shuffle,
            "sort" => Self::Sort,
            "loop" => Self::Loop,
            _ => Self::Help,
        })
    }
}
