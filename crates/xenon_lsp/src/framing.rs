use std::io::{BufRead, Write};

use crate::LspError;

pub fn read_message(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, LspError> {
    let mut content_length = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            return Ok(None);
        }
        if header == "\r\n" || header == "\n" {
            break;
        }
        let Some((name, value)) = header.split_once(':') else {
            return Err(LspError::Protocol("malformed LSP header".into()));
        };
        if name.eq_ignore_ascii_case("content-length") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| LspError::Protocol("invalid Content-Length".into()))?,
            );
        }
    }
    let length =
        content_length.ok_or_else(|| LspError::Protocol("missing Content-Length".into()))?;
    let mut body = vec![0; length];
    std::io::Read::read_exact(reader, &mut body)?;
    Ok(Some(body))
}

pub fn write_message(writer: &mut impl Write, body: &[u8]) -> Result<(), LspError> {
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(body)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};

    use super::*;

    #[test]
    fn round_trips_framed_message() {
        let mut bytes = Vec::new();
        write_message(&mut bytes, br#"{"jsonrpc":"2.0"}"#).unwrap();
        let mut reader = BufReader::new(Cursor::new(bytes));
        assert_eq!(
            read_message(&mut reader).unwrap(),
            Some(br#"{"jsonrpc":"2.0"}"#.to_vec())
        );
    }

    #[test]
    fn rejects_missing_length() {
        let mut reader = BufReader::new(Cursor::new(b"X-Test: yes\r\n\r\n"));
        assert!(matches!(
            read_message(&mut reader),
            Err(LspError::Protocol(_))
        ));
    }
}
