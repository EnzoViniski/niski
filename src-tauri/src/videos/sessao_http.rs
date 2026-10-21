//! Fork (Editor de vídeo, edição a dois): HTTP/1.1 mínimo para a sessão a dois, sem dependência nova.
//!
//! Host: servidor só em `127.0.0.1`, que o `tailscale serve --http` expõe **só na tailnet** (nunca
//! Funnel). O Serve põe `Tailscale-User-Login` em cada pedido (também para quem recebeu a máquina
//! compartilhada de outra tailnet, KB 1312). Cada pedido precisa do token da sessão **e** de um login
//! da lista de permitidos. Login fora da lista vira um aviso "pedido" para o host permitir.
//!
//! Rotas (prefixo opcional `/medza-video`):
//! - `GET  /ola`               → `{projeto, nome, hyperframes}`
//! - `GET  /eventos?c=<id>`    → SSE com as mensagens dos outros (Yjs vai em base64 dentro do JSON)
//! - `POST /enviar?c=<id>`     → manda uma mensagem (até 4 MB) para todos os outros
//! - `GET  /midia?arquivo=&hash=` → baixa um arquivo de `assets/` (confere o hash)
//! - `POST /midia?arquivo=&hash=` → sobe um arquivo para `assets/` (grava num `.tmp`, confere o SHA-256)
//!
//! O WebSocket não aparece na documentação do Serve; SSE e POST são HTTP comum e passam por ele.

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const MENSAGEM_MAXIMA: usize = 4 * 1024 * 1024;
const MIDIA_MAXIMA: u64 = 8 * 1024 * 1024 * 1024;
const BATIDA: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum EventoDaSessao {
    /// Mensagem de outra pessoa (o corpo é do front).
    Mensagem { de: String, login: Option<String>, corpo: String },
    Entrou { cliente: String, login: String },
    Saiu { cliente: String, login: String },
    /// Alguém fora da lista tentou entrar (o host decide).
    Pedido { login: String },
    MidiaRecebida { arquivo: String },
    /// Convidado: a conexão caiu; o app tenta de novo sozinho.
    Caiu { erro: String },
    Voltou,
}

pub type Aviso = Arc<dyn Fn(EventoDaSessao) + Send + Sync>;

pub struct Pedido {
    pub metodo: String,
    pub caminho: String,
    pub consulta: HashMap<String, String>,
    pub cabecalhos: HashMap<String, String>,
}

fn decodificar(t: &str) -> String {
    let bytes = t.as_bytes();
    let mut saida = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => { saida.push(b' '); i += 1; }
            b'%' if i + 3 <= bytes.len() => match std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                Some(v) => { saida.push(v); i += 3; }
                None => { saida.push(b'%'); i += 1; }
            },
            b => { saida.push(b); i += 1; }
        }
    }
    String::from_utf8_lossy(&saida).into_owned()
}

pub fn codificar(t: &str) -> String {
    t.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// Lê a linha de pedido e os cabeçalhos (o corpo fica no leitor).
pub fn ler_cabeca(leitor: &mut BufReader<TcpStream>) -> io::Result<Pedido> {
    let mut linha = String::new();
    if leitor.read_line(&mut linha)? == 0 { return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "sem pedido")); }
    let mut partes = linha.split_whitespace();
    let metodo = partes.next().unwrap_or_default().to_owned();
    let alvo = partes.next().unwrap_or("/").to_owned();
    let mut cabecalhos = HashMap::new();
    for _ in 0..100 {
        let mut l = String::new();
        if leitor.read_line(&mut l)? == 0 { break; }
        let l = l.trim_end();
        if l.is_empty() { break; }
        if let Some((n, v)) = l.split_once(':') { cabecalhos.insert(n.trim().to_ascii_lowercase(), v.trim().to_owned()); }
    }
    let (caminho, q) = alvo.split_once('?').unwrap_or((&alvo, ""));
    let consulta = q.split('&').filter(|p| !p.is_empty()).map(|p| {
        let (k, v) = p.split_once('=').unwrap_or((p, ""));
        (decodificar(k), decodificar(v))
    }).collect();
    let caminho = caminho.strip_prefix("/medza-video").unwrap_or(caminho).to_owned();
    Ok(Pedido { metodo, caminho: if caminho.is_empty() { "/".into() } else { caminho }, consulta, cabecalhos })
}

