// Fork (Tarefas): as quatro visões — Meu dia, Quadro (kanban), Semana e Lista.
import { useState, type DragEvent, type ReactNode } from "react";
import { aberta, coluna, diaDe, montarDia, paraData, posicaoEntre, rotuloDoDia, somarDias } from "./dominio";
import { Avatar, CartaoDaTarefa, FichaDeData, FichaDePrioridade } from "./CartaoDaTarefa";
import { categoria, DICA_DO_STATUS, ROTULO_DO_STATUS, STATUS, type Pessoa, type Status, type Tarefa } from "./tipos";

export interface PropsDasVisoes {
  tarefas: Tarefa[];
  hoje: string;
  pessoas: Pessoa[];
  eu: Pessoa;
  abertaId: string | null;
  ocultarFeitasAntigas: boolean;
  onAbrir: (id: string) => void;
  onAlternar: (t: Tarefa) => void;
  onMover: (id: string, mudanca: Partial<Tarefa>) => void;
  onNovaEm: (campos: Partial<Tarefa>) => void;
}

const COR_DO_STATUS: Record<Status, string> = { a_fazer: "rgb(var(--c-text-muted))", fazendo: "rgb(var(--c-accent))", revisao: "rgb(var(--c-warning))", feito: "rgb(var(--c-success))" };

function Grupo({ titulo, tarefas, perigo, vazio, children }: { titulo: string; tarefas: Tarefa[]; perigo?: boolean; vazio?: string; children: (t: Tarefa) => ReactNode }) {
  if (!tarefas.length && !vazio) return null;
  return (
    <section className="grupo">
      <h3 className={`grupo-titulo${perigo ? " is-perigo" : ""}`}>{titulo} <span className="contagem">{tarefas.length}</span></h3>
      {tarefas.length ? tarefas.map(children) : <p className="grupo-vazio">{vazio}</p>}
    </section>
  );
}

function Anel({ feitas, total }: { feitas: number; total: number }) {
  const r = 26, c = 2 * Math.PI * r, p = total ? feitas / total : 0;
  return (
    <div className="anel" role="img" aria-label={`${feitas} de ${total} tarefas de hoje feitas`}>
      <svg width="64" height="64" viewBox="0 0 64 64"><circle className="anel-fundo" cx="32" cy="32" r={r} /><circle className="anel-valor" cx="32" cy="32" r={r} strokeDasharray={c} strokeDashoffset={c * (1 - p)} /></svg>
      <span>{total ? `${Math.round(p * 100)}%` : "—"}</span>
    </div>
  );
}

function saudacao(agora: Date) {
  const h = agora.getHours();
  return h < 5 ? "Boa madrugada" : h < 12 ? "Bom dia" : h < 18 ? "Boa tarde" : "Boa noite";
}

