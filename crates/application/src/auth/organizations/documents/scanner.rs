//! Antivírus externo com protocolo INSTREAM. Sem resposta conclusiva, quarentena.
use std::{net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScanVerdict {
    Clean,
    Prohibited,
    Unavailable,
}

pub(super) async fn scan(bytes: &[u8]) -> ScanVerdict {
    let address = std::env::var("CANTEIRO_DOCUMENT_SCANNER")
        .ok()
        .and_then(|v| v.parse::<SocketAddr>().ok());
    let Some(address) = address else {
        return ScanVerdict::Unavailable;
    };
    scan_at(address, bytes).await
}

async fn scan_at(address: SocketAddr, bytes: &[u8]) -> ScanVerdict {
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        let mut stream = TcpStream::connect(address).await?;
        stream.write_all(b"zINSTREAM\0").await?;
        for chunk in bytes.chunks(65_536) {
            stream
                .write_all(&(chunk.len() as u32).to_be_bytes())
                .await?;
            stream.write_all(chunk).await?;
        }
        stream.write_all(&0u32.to_be_bytes()).await?;
        stream.flush().await?;
        let mut response = Vec::new();
        loop {
            let b = stream.read_u8().await?;
            if b == 0 || b == b'\n' {
                break;
            }
            if response.len() >= 1024 {
                return Ok::<_, std::io::Error>(ScanVerdict::Unavailable);
            }
            response.push(b);
        }
        Ok(if response == b"stream: OK" {
            ScanVerdict::Clean
        } else if response.starts_with(b"stream: ") && response.ends_with(b" FOUND") {
            ScanVerdict::Prohibited
        } else {
            ScanVerdict::Unavailable
        })
    })
    .await;
    match result {
        Ok(Ok(verdict)) => verdict,
        _ => ScanVerdict::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn fixture(reply: &'static [u8]) -> SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut command = [0; 10];
            stream.read_exact(&mut command).await.unwrap();
            assert_eq!(&command, b"zINSTREAM\0");
            let mut received = Vec::new();
            loop {
                let size = stream.read_u32().await.unwrap();
                if size == 0 {
                    break;
                }
                let mut chunk = vec![0; size as usize];
                stream.read_exact(&mut chunk).await.unwrap();
                received.extend(chunk);
            }
            assert_eq!(received, b"conteudo ficticio");
            stream.write_all(reply).await.unwrap();
        });
        addr
    }
    #[tokio::test]
    async fn protocol_requires_exact_clean_result_and_transmits_original_bytes() {
        assert_eq!(
            scan_at(fixture(b"stream: OK\0").await, b"conteudo ficticio").await,
            ScanVerdict::Clean
        );
        assert_eq!(
            scan_at(
                fixture(b"stream: TEST-SIGNATURE FOUND\0").await,
                b"conteudo ficticio"
            )
            .await,
            ScanVerdict::Prohibited
        );
        assert_eq!(
            scan_at(fixture(b"stream: OK ERROR\0").await, b"conteudo ficticio").await,
            ScanVerdict::Unavailable
        );
    }
}
