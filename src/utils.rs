use std::process::Command;

pub fn do_ls(arg: &str) -> Vec<String> {
    let output = Command::new("find")
        .args(["-type", "f", "-L", arg])
        .output()
        .expect("failed to execute find");

    let output = String::from_utf8_lossy(&output.stdout);
    log::info!("find output: {output}");
    let files: Vec<String> = output.lines().map(|s| s.to_string()).collect();

    files
}