export function MeuDia(p: PropsDasVisoes) {
  const dia = montarDia(p.tarefas, p.hoje);
  const [verFeitas, setVerFeitas] = useState(false);
  const agora = new Date();
  const totalHoje = dia.hoje.length + dia.feitasHoje.length;
  const cartao = (t: Tarefa, esconderData = false) => (
    <CartaoDaTarefa key={t.id} t={t} hoje={p.hoje} pessoas={p.pessoas} aberta={p.abertaId === t.id} esconderData={esconderData} onAbrir={p.onAbrir} onAlternar={p.onAlternar} />
  );
  const comHora = [...dia.hoje, ...dia.feitasHoje.filter((t) => t.data === p.hoje)].filter((t) => t.hora).sort((a, b) => a.hora.localeCompare(b.hora));
  const horaAgora = `${String(agora.getHours()).padStart(2, "0")}:${String(agora.getMinutes()).padStart(2, "0")}`;
  const proximaHora = comHora.find((t) => t.hora >= horaAgora && t.status !== "feito")?.id;
  const frase = !totalHoje && !dia.atrasadas.length ? "Nada marcado para hoje. Que tal puxar algo de \"Sem data\"?"
    : dia.hoje.length === 0 ? "Tudo de hoje feito. Mandou bem! 🎉"
    : `${dia.hoje.length} ${dia.hoje.length === 1 ? "tarefa falta" : "tarefas faltam"} hoje${dia.atrasadas.length ? ` · ${dia.atrasadas.length} atrasada${dia.atrasadas.length > 1 ? "s" : ""}` : ""}.`;
  return (
    <div className="meu-dia">
      <div className="flex min-w-0 flex-col gap-5">
        <div className="meu-dia-saudacao glass">
          <Anel feitas={dia.feitasHoje.length} total={totalHoje} />
          <div className="min-w-0">
            <h2>{saudacao(agora)}{p.eu.nome ? `, ${p.eu.nome.split(" ")[0]}` : ""}</h2>
            <p className="text-sm text-text-secondary">{frase}</p>
            <p className="mt-0.5 text-xs text-text-muted">{((d) => d.charAt(0).toUpperCase() + d.slice(1))(agora.toLocaleDateString("pt-BR", { weekday: "long", day: "numeric", month: "long" }))}</p>
          </div>
        </div>
        <Grupo titulo="Atrasadas" perigo tarefas={dia.atrasadas}>{(t) => cartao(t)}</Grupo>
        <Grupo titulo="Hoje" tarefas={dia.hoje} vazio="Nenhuma tarefa aberta para hoje. Digite acima para criar uma.">{(t) => cartao(t, true)}</Grupo>
        <Grupo titulo="Sem data" tarefas={dia.semData}>{(t) => cartao(t)}</Grupo>
        <Grupo titulo="Próximos 7 dias" tarefas={dia.proximas}>{(t) => cartao(t)}</Grupo>
        {dia.feitasHoje.length > 0 && (
          <section className="grupo">
            <button type="button" className="grupo-titulo text-left" onClick={() => setVerFeitas((v) => !v)} aria-expanded={verFeitas}>
              {verFeitas ? "▾" : "▸"} Feitas hoje <span className="contagem">{dia.feitasHoje.length}</span>
            </button>
            {verFeitas && dia.feitasHoje.map((t) => cartao(t))}
          </section>
        )}
      </div>
      <aside className="flex flex-col gap-4">
        <div className="agenda glass">
          <h3 className="grupo-titulo mb-1">Agenda de hoje</h3>
          {comHora.length ? comHora.map((t) => (
            <button key={t.id} type="button" onClick={() => p.onAbrir(t.id)}
              className={`agenda-item${t.id === proximaHora ? " is-agora" : ""}${t.status === "feito" || t.hora < horaAgora ? " is-passou" : ""}`}>
              <time>{t.hora}</time>
              <span className="min-w-0 truncate text-text-primary">{categoria(t.categoria).emoji} {t.titulo}</span>
            </button>
          )) : <p className="text-xs text-text-muted">Sem horários marcados hoje.</p>}
        </div>
        <ResumoDasPessoas {...p} />
      </aside>
    </div>
  );
}

function ResumoDasPessoas({ tarefas, pessoas, hoje }: PropsDasVisoes) {
  if (pessoas.length < 2) return null;
  return (
    <div className="agenda glass">
      <h3 className="grupo-titulo mb-1">Hoje de cada um</h3>
      {pessoas.map((pessoa) => {
        const delas = tarefas.filter((t) => !t.apagada && t.responsavel === pessoa.id && (t.data === hoje || (t.data && t.data < hoje && aberta(t))));
        const feitas = delas.filter((t) => t.status === "feito" || (t.concluidaEm && diaDe(new Date(t.concluidaEm)) === hoje)).length;
        return (
          <div key={pessoa.id} className="flex items-center gap-2 py-1 text-sm">
            <Avatar pessoa={pessoa} />
            <span className="flex-1 truncate text-text-primary">{pessoa.nome}</span>
            <span className="text-xs text-text-muted">{feitas}/{delas.length}</span>
          </div>
        );
      })}
    </div>
  );
}

/** Onde soltar numa lista vertical: índice antes do cartão cujo meio está abaixo do ponteiro. */
function indiceDeSoltar(e: DragEvent, lista: HTMLElement | null): number {
  if (!lista) return 0;
  const cartoes = [...lista.querySelectorAll<HTMLElement>("[data-tarefa]")];
  const i = cartoes.findIndex((c) => { const r = c.getBoundingClientRect(); return e.clientY < r.top + r.height / 2; });
  return i < 0 ? cartoes.length : i;
}

