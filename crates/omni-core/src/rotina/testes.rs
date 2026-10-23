//! Testes dos dados da rotina: leitura tolerante, migração, operações e montagem do dia/semana.
use super::alimentacao::Alimentacao;
use super::operacoes::{executar, Agora};
use super::plano::Plano;
use super::treino::Treino;
use super::*;
use serde_json::json;

fn agora(data_: &str, hora_: &str) -> Agora {
    let hoje = data(data_).unwrap();
    let h = hora(hora_).unwrap();
    Agora { ms: hoje.and_time(h).and_utc().timestamp_millis(), hoje, hora: h }
}

fn op(dir: &Path, nome: &str, args: serde_json::Value, a: &Agora) -> serde_json::Value {
    executar(dir, nome, &args, a).unwrap_or_else(|e| panic!("{nome}: {e}"))
}

#[test]
fn arquivo_ausente_vira_padrao_e_ilegivel_ganha_copia() {
    let dir = tempfile::tempdir().unwrap();
    let plano: Plano = ler(dir.path());
    assert_eq!(plano.areas.len(), 5);
    assert!(plano.areas.iter().any(|a| a.id == "aircook" && a.etiqueta == "aircook"));
    fs::create_dir_all(pasta(dir.path())).unwrap();
    fs::write(pasta(dir.path()).join("treino.json"), "{ quebrado").unwrap();
    let t: Treino = ler(dir.path());
    assert_eq!(t.meta_semanal, 4);
    let copias: Vec<_> = fs::read_dir(pasta(dir.path())).unwrap().flatten()
        .filter(|e| e.file_name().to_string_lossy().contains("ilegivel")).collect();
    assert_eq!(copias.len(), 1, "o original fica guardado para recuperação");
}

#[test]
fn item_ilegivel_na_lista_e_pulado_e_numero_em_texto_vale() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(pasta(dir.path())).unwrap();
    // Arquivo sem `versao` (formato 1), uma sessão quebrada no meio e carga como texto.
    fs::write(pasta(dir.path()).join("treino.json"), json!({
        "sessoes": [
            {"id": "s1", "data": "2026-10-20", "series": [{"exercicio": "Supino", "carga_kg": 60, "repeticoes": 8}]},
            "isto não é uma sessão",
            {"id": "s2", "data": "2026-10-22", "series": [{"exercicio": "supino", "carga_kg": 62.5, "repeticoes": 6}]}
        ],
        "fichas": [{"id": "f1", "nome": "A", "exercicios": [{"nome": "Supino", "carga_kg": "62,5kg"}]}],
        "campo_de_versao_futura": true
    }).to_string()).unwrap();
    let t: Treino = ler(dir.path());
    assert_eq!(t.sessoes.len(), 2);
    assert_eq!(t.fichas[0].exercicios[0].carga_kg, Some(62.5));
    let e = t.evolucao("SUPINO");
    assert_eq!(e.pontos.len(), 2);
    assert_eq!(e.recorde_carga, 62.5);
    assert!(e.pontos[1].recorde);
}

#[test]
fn nao_regrava_arquivo_de_versao_mais_nova() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(pasta(dir.path())).unwrap();
    fs::write(pasta(dir.path()).join("plano.json"), json!({"versao": VERSAO + 1, "memoria": "do futuro"}).to_string()).unwrap();
    let a = agora("2026-10-23", "10:00");
    assert!(executar(dir.path(), "anotar_memoria", &json!({"texto": "x"}), &a).is_err());
    let bruto = fs::read_to_string(pasta(dir.path()).join("plano.json")).unwrap();
    assert!(bruto.contains("do futuro"));
}

