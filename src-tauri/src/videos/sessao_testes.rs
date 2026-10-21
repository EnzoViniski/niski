use super::*;
use super::super::sessao_http::{self, Host};
use std::io::Write as _;
use std::net::TcpStream;
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn convite_vai_e_volta_e_recusa_lixo() {
    for dados in [&b""[..], b"a", b"ab", b"abc", b"abcd", "çãõ€".as_bytes()] {
        assert_eq!(de_base64url(&base64url(dados)).unwrap(), dados);
    }
    let c = Convite { v: 1, bases: vec!["http://100.64.0.1:47480".into()], t: "0123456789abcdef0123".into(), p: "v-abc-1".into(), n: "Enzo".into() };
    assert_eq!(ler_convite(&montar_convite(&c)).unwrap(), c);
    assert!(ler_convite("qualquer coisa").is_err());
    assert!(ler_convite("medza-video:%%%").is_err());
    let ruim = Convite { bases: vec!["https://publico.exemplo.com".into()], ..c.clone() };
    assert!(ler_convite(&montar_convite(&ruim)).is_err(), "só http dentro da tailnet");
    let ruim = Convite { p: "../fora".into(), ..c };
    assert!(ler_convite(&montar_convite(&ruim)).is_err());
}

#[test]
fn serve_so_na_tailnet_nunca_funnel() {
    let liga = argumentos_do_serve(5555).join(" ");
    assert_eq!(liga, "serve --bg --yes --http=47480 http://127.0.0.1:5555");
    assert!(!liga.contains("funnel"));
    assert_eq!(argumentos_do_serve_off().join(" "), "serve --http=47480 off");
}

/// Pedido cru ao servidor, como o `tailscale serve` faria (com ou sem o header de identidade).
fn cru(porta: u16, linha: &str, login: Option<&str>, corpo: &[u8]) -> (u16, String) {
    let mut s = TcpStream::connect(("127.0.0.1", porta)).unwrap();
    let cab = login.map(|l| format!("Tailscale-User-Login: {l}\r\n")).unwrap_or_default();
    write!(s, "{linha} HTTP/1.1\r\nHost: x\r\n{cab}Content-Length: {}\r\n\r\n", corpo.len()).unwrap();
    s.write_all(corpo).unwrap();
    let mut resposta = String::new();
    use std::io::Read;
    let _ = s.read_to_string(&mut resposta);
    let status = resposta.split_whitespace().nth(1).and_then(|x| x.parse().ok()).unwrap_or(0);
    (status, resposta)
}

fn host_de_teste(pasta: &std::path::Path) -> (Arc<Host>, mpsc::Receiver<sessao_http::EventoDaSessao>) {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let aviso: sessao_http::Aviso = Arc::new(move |e| { let _ = tx.lock().unwrap().send(e); });
    let host = Host::iniciar("v-teste-1", "Enzo", pasta, "token-de-teste-0123456789", vec!["primo@exemplo.com".into()], aviso).unwrap();
    (host, rx)
}

#[test]
fn host_exige_token_e_login_permitido() {
    let dir = tempfile::tempdir().unwrap();
    let (host, rx) = host_de_teste(dir.path());
    let ok = "GET /medza-video/ola?t=token-de-teste-0123456789";
    assert_eq!(cru(host.porta, "GET /ola?t=errado", Some("primo@exemplo.com"), b"").0, 401);
    assert_eq!(cru(host.porta, ok, None, b"").0, 401, "sem identidade do Tailscale");
    let (status, _) = cru(host.porta, ok, Some("estranho@exemplo.com"), b"");
    assert_eq!(status, 403);
    assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), sessao_http::EventoDaSessao::Pedido { login: "estranho@exemplo.com".into() });
    let (status, corpo) = cru(host.porta, ok, Some("Primo@Exemplo.com"), b"");
    assert_eq!(status, 200, "{corpo}");
    assert!(corpo.contains("\"projeto\":\"v-teste-1\""));
    host.permitir("estranho@exemplo.com");
    assert_eq!(cru(host.porta, ok, Some("estranho@exemplo.com"), b"").0, 200);
    host.parar();
}