export function Quadro(p: PropsDasVisoes) {
  const [arrastando, setArrastando] = useState<string | null>(null);
  const [alvo, setAlvo] = useState<{ status: Status; indice: number } | null>(null);
  const soltar = (status: Status, e: DragEvent<HTMLElement>, itens: Tarefa[]) => {
    e.preventDefault();
    const id = e.dataTransfer.getData("text/x-tarefa") || arrastando;
    setAlvo(null); setArrastando(null);
    if (!id) return;
    const resto = itens.filter((t) => t.id !== id);
    const indice = Math.min(alvo?.status === status ? alvo.indice : resto.length, resto.length);
    const posicao = posicaoEntre(resto[indice - 1]?.posicao, resto[indice]?.posicao);
    const atual = p.tarefas.find((t) => t.id === id);
    if (!atual) return;
    if (status === "feito" && atual.status !== "feito") { p.onAlternar(atual); return; }
    p.onMover(id, { status, posicao, ...(atual.status === "feito" && status !== "feito" ? { concluidaEm: "" } : {}) });
  };
  return (
    <div className="quadro">
      {STATUS.map((status) => {
        const itens = coluna(p.tarefas, status, p.hoje, p.ocultarFeitasAntigas);
        const marcar = alvo?.status === status ? alvo.indice : -1;
        return (
          <section key={status} className={`quadro-coluna${alvo?.status === status ? " is-alvo" : ""}`} aria-label={ROTULO_DO_STATUS[status]}
            onDragOver={(e) => { e.preventDefault(); const lista = e.currentTarget.querySelector<HTMLElement>(".quadro-coluna-lista"); setAlvo({ status, indice: indiceDeSoltar(e, lista) }); }}
            onDragLeave={(e) => { if (!e.currentTarget.contains(e.relatedTarget as Node)) setAlvo(null); }}
            onDrop={(e) => soltar(status, e, itens)}>
            <header className="quadro-coluna-topo" title={DICA_DO_STATUS[status]}>
              <span className="bolinha" style={{ background: COR_DO_STATUS[status] }} />
              <strong>{ROTULO_DO_STATUS[status]}</strong>
              <span className="text-xs text-text-muted">{itens.length}</span>
              <span className="flex-1" />
              {status !== "feito" && (
                <button type="button" className="tarefa-mini" title={`Nova tarefa em ${ROTULO_DO_STATUS[status]}`} aria-label={`Nova tarefa em ${ROTULO_DO_STATUS[status]}`}
                  onClick={() => p.onNovaEm({ status })}>+</button>
              )}
            </header>
            <div className="quadro-coluna-lista">
              {itens.map((t, i) => (
                <div key={t.id} className="flex flex-col gap-2">
                  {marcar === i && arrastando !== t.id && <div className="quadro-marca" />}
                  <CartaoDaTarefa t={t} hoje={p.hoje} pessoas={p.pessoas} aberta={p.abertaId === t.id} onAbrir={p.onAbrir} onAlternar={p.onAlternar}
                    arrastavel arrastando={arrastando === t.id} onArrastar={setArrastando} />
                </div>
              ))}
              {marcar === itens.length && <div className="quadro-marca" />}
              {!itens.length && <p className="grupo-vazio">{status === "feito" ? "Arraste aqui para concluir" : "Solte uma tarefa aqui"}</p>}
            </div>
          </section>
        );
      })}
    </div>
  );
}

