use std::process::Command;

pub fn do_ls(arg: &str) -> Vec<String> {
    let output = Command::new("find")
        .args([arg, "-type", "f"])
        .output()
        .expect("failed to execute find");

    let output = String::from_utf8_lossy(&output.stdout);
    let files: Vec<String> = output.lines().map(|s| s.to_string()).collect();

    files
}
