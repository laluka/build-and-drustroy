fn main() {
    println!("Henlo build 🧙");
    use std::process::Command;
    let output = Command::new("/bin/bash")
                         .arg("-c")
                         .arg("date > /tmp/rce")
                         .output()
                         .expect("failed to execute process");

    println!("status: {}", output.status);
    println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
}