use std::process::Command;

pub fn do_ls(arg: &str) -> Vec<String> {
    let output = Command::new("find")
        .args(["music/", "-type", "f", "-path", arg])
        .output()
        .expect("failed to execute find");

    let output = String::from_utf8_lossy(&output.stdout);
    let files: Vec<String> = output.lines().map(|s| s.to_string()).collect();

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
