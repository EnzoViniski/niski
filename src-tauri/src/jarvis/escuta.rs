//! Fork: Jarvis. Decide quando o Enzo terminou de falar. O reconhecimento da Apple fecha um trecho a
//! cada pausa curta; uma frase com respiro no meio viraria duas perguntas. Os trechos finais se
//! juntam e só vão para o Claude depois de um silêncio (nenhum parcial novo) de `ESPERA`.
use std::time::{Duration, Instant};

pub const ESPERA: Duration = Duration::from_millis(750);

#[derive(Default)]
pub struct Escuta {
    trechos: Vec<String>,
    prazo: Option<Instant>,
}

impl Escuta {
    /// Ainda falando: empurra o prazo.
    pub fn parcial(&mut self, agora: Instant) {
        if self.prazo.is_some() || !self.trechos.is_empty() { self.prazo = Some(agora + ESPERA); }
    }

    pub fn trecho_final(&mut self, texto: &str, agora: Instant) {
        let texto = texto.trim();
        if texto.is_empty() { return; }
        self.trechos.push(texto.to_owned());
        self.prazo = Some(agora + ESPERA);
    }

    /// A fala inteira, quando o silêncio passou do prazo.
    pub fn pronta(&mut self, agora: Instant) -> Option<String> {
        if self.prazo.is_some_and(|prazo| agora >= prazo) {
            self.prazo = None;
            let fala = std::mem::take(&mut self.trechos).join(" ");
            return (!fala.trim().is_empty()).then_some(fala);
        }
        None
    }

    pub fn esquecer(&mut self) {
        self.trechos.clear();
        self.prazo = None;
    }
}

fn normalizar(texto: &str) -> Vec<String> {
    texto.to_lowercase().chars()
        .map(|c| match c { 'á' | 'à' | 'â' | 'ã' => 'a', 'é' | 'ê' => 'e', 'í' => 'i', 'ó' | 'ô' | 'õ' => 'o', 'ú' => 'u', 'ç' => 'c', c => c })
        .collect::<String>()
        .split(|c: char| !c.is_alphanumeric()).filter(|p| !p.is_empty()).map(str::to_owned).collect()
}

/// "Medza, para", "para tudo", "chega", "silêncio": fala curta que só pede para calar. "Manda para o
/// Claude" não conta (fala longa ou "para" no meio).
pub fn pede_para_parar(texto: &str) -> bool {
    // Fork: "niski" e as variações do ditado entram com o nome novo (21/10/2026); "medza" segue valendo.
    const NOMES: [&str; 9] = ["niski", "nisque", "nisq", "medza", "medes", "jarvis", "claudio", "chat", "ok"];
    const PARAR: [&str; 8] = ["para", "pare", "parar", "chega", "silencio", "cala", "calado", "stop"];
    let palavras: Vec<String> = normalizar(texto).into_iter()
        .filter(|p| !NOMES.contains(&p.as_str()) && !matches!(p.as_str(), "ola" | "oi" | "pode" | "tudo" | "ai" | "por" | "favor"))
        .collect();
    !palavras.is_empty() && palavras.len() <= 2 && palavras.iter().all(|p| PARAR.contains(&p.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn junta_trechos_ate_o_silencio() {
        let t0 = Instant::now();
        let mut e = Escuta::default();
        e.trecho_final("abre a conversa", t0);
        assert_eq!(e.pronta(t0 + Duration::from_millis(300)), None);
        e.parcial(t0 + Duration::from_millis(400));
        e.trecho_final("do Codex", t0 + Duration::from_millis(600));
        assert_eq!(e.pronta(t0 + Duration::from_millis(1200)), None);
        assert_eq!(e.pronta(t0 + Duration::from_millis(1400)).as_deref(), Some("abre a conversa do Codex"));
        assert_eq!(e.pronta(t0 + Duration::from_secs(5)), None);
    }

    #[test]
    fn parcial_sozinho_nao_cria_fala() {
        let t0 = Instant::now();
        let mut e = Escuta::default();
        e.parcial(t0);
        assert_eq!(e.pronta(t0 + Duration::from_secs(2)), None);
    }

    #[test]
    fn reconhece_pedido_de_parar() {
        for frase in ["Medza, para.", "Para!", "para tudo", "Chega", "Silêncio", "Jarvis, pode parar", "Olá Medza, para", "Niski, para."] {
            assert!(pede_para_parar(frase), "{frase}");
        }
        for frase in ["manda para o Claude", "para que serve isso?", "abre o Codex", "", "olá Medza"] {
            assert!(!pede_para_parar(frase), "{frase}");
        }
    }
}