fn escrever(stream: &mut TcpStream, status: u16, tipo: &str, corpo: &[u8]) -> io::Result<()> {
    let texto = match status { 200 => "OK", 204 => "No Content", 400 => "Bad Request", 401 => "Unauthorized", 403 => "Forbidden", 404 => "Not Found", 413 => "Payload Too Large", _ => "Error" };
    write!(stream, "HTTP/1.1 {status} {texto}\r\nContent-Type: {tipo}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", corpo.len())?;
    stream.write_all(corpo)?;
    stream.flush()
}

fn erro_json(stream: &mut TcpStream, status: u16, mensagem: &str) {
    let _ = escrever(stream, status, "application/json", serde_json::json!({ "erro": mensagem }).to_string().as_bytes());
}

/// Nome de arquivo em `assets/` (um nível, sem ponto no começo).
pub fn arquivo_de_midia_valido(arquivo: &str) -> Option<&str> {
    let nome = arquivo.strip_prefix("assets/")?;
    (!nome.is_empty() && nome.len() <= 120 && !nome.starts_with('.') && !nome.contains(['/', '\\', '\0']) && nome != "..").then_some(nome)
}

fn hash_valido(h: &str) -> bool { h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()) }

// ——— host ———

struct Cliente { id: String, login: String, tx: Sender<String> }

pub struct Host {
    pub token: String,
    pub projeto: String,
    pub nome: String,
    pub pasta: PathBuf,
    pub permitidos: Mutex<Vec<String>>,
    clientes: Mutex<Vec<Cliente>>,
    parar: AtomicBool,
    pub porta: u16,
    aviso: Aviso,
}

impl Host {
    pub fn iniciar(projeto: &str, nome: &str, pasta: &Path, token: &str, permitidos: Vec<String>, aviso: Aviso) -> io::Result<Arc<Host>> {
        let ouvinte = TcpListener::bind("127.0.0.1:0")?;
        let porta = ouvinte.local_addr()?.port();
        let host = Arc::new(Host {
            token: token.to_owned(), projeto: projeto.to_owned(), nome: nome.to_owned(), pasta: pasta.to_owned(),
            permitidos: Mutex::new(permitidos.into_iter().map(|l| l.to_ascii_lowercase()).collect()), clientes: Mutex::new(Vec::new()),
            parar: AtomicBool::new(false), porta, aviso,
        });
        let h = host.clone();
        std::thread::spawn(move || {
            for conexao in ouvinte.incoming() {
                if h.parar.load(Ordering::SeqCst) { break; }
                let Ok(stream) = conexao else { continue };
                let h2 = h.clone();
                std::thread::spawn(move || { let _ = h2.atender(stream); });
            }
        });
        Ok(host)
    }

