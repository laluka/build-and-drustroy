use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode, HeaderMap};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use serde_json::Value;
use tokio::signal;
use tempfile::tempdir; // Add this import
use tokio::process::Command; // Add this import
use tokio::fs;
// Removed: use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;
    println!("Server running on http://{}", addr);

    let shutdown_signal = signal::ctrl_c();

    tokio::select! {
        _ = async {
            loop {
                let (stream, _) = listener.accept().await?;
                let io = TokioIo::new(stream);

                tokio::task::spawn(async move {
                    if let Err(err) = http1::Builder::new()
                        .serve_connection(io, service_fn(echo))
                        .await
                    {
                        eprintln!("Error serving connection: {:?}", err);
                    }
                });
            }
            #[allow(unreachable_code)]
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        } => {},
        _ = shutdown_signal => {
            println!("Shutdown signal received.");
        },
    }

    println!("Server shutting down.");
    Ok(())
}

async fn echo(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => {
            let mut response = Response::new(full(
                "If you're here, you should read more code.",
            ));
            response.headers_mut().insert("Content-Type", "text/plain".parse().unwrap());
            Ok(response)
        },
        (&Method::POST, "/remote-build") => {
            // Validate Content-Type
            if !is_content_type_json(req.headers()) {
                let mut response = Response::new(full("Content-Type must be application/json"));
                *response.status_mut() = StatusCode::UNSUPPORTED_MEDIA_TYPE;
                return Ok(response);
            }

            let b: Bytes = req.collect().await?.to_bytes();
            let s: Value = match serde_json::from_slice(&b) {
                Ok(json) => json,
                Err(e) => {
                    eprintln!("JSON parsing error: {:?}", e);
                    let mut response = Response::new(full("Error parsing JSON"));
                    *response.status_mut() = StatusCode::BAD_REQUEST;
                    return Ok(response);
                },
            };

            if let Value::Object(map) = s {
                // Validate filenames
                for filename in map.keys() {
                    if !filename.ends_with(".rs") || filename.contains("..") {
                        let mut response = Response::new(full("Grrrrr(ust)."));
                        *response.status_mut() = StatusCode::FORBIDDEN;
                        return Ok(response);
                    }
                }

                // Create temp directory
                let temp_dir = match tempdir() {
                    Ok(dir) => dir,
                    Err(e) => {
                        eprintln!("Failed to create temp directory: {:?}", e);
                        let mut response = Response::new(full("Internal Server Error"));
                        *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                        return Ok(response);
                    },
                };

                // Create src directory
                let src_dir = temp_dir.path().join("src");
                if let Err(e) = fs::create_dir_all(&src_dir).await {
                    eprintln!("Failed to create src directory: {:?}", e);
                    let mut response = Response::new(full("Internal Server Error"));
                    *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                    return Ok(response);
                }

                // Write files to src directory
                for (filename, content) in &map {
                    // Ensure that the content is a string
                    let content_str = match content.as_str() {
                        Some(s) => s,
                        None => {
                            eprintln!("Content for file {} is not a string", filename);
                            let mut response = Response::new(full("Invalid content type for file"));
                            *response.status_mut() = StatusCode::BAD_REQUEST;
                            return Ok(response);
                        }
                    };

                    let file_path = temp_dir.path().join(filename);
                    if let Some(parent) = file_path.parent() {
                        if let Err(e) = fs::create_dir_all(parent).await {
                            eprintln!("Failed to create directories for {}: {:?}", filename, e);
                            let mut response = Response::new(full("Internal Server Error"));
                            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                            return Ok(response);
                        }
                    }
                    if let Err(e) = fs::write(&file_path, content_str.as_bytes()).await {
                        eprintln!("Failed to write file {}: {:?}", filename, e);
                        let mut response = Response::new(full("Internal Server Error"));
                        *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                        return Ok(response);
                    }
                }

                // Create Cargo.toml
                let cargo_toml = r#"
[package]
name = "temp_build"
version = "0.1.0"
edition = "2021"

[dependencies]
"#;
                if let Err(e) = fs::write(temp_dir.path().join("Cargo.toml"), cargo_toml).await {
                    eprintln!("Failed to write Cargo.toml: {:?}", e);
                    let mut response = Response::new(full("Internal Server Error"));
                    *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                    return Ok(response);
                }

                // Run cargo build --release
                let build_output = match Command::new("cargo")
                    .args(&["build", "--release"])
                    .current_dir(temp_dir.path())
                    .output()
                    .await
                {
                    Ok(output) => output,
                    Err(e) => {
                        eprintln!("Failed to execute cargo build: {:?}", e);
                        let mut response = Response::new(full("Internal Server Error"));
                        *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                        return Ok(response);
                    },
                };

                if !build_output.status.success() {
                    let stderr = String::from_utf8_lossy(&build_output.stderr);
                    eprintln!("Build failed: {}", stderr);
                    let mut response = Response::new(full(format!("Build failed: {}", stderr)));
                    *response.status_mut() = StatusCode::BAD_REQUEST;
                    return Ok(response);
                }

                // Determine binary name based on OS
                let binary_name = if cfg!(windows) {
                    "temp_build.exe"
                } else {
                    "temp_build"
                };

                let binary_path = temp_dir.path()
                    .join("target")
                    .join("release")
                    .join(binary_name);

                if !binary_path.exists() {
                    eprintln!("Built binary not found at {:?}", binary_path);
                    let mut response = Response::new(full("Built binary not found"));
                    *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                    return Ok(response);
                }

                // Read the binary file
                let binary = match fs::read(&binary_path).await {
                    Ok(data) => data,
                    Err(e) => {
                        eprintln!("Failed to read binary: {:?}", e);
                        let mut response = Response::new(full("Internal Server Error"));
                        *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                        return Ok(response);
                    },
                };

                // Optionally, you can drop the temp_dir here, but it's automatically cleaned up

                // Respond with the binary
                let response = Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/octet-stream")
                    .header("Content-Disposition", "attachment; filename=\"binary\"")
                    .body(full(binary))
                    .unwrap();

                return Ok(response);
            }

            let mut response = Response::new(full("Invalid JSON structure"));
            *response.status_mut() = StatusCode::BAD_REQUEST;
            Ok(response)
        }
        _ => {
            let mut not_found = Response::new(empty());
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

fn is_content_type_json(headers: &HeaderMap) -> bool {
    match headers.get("Content-Type") {
        Some(ct) => ct.to_str().map(|s| s.starts_with("application/json")).unwrap_or(false),
        None => false,
    }
}

fn empty() -> BoxBody<Bytes, hyper::Error> {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}
