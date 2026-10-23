//! Fork: credencial emitida pelo próprio engine para o Jarvis; não é uma credencial de provedor.
//! Só vive na sessão nativa e no ambiente do processo oficial. Nunca serializar para a janela.
use omni_protocol::{EngineRequest, EngineResponse};
use serde_json::{json, Value};

#[derive(Clone)]
pub struct Acesso {
    url: String,
    token: String,
}

pub fn abrir() -> Result<Acesso, String> {
    let resposta = crate::engine_client::authenticated_request(|token| EngineRequest::Api {
        token,
        method: "POST".into(),
        path: "/orquestracao/jarvis".into(),
        body: Some("{}".into()),
        body_base64: None,
        headers: Vec::new(),
    })?;
    match resposta {
        EngineResponse::Api{status:200,body,..}=>{
            let v:Value=serde_json::from_str(&body).map_err(|_|"O engine devolveu uma resposta inválida ao Jarvis")?;
            Ok(Acesso{url:v["url"].as_str().ok_or("Sem endereço das ferramentas")?.into(),token:v["token"].as_str().ok_or("Sem permissão para as ferramentas")?.into()})
        }
        _=>Err("O engine não conseguiu preparar as ferramentas do Jarvis. Atualize o app e tente novamente".into())
    }
}

impl Acesso {
    pub fn ambiente(&self) -> (String, String) {
        (
            omni_core::orquestracao::VARIAVEL_DO_TOKEN.into(),
            self.token.clone(),
        )
    }
    pub fn config(&self) -> Value {
        json!({"mcpServers":{"medza":{"type":"http","url":self.url,"headers":{"Authorization":"Bearer ${MEDZA_ORQ_TOKEN}"}}}})
    }
}

pub fn fechar() {
    let _ = crate::engine_client::authenticated_request(|token| EngineRequest::Api {
        token,
        method: "DELETE".into(),
        path: "/orquestracao/jarvis".into(),
        body: None,
        body_base64: None,
        headers: Vec::new(),
    });
}
