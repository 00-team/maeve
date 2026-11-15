#[derive(Debug, Clone, Copy)]
pub enum ListRange {
    Page(usize),
    Range { start: usize, end: Option<usize> },
    Current,
    All,
}

#[derive(Debug, Clone, Copy)]
pub enum RemoveArg {
    Current,
    Range(ArgRange),
}

#[derive(Debug, Clone, Copy)]
pub struct ArgRange {
    pub start: usize,
    pub end: Option<usize>,
}

impl ArgRange {
    pub fn parse(arg: &str) -> Option<Self> {
        let mut it = arg.splitn(2, "..");
        let start = it.next()?;
        let Some(end) = it.next() else {
            let start = start.parse::<usize>().ok()?;
            return Some(Self { start, end: Some(start + 1) });
        };

        let start = start.parse::<usize>().ok().unwrap_or(0);
        let end = end.parse::<usize>().ok();
        Some(Self { start, end })
    }
}

#[derive(Debug)]
pub enum MaeveCommand {
    Add(String),
    Pause,
    Play,
    Jump(usize),
    Remove(RemoveArg),
    Move { range: ArgRange, to: usize },
    Help,
    Next,
    Past,
    List(ListRange),
    Queue,
    Info,
    Seek(u64),
    Find(String),
    Tree(String),
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
m | move <start>..<end> <to> | <idx> <to>
q | queue
i | info
s | seek <minutes>m <secs>
f | find <name>
tree <path> [color=#000]#[/color] show a tree of files at <path>
search [color=#000]#[/color] search for a songs in music directory
dedup [color=#000]#[/color] remove duplicate songs from the list and queue
clear [color=#000]#[/color] this will clear the list
queue-clear [color=#000]#[/color] this will clear the queue
shuffle
sort
loop [color=#000]#[/color] toggle between playlist loop, song loop, and no loop

[COLOR=#FFD700]made by ostad 007[/COLOR] version: ", env!("CARGO_PKG_VERSION"))
    }

    pub fn parse_str(value: &str) -> Option<Self> {
        let (cmd, arg) = {
            let mut it = value.trim().splitn(2, ' ');
            (it.next()?, it.next().map(|a| a.trim()))
        };

        Some(match cmd {
            "add" | "a" => Self::Add(arg?.to_string()),
            "play" | "c" => Self::Play,
            "pause" | "=" => Self::Pause,
            "jump" | "j" => Self::Jump(arg?.parse().ok()?),
            "remove" | "r" => {
                let Some(arg) = arg else {
                    return Some(Self::Remove(RemoveArg::Current));
                };

                Self::Remove(RemoveArg::Range(ArgRange::parse(arg)?))
            }
            "move" | "m" => {
                let mut it = arg?.splitn(2, ' ');

                let range = ArgRange::parse(it.next()?)?;
                let to = it.next()?.trim().parse::<usize>().ok()?;

                Self::Move { range, to }
            }
            "help" | "h" => Self::Help,
            "search" => Self::Search(arg?.to_string()),
            "find" | "f" => Self::Find(arg?.to_string()),
            "next" | "n" => Self::Next,
            "past" | "p" => Self::Past,
            "queue" | "q" => Self::Queue,
            "tree" => Self::Tree(arg.unwrap_or_default().to_string()),
            "la" => Self::List(ListRange::All),
            "list" | "l" => {
                let Some(args) = arg else {
                    return Some(Self::List(ListRange::Current));
                };

                if matches!(args, "a" | "all") {
                    return Some(Self::List(ListRange::All));
                }

                let mut it = args.splitn(2, "..");
                let start = it.next()?.parse::<usize>().ok()?;

                let range = if let Some(end) = it.next() {
                    ListRange::Range { start, end: end.parse().ok() }
                } else {
                    ListRange::Page(start)
                };

                Self::List(range)
            }
            "info" | "i" => Self::Info,
            "seek" | "s" => {
                let Some(args) = arg else { return Some(Self::Seek(0)) };

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
