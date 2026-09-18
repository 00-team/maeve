use std::process::Command;

pub fn do_ls(arg: &str) -> Vec<String> {
    let output = Command::new("find")
        .args(["-type", "f", arg])
        .output()
        .expect("failed to execute ls");

    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect();

    files
}
