//! Fork: WebSocket sobre socket Unix, sem listener TCP nem daemon global.
//! O `app-server proxy` 0.163.0 só retransmite bytes: não é um adaptador JSONL.
use super::*;
use omni_core::codex_protocolo::MENSAGEM_MAX;

pub(super) fn quadro(writer: &mut dyn Write, opcode: u8, bytes: &[u8]) -> Result<()> {
    let mascara: [u8; 4] = rand::random();
    writer.write_all(&[0x80 | opcode])?;
    match bytes.len() {
        n @ 0..=125 => writer.write_all(&[0x80 | n as u8])?,
        n @ 126..=65535 => {
            writer.write_all(&[0xfe])?;
            writer.write_all(&(n as u16).to_be_bytes())?;
        }
        n => {
            writer.write_all(&[0xff])?;
            writer.write_all(&(n as u64).to_be_bytes())?;
        }
    }
    writer.write_all(&mascara)?;
    let dados: Vec<_> = bytes
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ mascara[i % 4])
        .collect();
    writer.write_all(&dados)?;
    writer.flush()?;
    Ok(())
}

pub(super) fn receber(
    reader: &mut dyn Read,
    writer: &Mutex<Box<dyn Write + Send>>,
) -> Result<Vec<u8>> {
    let mut texto = Vec::new();
    let mut fragmentado = false;
    loop {
        let mut cab = [0; 2];
        reader.read_exact(&mut cab)?;
        let op = cab[0] & 15;
        let fim = cab[0] & 128 != 0;
        anyhow::ensure!(
            cab[0] & 0x70 == 0 && cab[1] & 128 == 0,
            "Quadro WebSocket inválido"
        );
        let tamanho = match cab[1] & 127 {
            126 => {
                let mut n = [0; 2];
                reader.read_exact(&mut n)?;
                u16::from_be_bytes(n) as u64
            }
            127 => {
                let mut n = [0; 8];
                reader.read_exact(&mut n)?;
                u64::from_be_bytes(n)
            }
            n => n as u64,
        };
        anyhow::ensure!(
            tamanho <= MENSAGEM_MAX as u64 && texto.len() as u64 + tamanho <= MENSAGEM_MAX as u64,
            "Quadro grande demais"
        );
        if op >= 8 {
            anyhow::ensure!(fim && tamanho <= 125, "Controle WebSocket inválido");
        }
        let mut dados = vec![0; tamanho as usize];
        reader.read_exact(&mut dados)?;
        match op {
            8 => return Err(anyhow!("Socket encerrado")),
            9 => quadro(
                &mut **writer.lock().map_err(|_| anyhow!("writer poisoned"))?,
                10,
                &dados,
            )?,
            10 => {}
            1 if !fragmentado => {
                texto.extend(dados);
                if fim {
                    return Ok(texto);
                }
                fragmentado = true;
            }
            0 if fragmentado => {
                texto.extend(dados);
                if fim {
                    return Ok(texto);
                }
            }
            _ => return Err(anyhow!("Opcode WebSocket inesperado")),
        }
    }
}

#[cfg(unix)]
pub(super) fn conectar(socket: &Path) -> Result<(Box<dyn Read + Send>, Box<dyn Write + Send>)> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use std::os::unix::net::UnixStream;
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;
    let chave = STANDARD.encode(rand::random::<[u8; 16]>());
    write!(stream,"GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {chave}\r\nSec-WebSocket-Version: 13\r\n\r\n")?;
    stream.flush()?;
    let mut reader = std::io::BufReader::new(stream.try_clone()?);
    let mut cab = Vec::new();
    loop {
        let mut b = [0];
        reader.read_exact(&mut b)?;
        cab.push(b[0]);
        anyhow::ensure!(cab.len() <= 16_384, "Handshake grande demais");
        if cab.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let cab = std::str::from_utf8(&cab)?;
    validar_handshake(cab, &chave)?;
    stream.set_read_timeout(None)?;
    Ok((Box::new(reader), Box::new(stream)))
}

