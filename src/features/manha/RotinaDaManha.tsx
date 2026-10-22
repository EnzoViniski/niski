// Fork (Manhã): a sessão em tela cheia — um card por questão, alternativas A–D, explicação depois
// de responder e o resumo do dia no fim (acertos, sequência, tarefas de hoje).
import { useEffect, useRef, useState } from "react";
import { Button } from "../../components/ui";
import { EMOJI_DO_TEMA, ROTULO_DO_TEMA } from "./gerador";
import { FONTES, registrarRevisao } from "./fontes";
import { adiar, diaLocal, iniciarSessao, montarQuestoes, pularHoje, responder, sequencia, terminar, useManha } from "./rotina";
import { montarDia } from "../tarefas/dominio";
import { useTarefas } from "../tarefas/loja";
import "./manha.css";

const LETRAS = ["A", "B", "C", "D"];

function saudacao(agora: Date) {
  const h = agora.getHours();
  return h < 12 ? "Bom dia" : h < 18 ? "Boa tarde" : "Boa noite";
}

export function RotinaDaManha({ onFechar, onAbrirTarefas }: { onFechar: () => void; onAbrirTarefas: () => void }) {
  const { ajustes, sessao, historico } = useManha();
  const hoje = diaLocal(new Date());
  const deHoje = sessao && sessao.dia === hoje && sessao.questoes.length ? sessao : null;
  const [preparando, setPreparando] = useState(false);
  const [erro, setErro] = useState("");
  const pedido = useRef(false);

  // Monta as questões de hoje uma vez (a IA do tema livre pode levar alguns segundos).
  useEffect(() => {
    if (deHoje || pedido.current) return;
    pedido.current = true;
    setPreparando(true);
    void montarQuestoes(ajustes, historico, hoje, FONTES)
      .then((questoes) => iniciarSessao(questoes))
      .catch((e) => setErro(e instanceof Error ? e.message : String(e)))
      .finally(() => setPreparando(false));
  }, [deHoje, ajustes, historico, hoje]);

  // Reabrir no meio continua na primeira sem resposta; tudo respondido vai direto ao resumo.
  const atual = deHoje ? deHoje.respostas.findIndex((r) => r === null) : -1;
  const [escolhido, setIndice] = useState<number | null>(null);
  const indice = escolhido ?? (atual >= 0 ? atual : deHoje?.questoes.length ?? 0);
  // Fixa a posição ao abrir: responder não pode pular sozinho para a próxima (a explicação fica na tela).
  useEffect(() => { if (deHoje && escolhido === null) setIndice(indice); }, [deHoje, escolhido, indice]);
  const terminou = !!deHoje && (deHoje.fim !== "" || indice >= deHoje.questoes.length);

  const fecharComo = (acao: () => void) => { acao(); onFechar(); };
  const nome = (() => { try { return (JSON.parse(localStorage.getItem("medza-tarefas-ajustes") ?? "{}") as { eu?: { nome?: string } }).eu?.nome?.split(" ")[0] ?? ""; } catch { return ""; } })();

  return (
    <div className="manha" role="dialog" aria-modal="true" aria-label="Aquecimento da manhã">
      <div className="manha-fundo" aria-hidden="true" />
      <header className="manha-topo" data-tauri-drag-region>
        <div>
          <p className="manha-sobre">{new Date().getHours() >= 18 || new Date().getHours() < 5 ? "🌙" : "☀️"} {saudacao(new Date())}{nome ? `, ${nome}` : ""}</p>
          <h1 className="manha-titulo">Aquecimento do cérebro</h1>
        </div>
        <span className="flex-1" />
        {!terminou && (
          <>
            <Button size="sm" variant="ghost" onClick={() => fecharComo(() => adiar(30))} title="Volta em 30 minutos">Agora não</Button>
            <Button size="sm" variant="ghost" onClick={() => fecharComo(() => pularHoje())}>Pular hoje</Button>
          </>
        )}
      </header>

      <main className="manha-centro">
        {erro ? (
          <div className="manha-card"><p className="text-sm text-danger" role="alert">Não consegui preparar as questões: {erro}</p><Button onClick={onFechar}>Fechar</Button></div>
        ) : preparando || !deHoje ? (
          <div className="manha-card manha-preparando"><span className="manha-pulso" aria-hidden="true" />Preparando as questões de hoje…</div>
        ) : terminou ? (
          <Resumo onFechar={() => { terminar(); onFechar(); }} onAbrirTarefas={() => { terminar(); onAbrirTarefas(); onFechar(); }} mostrarTarefas={ajustes.tarefasNoFim} />
        ) : (
          <Pergunta key={deHoje.questoes[indice].id} indice={indice} total={deHoje.questoes.length}
            onProxima={() => { if (indice + 1 >= deHoje.questoes.length) terminar(); setIndice(indice + 1); }} />
        )}
      </main>
      {deHoje && !terminou && (
        <footer className="manha-pontos" aria-hidden="true">
          {deHoje.questoes.map((q, i) => (
            <span key={q.id} className={`manha-ponto${i === indice ? " is-atual" : ""}${deHoje.respostas[i] === null ? "" : deHoje.respostas[i] === q.correta ? " is-certa" : " is-errada"}`} />
          ))}
        </footer>
      )}
      <span className="sr-only">{historico.length ? `Sequência de ${sequencia(historico, hoje)} dias` : ""}</span>
    </div>
  );
}

