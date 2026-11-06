use std::fs;
use std::io;
use std::path::Path;

/// Neutralise BBCode so file names like "[b]x" can't inject formatting.
/// TeamSpeak has no escape syntax, so a zero-width space after '[' breaks the tag.
fn escape_bb(s: &str) -> String {
    s.replace('[', "[\u{200B}")
}

fn dir_entries(path: &Path) -> io::Result<Vec<(String, bool)>> {
    let mut entries: Vec<(String, bool)> = fs::read_dir(path)?
        .filter_map(|e| e.ok())
        .map(|e| {
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            (e.file_name().to_string_lossy().into_owned(), is_dir)
        })
        .collect();

    // Directories first, then alphabetical (case-insensitive)
    entries.sort_by(|a, b| {
        b.1.cmp(&a.1).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
    });
    Ok(entries)
}

fn fmt_name(name: &str, is_dir: bool) -> String {
    let name = escape_bb(name);
    if is_dir {
        format!("[color=#3399ff][b]{}/[/b][/color]", name)
    } else {
        name
    }
}

/// Builds a tree (max 2 levels deep) formatted with TeamSpeak BBCode.
pub fn teamspeak_tree<P: AsRef<Path>>(path: P) -> io::Result<String> {
    // Non-breaking space: TeamSpeak may trim/collapse regular leading spaces
    const NB: &str = "\u{00A0}\u{00A0}\u{00A0}\u{00A0}";

    let root = path.as_ref();
    let mut out = String::new();

    let root_name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());
    out.push_str(&format!(
        "\n[color=#ffaa00][b]{}/[/b][/color]\n",
        escape_bb(&root_name)
    ));

    let level1 = dir_entries(root)?;
    let count1 = level1.len();

    let is_music_root = root_name == "music/";

    for (i, (name, is_dir)) in level1.iter().enumerate() {
        let last1 = i + 1 == count1;
        let branch1 = if last1 { "└── " } else { "├── " };
        out.push_str(&format!("{}{}\n", branch1, fmt_name(name, *is_dir)));

        if is_music_root {
            continue;
        }

        if *is_dir && let Ok(level2) = dir_entries(&root.join(name)) {
            let count2 = level2.len();
            let indent = if last1 { NB } else { "│\u{00A0}\u{00A0}\u{00A0}" };

            for (j, (n2, d2)) in level2.iter().enumerate() {
                let branch2 =
                    if j + 1 == count2 { "└── " } else { "├── " };
                out.push_str(&format!(
                    "{}{}{}\n",
                    indent,
                    branch2,
                    fmt_name(n2, *d2)
                ));
            }
        }
    }

    Ok(out)
}