export function Semana(p: PropsDasVisoes & { inicio: string }) {
  const [alvo, setAlvo] = useState<string | null>(null);
  const [arrastando, setArrastando] = useState<string | null>(null);
  const dias = Array.from({ length: 7 }, (_, i) => somarDias(p.inicio, i));
  return (
    <div className="semana">
      {dias.map((dia) => {
        const itens = p.tarefas.filter((t) => !t.apagada && t.data === dia && (t.status !== "feito" || !p.ocultarFeitasAntigas || dia >= somarDias(p.hoje, -7)))
          .sort((a, b) => (a.status === "feito" ? 1 : 0) - (b.status === "feito" ? 1 : 0) || (a.hora || "99").localeCompare(b.hora || "99"));
        const d = paraData(dia);
        return (
          <section key={dia} className={`semana-dia${dia === p.hoje ? " is-hoje" : ""}${alvo === dia ? " is-alvo" : ""}`}
            onDragOver={(e) => { e.preventDefault(); setAlvo(dia); }}
            onDragLeave={(e) => { if (!e.currentTarget.contains(e.relatedTarget as Node)) setAlvo(null); }}
            onDrop={(e) => { e.preventDefault(); const id = e.dataTransfer.getData("text/x-tarefa") || arrastando; setAlvo(null); setArrastando(null); if (id) p.onMover(id, { data: dia }); }}>
            <header className="semana-dia-topo">
              <strong>{d.getDate()}</strong>{d.toLocaleDateString("pt-BR", { weekday: "short" }).replace(".", "")}
              <span className="flex-1" />
              <button type="button" className="tarefa-mini" aria-label={`Nova tarefa em ${rotuloDoDia(dia, p.hoje)}`} onClick={() => p.onNovaEm({ data: dia })}>+</button>
            </header>
            {itens.map((t) => (
              <CartaoDaTarefa key={t.id} t={t} hoje={p.hoje} pessoas={p.pessoas} esconderData aberta={p.abertaId === t.id} onAbrir={p.onAbrir} onAlternar={p.onAlternar}
                arrastavel arrastando={arrastando === t.id} onArrastar={setArrastando} />
            ))}
          </section>
        );
      })}
    </div>
  );
}

export function Lista(p: PropsDasVisoes) {
  const [ordem, setOrdem] = useState<"data" | "prioridade" | "status">("data");
  const peso = { urgente: 0, alta: 1, media: 2, baixa: 3 };
  const itens = p.tarefas.filter((t) => !t.apagada && !(p.ocultarFeitasAntigas && t.status === "feito" && t.concluidaEm && diaDe(new Date(t.concluidaEm)) < somarDias(p.hoje, -7)))
    .sort((a, b) => ordem === "prioridade" ? peso[a.prioridade] - peso[b.prioridade]
      : ordem === "status" ? STATUS.indexOf(a.status) - STATUS.indexOf(b.status)
      : (a.data || "9999").localeCompare(b.data || "9999") || (a.hora || "99").localeCompare(b.hora || "99"));
  const cabeca = (rotulo: string, chave?: typeof ordem) => (
    <th>{chave ? <button type="button" onClick={() => setOrdem(chave)} className={ordem === chave ? "text-accent" : ""}>{rotulo}{ordem === chave ? " ↓" : ""}</button> : rotulo}</th>
  );
  return (
    <table className="lista-tabela">
      <thead><tr><th aria-label="Concluir" />{cabeca("Tarefa")}{cabeca("Quando", "data")}{cabeca("Prioridade", "prioridade")}{cabeca("Status", "status")}{cabeca("Categoria")}{cabeca("Quem")}</tr></thead>
      <tbody>
        {itens.map((t) => (
          <tr key={t.id} onClick={() => p.onAbrir(t.id)} className={p.abertaId === t.id ? "text-accent" : ""}>
            <td><input type="checkbox" aria-label={`Concluir "${t.titulo}"`} checked={t.status === "feito"} onClick={(e) => e.stopPropagation()} onChange={() => p.onAlternar(t)} className="accent-[rgb(var(--c-success))]" /></td>
            <td className={`lista-titulo${t.status === "feito" ? " line-through opacity-60" : ""}`}>{t.titulo}</td>
            <td><FichaDeData t={t} hoje={p.hoje} /></td>
            <td><FichaDePrioridade t={t} />{t.prioridade === "media" || t.prioridade === "baixa" ? <span className="text-xs">{t.prioridade === "media" ? "Média" : "Baixa"}</span> : null}</td>
            <td>{ROTULO_DO_STATUS[t.status]}</td>
            <td>{categoria(t.categoria).emoji} {categoria(t.categoria).rotulo}</td>
            <td><Avatar pessoa={p.pessoas.find((x) => x.id === t.responsavel)} /></td>
          </tr>
        ))}
        {!itens.length && <tr><td colSpan={7} className="py-8 text-center text-text-muted">Nenhuma tarefa com esses filtros.</td></tr>}
      </tbody>
    </table>
  );
}