    pub fn parar(&self) {
        self.parar.store(true, Ordering::SeqCst);
        // Acorda o `accept` para o laço ver o sinal.
        let _ = TcpStream::connect(("127.0.0.1", self.porta));
        self.clientes.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    pub fn permitir(&self, login: &str) {
        let mut p = self.permitidos.lock().unwrap_or_else(|e| e.into_inner());
        let l = login.to_ascii_lowercase();
        if !p.contains(&l) { p.push(l); }
    }

    /// Manda para todos os convidados, menos `exceto`.
    pub fn difundir(&self, de: &str, login: Option<&str>, corpo: &str, exceto: Option<&str>) {
        let envelope = serde_json::json!({ "de": de, "login": login, "corpo": corpo }).to_string();
        let mut clientes = self.clientes.lock().unwrap_or_else(|e| e.into_inner());
        clientes.retain(|c| exceto == Some(c.id.as_str()) || c.tx.send(envelope.clone()).is_ok());
    }

    pub fn convidados(&self) -> Vec<(String, String)> {
        self.clientes.lock().unwrap_or_else(|e| e.into_inner()).iter().map(|c| (c.id.clone(), c.login.clone())).collect()
    }

    fn atender(&self, stream: TcpStream) -> io::Result<()> {
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        let mut leitor = BufReader::new(stream.try_clone()?);
        let pedido = ler_cabeca(&mut leitor)?;
        let mut saida = stream;
        if pedido.consulta.get("t").map(String::as_str) != Some(self.token.as_str()) { erro_json(&mut saida, 401, "Convite inválido ou sessão encerrada."); return Ok(()); }
        let Some(login) = pedido.cabecalhos.get("tailscale-user-login").map(|l| l.to_ascii_lowercase()).filter(|l| !l.is_empty()) else {
            erro_json(&mut saida, 401, "Entre pelo Tailscale (sem identidade da tailnet)."); return Ok(());
        };
        if !self.permitidos.lock().unwrap_or_else(|e| e.into_inner()).contains(&login) {
            (self.aviso)(EventoDaSessao::Pedido { login: login.clone() });
            erro_json(&mut saida, 403, "Ainda não permitido. Peça para quem abriu a sessão aceitar o seu login do Tailscale.");
            return Ok(());
        }
        let cliente = pedido.consulta.get("c").cloned().filter(|c| !c.is_empty() && c.len() <= 64 && c.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')).unwrap_or_else(|| "convidado".into());
        match (pedido.metodo.as_str(), pedido.caminho.as_str()) {
            ("GET", "/ola") => {
                let corpo = serde_json::json!({ "projeto": self.projeto, "nome": self.nome, "hyperframes": super::projetos::VERSAO_DO_HYPERFRAMES, "login": login }).to_string();
                escrever(&mut saida, 200, "application/json", corpo.as_bytes())
            }
            ("GET", "/eventos") => self.eventos(saida, cliente, login),
            ("POST", "/enviar") => {
                let tamanho: usize = pedido.cabecalhos.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
                if tamanho == 0 || tamanho > MENSAGEM_MAXIMA { erro_json(&mut saida, 413, "Mensagem grande demais."); return Ok(()); }
                let mut corpo = vec![0u8; tamanho];
                leitor.read_exact(&mut corpo)?;
                let corpo = String::from_utf8(corpo).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "utf8"))?;
                if serde_json::from_str::<serde_json::Value>(&corpo).is_err() { erro_json(&mut saida, 400, "Mensagem inválida."); return Ok(()); }
                self.difundir(&cliente, Some(&login), &corpo, Some(&cliente));
                (self.aviso)(EventoDaSessao::Mensagem { de: cliente, login: Some(login), corpo });
                escrever(&mut saida, 204, "text/plain", b"")
            }
            ("GET", "/midia") => self.baixar(saida, &pedido),
            ("POST", "/midia") => self.receber(leitor, saida, &pedido),
            _ => { erro_json(&mut saida, 404, "Rota desconhecida."); Ok(()) }
        }
    }

