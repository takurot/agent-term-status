use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ats_config::Config;
use tracing_subscriber::{fmt, prelude::*};

#[derive(Debug, thiserror::Error)]
pub enum LogInitError {
    #[error("failed to create log directory: {0}")]
    Io(#[from] io::Error),
}

pub fn init_logging(logs_dir: &Path, config: &Config) -> Result<(), LogInitError> {
    std::fs::create_dir_all(logs_dir)?;

    let file_appender = tracing_appender::rolling::daily(logs_dir, "ats-daemon.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    Box::leak(Box::new(guard));

    let env_filter = tracing_subscriber::EnvFilter::try_from_env("ATS_LOG").unwrap_or_else(|_| {
        let level = match config.logging.level {
            ats_config::config::LogLevel::Trace => "trace",
            ats_config::config::LogLevel::Debug => "debug",
            ats_config::config::LogLevel::Info => "info",
            ats_config::config::LogLevel::Warn => "warn",
            ats_config::config::LogLevel::Error => "error",
        };
        tracing_subscriber::EnvFilter::new(format!("ats_daemon={level}"))
    });

    let home = dirs::home_dir();
    let redact = config.logging.redact;
    let writer = RedactingWriter::new(non_blocking, redact, home);
    let writer = Mutex::new(writer);

    let json_layer = fmt::layer()
        .json()
        .with_writer(writer)
        .with_target(false)
        .with_current_span(false)
        .with_thread_ids(false)
        .with_thread_names(false);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(json_layer)
        .init();

    Ok(())
}

pub struct RedactingWriter<W: Write + Send + 'static> {
    inner: W,
    enabled: bool,
    home_dir: Option<PathBuf>,
}

impl<W: Write + Send + 'static> RedactingWriter<W> {
    fn new(inner: W, enabled: bool, home_dir: Option<PathBuf>) -> Self {
        Self {
            inner,
            enabled,
            home_dir,
        }
    }
}

impl<W: Write + Send + 'static> Write for RedactingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.enabled {
            return self.inner.write(buf);
        }

        let s = String::from_utf8_lossy(buf);
        let redacted = redact_string(&s, self.home_dir.as_deref());
        let redacted_bytes = redacted.as_bytes();
        self.inner.write_all(redacted_bytes)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn redact_string(s: &str, home_dir: Option<&Path>) -> String {
    let mut result = sanitize_control_chars(s);

    if let Some(home) = home_dir {
        if let Some(home_str) = home.to_str() {
            result = result.replace(home_str, "~");
        }
    }

    result
}

fn sanitize_control_chars(s: &str) -> String {
    s.chars()
        .filter(|c| {
            let code = *c as u32;
            if code <= 0x1F || code == 0x7F {
                return false;
            }
            if (0x80..=0x9F).contains(&code) {
                return false;
            }
            if (0xD800..=0xDFFF).contains(&code) {
                return false;
            }
            if code == 0xFFFD {
                return false;
            }
            true
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_c0_control_chars() {
        let input = "hello\x00world\x1bmore";
        let result = sanitize_control_chars(input);
        assert_eq!(result, "helloworldmore");
    }

    #[test]
    fn sanitize_strips_del() {
        let input = "test\x7fdata";
        let result = sanitize_control_chars(input);
        assert_eq!(result, "testdata");
    }

    #[test]
    fn sanitize_strips_c1_chars() {
        let input = "\u{0080}foo\u{009F}bar";
        let result = sanitize_control_chars(input);
        assert_eq!(result, "foobar");
    }

    #[test]
    fn redacting_writer_handles_invalid_utf8() {
        let mut writer = RedactingWriter::new(Vec::new(), true, None);
        writer
            .write_all(&[
                b'h', b'e', b'l', b'l', b'o', 0xC0, 0x80, b'w', b'o', b'r', b'l', b'd',
            ])
            .unwrap();
        let output = String::from_utf8_lossy(&writer.inner);
        assert!(
            !output.contains('\u{FFFD}'),
            "replacement chars should be filtered"
        );
    }

    #[test]
    fn sanitize_preserves_printable_ascii() {
        let input = "Hello, world! 123 ABC @#$%";
        let result = sanitize_control_chars(input);
        assert_eq!(result, input);
    }

    #[test]
    fn sanitize_preserves_unicode() {
        let input = "日本語テスト";
        let result = sanitize_control_chars(input);
        assert_eq!(result, input);
    }

    #[test]
    fn redact_replace_home_path() {
        let home = Path::new("/Users/testuser");
        let input = "File at /Users/testuser/project/file.txt";
        let result = redact_string(input, Some(home));
        assert_eq!(result, "File at ~/project/file.txt");
    }

    #[test]
    fn redact_multiple_home_occurrences() {
        let home = Path::new("/Users/testuser");
        let input = "/Users/testuser/src and /Users/testuser/docs";
        let result = redact_string(input, Some(home));
        assert_eq!(result, "~/src and ~/docs");
    }

    #[test]
    fn redact_without_home_dir_no_change() {
        let input = "/Users/testuser/file.txt";
        let result = redact_string(input, None);
        assert_eq!(result, input);
    }

    #[test]
    fn redact_no_false_replacement() {
        let home = Path::new("/Users/testuser");
        let input = "User: testuser at /Users/testuser/src";
        let result = redact_string(input, Some(home));
        assert_eq!(result, "User: testuser at ~/src");
    }

    #[test]
    fn redacting_writer_passes_unredacted_when_disabled() {
        let mut writer =
            RedactingWriter::new(Vec::new(), false, Some(PathBuf::from("/Users/test")));
        write!(writer, "/Users/test/file").unwrap();
        assert_eq!(String::from_utf8(writer.inner).unwrap(), "/Users/test/file");
    }

    #[test]
    fn redacting_writer_redacts_when_enabled() {
        let mut writer = RedactingWriter::new(Vec::new(), true, Some(PathBuf::from("/Users/test")));
        write!(writer, "/Users/test/file").unwrap();
        assert_eq!(String::from_utf8(writer.inner).unwrap(), "~/file");
    }

    #[test]
    fn redacting_writer_handles_control_chars() {
        let mut writer = RedactingWriter::new(Vec::new(), true, Some(PathBuf::from("/Users/test")));
        write!(writer, "hello\x00world\x1bmore").unwrap();
        assert_eq!(String::from_utf8(writer.inner).unwrap(), "helloworldmore");
    }
}