function Pergunta({ indice, total, onProxima }: { indice: number; total: number; onProxima: () => void }) {
  const { sessao } = useManha();
  const q = sessao?.questoes[indice];
  const resposta = sessao?.respostas[indice] ?? null;
  const inicio = useRef(Date.now());
  const proximo = useRef<HTMLButtonElement>(null);

  const escolher = (i: number) => {
    if (!q || resposta !== null) return;
    responder(indice, i, (Date.now() - inicio.current) / 1000);
    void registrarRevisao(q, i === q.correta);
  };
  useEffect(() => { if (resposta !== null) proximo.current?.focus(); }, [resposta]);
  useEffect(() => {
    const tecla = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLElement && e.target.closest("input, textarea")) return;
      const n = ["1", "2", "3", "4"].indexOf(e.key) >= 0 ? Number(e.key) - 1 : LETRAS.indexOf(e.key.toUpperCase());
      if (resposta === null && n >= 0 && n < 4) { e.preventDefault(); escolher(n); }
      else if (resposta !== null && e.key === "Enter" && document.activeElement !== proximo.current) { e.preventDefault(); onProxima(); }
    };
    window.addEventListener("keydown", tecla);
    return () => window.removeEventListener("keydown", tecla);
  });
  if (!q) return null;
  const acertou = resposta === q.correta;

  return (
    <article className="manha-card manha-entra">
      <div className="manha-card-topo">
        <span className="manha-tema">{EMOJI_DO_TEMA[q.tema]} {q.origem ? q.origem.nome : ROTULO_DO_TEMA[q.tema]}</span>
        <span className="text-xs text-text-muted">{indice + 1} de {total}</span>
      </div>
      <h2 className="manha-enunciado">{q.enunciado}</h2>
      <div className="manha-opcoes" role="group" aria-label="Alternativas">
        {q.opcoes.map((opcao, i) => {
          const estado = resposta === null ? "" : i === q.correta ? " is-certa" : i === resposta ? " is-errada" : " is-apagada";
          return (
            <button key={i} type="button" className={`manha-opcao${estado}`} onClick={() => escolher(i)} disabled={resposta !== null} aria-pressed={resposta === i}>
              <span className="manha-letra">{LETRAS[i]}</span>
              <span className="flex-1 text-left">{opcao}</span>
              {resposta !== null && i === q.correta && <span aria-label="Correta">✓</span>}
              {resposta === i && i !== q.correta && <span aria-label="Sua resposta">✗</span>}
            </button>
          );
        })}
      </div>
      {resposta !== null ? (
        <div className={`manha-feedback${acertou ? " is-certa" : " is-errada"}`} role="status">
          <strong>{acertou ? "Isso aí! 🎯" : `Quase. A certa é ${LETRAS[q.correta]}.`}</strong>
          {q.explicacao && <p>{q.explicacao}</p>}
          <div className="flex justify-end">
            <Button ref={proximo} variant="primary" onClick={onProxima}>{indice + 1 >= total ? "Ver resultado" : "Próxima →"}</Button>
          </div>
        </div>
      ) : (
        <p className="text-center text-[11px] text-text-muted">Clique ou aperte A–D (ou 1–4)</p>
      )}
    </article>
  );
}

