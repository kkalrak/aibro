use crate::{
    agent::{Agent, Command},
    runtime::Work,
};
use anyhow::Result;
use serde_json::json;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use tiny_http::{Header, Response, Server, StatusCode};
pub struct Service {
    stop: Arc<AtomicBool>,
    connection: PathBuf,
}
impl Service {
    pub fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = std::fs::remove_file(self.connection);
    }
}
pub fn start(sender: mpsc::Sender<Work>, root: &Path) -> Result<Service> {
    let server = Server::http("127.0.0.1:0").map_err(|e| anyhow::anyhow!("{e}"))?;
    let address = server.server_addr().to_ip().unwrap();
    let token = uuid::Uuid::new_v4().to_string();
    let connection = root.join("connection.json");
    std::fs::write(
        &connection,
        serde_json::to_vec_pretty(
            &json!({"url":format!("http://{address}"),"token":token,"pid":std::process::id(),"protocol":1}),
        )?,
    )?;
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let log_path = root.join("actions.jsonl");
    std::thread::spawn(move || {
        let mut agent = Agent::new(sender);
        while !stopped.load(Ordering::Relaxed) {
            let mut request = match server.recv_timeout(Duration::from_millis(100)) {
                Ok(Some(r)) => r,
                Ok(None) => continue,
                Err(_) => break,
            };
            let auth = request.headers().iter().any(|h| {
                h.field.equiv("Authorization") && h.value.as_str() == format!("Bearer {token}")
            });
            let origin = request.headers().iter().any(|h| h.field.equiv("Origin"));
            if !auth || origin {
                let _ = request.respond(
                    Response::from_string(r#"{"ok":false,"error":"Unauthorized"}"#)
                        .with_status_code(StatusCode(401)),
                );
                continue;
            }
            if request.url() == "/health" && request.method().as_str() == "GET" {
                let _ = request.respond(Response::from_string(r#"{"ok":true,"protocol":1}"#));
                continue;
            }
            if request.url() != "/command" || request.method().as_str() != "POST" {
                let _ = request.respond(
                    Response::from_string(r#"{"ok":false,"error":"Use POST /command"}"#)
                        .with_status_code(StatusCode(404)),
                );
                continue;
            }
            let start = Instant::now();
            let id = uuid::Uuid::new_v4().to_string();
            let mut body = String::new();
            let parsed = request
                .as_reader()
                .take(1_048_577)
                .read_to_string(&mut body)
                .map_err(|e| e.to_string())
                .and_then(|_| {
                    if body.len() > 1_048_576 {
                        return Err("Request exceeds 1 MiB".into());
                    }
                    serde_json::from_str::<Command>(&body).map_err(|e| e.to_string())
                });
            let op = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v["op"].as_str().map(str::to_owned))
                .unwrap_or_default();
            let result =
                parsed.and_then(|command| agent.execute(command).map_err(|e| format!("{e:#}")));
            let success = result.is_ok();
            let response = match result {
                Ok(data) => json!({"ok":true,"id":id,"data":data}),
                Err(error) => json!({"ok":false,"id":id,"error":error}),
            };
            let record =
                json!({"id":id,"op":op,"ok":success,"elapsed_ms":start.elapsed().as_millis()});
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
            {
                let _ = writeln!(f, "{record}");
            }
            let response = Response::from_string(response.to_string())
                .with_status_code(StatusCode(if success { 200 } else { 400 }))
                .with_header(
                    Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap(),
                );
            let _ = request.respond(response);
        }
    });
    Ok(Service { stop, connection })
}
