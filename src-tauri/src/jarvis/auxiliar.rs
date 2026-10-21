//! Fork: Jarvis. Processo de voz (`src-tauri/jarvis-voz/main.swift`): ouve e fala no próprio Mac.
//! Conversa por JSON numa linha; aqui só se abre, manda comando e lê evento.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};

/// Metadados públicos da voz local. Nenhum áudio ou preferência do sistema é acessado aqui.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VozDisponivel {
    pub id: String,
    pub nome: String,
    pub idioma: String,
    pub qualidade: String,
    pub genero: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CatalogoDeVozes {
    #[serde(default)]
    pub vozes: Vec<VozDisponivel>,
    #[serde(default)]
    pub voz: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum Evento {
    Pronto {
        #[serde(default)] eco: bool,
        #[serde(default)] vozes: Vec<VozDisponivel>,
        #[serde(default)] voz: Option<String>,
    },
    Vozes { vozes: Vec<VozDisponivel>, voz: Option<String> },
    BaixandoModelo,
    Parcial { texto: String },
    Final { texto: String },
    Nivel { mic: f32, fala: f32 },
    Falando { id: u64 },
    Falou { id: u64 },
    Calado,
    Interrompido,
    Eco,
    Aviso { msg: String },
    Erro { msg: String, #[serde(default)] codigo: Option<String> },
}

/// Onde está o binário: em dev, ao lado do engine em `src-tauri/binaries`; no app, junto do executável.
pub fn caminho() -> Option<PathBuf> {
    if let Some(caminho) = std::env::var_os("OMNI_JARVIS_VOZ") { return Some(PathBuf::from(caminho)); }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries")
        .join(format!("medza-voz-{}", env!("OMNI_TARGET_TRIPLE")));
    if cfg!(debug_assertions) && dev.is_file() { return Some(dev); }
    let vizinho = std::env::current_exe().ok()?.with_file_name("medza-voz");
    if vizinho.is_file() { Some(vizinho) } else { dev.is_file().then_some(dev) }
}

/// O modo de catálogo termina sem pedir acesso nem iniciar o microfone. Limita a espera se o
/// serviço de vozes do macOS não responder; sua saída contém apenas nomes e tipos de voz.
pub fn listar_vozes(executavel: &Path) -> Result<CatalogoDeVozes, String> {
    let mut filho = Command::new(executavel).arg("--vozes")
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().map_err(|_| "Não foi possível consultar as vozes locais do Jarvis".to_owned())?;
    let saida = filho.stdout.take().ok_or("A consulta não abriu a saída de vozes")?;
    // Esvazia o pipe enquanto o filho trabalha: catálogos grandes não ficam presos no buffer.
    let leitor = std::thread::spawn(move || {
        let mut dados = Vec::new();
        saida.take(256 * 1024).read_to_end(&mut dados).map(|_| dados)
    });
    let prazo = Instant::now() + Duration::from_secs(8);
    let resultado = loop {
        match filho.try_wait() {
            Ok(Some(status)) if status.success() => break Ok(()),
            Ok(Some(_)) => break Err("O macOS não conseguiu listar as vozes locais".to_owned()),
            Ok(None) if Instant::now() < prazo => std::thread::sleep(Duration::from_millis(25)),
            _ => {
                let _ = filho.kill();
                let _ = filho.wait();
                break Err("A consulta de vozes demorou demais. Tente novamente".to_owned());
            }
        }
    };
    let dados = leitor.join().map_err(|_| "A leitura de vozes locais parou".to_owned())?
        .map_err(|_| "Não foi possível ler as vozes locais".to_owned())?;
    resultado?;
    serde_json::from_slice(&dados).map_err(|_| "A lista de vozes locais veio em um formato inesperado".into())
}

pub struct Auxiliar {
    filho: Child,
    entrada: ChildStdin,
}

impl Auxiliar {
    pub fn abrir(executavel: &Path, mut ao_evento: impl FnMut(Option<Evento>) + Send + 'static) -> Result<Self, String> {
        let mut filho = Command::new(executavel)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
            .spawn().map_err(|e| format!("A voz do Jarvis não abriu: {e}"))?;
        let entrada = filho.stdin.take().ok_or("sem stdin")?;
        let saida = filho.stdout.take().ok_or("sem stdout")?;
        std::thread::spawn(move || {
            for linha in BufReader::new(saida).lines().map_while(Result::ok) {
                if let Ok(evento) = serde_json::from_str::<Evento>(&linha) { ao_evento(Some(evento)); }
            }
            ao_evento(None);
        });
        Ok(Self { filho, entrada })
    }

    fn mandar(&mut self, comando: Value) {
        let _ = writeln!(self.entrada, "{comando}").and_then(|_| self.entrada.flush());
    }

    pub fn ouvir(&mut self, ligado: bool) { self.mandar(json!({ "cmd": if ligado { "ouvir" } else { "pausar" } })); }
    pub fn falar(&mut self, id: u64, texto: &str) { self.mandar(json!({ "cmd": "falar", "id": id, "texto": texto })); }
    pub fn calar(&mut self) { self.mandar(json!({ "cmd": "calar" })); }
    /// Uma string vazia volta à escolha automática de voz masculina local.
    pub fn selecionar_voz(&mut self, id: &str) { self.mandar(json!({ "cmd": "voz", "id": id })); }
}

impl Drop for Auxiliar {
    fn drop(&mut self) {
        self.mandar(json!({ "cmd": "sair" }));
        // Meio segundo para soltar o microfone direito; depois, à força.
        for _ in 0..10 {
            if matches!(self.filho.try_wait(), Ok(Some(_))) { return; }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let _ = self.filho.kill();
        let _ = self.filho.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_os_eventos_do_auxiliar() {
        let ler = |s: &str| serde_json::from_str::<Evento>(s).unwrap();
        assert_eq!(ler(r#"{"ev":"final","texto":"oi"}"#), Evento::Final { texto: "oi".into() });
        assert_eq!(ler(r#"{"ev":"nivel","mic":0.5,"fala":0}"#), Evento::Nivel { mic: 0.5, fala: 0.0 });
        assert_eq!(ler(r#"{"ev":"falou","id":3}"#), Evento::Falou { id: 3 });
        assert_eq!(ler(r#"{"ev":"pronto","vozes":[],"voz":"x","eco":true}"#),
            Evento::Pronto { eco: true, vozes: Vec::new(), voz: Some("x".into()) });
        assert_eq!(ler(r#"{"ev":"pronto","eco":false}"#),
            Evento::Pronto { eco: false, vozes: Vec::new(), voz: None });
        assert_eq!(ler(r#"{"ev":"vozes","vozes":[],"voz":null}"#),
            Evento::Vozes { vozes: Vec::new(), voz: None });
        assert_eq!(ler(r#"{"ev":"erro","msg":"sem mic","codigo":"microfone"}"#),
            Evento::Erro { msg: "sem mic".into(), codigo: Some("microfone".into()) });
        assert_eq!(ler(r#"{"ev":"baixando_modelo"}"#), Evento::BaixandoModelo);
    }

    #[test]
    fn le_catalogo_sem_tocar_no_microfone() {
        let catalogo: CatalogoDeVozes = serde_json::from_str(r#"{
            "vozes":[{"id":"voz-local","nome":"Voz","idioma":"pt-BR","qualidade":"premium","genero":"masculino"}],
            "voz":"voz-local"
        }"#).unwrap();
        assert_eq!(catalogo.voz.as_deref(), Some("voz-local"));
        assert_eq!(catalogo.vozes[0].genero, "masculino");
        assert_eq!(catalogo.vozes[0].qualidade, "premium");
        assert!(serde_json::from_str::<CatalogoDeVozes>(r#"{"vozes":false}"#).is_err());
    }

    #[test]
    #[cfg(unix)]
    fn consulta_catalogo_por_um_auxiliar_falso() {
        use std::os::unix::fs::PermissionsExt;
        let pasta = tempfile::tempdir().unwrap();
        let programa = pasta.path().join("voz-falsa");
        std::fs::write(&programa, "#!/bin/sh\n[ \"$1\" = \"--vozes\" ] || exit 2\nprintf '%s\\n' '{\"vozes\":[],\"voz\":null}'\n").unwrap();
        std::fs::set_permissions(&programa, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(listar_vozes(&programa).unwrap(), CatalogoDeVozes::default());
    }
}
