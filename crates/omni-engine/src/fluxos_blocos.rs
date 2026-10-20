//! Fork: execução dos blocos novos; nunca usa /loop nem simula /goal com prompts comuns.
use super::*;

pub(super) fn trava_do_executor(id: &str) -> Arc<Mutex<()>> {
    static M: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();
    M.get_or_init(Default::default)
        .lock()
        .expect("workers")
        .entry(id.into())
        .or_default()
        .clone()
}

pub(super) fn conferir_goal(
    cli: &str,
    perfil: Option<&omni_core::Profile>,
    pasta: &str,
) -> Result<()> {
    if cli != "claude" {
        return Err(anyhow!(
            "Objetivo nativo requer Claude: /goal no codex exec ainda não está documentado"
        ));
    }
    let mut arquivos = vec![
        PathBuf::from(pasta).join(".claude/settings.json"),
        PathBuf::from(pasta).join(".claude/settings.local.json"),
    ];
    if let Some(p) = perfil {
        arquivos.push(PathBuf::from(&p.config_dir).join("settings.json"));
    } else if let Some(config) = env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .or_else(|| env::var_os("USERPROFILE"))
                .map(|h| PathBuf::from(h).join(".claude"))
        })
    {
        arquivos.push(config.join("settings.json"));
    }
    #[cfg(target_os = "macos")]
    arquivos.push(PathBuf::from(
        "/Library/Application Support/ClaudeCode/managed-settings.json",
    ));
    for arquivo in arquivos {
        match fs::read(&arquivo) {
            Ok(bytes) => {
                let v: Value =
                    serde_json::from_slice(&bytes).context("Configuração do Claude inválida")?;
                if v["disableAllHooks"] == true || v["allowManagedHooksOnly"] == true {
                    return Err(anyhow!("/goal indisponível: ganchos desativados ou restritos pelas configurações do Claude"));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                return Err(anyhow!(
                    "Não consegui conferir as configurações de ganchos do Claude"
                ))
            }
        }
    }
    Ok(())
}

pub(super) fn processar_repetir(
    state: &EngineState,
    v: &Viva,
    no: &fluxos::No,
    ctx: &Contexto,
) -> Result<String> {
    let (max, intervalo, parar) = fluxos::blocos::repetir(no).map_err(|e| anyhow!(e))?;
    let terminou = {
        let e = v.lock().map_err(|_| anyhow!("execucao"))?;
        e.nos[&no.id].voltas >= max
            || (e.nos[&no.id].voltas > 0 && parar && fluxos::veredito(&ctx.anterior) == Some(true))
    };
    if terminou {
        definir_saida(state, v, &no.id, ctx.anterior.clone());
        return Ok("fim".into());
    }
    let prazo = mexer(state, v, |e| {
        let n = e.nos.get_mut(&no.id).expect("no");
        if n.voltas > 0 && n.proxima_ms == 0 {
            n.proxima_ms = now_ms().saturating_add(intervalo);
        }
        n.atividade = if n.proxima_ms > now_ms() {
            "Esperando o intervalo; você pode pausar ou cancelar".into()
        } else {
            format!("Iteração {} de {max}", n.voltas + 1)
        };
        n.proxima_ms
    });
    esperar_ate(prazo, || v.lock().is_ok_and(|e| e.status == "rodando"))?;
    mexer(state, v, |e| {
        let n = e.nos.get_mut(&no.id).expect("no");
        n.voltas += 1;
        n.proxima_ms = 0;
    });
    definir_saida(state, v, &no.id, ctx.anterior.clone());
    Ok("corpo".into())
}

fn esperar_ate(prazo: u64, vivo: impl Fn() -> bool) -> Result<()> {
    loop {
        if !vivo() {
            return Err(anyhow!(PAUSADO));
        }
        let falta = prazo.saturating_sub(now_ms());
        if falta == 0 {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(falta.min(100)));
    }
}

#[cfg(test)]
#[path = "fluxos_blocos_testes.rs"]
mod testes;