    fn eventos(&self, mut saida: TcpStream, cliente: String, login: String) -> io::Result<()> {
        saida.set_read_timeout(None)?;
        write!(saida, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nX-Accel-Buffering: no\r\nConnection: keep-alive\r\n\r\n: ola\n\n")?;
        saida.flush()?;
        let (tx, rx) = mpsc::channel::<String>();
        {
            let mut clientes = self.clientes.lock().unwrap_or_else(|e| e.into_inner());
            clientes.retain(|c| c.id != cliente);
            clientes.push(Cliente { id: cliente.clone(), login: login.clone(), tx });
        }
        (self.aviso)(EventoDaSessao::Entrou { cliente: cliente.clone(), login: login.clone() });
        let resultado = loop {
            if self.parar.load(Ordering::SeqCst) { break Ok(()); }
            let escrita = match rx.recv_timeout(BATIDA) {
                Ok(m) => write!(saida, "data: {}\n\n", m.replace('\n', " ")),
                // Batida: o proxy e o cliente sabem que a conexão está viva.
                Err(mpsc::RecvTimeoutError::Timeout) => write!(saida, ": batida\n\n"),
                Err(mpsc::RecvTimeoutError::Disconnected) => break Ok(()),
            };
            if let Err(e) = escrita.and_then(|_| saida.flush()) { break Err(e); }
        };
        self.clientes.lock().unwrap_or_else(|e| e.into_inner()).retain(|c| c.id != cliente);
        (self.aviso)(EventoDaSessao::Saiu { cliente, login });
        resultado
    }

    fn baixar(&self, mut saida: TcpStream, pedido: &Pedido) -> io::Result<()> {
        let (Some(nome), Some(hash)) = (pedido.consulta.get("arquivo").and_then(|a| arquivo_de_midia_valido(a)), pedido.consulta.get("hash")) else { erro_json(&mut saida, 400, "Mídia inválida."); return Ok(()) };
        let caminho = self.pasta.join("assets").join(nome);
        let Some(tamanho) = super::arquivos::arquivo_comum(&caminho) else { erro_json(&mut saida, 404, "Essa mídia não está no PC de quem abriu a sessão."); return Ok(()) };
        if !hash_valido(hash) || super::arquivos::sha256_do_arquivo(&caminho)? != *hash { erro_json(&mut saida, 404, "A mídia mudou no PC de quem abriu a sessão."); return Ok(()); }
        saida.set_write_timeout(Some(Duration::from_secs(120)))?;
        write!(saida, "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {tamanho}\r\nConnection: close\r\n\r\n")?;
        io::copy(&mut std::fs::File::open(&caminho)?, &mut saida)?;
        saida.flush()
    }

    fn receber(&self, mut leitor: BufReader<TcpStream>, mut saida: TcpStream, pedido: &Pedido) -> io::Result<()> {
        let (Some(arquivo), Some(hash)) = (pedido.consulta.get("arquivo").cloned(), pedido.consulta.get("hash").cloned()) else { erro_json(&mut saida, 400, "Mídia inválida."); return Ok(()) };
        let Some(nome) = arquivo_de_midia_valido(&arquivo).map(str::to_owned) else { erro_json(&mut saida, 400, "Mídia inválida."); return Ok(()) };
        let tamanho: u64 = pedido.cabecalhos.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
        if !hash_valido(&hash) || tamanho == 0 || tamanho > MIDIA_MAXIMA { erro_json(&mut saida, 413, "Mídia inválida ou grande demais."); return Ok(()); }
        let r = receber_arquivo(&mut leitor, tamanho, &self.pasta.join("assets"), &nome, &hash);
        match r {
            Ok(()) => { (self.aviso)(EventoDaSessao::MidiaRecebida { arquivo }); escrever(&mut saida, 204, "text/plain", b"") }
            Err(e) => { erro_json(&mut saida, 400, &e.to_string()); Ok(()) }
        }
    }
}

/// Lê `tamanho` bytes para `<pasta>/.<nome>.recebendo` e só põe no lugar se o SHA-256 bater.
/// Arquivo que já existe com o mesmo hash fica; com outro hash, não é sobrescrito.
pub fn receber_arquivo(leitor: &mut impl Read, tamanho: u64, pasta: &Path, nome: &str, hash: &str) -> io::Result<()> {
    super::arquivos::criar_pasta(pasta)?;
    let destino = pasta.join(nome);
    if super::arquivos::arquivo_comum(&destino).is_some() {
        if super::arquivos::sha256_do_arquivo(&destino)? == hash { let _ = io::copy(&mut leitor.take(tamanho), &mut io::sink()); return Ok(()); }
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, "Já existe outro arquivo com esse nome."));
    }
    let temporario = pasta.join(format!(".{nome}.{}.recebendo", std::process::id()));
    let resultado = (|| {
        let mut arquivo = std::fs::File::create(&temporario)?;
        let copiados = io::copy(&mut leitor.take(tamanho), &mut arquivo)?;
        arquivo.sync_all()?;
        if copiados != tamanho { return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "A transferência parou no meio.")); }
        if super::arquivos::sha256_do_arquivo(&temporario)? != hash { return Err(io::Error::new(io::ErrorKind::InvalidData, "O arquivo chegou diferente (hash não bate).")); }
        std::fs::rename(&temporario, &destino)
    })();
    if resultado.is_err() { let _ = std::fs::remove_file(&temporario); }
    resultado
}

// ——— convidado ———

pub struct Convidado {
    pub base: String,
    pub token: String,
    pub cliente: String,
    parar: AtomicBool,
}

fn endereco(base: &str) -> io::Result<(String, std::net::SocketAddr)> {
    let sem = base.trim_end_matches('/').strip_prefix("http://").ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Endereço inválido."))?;
    let (hostport, prefixo) = sem.split_once('/').map(|(h, p)| (h.to_owned(), format!("/{p}"))).unwrap_or((sem.to_owned(), String::new()));
    let addr = hostport.to_socket_addrs()?.next().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Não achei o PC de quem abriu a sessão."))?;
    Ok((prefixo, addr))
}

