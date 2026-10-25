//! Fork (segurança, 0.34.0): o que pode entrar numa linha de comando digitada no shell da PTY.
//!
//! O agente é digitado no shell (`initial_command`), então ids de sessão, nomes de modelo e esforço
//! viram texto que o zsh interpreta. Um id vindo de fora (gancho HTTP, pedido do celular, nome de
//! arquivo) com `;`, `$()` ou espaço executaria comando no Mac a cada retomada. Aqui só passa o
//! alfabeto que esses valores têm de verdade; o resto é recusado, nunca "limpo".

/// Id de sessão/conversa/thread das CLIs (UUID e afins): letras, números, `-` e `_`, até 128.
pub fn id_de_sessao_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Valor de opção como modelo (`claude-opus-5-5`, `gpt-6.1-sol`, `opus[1m]`) ou esforço (`high`):
/// letras, números e `-._:/[]`, até 100. Sem espaço, aspas, `$`, `;`, `|`, `&`, `<`, `>`, `` ` ``.
pub fn valor_de_opcao_valido(valor: &str) -> bool {
    !valor.is_empty() && valor.len() <= 100 && !valor.starts_with('-')
        && valor.bytes().all(|b| b.is_ascii_alphanumeric() || b"-._:/[]".contains(&b))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn id_aceita_uuid_e_recusa_shell() {
        assert!(id_de_sessao_valido("4f1c2b7e-9a0d-4c3e-8b21-0e5f6a7b8c9d"));
        assert!(id_de_sessao_valido("thread_01"));
        for ruim in ["", "x;open -a Calculator", "a$(id)", "a b", "a`id`", "../x", "a|b", &"a".repeat(129)] {
            assert!(!id_de_sessao_valido(ruim), "{ruim}");
        }
    }

    #[test]
    fn opcao_aceita_modelos_reais_e_recusa_shell() {
        for bom in ["claude-opus-5-5", "gpt-6.1-sol", "opus[1m]", "high", "xhigh", "gemini-4-argon"] {
            assert!(valor_de_opcao_valido(bom), "{bom}");
        }
        for ruim in ["", "--dangerously-skip-permissions", "high; rm -rf ~", "a$(id)", "a b", "\"x\"", "a>b"] {
            assert!(!valor_de_opcao_valido(ruim), "{ruim}");
        }
    }
}
