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