#[test]
fn planejar_semana_grava_blocos_metas_e_marca_planejada() {
    let dir = tempfile::tempdir().unwrap();
    let a = agora("2026-10-25", "20:30"); // domingo
    let r = op(dir.path(), "planejar_semana", json!({
        "semana": "2026-10-26",
        "foco": "Entregar o projeto da Aircook",
        "blocos_fixos": [
            {"titulo": "Aula de Cálculo", "area": "faculdade", "tipo": "aula", "dias": ["seg", "qua"], "inicio": "19:00", "fim": "22:30"},
            {"titulo": "Medza", "area": "medza", "tipo": "trabalho", "dias": [0,1,2,3,4], "inicio": "9h", "fim": "18:00"},
            {"titulo": "Academia", "area": "saude", "tipo": "academia", "dias": [0,2,4], "inicio": "06:30", "fim": "07:30"},
            {"titulo": "Dormir", "tipo": "sono", "dias": [0,1,2,3,4,5,6], "inicio": "23:30", "fim": "07:00"}
        ],
        "blocos_da_semana": [{"titulo": "Reunião Aircook", "area": "aircook", "tipo": "trabalho", "data": "2026-10-28", "inicio": "14:00", "fim": "15:00"}],
        "metas": ["Ler 30 páginas", {"texto": "Treinar 4x", "alvo": 4, "unidade": "treinos", "area": "saude"}]
    }), &a);
    assert_eq!(r["planejada"], true);
    assert_eq!(r["blocos_fixos"], 4);
    let plano: Plano = ler(dir.path());
    assert!(plano::planejada(&plano, data("2026-10-26").unwrap()));
    let sono = plano.blocos.iter().find(|b| b.tipo == "sono").unwrap();
    assert!(!sono.lembrar, "sono não lembra por padrão");
    let quarta = plano.blocos_do_dia(data("2026-10-28").unwrap());
    let titulos: Vec<&str> = quarta.iter().map(|b| b.titulo.as_str()).collect();
    assert_eq!(titulos, ["Academia", "Medza", "Reunião Aircook", "Aula de Cálculo", "Dormir"]);
    // Replanejar troca os fixos, mas mantém os avulsos.
    op(dir.path(), "planejar_semana", json!({"semana": "2026-10-26", "blocos_fixos": [{"titulo": "Medza", "dias": [0], "inicio": "09:00", "fim": "18:00"}]}), &a);
    let plano: Plano = ler(dir.path());
    assert_eq!(plano.blocos.len(), 2);
    assert!(executar(dir.path(), "salvar_bloco", &json!({"titulo": "x", "inicio": "10:00"}), &a).is_err(), "fixo sem dias");
    assert!(executar(dir.path(), "salvar_bloco", &json!({"titulo": "x", "inicio": "25:00", "dias": [1]}), &a).is_err());
}

#[test]
fn imprevisto_cancela_e_move_so_naquele_dia() {
    let dir = tempfile::tempdir().unwrap();
    let a = agora("2026-10-27", "08:00"); // terça
    let b = op(dir.path(), "salvar_bloco", json!({"titulo": "Medza", "area": "medza", "tipo": "trabalho", "dias": [0,1,2,3,4], "inicio": "09:00", "fim": "18:00"}), &a);
    let id = b["bloco"].as_str().unwrap().to_owned();
    let c = op(dir.path(), "salvar_bloco", json!({"titulo": "Aula", "tipo": "aula", "dias": [1], "inicio": "19:00", "fim": "22:00"}), &a);
    let aula = c["bloco"].as_str().unwrap().to_owned();
    op(dir.path(), "ajustar_dia", json!({"data": "2026-10-27", "nota": "Consulta médica de manhã",
        "mudancas": [{"bloco": id, "acao": "mover", "inicio": "11:00"}, {"bloco": aula, "acao": "cancelar"}],
        "novos_blocos": [{"titulo": "Consulta", "tipo": "pessoal", "inicio": "09:00", "fim": "10:30"}]}), &a);
    let plano: Plano = ler(dir.path());
    let hoje = plano.blocos_do_dia(a.hoje);
    assert_eq!(hoje.iter().map(|b| (b.titulo.as_str(), b.inicio.as_str())).collect::<Vec<_>>(), [("Consulta", "09:00"), ("Medza", "11:00")]);
    assert!(hoje[1].mudado.is_some());
    let quarta = plano.blocos_do_dia(data("2026-10-28").unwrap());
    assert_eq!(quarta[0].inicio, "09:00", "a exceção vale só no dia");
    assert_eq!(plano.semana(segunda_de(a.hoje)).imprevistos.len(), 1);
    op(dir.path(), "ajustar_dia", json!({"data": "2026-10-27", "mudancas": [{"bloco": aula, "acao": "restaurar"}]}), &a);
    let plano: Plano = ler(dir.path());
    assert!(plano.blocos_do_dia(a.hoje).iter().any(|b| b.titulo == "Aula"));
}

