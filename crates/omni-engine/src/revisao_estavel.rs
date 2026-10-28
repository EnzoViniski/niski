//! O texto da tela que entra na "versão" (`Interaction::revision`, o `If-Match` do chat).
//!
//! A versão existe para nunca digitar ou aprovar numa tela que o usuário não viu (um diálogo novo, outro
//! pedido). Mas o Codex 0.163 ocioso fica girando um indicador braille na linha de status
//! (`GPT-6-Luna low · ~/omni · ⠋`, visto na CLI real em 2026-10-20): a versão mudava a cada quadro e o
//! chat levava 412 "O contexto mudou" ao trocar modelo — e o envio podia cair em "Contexto mudou antes
//! da execução". Só os glifos de animação saem; texto, números e opções continuam contando.

/// Os quadros dos indicadores de "trabalhando" (braille do Codex, estrelinhas do Claude Code).
fn e_quadro_de_animacao(c: char) -> bool {
    ('\u{2800}'..='\u{28FF}').contains(&c) || matches!(c, '✻' | '✶' | '✳' | '✢' | '✽' | '·' | '∗' | '⏺' | '◐' | '◓' | '◑' | '◒')
}

pub(crate) fn texto_para_revisao(tela: &str) -> String {
    tela.chars().filter(|c| !e_quadro_de_animacao(*c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indicador_girando_nao_muda_a_versao_mas_o_texto_muda() {
        let quadro = |g: char| format!("› Ask Codex to do anything\n\n  GPT-6-Luna low · ~/omni · {g}\n  ? for shortcuts");
        assert_eq!(texto_para_revisao(&quadro('⠋')), texto_para_revisao(&quadro('⠙')));
        assert_ne!(texto_para_revisao("Run rm a.txt?\n› 1. Yes"), texto_para_revisao("Run rm b.txt?\n› 1. Yes"));
        assert_ne!(texto_para_revisao("1. Yes\n2. No"), texto_para_revisao("1. Yes\n2. Yes, always"));
    }
}
