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
                for filename in map.keys() {
                    if !filename.ends_with(".rs") || filename.contains("..") {
                        let mut response = Response::new(full("Grrrrr(ust)."));
                        *response.status_mut() = StatusCode::FORBIDDEN;
                        return Ok(response);
                    }
                    println!("filename: {}", filename);
                    // curl -H 'Content-Type: application/json' http://127.0.0.1:3000/remote-build -d '{"src/main.rs":"foo","build.rs":"bar"}'
                    // TODO create temp directory
                    // TODO write files form map on the temp directory
                    // TODO run cargo build
                    // TODO Finally serve the binary
                    // TODO Finally remove the temp directory
                }
            } else {
                println!("Expected an object in JSON blob");
            }

            let mut response = Response::new(full(["Work done, TODO serve final bin"].concat()));
            response.headers_mut().insert("Content-Type", "text/plain".parse().unwrap());
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