#[test]
fn dia_junta_blocos_tarefas_refeicoes_e_aponta_agora_e_proximo() {
    let dir = tempfile::tempdir().unwrap();
    let a = agora("2026-10-27", "09:30");
    op(dir.path(), "salvar_bloco", json!({"titulo": "Medza", "area": "medza", "tipo": "trabalho", "dias": [1], "inicio": "09:00", "fim": "12:00"}), &a);
    let ctx = crate::tarefas::contexto(dir.path()).unwrap();
    crate::tarefas::criar(&ctx.raiz, &json!({"titulo": "Call Aircook", "data": "2026-10-27", "hora": "14:00", "etiquetas": ["Aircook"], "categoria": "trabalho"}), "", 1, 1).unwrap();
    crate::tarefas::criar(&ctx.raiz, &json!({"titulo": "Lista de exercícios", "data": "2026-10-27", "categoria": "faculdade"}), "", 2, 2).unwrap();
    crate::tarefas::criar(&ctx.raiz, &json!({"titulo": "Atrasada", "data": "2026-10-20"}), "", 3, 3).unwrap();
    let dia = op(dir.path(), "ler_dia", json!({}), &a);
    let itens: Vec<(&str, &str)> = dia["itens"].as_array().unwrap().iter().map(|i| (i["titulo"].as_str().unwrap(), i["tipo"].as_str().unwrap())).collect();
    assert_eq!(itens[0], ("Café da manhã", "refeicao"));
    assert!(itens.contains(&("Medza", "bloco")) && itens.contains(&("Call Aircook", "tarefa")));
    let call = dia["itens"].as_array().unwrap().iter().find(|i| i["titulo"] == "Call Aircook").unwrap();
    assert_eq!(call["area"], "aircook", "etiqueta vence a categoria");
    assert_eq!(dia["sem_hora"].as_array().unwrap().len(), 2, "sem hora e atrasada");
    assert_eq!(dia["agora"], "bloco:".to_owned() + dia["itens"].as_array().unwrap().iter().find(|i| i["titulo"] == "Medza").unwrap()["alvo"].as_str().unwrap());
    assert_eq!(dia["proximo"], "refeicao:almoco");
    // Marcar feito tira do "agora".
    let item = dia["agora"].as_str().unwrap().to_owned();
    op(dir.path(), "marcar", json!({"item": item}), &a);
    let dia = op(dir.path(), "ler_dia", json!({}), &a);
    assert!(dia["agora"].is_null());
    // Tarefa marcada pela rotina conclui a tarefa do app.
    let id = call["alvo"].as_str().unwrap();
    op(dir.path(), "marcar", json!({"item": format!("tarefa:{id}")}), &a);
    assert_eq!(crate::tarefas::ler(&ctx.raiz, id).unwrap()["status"], "feito");
}

