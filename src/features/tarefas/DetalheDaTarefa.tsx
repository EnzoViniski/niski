// Fork (Tarefas): gaveta à direita com todos os campos de uma tarefa. Cada mudança grava na hora.
import { useEffect, useRef, useState } from "react";
import { Button, Input, Select, Textarea } from "../../components/ui";
import { CloseIcon, TrashIcon } from "../../components/ui/PixelIcon";
import { Avatar, BotaoDeConcluir } from "./CartaoDaTarefa";
import { concluir, novoId, reabrir, rotuloDoDia } from "./dominio";
import {
  CATEGORIAS, PRIORIDADES, RECORRENCIAS, ROTULO_DA_PRIORIDADE, ROTULO_DA_RECORRENCIA, ROTULO_DO_STATUS, STATUS,
  categoria, type Pessoa, type Tarefa,
} from "./tipos";

interface Props {
  t: Tarefa;
  todas: Tarefa[];
  pessoas: Pessoa[];
  eu: Pessoa;
  hoje: string;
  onMudar: (t: Tarefa) => void;
  onApagar: (id: string) => void;
  onAbrir: (id: string) => void;
  onFechar: () => void;
}

/** Texto que só grava ao sair do campo (ou depois de uma pausa), para não gravar a cada tecla. */
function useRascunho(valor: string, gravar: (v: string) => void) {
  const [texto, setTexto] = useState(valor);
  const ultimo = useRef(valor);
  useEffect(() => { if (valor !== ultimo.current) { setTexto(valor); ultimo.current = valor; } }, [valor]);
  useEffect(() => {
    if (texto === ultimo.current) return;
    const timer = setTimeout(() => { ultimo.current = texto; gravar(texto); }, 700);
    return () => clearTimeout(timer);
  }, [texto, gravar]);
  const confirmar = () => { if (texto !== ultimo.current) { ultimo.current = texto; gravar(texto); } };
  return [texto, setTexto, confirmar] as const;
}

