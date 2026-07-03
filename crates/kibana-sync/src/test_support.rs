use crate::{Auth, KibanaClient, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use url::Url;

#[derive(Clone, Debug)]
pub(crate) struct MockResponse {
    pub method: &'static str,
    pub path: &'static str,
    pub status: u16,
    pub body: Value,
}

#[derive(Clone, Debug)]
pub(crate) struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

#[derive(Debug)]
pub(crate) struct TestServer {
    url: Url,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
}

impl TestServer {
    pub(crate) fn new(responses: Vec<MockResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("test server address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let thread_requests = requests.clone();

        thread::spawn(move || {
            for expected in responses {
                let Ok((stream, _)) = listener.accept() else {
                    break;
                };
                handle_connection(stream, expected, &thread_requests);
            }
        });

        Self {
            url: server_url(addr),
            requests,
        }
    }

    pub(crate) fn client(&self) -> Result<KibanaClient> {
        KibanaClient::builder(self.url.clone())
            .auth(Auth::None)
            .spaces([
                ("default".to_string(), "Default".to_string()),
                ("esdiag".to_string(), "Esdiag".to_string()),
            ])
            .max_concurrency(1)
            .build()
    }

    pub(crate) fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().expect("requests lock").clone()
    }
}

fn server_url(addr: SocketAddr) -> Url {
    Url::parse(&format!("http://{addr}/")).expect("server url")
}

fn handle_connection(
    mut stream: TcpStream,
    expected: MockResponse,
    requests: &Arc<Mutex<Vec<RecordedRequest>>>,
) {
    match read_request(&mut stream) {
        Ok(request) => {
            let matches = request.method == expected.method && request.path == expected.path;
            requests.lock().expect("requests lock").push(request);

            if matches {
                write_response(&mut stream, expected.status, &expected.body);
            } else {
                write_response(
                    &mut stream,
                    500,
                    &serde_json::json!({
                        "error": "unexpected request",
                        "expected": {"method": expected.method, "path": expected.path}
                    }),
                );
            }
        }
        Err(err) => {
            write_response(
                &mut stream,
                500,
                &serde_json::json!({"error": err.to_string()}),
            );
        }
    }
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<RecordedRequest> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    let header_end = loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break find_header_end(&bytes).unwrap_or(bytes.len());
        }
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(index) = find_header_end(&bytes) {
            break index;
        }
    };

    let headers_bytes = &bytes[..header_end];
    let headers_text = String::from_utf8_lossy(headers_bytes);
    let mut lines = headers_text.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default().to_string();
    let path = request_parts.next().unwrap_or_default().to_string();

    let mut headers = HashMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = header_end + 4;
    let mut body_bytes = bytes.get(body_start..).unwrap_or_default().to_vec();
    while body_bytes.len() < content_length {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        body_bytes.extend_from_slice(&buffer[..read]);
    }
    body_bytes.truncate(content_length);

    Ok(RecordedRequest {
        method,
        path,
        headers,
        body: String::from_utf8_lossy(&body_bytes).into_owned(),
    })
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn write_response(stream: &mut TcpStream, status: u16, body: &Value) {
    let body = serde_json::to_string(body).expect("serialize response body");
    let reason = match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        404 => "Not Found",
        409 => "Conflict",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}
