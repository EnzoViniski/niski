//! Fork (Onda 2, item 3): modo statusline do `omni-shim`.
//!
//! O `--settings` da sessão (só com ganchos já confirmados e a conta com "Ler uso pela statusline")
//! põe `statusLine.command = '<omni-shim>' --medza-statusline <porta>`. A cada atualização:
//! 1. lê o stdin inteiro (o JSON que o Claude Code manda);
//! 2. `POST /statusline` em `127.0.0.1:<porta>` com o token da PTY (TCP cru, HTTP/1.1, no máximo
//!    300 ms no total — servidor mudo não atrasa a linha);
//! 3. roda a statusline do usuário (`MEDZA_STATUSLINE_DO_USUARIO`, resolvida pelo engine) com o mesmo
//!    stdin e repassa a saída; sem ela, imprime uma linha curta própria (modelo · contexto %).
//!
//! Nada é gravado nem logado aqui: o conteúdo só passa adiante.

use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub const TEMPO_MAXIMO: Duration = Duration::from_millis(300);

/// `argumentos`: o que vem depois de `--medza-statusline` (a porta).
pub fn rodar(argumentos: &[String], token: &str) -> i32 {
    let mut entrada = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut entrada);
    if let Some(porta) = argumentos.first().and_then(|p| p.parse::<u16>().ok()) {
        let _ = enviar(porta, token, &entrada, TEMPO_MAXIMO);
    }
    let (codigo, saida) = match std::env::var(omni_core::ganchos::VARIAVEL_DA_STATUSLINE_DO_USUARIO).ok().filter(|c| !c.trim().is_empty()) {
        Some(comando) => executar_do_usuario(&comando, &entrada).unwrap_or((1, Vec::new())),
        None => (0, linha_propria(&entrada).into_bytes()),
    };
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(&saida);
    let _ = stdout.flush();
    codigo
}

/// Repassa o JSON ao engine. Erro ou demora: desiste calado (a linha do usuário não pode esperar).
pub fn enviar(porta: u16, token: &str, corpo: &[u8], limite: Duration) -> std::io::Result<()> {
    let inicio = Instant::now();
    let resto = || limite.saturating_sub(inicio.elapsed()).max(Duration::from_millis(1));
    let mut conexao = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], porta)), limite)?;
    conexao.set_write_timeout(Some(resto()))?;
    write!(conexao, "POST /statusline HTTP/1.1\r\nHost: 127.0.0.1:{porta}\r\nAuthorization: Bearer {token}\r\n\
        Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", corpo.len())?;
    conexao.write_all(corpo)?;
    conexao.flush()?;
    // O recibo (o engine grava antes de responder); sem ele até o limite, segue.
    conexao.set_read_timeout(Some(resto()))?;
    let mut recibo = [0_u8; 32];
    let _ = conexao.read(&mut recibo);
    Ok(())
}

/// Roda a statusline do usuário com o mesmo stdin, como o Claude Code rodaria.
pub fn executar_do_usuario(comando: &str, entrada: &[u8]) -> std::io::Result<(i32, Vec<u8>)> {
    let mut processo = if cfg!(windows) {
        let mut c = Command::new("cmd"); c.args(["/C", comando]); c
    } else {
        let mut c = Command::new("sh"); c.args(["-c", comando]); c
    };
    let mut filho = processo.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).spawn()?;
    if let Some(mut stdin) = filho.stdin.take() { let _ = stdin.write_all(entrada); }
    let saida = filho.wait_with_output()?;
    Ok((saida.status.code().unwrap_or(1), saida.stdout))
}

/// Linha curta quando o usuário não tem statusline: "Opus · contexto 34%".
pub fn linha_propria(entrada: &[u8]) -> String {
    let valor = serde_json::from_slice(entrada).unwrap_or(serde_json::Value::Null);
    let medicao = omni_core::ganchos::medicao(&valor, 0);
    let partes: Vec<String> = [medicao.modelo, medicao.contexto_pct.map(|p| format!("contexto {}%", p.round()))]
        .into_iter().flatten().collect();
    if partes.is_empty() { "Niski".into() } else { partes.join(" · ") }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn manda_o_json_com_o_token_e_espera_o_recibo() {
        let servidor = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = servidor.local_addr().unwrap().port();
        let recebido = std::thread::spawn(move || {
            let (mut conexao, _) = servidor.accept().unwrap();
            conexao.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut tudo = Vec::new();
            let mut pedaco = [0_u8; 1024];
            while let Ok(n) = conexao.read(&mut pedaco) {
                if n == 0 { break; }
                tudo.extend_from_slice(&pedaco[..n]);
                if tudo.ends_with(b"}") { break; }
            }
            conexao.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").unwrap();
            String::from_utf8(tudo).unwrap()
        });
        enviar(porta, "tok-123", br#"{"model":{"display_name":"Opus"}}"#, Duration::from_secs(2)).unwrap();
        let pedido = recebido.join().unwrap();
        assert!(pedido.starts_with("POST /statusline HTTP/1.1\r\n"), "{pedido}");
        assert!(pedido.contains(&format!("Host: 127.0.0.1:{porta}\r\n")));
        assert!(pedido.contains("Authorization: Bearer tok-123\r\n"));
        assert!(pedido.ends_with(r#"{"model":{"display_name":"Opus"}}"#));
    }

    #[test]
    fn servidor_mudo_nao_segura_a_linha() {
        let servidor = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = servidor.local_addr().unwrap().port();
        let _mantem = std::thread::spawn(move || { let conexao = servidor.accept(); std::thread::sleep(Duration::from_secs(3)); drop(conexao); });
        let inicio = Instant::now();
        let _ = enviar(porta, "t", b"{}", TEMPO_MAXIMO);
        assert!(inicio.elapsed() < Duration::from_millis(900), "{:?}", inicio.elapsed());
        // Ninguém escutando: falha na hora, sem pânico.
        let livre = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        assert!(enviar(livre, "t", b"{}", TEMPO_MAXIMO).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn statusline_do_usuario_recebe_o_mesmo_stdin() {
        assert_eq!(executar_do_usuario("echo oi", b"{}").unwrap(), (0, b"oi\n".to_vec()));
        assert_eq!(executar_do_usuario("cat", br#"{"a":1}"#).unwrap().1, br#"{"a":1}"#.to_vec());
        assert_eq!(executar_do_usuario("exit 3", b"").unwrap().0, 3);
    }

    #[test]
    fn linha_propria_curta() {
        assert_eq!(linha_propria(br#"{"model":{"display_name":"Opus"},"context_window":{"used_percentage":33.6}}"#), "Opus · contexto 34%");
        assert_eq!(linha_propria(b"lixo"), "Niski");
    }
}
