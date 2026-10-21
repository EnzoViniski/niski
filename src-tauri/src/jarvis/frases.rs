//! Fork: Jarvis. Corta a resposta do Claude, que chega aos pedaços, em frases para falar. A voz
//! começa na primeira frase pronta, sem esperar a resposta inteira. Também tira a marcação que não
//! se fala (asteriscos, crases, títulos, marcadores de lista).

/// Frase curta demais ("Ok.") espera a próxima para a voz não soar picada.
const MINIMO: usize = 12;

#[derive(Default)]
pub struct Frases {
    pendente: String,
}

impl Frases {
    /// Junta o pedaço novo e devolve as frases que ficaram completas.
    pub fn juntar(&mut self, pedaco: &str) -> Vec<String> {
        self.pendente.push_str(pedaco);
        let mut prontas = Vec::new();
        loop {
            let Some(corte) = proximo_corte(&self.pendente) else { break };
            let frase: String = self.pendente.drain(..corte).collect();
            let limpa = limpar(&frase);
            if !limpa.is_empty() { prontas.push(limpa); }
        }
        prontas
    }

    /// Fim da resposta: o que sobrou vira a última frase.
    pub fn terminar(&mut self) -> Option<String> {
        let resto = limpar(&std::mem::take(&mut self.pendente));
        (!resto.is_empty()).then_some(resto)
    }
}

/// Posição logo depois do fim de uma frase com tamanho mínimo, ou de uma quebra de linha. Só corta
/// pontuação seguida de espaço: "2.5" e "v0.8" seguem inteiros.
fn proximo_corte(texto: &str) -> Option<usize> {
    let mut anterior = ' ';
    for (i, c) in texto.char_indices() {
        if c == '\n' && !texto[..i].trim().is_empty() { return Some(i + 1); }
        if matches!(anterior, '.' | '!' | '?' | '…' | ';' | ':') && c.is_whitespace() && texto[..i].trim().chars().count() >= MINIMO {
            return Some(i + c.len_utf8());
        }
        anterior = c;
    }
    None
}

fn limpar(frase: &str) -> String {
    let mut linha = frase.trim();
    for prefixo in ["- ", "* ", "• "] {
        if let Some(resto) = linha.strip_prefix(prefixo) { linha = resto; }
    }
    let linha = linha.trim_start_matches('#').trim();
    linha.chars().filter(|c| !matches!(c, '*' | '`' | '_' | '#')).collect::<String>().trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solta_frase_completa_assim_que_chega() {
        let mut f = Frases::default();
        assert!(f.juntar("Oi Enzo, tudo").is_empty());
        assert_eq!(f.juntar(" certo por aqui. O Codex"), vec!["Oi Enzo, tudo certo por aqui."]);
        assert_eq!(f.terminar().as_deref(), Some("O Codex"));
    }

    #[test]
    fn frase_curta_espera_a_proxima() {
        let mut f = Frases::default();
        assert!(f.juntar("Ok. ").is_empty());
        assert_eq!(f.juntar("Abri a conversa do Claude. "), vec!["Ok. Abri a conversa do Claude."]);
    }

    #[test]
    fn tira_marcacao_que_nao_se_fala() {
        let mut f = Frases::default();
        assert_eq!(f.juntar("## Resumo\n- **Codex** terminou o `build`.\n"), vec!["Resumo", "Codex terminou o build."]);
        assert_eq!(f.terminar(), None);
    }

    #[test]
    fn interrogacao_e_exclamacao_cortam() {
        let mut f = Frases::default();
        assert_eq!(f.juntar("Quer que eu abra agora? Posso fazer isso já! "),
            vec!["Quer que eu abra agora?", "Posso fazer isso já!"]);
    }
}
