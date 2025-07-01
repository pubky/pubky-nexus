use std::process::Command;

fn main() {
    // Get the latest commit hash
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("Failed to execute git");

    let git_hash = String::from_utf8(output.stdout).expect("Invalid UTF-8 sequence");
    let git_hash = git_hash.trim();

    // Write the commit hash to an environment variable
    println!("cargo:rustc-env=GIT_COMMIT_HASH={git_hash}");
}
