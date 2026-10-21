//! Fork: Jarvis. Processo de voz (`src-tauri/jarvis-voz/main.swift`): ouve e fala no próprio Mac.
//! Conversa por JSON numa linha; aqui só se abre, manda comando e lê evento.
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum Evento {
    Pronto { #[serde(default)] eco: bool },
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
        assert_eq!(ler(r#"{"ev":"pronto","vozes":[],"voz":"x","eco":true}"#), Evento::Pronto { eco: true });
        assert_eq!(ler(r#"{"ev":"erro","msg":"sem mic","codigo":"microfone"}"#),
            Evento::Erro { msg: "sem mic".into(), codigo: Some("microfone".into()) });
        assert_eq!(ler(r#"{"ev":"baixando_modelo"}"#), Evento::BaixandoModelo);
    }
}
