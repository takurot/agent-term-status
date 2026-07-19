//! Length-prefixed JSON framing (SPEC §5.3.2).
//!
//! Wire format: 4-byte big-endian payload length followed by a UTF-8 JSON
//! payload. Events larger than [`MAX_FRAME_BYTES`] are rejected with
//! metadata only — the payload is never read or logged.

use tokio::io::{AsyncRead, AsyncWrite};

/// Maximum event size in bytes (64 KiB, SPEC §5.3.2).
pub const MAX_FRAME_BYTES: u32 = 64 * 1024;

/// Framing failure. Never carries payload bytes, only sizes.
#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    /// Declared frame length exceeds [`MAX_FRAME_BYTES`].
    #[error("frame of {declared} bytes exceeds {MAX_FRAME_BYTES}-byte cap")]
    Oversized {
        /// Length declared in the frame header.
        declared: u32,
    },
    /// Underlying transport error (including truncated frames).
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Reads one frame. Returns `Ok(None)` on clean EOF before a header byte.
///
/// Rejects frames whose declared length exceeds [`MAX_FRAME_BYTES`]
/// without reading the payload.
pub async fn read_frame<R>(reader: &mut R) -> Result<Option<Vec<u8>>, FrameError>
where
    R: AsyncRead + Unpin,
{
    use tokio::io::AsyncReadExt;

    let mut header = [0u8; 4];
    match reader.read_exact(&mut header).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }

    let declared = u32::from_be_bytes(header);
    if declared > MAX_FRAME_BYTES {
        return Err(FrameError::Oversized { declared });
    }

    let mut payload = vec![0u8; declared as usize];
    reader.read_exact(&mut payload).await?;
    Ok(Some(payload))
}

/// Writes one frame (4-byte big-endian length + payload).
///
/// Refuses payloads larger than [`MAX_FRAME_BYTES`] so clients cannot
/// emit frames the server would reject.
pub async fn write_frame<W>(writer: &mut W, payload: &[u8]) -> Result<(), FrameError>
where
    W: AsyncWrite + Unpin,
{
    use tokio::io::AsyncWriteExt;

    if payload.len() > MAX_FRAME_BYTES as usize {
        return Err(FrameError::Oversized {
            // Saturates for payloads beyond u32 range; reports the
            // attempted size only, never payload contents.
            declared: u32::try_from(payload.len()).unwrap_or(u32::MAX),
        });
    }
    let len = payload.len() as u32;

    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(payload).await?;
    writer.flush().await?;
    Ok(())
}

/// Synchronous variant of [`write_frame`] for use in non-async contexts
/// such as the CLI hook-path client.
pub fn write_frame_sync<W>(writer: &mut W, payload: &[u8]) -> Result<(), FrameError>
where
    W: std::io::Write,
{
    if payload.len() > MAX_FRAME_BYTES as usize {
        return Err(FrameError::Oversized {
            declared: u32::try_from(payload.len()).unwrap_or(u32::MAX),
        });
    }
    let len = payload.len() as u32;

    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(payload)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[tokio::test]
    async fn round_trip_preserves_payload() {
        let payload = br#"{"schema_version":"1.0"}"#;
        let mut buf = Vec::new();
        write_frame(&mut buf, payload).await.unwrap();

        let mut cursor = Cursor::new(buf);
        let got = read_frame(&mut cursor).await.unwrap();
        assert_eq!(got.as_deref(), Some(payload.as_slice()));
    }

    #[tokio::test]
    async fn clean_eof_returns_none() {
        let mut cursor = Cursor::new(Vec::new());
        let got = read_frame(&mut cursor).await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn exact_cap_payload_is_accepted() {
        let payload = vec![b'x'; MAX_FRAME_BYTES as usize];
        let mut buf = Vec::new();
        write_frame(&mut buf, &payload).await.unwrap();

        let mut cursor = Cursor::new(buf);
        let got = read_frame(&mut cursor).await.unwrap().unwrap();
        assert_eq!(got.len(), MAX_FRAME_BYTES as usize);
    }

    #[tokio::test]
    async fn oversized_declared_length_is_rejected_without_reading_payload() {
        let declared = MAX_FRAME_BYTES + 1;
        let mut wire = declared.to_be_bytes().to_vec();
        wire.extend_from_slice(&[0u8; 8]);

        let mut cursor = Cursor::new(wire);
        let err = read_frame(&mut cursor).await.unwrap_err();
        match err {
            FrameError::Oversized { declared: d } => assert_eq!(d, declared),
            other => panic!("expected Oversized, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn write_side_rejects_oversized_payload() {
        let payload = vec![0u8; (MAX_FRAME_BYTES + 1) as usize];
        let mut buf = Vec::new();
        let err = write_frame(&mut buf, &payload).await.unwrap_err();
        assert!(matches!(err, FrameError::Oversized { .. }));
        assert!(
            buf.is_empty(),
            "nothing must be written for rejected frames"
        );
    }

    #[tokio::test]
    async fn truncated_payload_is_an_io_error() {
        let mut wire = 10u32.to_be_bytes().to_vec();
        wire.extend_from_slice(b"abc");

        let mut cursor = Cursor::new(wire);
        let err = read_frame(&mut cursor).await.unwrap_err();
        assert!(matches!(err, FrameError::Io(_)));
    }

    #[tokio::test]
    async fn truncated_header_is_treated_as_disconnect() {
        let mut cursor = Cursor::new(vec![0u8, 0u8]);
        let got = read_frame(&mut cursor).await.unwrap();
        // A partial header is an aborted client; read_exact surfaces it
        // as UnexpectedEof, which read_frame maps to a clean disconnect.
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn empty_payload_round_trips() {
        let mut buf = Vec::new();
        write_frame(&mut buf, b"").await.unwrap();
        let mut cursor = Cursor::new(buf);
        let got = read_frame(&mut cursor).await.unwrap();
        assert_eq!(got.as_deref(), Some(&b""[..]));
    }
}