#[test]
fn treino_registro_rapido_recorde_e_frequencia() {
    let dir = tempfile::tempdir().unwrap();
    let a = agora("2026-10-20", "07:00");
    op(dir.path(), "salvar_ficha", json!({"id": "fa", "nome": "A", "titulo": "Peito", "exercicios": [{"nome": "Supino reto", "series": 4, "repeticoes": "8-10"}, "Crucifixo"]}), &a);
    op(dir.path(), "salvar_ficha", json!({"id": "fb", "nome": "B", "titulo": "Costas", "exercicios": ["Remada"]}), &a);
    let r = op(dir.path(), "registrar_serie", json!({"exercicio": "Supino reto", "carga_kg": 60, "repeticoes": 10}), &a);
    assert_eq!((r["ficha"].as_str(), r["recorde"].as_bool()), (Some("fa"), Some(false)), "primeira vez não é recorde");
    let r2 = op(dir.path(), "registrar_serie", json!({"exercicio": "Supino reto", "carga_kg": "62,5", "repeticoes": 8}), &a);
    assert_eq!(r2["sessao"], r["sessao"], "mesma sessão do dia");
    assert_eq!(r2["serie_do_exercicio"], 2);
    op(dir.path(), "concluir_treino", json!({}), &a);
    // Próximo treino: a ficha seguinte na rotação.
    let b = agora("2026-10-22", "07:00");
    let t = op(dir.path(), "ler_treino", json!({"exercicio": "supino reto"}), &b);
    assert_eq!(t["proxima_ficha"], "fb");
    let r3 = op(dir.path(), "registrar_serie", json!({"exercicio": "Supino Reto", "ficha": "A", "carga_kg": 65, "repeticoes": 6}), &b);
    assert_eq!(r3["recorde"], true);
    let t = op(dir.path(), "ler_treino", json!({"exercicio": "supino reto", "semanas": 2}), &b);
    assert_eq!(t["evolucao"]["recorde_carga"], 65.0);
    assert_eq!(t["evolucao"]["pontos"].as_array().unwrap().len(), 2);
    assert_eq!(t["frequencia"][1]["treinos"], 2);
    let sessao = r3["sessao"].as_str().unwrap();
    op(dir.path(), "remover_serie", json!({"sessao": sessao, "indice": 0}), &b);
    assert!(executar(dir.path(), "registrar_serie", &json!({"exercicio": "x", "carga_kg": -1, "repeticoes": 1}), &b).is_err());
    assert!(executar(dir.path(), "registrar_serie", &json!({"exercicio": "x", "carga_kg": 10}), &b).is_err());
}

#[test]
fn alimentacao_refeicao_do_plano_e_agua() {
    let dir = tempfile::tempdir().unwrap();
    let a = agora("2026-10-23", "12:40");
    op(dir.path(), "registrar_refeicao", json!({"nome": "almoço", "texto": "Arroz, feijão e frango", "kcal": 650, "proteina_g": "45"}), &a);
    op(dir.path(), "registrar_refeicao", json!({"texto": "Barra de proteína", "kcal": 200, "hora": "10:00"}), &a);
    let r = op(dir.path(), "registrar_agua", json!({}), &a);
    assert_eq!(r["agua_ml"], 250);
    op(dir.path(), "registrar_agua", json!({"copos": 3}), &a);
    let r = op(dir.path(), "registrar_agua", json!({"ml": -10000}), &a);
    assert_eq!(r["agua_ml"], 0, "nunca negativo");
    let al = op(dir.path(), "ler_alimentacao", json!({"dias": 3}), &a);
    assert_eq!(al["totais"]["kcal"], 850.0);
    assert_eq!(al["dia"]["refeicoes"][0]["texto"], "Barra de proteína", "ordenadas por hora");
    assert_eq!(al["dia"]["refeicoes"][1]["planejada"], "almoco");
    assert_eq!(al["historico"].as_array().unwrap().len(), 3);
    let dia = op(dir.path(), "ler_dia", json!({}), &a);
    let almoco = dia["itens"].as_array().unwrap().iter().find(|i| i["id"] == "refeicao:almoco").unwrap();
    assert_eq!(almoco["estado"], "feito");
    op(dir.path(), "salvar_plano_alimentar", json!({"refeicoes": [{"nome": "Café", "hora": "8h"}], "meta_agua_ml": 3000}), &a);
    let al: Alimentacao = ler(dir.path());
    assert_eq!((al.plano.len(), al.plano[0].hora.as_str(), al.meta_agua_ml), (1, "08:00", 3000));
}