export function DetalheDaTarefa({ t, todas, pessoas, eu, hoje, onMudar, onApagar, onAbrir, onFechar }: Props) {
  const mudar = (campos: Partial<Tarefa>) => onMudar({ ...t, ...campos });
  const atual = useRef(t);
  atual.current = t;
  const [titulo, setTitulo, confirmarTitulo] = useRascunho(t.titulo, (v) => onMudar({ ...atual.current, titulo: v.trim() || "Sem título" }));
  const [descricao, setDescricao, confirmarDescricao] = useRascunho(t.descricao, (v) => onMudar({ ...atual.current, descricao: v }));
  const [novoItem, setNovoItem] = useState("");
  const [comentario, setComentario] = useState("");
  const [busca, setBusca] = useState("");
  const [etiqueta, setEtiqueta] = useState("");
  const feita = t.status === "feito";
  const relacionadas = t.relacionadas.map((id) => todas.find((x) => x.id === id && !x.apagada)).filter((x): x is Tarefa => !!x);
  const candidatas = busca.trim()
    ? todas.filter((x) => !x.apagada && x.id !== t.id && !t.relacionadas.includes(x.id) && x.titulo.toLowerCase().includes(busca.toLowerCase())).slice(0, 6)
    : [];

  useEffect(() => {
    const tecla = (e: KeyboardEvent) => { if (e.key === "Escape" && !(e.target instanceof HTMLElement && e.target.closest("input, textarea"))) onFechar(); };
    window.addEventListener("keydown", tecla);
    return () => window.removeEventListener("keydown", tecla);
  }, [onFechar]);

  const relacionar = (outra: Tarefa) => {
    mudar({ relacionadas: [...t.relacionadas, outra.id] });
    // A relação vale dos dois lados.
    if (!outra.relacionadas.includes(t.id)) onMudar({ ...outra, relacionadas: [...outra.relacionadas, t.id] });
    setBusca("");
  };
  const desrelacionar = (outra: Tarefa) => {
    mudar({ relacionadas: t.relacionadas.filter((id) => id !== outra.id) });
    onMudar({ ...outra, relacionadas: outra.relacionadas.filter((id) => id !== t.id) });
  };

  return (
    <aside className="tarefa-gaveta" aria-label="Detalhes da tarefa">
      <header className="tarefa-gaveta-topo">
        <BotaoDeConcluir feita={feita} titulo={t.titulo} onClick={() => onMudar(feita ? reabrir(t) : concluir(t))} />
        <span className="text-sm text-text-secondary">{feita ? "Concluída" : t.recorrencia !== "nunca" ? "Concluir avança para a próxima vez" : "Marcar como feita"}</span>
        <span className="flex-1" />
        <Button size="sm" variant="ghost" aria-label="Apagar tarefa" title="Apagar" onClick={() => onApagar(t.id)}><TrashIcon className="h-4 w-4" /></Button>
        <Button size="sm" variant="ghost" aria-label="Fechar detalhes" onClick={onFechar}><CloseIcon className="h-4 w-4" /></Button>
      </header>
      <div className="tarefa-gaveta-corpo">
        <textarea className="tarefa-gaveta-titulo" rows={1} value={titulo} aria-label="Título"
          onChange={(e) => setTitulo(e.target.value.replace(/\n/g, ""))} onBlur={confirmarTitulo}
          onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); (e.target as HTMLTextAreaElement).blur(); } }} />

        <div className="tarefa-campos">
          <label htmlFor="t-status">Status</label>
          <Select id="t-status" size="sm" value={t.status} onChange={(status) => onMudar(status === "feito" && !feita ? concluir(t) : { ...t, status, ...(status !== "feito" ? { concluidaEm: "" } : {}) })}
            options={STATUS.map((s) => ({ value: s, label: ROTULO_DO_STATUS[s] }))} />
          <label htmlFor="t-quem">Responsável</label>
          <Select id="t-quem" size="sm" value={t.responsavel || "-"} onChange={(v) => mudar({ responsavel: v === "-" ? "" : v })}
            options={[{ value: "-", label: "Ninguém" }, ...pessoas.map((p) => ({ value: p.id, label: p.id === eu.id ? `${p.nome} (você)` : p.nome }))]} />
          <label htmlFor="t-data">Data</label>
          <div className="flex items-center gap-2">
            <Input id="t-data" size="sm" type="date" value={t.data} onChange={(e) => mudar({ data: e.target.value })} className="flex-1" />
            <Input aria-label="Hora" size="sm" type="time" value={t.hora} onChange={(e) => mudar({ hora: e.target.value, ...(e.target.value && !t.data ? { data: hoje } : {}) })} className="w-28" />
          </div>
          <span />
          <div className="flex flex-wrap gap-1">
            {[["Hoje", 0], ["Amanhã", 1], ["Em 1 semana", 7]].map(([rotulo, n]) => (
              <button key={rotulo} type="button" className="ficha ficha-filtro" onClick={() => { const d = new Date(); d.setDate(d.getDate() + Number(n)); mudar({ data: d.toLocaleDateString("sv-SE") }); }}>{rotulo}</button>
            ))}
            {t.data && <button type="button" className="ficha ficha-filtro" onClick={() => mudar({ data: "", hora: "" })}>Sem data</button>}
          </div>
          <label htmlFor="t-dur">Duração</label>
          <Select id="t-dur" size="sm" value={t.duracaoMin} onChange={(duracaoMin) => mudar({ duracaoMin })}
            options={[0, 15, 30, 45, 60, 90, 120, 180, 240].map((m) => ({ value: m, label: m === 0 ? "Não sei" : m < 60 ? `${m} min` : `${m / 60} h`.replace(".5", "h30").replace("h30 h", "h30") }))} />
          <label htmlFor="t-rep">Repete</label>
          <Select id="t-rep" size="sm" value={t.recorrencia} onChange={(recorrencia) => mudar({ recorrencia })}
            options={RECORRENCIAS.map((r) => ({ value: r, label: ROTULO_DA_RECORRENCIA[r] }))} />
          <label htmlFor="t-prio">Prioridade</label>
          <Select id="t-prio" size="sm" value={t.prioridade} onChange={(prioridade) => mudar({ prioridade })}
            options={PRIORIDADES.map((p) => ({ value: p, label: ROTULO_DA_PRIORIDADE[p] }))} />
          <label htmlFor="t-cat">Categoria</label>
          <Select id="t-cat" size="sm" value={t.categoria} onChange={(c) => mudar({ categoria: c })}
            options={CATEGORIAS.map((c) => ({ value: c.id, label: `${c.emoji} ${c.rotulo}` }))} />
          <label htmlFor="t-local">Local</label>
          <Input id="t-local" size="sm" value={t.local} placeholder="Endereço, sala, link da reunião…" onChange={(e) => mudar({ local: e.target.value })} />
          <label htmlFor="t-etq">Etiquetas</label>
          <div className="flex flex-wrap items-center gap-1">
            {t.etiquetas.map((e) => (
              <button key={e} type="button" className="ficha" title="Tirar etiqueta" onClick={() => mudar({ etiquetas: t.etiquetas.filter((x) => x !== e) })}>{e} ×</button>
            ))}
            <input id="t-etq" className="field min-w-[6rem] flex-1 px-2 py-1 text-xs" value={etiqueta} placeholder="+ etiqueta"
              onChange={(e) => setEtiqueta(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter" && etiqueta.trim()) { mudar({ etiquetas: [...new Set([...t.etiquetas, etiqueta.trim()])] }); setEtiqueta(""); } }} />
          </div>
        </div>

        <section className="tarefa-secao">
          <h3>Descrição</h3>
          <Textarea size="sm" rows={5} value={descricao} placeholder="Detalhes, links, o que levar, o que perguntar ao médico…"
            onChange={(e) => setDescricao(e.target.value)} onBlur={confirmarDescricao} />
        </section>

        <section className="tarefa-secao">
          <h3>Checklist {t.checklist.length > 0 && <span className="font-normal normal-case">· {t.checklist.filter((i) => i.feito).length}/{t.checklist.length}</span>}</h3>
          {t.checklist.map((item) => (
            <label key={item.id} className={`check-item${item.feito ? " is-feito" : ""}`}>
              <input type="checkbox" checked={item.feito} onChange={() => mudar({ checklist: t.checklist.map((i) => (i.id === item.id ? { ...i, feito: !i.feito } : i)) })} />
              <span className="flex-1">{item.texto}</span>
              <button type="button" aria-label={`Tirar "${item.texto}"`} onClick={(e) => { e.preventDefault(); mudar({ checklist: t.checklist.filter((i) => i.id !== item.id) }); }}>×</button>
            </label>
          ))}
          <Input size="sm" value={novoItem} placeholder="+ Adicionar passo (Enter)" onChange={(e) => setNovoItem(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter" && novoItem.trim()) { mudar({ checklist: [...t.checklist, { id: novoId("i"), texto: novoItem.trim(), feito: false }] }); setNovoItem(""); } }} />
        </section>

        <section className="tarefa-secao">
          <h3>Tarefas relacionadas</h3>
          {relacionadas.map((r) => (
            <div key={r.id} className="flex items-center gap-1">
              <button type="button" className="tarefa-relacionada flex-1" onClick={() => onAbrir(r.id)}>
                <span>{categoria(r.categoria).emoji}</span>
                <span className={`flex-1 truncate${r.status === "feito" ? " line-through opacity-60" : ""}`}>{r.titulo}</span>
                <span className="text-xs text-text-muted">{ROTULO_DO_STATUS[r.status]}</span>
              </button>
              <button type="button" className="tarefa-mini" aria-label={`Desfazer relação com "${r.titulo}"`} onClick={() => desrelacionar(r)}>×</button>
            </div>
          ))}
          <div className="relative">
            <Input size="sm" value={busca} placeholder="Buscar tarefa para relacionar…" onChange={(e) => setBusca(e.target.value)} />
            {candidatas.length > 0 && (
              <div className="glass absolute inset-x-0 top-full z-10 mt-1 flex flex-col p-1">
                {candidatas.map((c) => (
                  <button key={c.id} type="button" className="tarefa-relacionada" onClick={() => relacionar(c)}>
                    {categoria(c.categoria).emoji} <span className="flex-1 truncate">{c.titulo}</span>
                    <span className="text-xs text-text-muted">{c.data ? rotuloDoDia(c.data, hoje) : ""}</span>
                  </button>
                ))}
              </div>
            )}
          </div>
        </section>

        <section className="tarefa-secao">
          <h3>Comentários</h3>
          {t.comentarios.map((c) => {
            const autor = pessoas.find((p) => p.id === c.autor);
            return (
              <article key={c.id} className="comentario">
                <header><Avatar pessoa={autor} /><strong className="text-text-secondary">{autor?.nome ?? (c.autor || "Alguém")}</strong>
                  <span>{new Date(c.em).toLocaleString("pt-BR", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" })}</span></header>
                <p>{c.texto}</p>
              </article>
            );
          })}
          <Textarea size="sm" rows={2} value={comentario} placeholder="Escreva um comentário… (⌘Enter envia)" onChange={(e) => setComentario(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); enviar(); } }} />
          <div className="flex justify-end"><Button size="sm" variant="primary" disabled={!comentario.trim()} onClick={enviar}>Comentar</Button></div>
        </section>

        <p className="text-[11px] text-text-muted">
          Criada {t.criadaPor ? `por ${pessoas.find((p) => p.id === t.criadaPor)?.nome ?? t.criadaPor} ` : ""}em {new Date(t.criadaEm).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" })}
          {t.concluidaEm ? ` · concluída em ${new Date(t.concluidaEm).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" })}` : ""}
        </p>
      </div>
    </aside>
  );

  function enviar() {
    if (!comentario.trim()) return;
    mudar({ comentarios: [...t.comentarios, { id: novoId("c"), autor: eu.id, texto: comentario.trim(), em: new Date().toISOString() }] });
    setComentario("");
  }
}