#[test]
fn dois_convidados_trocam_mensagens_pelo_host() {
    let dir = tempfile::tempdir().unwrap();
    let (host, rx_do_host) = host_de_teste(dir.path());
    host.permitir("enzo@exemplo.com");
    // O convidado de verdade fala com o host pelo `tailscale serve`, que põe o header. No teste, um
    // proxy mínimo faz esse papel.
    let proxy = proxy_que_poe_login(host.porta, "primo@exemplo.com");
    let base = format!("http://127.0.0.1:{proxy}/medza-video");
    let a = sessao_http::Convidado::new(&base, "token-de-teste-0123456789", "cliente-a");
    assert_eq!(a.ola().unwrap()["nome"], "Enzo");
    let (tx_a, rx_a) = mpsc::channel();
    let tx_a = Mutex::new(tx_a);
    a.ouvir(Arc::new(move |e| { let _ = tx_a.lock().unwrap().send(e); }));
    // Espera o SSE registrar.
    for _ in 0..100 { if !host.convidados().is_empty() { break; } std::thread::sleep(Duration::from_millis(20)); }
    assert_eq!(host.convidados(), vec![("cliente-a".to_owned(), "primo@exemplo.com".to_owned())]);
    // Host → convidado.
    host.difundir("host", None, r#"{"t":"u","u":"AAEC"}"#, None);
    let recebido = rx_a.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(recebido, sessao_http::EventoDaSessao::Mensagem { de: "host".into(), login: None, corpo: r#"{"t":"u","u":"AAEC"}"#.into() });
    // Convidado → host (chega como evento para a tela do host).
    a.enviar(r#"{"t":"a","u":"BBB"}"#).unwrap();
    let mut achou = false;
    for _ in 0..10 {
        if let Ok(sessao_http::EventoDaSessao::Mensagem { de, login, corpo }) = rx_do_host.recv_timeout(Duration::from_secs(2)) {
            assert_eq!((de.as_str(), login.as_deref(), corpo.as_str()), ("cliente-a", Some("primo@exemplo.com"), r#"{"t":"a","u":"BBB"}"#));
            achou = true;
            break;
        }
    }
    assert!(achou);
    assert!(a.enviar("não é json").is_err());
    a.sair();
    host.parar();
}

#[test]
fn midia_sobe_e_desce_conferindo_o_hash() {
    let host_dir = tempfile::tempdir().unwrap();
    let convidado_dir = tempfile::tempdir().unwrap();
    let (host, _rx) = host_de_teste(host_dir.path());
    let proxy = proxy_que_poe_login(host.porta, "primo@exemplo.com");
    let base = format!("http://127.0.0.1:{proxy}");
    let c = sessao_http::Convidado::new(&base, "token-de-teste-0123456789", "c1");
    // Convidado sobe uma mídia nova.
    let assets_do_convidado = convidado_dir.path().join("assets");
    std::fs::create_dir_all(&assets_do_convidado).unwrap();
    std::fs::write(assets_do_convidado.join("broll.mp4"), vec![7u8; 300_000]).unwrap();
    let hash = super::super::arquivos::sha256_do_arquivo(&assets_do_convidado.join("broll.mp4")).unwrap();
    c.subir("assets/broll.mp4", &hash, &assets_do_convidado).unwrap();
    assert_eq!(std::fs::read(host_dir.path().join("assets/broll.mp4")).unwrap(), vec![7u8; 300_000]);
    // Outro convidado (pasta vazia) baixa a mesma mídia.
    let outro = tempfile::tempdir().unwrap();
    c.baixar("assets/broll.mp4", &hash, &outro.path().join("assets")).unwrap();
    assert_eq!(std::fs::read(outro.path().join("assets/broll.mp4")).unwrap().len(), 300_000);
    // Hash errado não grava; caminho para fora é recusado.
    let falso = "0".repeat(64);
    assert!(c.baixar("assets/broll.mp4", &falso, &tempfile::tempdir().unwrap().path().join("assets")).is_err());
    assert!(c.subir("assets/../fora.mp4", &hash, &assets_do_convidado).is_err());
    assert!(c.baixar("../fora", &hash, outro.path()).is_err());
    host.parar();
}

/// Proxy de teste: repassa a conexão para o host acrescentando `Tailscale-User-Login`, como o Serve.
fn proxy_que_poe_login(porta_do_host: u16, login: &'static str) -> u16 {
    use std::io::{BufRead, BufReader};
    let ouvinte = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for conexao in ouvinte.incoming() {
            let Ok(cliente) = conexao else { continue };
            std::thread::spawn(move || {
                let mut leitor = BufReader::new(cliente.try_clone().unwrap());
                let mut destino = TcpStream::connect(("127.0.0.1", porta_do_host)).unwrap();
                let mut linha = String::new();
                leitor.read_line(&mut linha).unwrap();
                destino.write_all(linha.as_bytes()).unwrap();
                write!(destino, "Tailscale-User-Login: {login}\r\n").unwrap();
                let mut volta = destino.try_clone().unwrap();
                let mut saida = cliente;
                std::thread::spawn(move || { let _ = std::io::copy(&mut volta, &mut saida); let _ = saida.shutdown(std::net::Shutdown::Both); });
                // Em fluxo (o cliente não fecha a escrita enquanto espera a resposta).
                let _ = std::io::copy(&mut leitor, &mut destino);
            });
        }
    });
    porta
}