function Resumo({ onFechar, onAbrirTarefas, mostrarTarefas }: { onFechar: () => void; onAbrirTarefas: () => void; mostrarTarefas: boolean }) {
  const { sessao, historico } = useManha();
  const tarefas = useTarefas();
  if (!sessao) return null;
  const hoje = diaLocal(new Date());
  const acertos = sessao.questoes.filter((q, i) => sessao.respostas[i] === q.correta).length;
  const total = sessao.questoes.length;
  const segundos = sessao.segundos.reduce((a, b) => a + b, 0);
  // O dia de hoje ainda pode não estar no histórico (terminar() roda ao sair desta tela).
  const dias = historico.some((d) => d.dia === hoje) ? historico : [...historico, { dia: hoje, acertos, total, segundos }];
  const seguidos = sequencia(dias, hoje);
  const ultimos = Array.from({ length: 7 }, (_, i) => { const d = new Date(); d.setDate(d.getDate() - (6 - i)); return diaLocal(d); });
  const dia = montarDia(tarefas.tarefas.filter((t) => !tarefas.ajustes.eu.id || t.responsavel === tarefas.ajustes.eu.id || !t.responsavel), hoje);
  const frase = acertos === total ? "Perfeito! Cérebro ligado. 🧠⚡" : acertos >= total * 0.6 ? "Boa! Mente acordada. 💪" : "Valeu o esforço — amanhã tem mais. 🌱";

  return (
    <article className="manha-card manha-entra manha-resumo">
      <div className="manha-placar"><strong>{acertos}</strong><span>/{total}</span></div>
      <p className="manha-enunciado text-center">{frase}</p>
      <div className="manha-numeros">
        <div><strong>🔥 {seguidos}</strong><span>{seguidos === 1 ? "dia seguido" : "dias seguidos"}</span></div>
        <div><strong>{Math.max(1, Math.round(segundos / 60))} min</strong><span>de aquecimento</span></div>
        <div><strong>{Math.round((acertos / Math.max(1, total)) * 100)}%</strong><span>de acerto</span></div>
      </div>
      <div className="manha-semana" aria-label="Últimos 7 dias">
        {ultimos.map((d) => {
          const feito = dias.find((x) => x.dia === d);
          const altura = feito ? 20 + 80 * (feito.acertos / Math.max(1, feito.total)) : 6;
          return (
            <div key={d} className="manha-barra-col" title={feito ? `${feito.acertos}/${feito.total}` : "Sem sessão"}>
              <span className={`manha-barra${feito ? "" : " is-vazia"}`} style={{ height: `${altura}%` }} />
              <small>{new Date(`${d}T12:00:00`).toLocaleDateString("pt-BR", { weekday: "narrow" })}</small>
            </div>
          );
        })}
      </div>
      {mostrarTarefas && (dia.hoje.length > 0 || dia.atrasadas.length > 0) && (
        <div className="manha-tarefas">
          <strong className="text-sm text-text-primary">Hoje você tem {dia.hoje.length} tarefa{dia.hoje.length === 1 ? "" : "s"}{dia.atrasadas.length ? ` e ${dia.atrasadas.length} atrasada${dia.atrasadas.length === 1 ? "" : "s"}` : ""}</strong>
          <ul>{[...dia.atrasadas, ...dia.hoje].slice(0, 4).map((t) => <li key={t.id}>{t.hora ? <time>{t.hora}</time> : <time>•</time>} {t.titulo}</li>)}</ul>
        </div>
      )}
      <div className="flex flex-wrap justify-center gap-2">
        {mostrarTarefas && <Button onClick={onAbrirTarefas}>Ver minhas tarefas</Button>}
        <Button variant="primary" autoFocus onClick={onFechar}>Começar o dia</Button>
      </div>
    </article>
  );
}
