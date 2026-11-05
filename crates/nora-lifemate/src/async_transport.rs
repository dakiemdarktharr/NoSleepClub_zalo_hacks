//! Keep local service I/O progressing while the caller performs synchronous work.
use crate::Result;

pub(crate) fn runtime() -> Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Read, net::TcpListener, sync::mpsc, thread, time::Duration};

    #[test]
    fn timed_out_http_closes_its_connection_without_another_block_on() {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = mpsc::sync_channel(1);
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let bytes = stream.read(&mut buffer).unwrap();
                if bytes == 0 || request.len() > 8192 {
                    return;
                }
                request.extend_from_slice(&buffer[..bytes]);
            }
            // No response: the client must time out and close the socket.
            let closed = matches!(stream.read(&mut buffer), Ok(0));
            let _ = sender.send(closed);
        });
        let runtime = runtime().unwrap();
        let client = runtime.block_on(async {
            reqwest::Client::builder()
                .no_proxy()
                .pool_max_idle_per_host(0)
                .build()
                .unwrap()
        });
        let error = runtime
            .block_on(async {
                client
                    .get(format!("http://{address}/stall"))
                    .timeout(Duration::from_millis(100))
                    .send()
                    .await
            })
            .unwrap_err();
        assert!(error.is_timeout());
        // The actual caller is synchronous (e.g. executing ASR). It must not
        // need another model request to drive the old connection's cleanup.
        let closed_while_synchronous = receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap_or(false);
        drop(client);
        drop(runtime);
        server.join().unwrap();
        assert!(
            closed_while_synchronous,
            "HTTP cleanup froze outside block_on"
        );
    }
}
