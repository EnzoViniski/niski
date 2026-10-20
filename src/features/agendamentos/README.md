# Agendamentos (Onda 3D)

O engine guarda `agendamentos.json` na sua própria pasta de dados e confere o calendário a cada
30 segundos, mesmo sem a janela do app. Precisa estar ligado e o computador acordado. Não é um
cron do sistema nem usa `/loop`. Escopo desta frente: conversa existente e novo agente; iniciar
fluxos pertence a outra integração e não é exposto aqui.

- Minutos/horas são tempo decorrido (mínimo 5 minutos); diário e semanal seguem o fuso **local do
  computador do engine**, com suas regras de DST. Segunda=0. Uma data única é convertida pelo
  engine, não pelo fuso do navegador do celular.
- Hora inexistente em recorrência avança para o primeiro minuto válido; hora repetida usa a
  primeira ocorrência, uma vez naquele dia. Data única em hora inexistente é recusada; em hora
  repetida usa a primeira ocorrência.
- Depois de sono, consome uma ocorrência vencida e calcula a próxima a partir de agora. Só há
  uma ocorrência pendente por agendamento; não acumula execuções perdidas.
- Pausar impede novas ocorrências; uma execução já pendente continua. Retomar pula os horários
  perdidos enquanto estava pausado. Rodar agora preserva a pausa e a próxima recorrência futura.
- Novo agente: resolve plataforma/conta contra o catálogo publicado e os perfis do engine,
  permite pasta/subpasta do projeto publicado, modelo seguro e worktree por execução via
  `omni_core::worktree_isolada`. Confere `limite_de_sessoes` antes do spawn e da retomada.
- Conversa fechada: `plano_de_retomada` + `retomar` (`--resume`), preservando conta/modelo da
  conversa. Histórico nativo reutiliza o id estável da conversa já retomada.
- Entrega: `enqueue(ActionKind::Fila)`, exatamente como o chat, e acompanha o resultado real da
  ação. Campo ocupado/rascunho e falta de vagas adiam 1 minuto, com item na Atividade. Erros
  conhecidos antes de qualquer escrita podem ser repetidos com nova chave; falha após uma
  escrita possivelmente parcial não é repetida. Execuções aguardando expiram em 24 horas.
- Histórico limitado às últimas 20 execuções: horário, estado, id da conversa. O prompt existe
  apenas na definição do agendamento e na ação de chat, nunca nos logs/histórico/Atividade.
- A ocorrência consumida é gravada antes de acionar a CLI. Reinício com ocorrência pendente
  marca `interrompido`; confira a conversa antes de Rodar agora. Não prometemos exactly-once
  entre arquivo e processo externo: evitamos duplicação automática quando o resultado é incerto.
- Arquivo inválido gera erro e é preservado, em vez de ser silenciosamente substituído.

## Extensão para 3B: conta automática

`conta_escolhida` em `crates/omni-engine/src/agendamentos.rs` é o ponto de extensão. Atualmente
`null` significa conta padrão da plataforma, id significa a conta escolhida, e `automatica`
é recusado. A 3B pode resolver sua política **antes de `resolve_spawn`**, registrar a escolha
como metadado e manter a conferência do limite. Nunca trocar automaticamente a conta de uma
conversa existente. O seletor mostra Automática indisponível até essa integração.

## API e UI

`GET/POST /agendamentos`; `POST /agendamentos/{id}/{rodar|pausar|retomar|apagar}`;
`GET /agendamentos/opcoes` (somente metadados); `POST /agendamentos/data {local: YYYY-MM-DDTHH:MM}`.
A lista retorna a próxima execução em epoch ms e o histórico. Novo/editar compartilham o POST
(com `id` para editar). Não se pode editar/apagar enquanto há execução pendente.

Desktop: botão Agendamentos e paleta ⌘K. Conversa: menu existente → Agendar esta mensagem,
com o rascunho preenchido, inclusive durante turno ou com a conversa fechada. Anexos não são
agendados: só o texto. O formulário mantém o rascunho original do compositor.

## Validação e pontos de integração

- Typecheck limpo; Vitest completo: **717 testes, 117 arquivos**, todos passando.
- Casos Rust específicos: **19 testes passando** (4 core + 15 engine), sempre com trava e touch.
  Incluem DST/fim de mês/semana, relógio falso, processo/PTY de CLI falsa, fila durante turno,
  rascunho protegido, retomada `--resume`, conta/modelo/pasta, teto de sessões, persistência antes
  do efeito, recuperação única, reinício incerto, pausa/rodar agora e histórico limitado.
- Arquivos compartilhados: `Cargo.lock`, `omni-core/src/lib.rs`, `omni-engine/Cargo.toml`,
  `mobile/mod.rs` (módulo, inicialização e uma única linha em `rotas()`), `atividade.rs`,
  `TitleBar.tsx`, `atividade/{tipos,navegacao}.ts` e
  `mobile/conversa/{RodapeDaConversa,ControlesDaConversa}.tsx`.
- Arquivos exclusivos da 2A: sem alterações. Sem commit, merge, push ou build do `.app`.
- `RTK.md` citado nas instruções não estava na worktree nem nos ancestrais consultados.

Rodada Rust geral final (`npm run build:engine` + `cargo test --no-fail-fast -p omni-engine
-p omni-core -p omni-protocol`, com trava/touch): build OK; **260 passaram, 7 ignorados**.
As mesmas **6 falhas de sandbox** da primeira rodada, fora dos arquivos desta frente:

- `economia::tests::ps_lista_este_processo`: `ps` indisponível.
- `interaction::tests::shell_com_filho_vivo_conta_como_ocupado`: detecção de filhos via `ps`.
- `tests::two_tcp_clients_can_read_independently`: bind TCP com `Operation not permitted`.
- `mobile::orquestracao::testes::cli_falsa_cria_recebe_tarefa_respeita_limite_espera_e_para`:
  detecção de CLI falsa via `ps`/PTY; campo aparece, mas a capacidade acusa CLI não ativa.
- `mobile::orquestracao::testes::criar_agente_em_worktree_isolada`: mesma entrega à CLI falsa.
- `mobile::orquestracao::testes::orquestrador_responde_permissao_do_subagente_e_arriscado_fica_com_o_usuario`:
  mesma detecção; fica em `abrindo` no sandbox.

Nenhuma CLI real foi usada. Os testes novos injetam a detecção do processo no caminho de
`execute_checked`, sem alterar código da fila/interaction nem enfraquecer as travas reais.