/// Faz um pedido simples e devolve (status, corpo). `corpo`: o que mandar (com Content-Length).
pub fn pedir(base: &str, metodo: &str, rota: &str, corpo: Option<&mut dyn Read>, tamanho: u64, limite: Duration) -> io::Result<(u16, Vec<u8>)> {
    let (prefixo, addr) = endereco(base)?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(8))?;
    stream.set_read_timeout(Some(limite))?;
    stream.set_write_timeout(Some(limite))?;
    write!(stream, "{metodo} {prefixo}{rota} HTTP/1.1\r\nHost: {addr}\r\nContent-Length: {tamanho}\r\nConnection: close\r\n\r\n")?;
    if let Some(c) = corpo { io::copy(&mut c.take(tamanho), &mut stream)?; }
    stream.flush()?;
    let mut leitor = BufReader::new(stream);
    let mut linha = String::new();
    leitor.read_line(&mut linha)?;
    let status: u16 = linha.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut tam: Option<usize> = None;
    loop {
        let mut l = String::new();
        if leitor.read_line(&mut l)? == 0 { break; }
        let l = l.trim_end();
        if l.is_empty() { break; }
        if let Some((n, v)) = l.split_once(':') { if n.eq_ignore_ascii_case("content-length") { tam = v.trim().parse().ok(); } }
    }
    let mut corpo = Vec::new();
    match tam { Some(t) => { corpo.resize(t, 0); leitor.read_exact(&mut corpo)?; } None => { leitor.read_to_end(&mut corpo)?; } }
    Ok((status, corpo))
}

fn mensagem_de_erro(status: u16, corpo: &[u8]) -> String {
    serde_json::from_slice::<serde_json::Value>(corpo).ok().and_then(|v| v["erro"].as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("Quem abriu a sessão respondeu {status}."))
}

impl Convidado {
    pub fn new(base: &str, token: &str, cliente: &str) -> Arc<Convidado> {
        Arc::new(Convidado { base: base.to_owned(), token: token.to_owned(), cliente: cliente.to_owned(), parar: AtomicBool::new(false) })
    }

    fn rota(&self, caminho: &str, extra: &str) -> String { format!("{caminho}?t={}&c={}{extra}", codificar(&self.token), codificar(&self.cliente)) }

    pub fn ola(&self) -> Result<serde_json::Value, String> {
        let (status, corpo) = pedir(&self.base, "GET", &self.rota("/ola", ""), None, 0, Duration::from_secs(10)).map_err(|e| format!("Não consegui falar com o PC de quem abriu a sessão: {e}"))?;
        if status != 200 { return Err(mensagem_de_erro(status, &corpo)); }
        serde_json::from_slice(&corpo).map_err(|_| "Resposta estranha de quem abriu a sessão.".into())
    }

    pub fn enviar(&self, corpo: &str) -> Result<(), String> {
        if corpo.len() > MENSAGEM_MAXIMA { return Err("Mensagem grande demais.".into()); }
        let mut leitor = corpo.as_bytes();
        let (status, resposta) = pedir(&self.base, "POST", &self.rota("/enviar", ""), Some(&mut leitor), corpo.len() as u64, Duration::from_secs(20)).map_err(|e| e.to_string())?;
        if status == 204 || status == 200 { Ok(()) } else { Err(mensagem_de_erro(status, &resposta)) }
    }

