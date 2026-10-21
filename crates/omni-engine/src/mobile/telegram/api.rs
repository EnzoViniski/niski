//! Nunca devolve `reqwest::Error` nem `description` da API: podem conter a URL/token ou conteúdo.
use super::*;

pub(super) struct Falha {
    pub texto: &'static str,
    pub espera_ms: u64,
}
impl Falha {
    pub fn disco() -> Self {
        Self {
            texto: "Não consegui guardar a configuração do Telegram.",
            espera_ms: 30_000,
        }
    }
}
pub(super) struct Cliente {
    http: reqwest::Client,
    base: String,
}
pub(super) trait BotApi: Send + Sync {
    fn chamar(
        &self,
        token: &str,
        metodo: &str,
        corpo: Value,
    ) -> impl std::future::Future<Output = Result<Value, Falha>> + Send;
}
impl BotApi for Cliente {
    async fn chamar(&self, token: &str, metodo: &str, corpo: Value) -> Result<Value, Falha> {
        self.chamar(token, metodo, corpo).await
    }
}
impl Cliente {
    #[cfg(test)]
    pub(super) fn local(base: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            base,
        }
    }
    pub fn novo() -> Result<Self, Falha> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .connect_timeout(std::time::Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| Falha {
                texto: "Não consegui iniciar a conexão com o Telegram.",
                espera_ms: 30_000,
            })?;
        Ok(Self {
            http,
            base: "https://api.telegram.org".into(),
        })
    }
    pub async fn chamar(&self, token: &str, metodo: &str, corpo: Value) -> Result<Value, Falha> {
        let resposta = self
            .http
            .post(format!("{}/bot{token}/{metodo}", self.base))
            .json(&corpo)
            .send()
            .await
            .map_err(|_| Falha {
                texto: "Sem conexão com o Telegram. Vou tentar novamente.",
                espera_ms: 30_000,
            })?;
        let status = resposta.status().as_u16();
        let valor: Value = resposta.json().await.map_err(|_| Falha {
            texto: "Resposta inválida do Telegram.",
            espera_ms: 30_000,
        })?;
        if (200..300).contains(&status) && valor["ok"] == true {
            return Ok(valor["result"].clone());
        }
        let codigo = valor["error_code"].as_u64().unwrap_or(status as u64);
        Err(Falha {
            texto: match codigo {
                401 | 404 => "Token do bot inválido. Conecte novamente com o token do BotFather.",
                403 => "O bot foi bloqueado no Telegram. Desbloqueie e conecte novamente.",
                409 => {
                    "Este bot está sendo usado por outra integração. Use um bot exclusivo do Medza."
                }
                429 => "O Telegram pediu uma pausa. O envio será tentado novamente.",
                _ => "O Telegram não aceitou o pedido. Confira o bot e tente novamente.",
            },
            espera_ms: valor["parameters"]["retry_after"]
                .as_u64()
                .unwrap_or(30)
                .clamp(3, 3_600)
                * 1_000,
        })
    }
}
