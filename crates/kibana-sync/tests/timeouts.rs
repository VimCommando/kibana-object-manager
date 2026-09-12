use kibana_sync::{Error, KibanaClient};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;
use url::Url;

#[tokio::test]
async fn request_deadline_stops_a_stalled_response() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = [0; 4096];
        let _ = stream.read(&mut request);
        std::thread::sleep(Duration::from_millis(500));
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
    });
    let client = KibanaClient::builder(Url::parse(&format!("http://{address}")).unwrap())
        .request_timeout(Duration::from_millis(100))
        .build()
        .unwrap();
    let result = client.get("/api/status").await;
    server.join().unwrap();
    assert!(matches!(result, Err(Error::Transport(error)) if error.is_timeout()));
}

#[test]
fn zero_deadlines_are_rejected_before_connecting() {
    for builder in [
        KibanaClient::builder(Url::parse("http://localhost:5601").unwrap())
            .request_timeout(Duration::ZERO),
        KibanaClient::builder(Url::parse("http://localhost:5601").unwrap())
            .connect_timeout(Duration::ZERO),
    ] {
        assert!(matches!(
            builder.build(),
            Err(Error::InvalidConfiguration(_))
        ));
    }
}