    pub fn baixar(&self, arquivo: &str, hash: &str, pasta_dos_assets: &Path) -> Result<(), String> {
        let nome = arquivo_de_midia_valido(arquivo).ok_or("Mídia inválida.")?;
        let (prefixo, addr) = endereco(&self.base).map_err(|e| e.to_string())?;
        let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(8)).map_err(|e| e.to_string())?;
        stream.set_read_timeout(Some(Duration::from_secs(120))).map_err(|e| e.to_string())?;
        let rota = self.rota("/midia", &format!("&arquivo={}&hash={hash}", codificar(arquivo)));
        write!(stream, "GET {prefixo}{rota} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n").map_err(|e| e.to_string())?;
        let mut leitor = BufReader::new(stream);
        let mut linha = String::new();
        leitor.read_line(&mut linha).map_err(|e| e.to_string())?;
        let status: u16 = linha.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let mut tamanho = 0u64;
        loop {
            let mut l = String::new();
            if leitor.read_line(&mut l).map_err(|e| e.to_string())? == 0 { break; }
            let l = l.trim_end();
            if l.is_empty() { break; }
            if let Some((n, v)) = l.split_once(':') { if n.eq_ignore_ascii_case("content-length") { tamanho = v.trim().parse().unwrap_or(0); } }
        }
        if status != 200 {
            let mut corpo = vec![0u8; (tamanho as usize).min(64 * 1024)];
            let _ = leitor.read_exact(&mut corpo);
            return Err(mensagem_de_erro(status, &corpo));
        }
        receber_arquivo(&mut leitor, tamanho, pasta_dos_assets, nome, hash).map_err(|e| e.to_string())
    }

    pub fn subir(&self, arquivo: &str, hash: &str, pasta_dos_assets: &Path) -> Result<(), String> {
        let nome = arquivo_de_midia_valido(arquivo).ok_or("Mídia inválida.")?;
        let caminho = pasta_dos_assets.join(nome);
        let tamanho = super::arquivos::arquivo_comum(&caminho).ok_or("A mídia não está neste PC.")?;
        let mut f = std::fs::File::open(&caminho).map_err(|e| e.to_string())?;
        let rota = self.rota("/midia", &format!("&arquivo={}&hash={hash}", codificar(arquivo)));
        let (status, corpo) = pedir(&self.base, "POST", &rota, Some(&mut f), tamanho, Duration::from_secs(600)).map_err(|e| e.to_string())?;
        if status == 204 || status == 200 { Ok(()) } else { Err(mensagem_de_erro(status, &corpo)) }
    }

    pub fn sair(&self) { self.parar.store(true, Ordering::SeqCst); }

    /// Fica ouvindo o SSE numa thread; cai → avisa e tenta de novo (2 s, 4 s… até 30 s) até `sair`.
    pub fn ouvir(self: &Arc<Self>, aviso: Aviso) {
        let eu = self.clone();
        std::thread::spawn(move || {
            let mut espera = Duration::from_secs(2);
            let mut caiu = false;
            while !eu.parar.load(Ordering::SeqCst) {
                let r = eu.uma_conexao(&aviso, &mut caiu, &mut espera);
                if eu.parar.load(Ordering::SeqCst) { break; }
                if !caiu { caiu = true; aviso(EventoDaSessao::Caiu { erro: r.err().unwrap_or_else(|| "A conexão fechou.".into()) }); }
                std::thread::sleep(espera);
                espera = (espera * 2).min(Duration::from_secs(30));
            }
        });
    }

    fn uma_conexao(&self, aviso: &Aviso, caiu: &mut bool, espera: &mut Duration) -> Result<(), String> {
        let (prefixo, addr) = endereco(&self.base).map_err(|e| e.to_string())?;
        let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(8)).map_err(|e| e.to_string())?;
        // Sem nada (nem a batida de 20 s) por 60 s: conexão morta.
        stream.set_read_timeout(Some(Duration::from_secs(60))).map_err(|e| e.to_string())?;
        write!(stream, "GET {prefixo}{} HTTP/1.1\r\nHost: {addr}\r\nAccept: text/event-stream\r\n\r\n", self.rota("/eventos", "")).map_err(|e| e.to_string())?;
        let mut leitor = BufReader::new(stream);
        let mut linha = String::new();
        leitor.read_line(&mut linha).map_err(|e| e.to_string())?;
        let status: u16 = linha.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let mut tamanho = 0usize;
        loop {
            let mut l = String::new();
            if leitor.read_line(&mut l).map_err(|e| e.to_string())? == 0 || l.trim_end().is_empty() { break; }
            if let Some((n, v)) = l.trim_end().split_once(':') { if n.eq_ignore_ascii_case("content-length") { tamanho = v.trim().parse().unwrap_or(0); } }
        }
        if status != 200 {
            let mut corpo = vec![0u8; tamanho.min(64 * 1024)];
            let _ = leitor.read_exact(&mut corpo);
            return Err(mensagem_de_erro(status, &corpo));
        }
        if *caiu { *caiu = false; aviso(EventoDaSessao::Voltou); }
        *espera = Duration::from_secs(2);
        loop {
            if self.parar.load(Ordering::SeqCst) { return Ok(()); }
            let mut l = String::new();
            if leitor.read_line(&mut l).map_err(|e| e.to_string())? == 0 { return Err("A conexão fechou.".into()); }
            let Some(dado) = l.trim_end().strip_prefix("data: ") else { continue };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(dado) else { continue };
            let (Some(de), Some(corpo)) = (v["de"].as_str(), v["corpo"].as_str()) else { continue };
            aviso(EventoDaSessao::Mensagem { de: de.to_owned(), login: v["login"].as_str().map(str::to_owned), corpo: corpo.to_owned() });
        }
    }
}
