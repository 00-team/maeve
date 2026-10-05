use std::{process::Command, time::Duration};

pub fn do_ls(arg: &str) -> Vec<String> {
    let output = Command::new("find")
        .args(["music/", "-type", "f", "-path", arg])
        .output()
        .expect("failed to execute find");

    let output = String::from_utf8_lossy(&output.stdout);
    let mut files: Vec<String> =
        output.lines().map(|s| s.to_string()).collect();

    files.sort();

    files
}

pub fn split_at_line(input: &str, max_bytes: usize) -> Vec<&str> {
    let mut chunks = Vec::with_capacity(24);
    let mut remaining = input;
    assert_ne!(max_bytes, 0, "cannot split by 0 size");

    while remaining.len() > max_bytes {
        let mut split_at = max_bytes;

        while !remaining.is_char_boundary(split_at) {
            split_at -= 1;
        }

        if let Some(pos) = remaining[..split_at].rfind('\n') {
            split_at = pos;
        }

        chunks.push(&remaining[..split_at]);
        remaining = &remaining[split_at..];
    }

    if !remaining.is_empty() {
        chunks.push(remaining);
    }

    chunks
}

pub fn fmt_dur(d: Duration) -> String {
    let secs = d.as_secs();

    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let secs = secs % 60;

    let ms = format!("{mins:02}\u{200B}:\u{200B}{secs:02}");
    if hours > 0 {
        return format!("{hours:02}\u{200B}:\u{200B}{ms}");
    }

    ms
}

pub fn fmt_megabytes(bytes: usize) -> String {
    let mb = bytes / (1024 * 1024);

    let mut result = String::new();
    for (i, c) in mb.to_string().chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }

    let cm = result.chars().rev().collect::<String>();
    format!("{cm} MB")
}