#[test]
fn metas_memoria_ajustes_e_operacao_desconhecida() {
    let dir = tempfile::tempdir().unwrap();
    let a = agora("2026-10-23", "12:40");
    let m = op(dir.path(), "salvar_meta", json!({"texto": "Treinar", "alvo": 4}), &a);
    let id = m["meta"].as_str().unwrap();
    let m = op(dir.path(), "salvar_meta", json!({"id": id, "somar": 1}), &a);
    assert_eq!(m["progresso"], 0.25);
    op(dir.path(), "anotar_memoria", json!({"texto": "Prefere treinar cedo"}), &a);
    op(dir.path(), "anotar_memoria", json!({"texto": "Quinta é dia corrido", "modo": "acrescentar"}), &a);
    let s = op(dir.path(), "ler_semana", json!({}), &a);
    assert_eq!(s["memoria"], "Prefere treinar cedo\nQuinta é dia corrido");
    assert_eq!(s["dias"].as_array().unwrap().len(), 7);
    assert!(executar(dir.path(), "salvar_ajustes", &json!({"plataforma": "antigravity"}), &a).is_err());
    let aj = op(dir.path(), "salvar_ajustes", json!({"plataforma": "codex", "modo_conversa": "continua"}), &a);
    assert!(aj.get("conversas").is_none());
    assert!(executar(dir.path(), "apagar_tudo", &json!({}), &a).is_err());
    assert!(operacoes::OPERACOES.iter().all(|o| operacoes::existe(o)));
}

#[test]
fn conversa_da_semana_semanal_e_continua() {
    let mut a = ajustes::Ajustes::default();
    a.definir_conversa("2026-10-19", "conv-1");
    a.definir_conversa("2026-10-26", "conv-2");
    assert_eq!(a.conversa_da_semana("2026-10-19").as_deref(), Some("conv-1"));
    assert_eq!(a.conversa_da_semana("2026-11-02"), None);
    a.modo_conversa = "continua".into();
    assert_eq!(a.conversa_da_semana("2026-11-02").as_deref(), Some("conv-2"));
}

#[test]
fn dias_e_horas_aceitam_formas_comuns() {
    assert_eq!(dias_de(Some(&json!(["seg", "Sábado", 6, "dom"]))).unwrap(), vec![0, 5, 6]);
    assert!(dias_de(Some(&json!(["xyz"]))).is_err());
    assert_eq!(normalizar_hora("7h", "h").unwrap(), "07:00");
    assert_eq!(normalizar_hora("7h45", "h").unwrap(), "07:45");
    assert_eq!(duracao("23:30", "07:00"), 450);
    assert_eq!(chave("  Supino   Reto "), chave("supino reto"));
}

#[test]
fn responder_aviso_duas_vezes_nao_repete_o_efeito() {
    // Fork (Rotina): dois toques no botão do Telegram (ou tela e Telegram juntos) chegam como duas respostas.
    let dir = tempfile::tempdir().unwrap();
    let a = agora("2026-10-23", "10:00");
    let disparo = |chave: &str, tipo: &str| lembretes::Disparo { chave: chave.into(), tipo: tipo.into(), titulo: "x".into(),
        data: "2026-10-23".into(), alvo: tipo.into(), botoes: vec!["feito".into(), "adiar".into()], ..Default::default() };
    let mut estado = lembretes::Estado::default();
    estado.registrar(&[disparo("agua:2026-10-23:08:30", "agua"), disparo("lembrete:l1:2026-10-23", "lembrete")], a.ms);
    gravar(dir.path(), &estado).unwrap();

    let primeiro = op(dir.path(), "responder_aviso", json!({"chave": "agua:2026-10-23:08:30", "acao": "copo"}), &a);
    let segundo = op(dir.path(), "responder_aviso", json!({"chave": "agua:2026-10-23:08:30", "acao": "copo"}), &a);
    assert_eq!(primeiro["agua_ml"], 250);
    assert_eq!(segundo["ja_respondido"], "copo");
    assert_eq!(ler::<Alimentacao>(dir.path()).dia(a.hoje).agua_ml, 250, "um copo só");

    op(dir.path(), "responder_aviso", json!({"chave": "lembrete:l1:2026-10-23", "acao": "adiar"}), &a);
    op(dir.path(), "responder_aviso", json!({"chave": "lembrete:l1:2026-10-23", "acao": "adiar"}), &a);
    assert_eq!(ler::<lembretes::Estado>(dir.path()).adiados.len(), 1, "um adiado só");
    // "visto" continua valendo (só esconde).
    assert!(executar(dir.path(), "responder_aviso", &json!({"chave": "lembrete:l1:2026-10-23", "acao": "visto"}), &a).is_ok());
}