fn validar_handshake(cab: &str, chave: &str) -> Result<()> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use sha1::{Digest, Sha1};
    let header = |nome: &str| {
        cab.lines()
            .filter_map(|l| l.split_once(':'))
            .find(|(k, _)| k.eq_ignore_ascii_case(nome))
            .map(|(_, v)| v.trim())
    };
    anyhow::ensure!(
        header("Upgrade").is_some_and(|v| v.eq_ignore_ascii_case("websocket"))
            && header("Connection").is_some_and(|v| v
                .split(',')
                .any(|p| p.trim().eq_ignore_ascii_case("Upgrade"))),
        "Upgrade WebSocket inválido"
    );
    anyhow::ensure!(
        cab.lines()
            .next()
            .is_some_and(|l| l.starts_with("HTTP/1.1 101 ")),
        "Handshake recusado"
    );
    let esperado = STANDARD.encode(Sha1::digest(format!(
        "{chave}258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
    )));
    anyhow::ensure!(
        cab.lines().any(|l| l.split_once(':').is_some_and(|(k, v)| k
            .eq_ignore_ascii_case("Sec-WebSocket-Accept")
            && v.trim() == esperado)),
        "Handshake inválido"
    );
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    #[cfg(unix)]
    #[test]
    #[ignore = "Integração requer listen Unix, bloqueado pelo sandbox; parser e quadros têm testes sem socket"]
    fn handshake_unix_sintetico_confere_desafio_e_recebe_texto() {
        use base64::{engine::general_purpose::STANDARD, Engine};
        use sha1::{Digest, Sha1};
        use std::{io::BufRead, os::unix::net::UnixListener};
        let dir = tempfile::tempdir_in("/tmp").unwrap();
        let socket = dir.path().join("s");
        let listener = UnixListener::bind(&socket).unwrap();
        let servidor = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut chave = String::new();
            loop {
                let mut l = String::new();
                reader.read_line(&mut l).unwrap();
                if l == "\r\n" {
                    break;
                }
                if let Some(v) = l.strip_prefix("Sec-WebSocket-Key: ") {
                    chave = v.trim().into();
                }
            }
            let esperado = STANDARD.encode(Sha1::digest(format!(
                "{chave}258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
            )));
            write!(stream,"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {esperado}\r\n\r\n").unwrap();
            stream.write_all(b"\x81\x02{}").unwrap();
            stream.flush().unwrap();
        });
        let (mut reader, writer) = conectar(&socket).unwrap();
        let writer = Mutex::new(writer);
        assert_eq!(receber(&mut *reader, &writer).unwrap(), b"{}");
        servidor.join().unwrap();
    }
    #[test]
    fn handshake_confere_vetor_do_rfc6455_e_rejeita_desafio_errado() {
        // RFC 6455 §1.3, exemplo publicado (chave e aceite não são credenciais).
        let resposta="HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";
        assert!(validar_handshake(resposta, "dGhlIHNhbXBsZSBub25jZQ==").is_ok());
        assert!(validar_handshake(resposta, "outra-chave").is_err());
        assert!(validar_handshake(
            &resposta.replace("Upgrade: websocket\r\n", ""),
            "dGhlIHNhbXBsZSBub25jZQ=="
        )
        .is_err());
        assert!(validar_handshake(
            &resposta.replace("101 Switching Protocols", "403 Forbidden"),
            "dGhlIHNhbXBsZSBub25jZQ=="
        )
        .is_err());
    }
    #[test]
    fn ping_fragmentos_e_limite() {
        let bytes = [
            0x01, 2, b'{', b'"', 0x89, 1, b'p', 0x80, 4, b'x', b'"', b':', b'}',
        ];
        let writer: Mutex<Box<dyn Write + Send>> = Mutex::new(Box::new(Vec::<u8>::new()));
        assert_eq!(receber(&mut &bytes[..], &writer).unwrap(), br#"{"x":}"#);
        assert!(receber(&mut &[0x81, 127, 0, 0, 0, 0, 1, 0, 0, 0][..], &writer).is_err());
        assert!(receber(&mut &[0x80, 0][..], &writer).is_err());
    }
    #[test]
    fn cliente_envia_mascara() {
        let mut bytes = Vec::new();
        quadro(&mut bytes, 1, b"abc").unwrap();
        assert_eq!(&bytes[..2], &[0x81, 0x83]);
        assert_eq!(
            (0..3)
                .map(|i| bytes[6 + i] ^ bytes[2 + i % 4])
                .collect::<Vec<_>>(),
            b"abc"
        );
    }
}
