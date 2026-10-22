// Fork (Tarefas): a tela — criação rápida, filtros, as quatro visões e a gaveta de detalhes.
// Mora no painel por cima do app (`PainelDeTarefas.tsx`); `acoes` e `lateral` são do painel (IA, lista flutuante).
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Button, Input, SegmentedControl } from "../../components/ui";
import { Avatar } from "./CartaoDaTarefa";
import { concluir, diaDe, filtrar, lerCriacaoRapida, paraData, reabrir, rotuloDoDia, somarDias } from "./dominio";
import { DetalheDaTarefa } from "./DetalheDaTarefa";
import { adicionar, apagar, desfazerApagar, editar, pessoasConhecidas, salvar, salvarAjustes, useTarefas } from "./loja";
import { CATEGORIAS, CORES_DE_PESSOA, ROTULO_DA_PRIORIDADE, ROTULO_DA_RECORRENCIA, categoria, type AjustesDasTarefas, type Tarefa } from "./tipos";
import { Lista, MeuDia, Quadro, Semana } from "./Visoes";
import "./tarefas.css";

function useHoje() {
  const [hoje, setHoje] = useState(() => diaDe(new Date()));
  useEffect(() => {
    const timer = setInterval(() => setHoje(diaDe(new Date())), 60_000);
    return () => clearInterval(timer);
  }, []);
  return hoje;
}

/** Segunda-feira da semana de `dia`. */
function inicioDaSemana(dia: string) {
  const d = paraData(dia).getDay();
  return somarDias(dia, -((d + 6) % 7));
}

export function TarefasView({ onClose, onAbrirAjustes, acoes, lateral }: { onClose: () => void; onAbrirAjustes: () => void; acoes?: ReactNode; lateral?: ReactNode }) {
  const e = useTarefas();
  const { ajustes } = e;
  const hoje = useHoje();
  const pessoas = pessoasConhecidas(e);
  const [abertaId, setAbertaId] = useState<string | null>(null);
  const [rapida, setRapida] = useState("");
  const [busca, setBusca] = useState("");
  const [desfazer, setDesfazer] = useState<{ id: string; titulo: string } | null>(null);
  const [semana, setSemana] = useState(() => inicioDaSemana(hoje));
  const campo = useRef<HTMLInputElement>(null);

  const visiveis = useMemo(() => filtrar(e.tarefas, { pessoa: ajustes.filtroPessoa, eu: ajustes.eu.id, categoria: ajustes.filtroCategoria, busca }),
    [e.tarefas, ajustes.filtroPessoa, ajustes.eu.id, ajustes.filtroCategoria, busca]);
  const aberta = abertaId ? e.tarefas.find((t) => t.id === abertaId && !t.apagada) : undefined;
  const previa = rapida.trim() ? lerCriacaoRapida(rapida, hoje, pessoas, ajustes.eu.id) : null;

  // ⌘N / N: foca a criação rápida, de qualquer lugar da tela.
  useEffect(() => {
    const tecla = (ev: KeyboardEvent) => {
      const digitando = ev.target instanceof HTMLElement && ev.target.closest("input, textarea, [contenteditable]");
      if ((ev.key === "n" && (ev.metaKey || ev.ctrlKey)) || (ev.key === "n" && !digitando && !ev.metaKey && !ev.ctrlKey && !ev.altKey)) {
        ev.preventDefault(); campo.current?.focus();
      }
    };
    window.addEventListener("keydown", tecla);
    return () => window.removeEventListener("keydown", tecla);
  }, []);
  useEffect(() => { if (!desfazer) return; const t = setTimeout(() => setDesfazer(null), 6000); return () => clearTimeout(t); }, [desfazer]);

  const criar = () => {
    if (!previa?.titulo) return;
    const t = adicionar({ ...previa, status: "a_fazer" });
    setRapida("");
    if (ajustes.visao === "semana" && t.data) setSemana(inicioDaSemana(t.data));
  };
  const alternar = useCallback((t: Tarefa) => { void salvar(t.status === "feito" ? reabrir(t) : concluir(t)).catch(() => undefined); }, []);
  const mover = useCallback((id: string, mudanca: Partial<Tarefa>) => { void editar(id, (t) => ({ ...t, ...mudanca })).catch(() => undefined); }, []);
  const novaEm = (campos: Partial<Tarefa>) => {
    const t = adicionar({ titulo: "Nova tarefa", ...campos });
    setAbertaId(t.id);
  };
  const tirar = (id: string) => {
    const t = e.tarefas.find((x) => x.id === id);
    void apagar(id).catch(() => undefined);
    setAbertaId(null);
    if (t) setDesfazer({ id, titulo: t.titulo });
  };

  if (!ajustes.eu.nome) return <BoasVindas onClose={onClose} onAbrirAjustes={onAbrirAjustes} />;

  const props = {
    tarefas: visiveis, hoje, pessoas, eu: ajustes.eu, abertaId, ocultarFeitasAntigas: ajustes.ocultarFeitasAntigas,
    onAbrir: setAbertaId, onAlternar: alternar, onMover: mover, onNovaEm: novaEm, onCriar: (campos: Partial<Tarefa> & { titulo: string }) => { adicionar(campos); },
  };
  const compartilhada = !!ajustes.pastaCompartilhada;

  return (
    <div className="tarefas">
      <div className="tarefas-principal">
        <header className="tarefas-topo">
          <div className="tarefas-linha">
            <h1 className="tarefas-titulo">Tarefas</h1>
            <span className={`tarefas-sinc truncate max-w-[16rem]${e.erro ? " is-erro" : compartilhada ? "" : " is-local"}`}
              title={e.erro || (compartilhada ? `Pasta compartilhada: ${ajustes.pastaCompartilhada}` : "Só neste computador")}>
              {e.erro ? "Erro ao sincronizar" : compartilhada ? `Compartilhada${pessoas.length > 1 ? ` com ${pessoas.filter((p) => p.id !== ajustes.eu.id).map((p) => p.nome.split(" ")[0]).join(", ")}` : ""}` : "Só neste computador"}
            </span>
            <span className="flex-1" />
            <SegmentedControl size="sm" value={ajustes.visao} onChange={(visao) => salvarAjustes({ visao })}
              options={[{ value: "dia", label: "Meu dia" }, { value: "quadro", label: "Quadro" }, { value: "semana", label: "Semana" }, { value: "lista", label: "Lista" }] satisfies { value: AjustesDasTarefas["visao"]; label: string }[]} />
            {acoes}
            <Button size="sm" variant="ghost" onClick={onAbrirAjustes} title="Ajustes: nome, cor e pasta compartilhada" aria-label="Ajustes das tarefas">⚙</Button>
            <Button size="sm" variant="ghost" onClick={onClose} aria-label="Fechar tarefas">×</Button>
          </div>

          <div className="tarefas-rapida glass">
            <span aria-hidden="true" className="text-lg text-accent">+</span>
            <input ref={campo} value={rapida} onChange={(ev) => setRapida(ev.target.value)} aria-label="Nova tarefa"
              placeholder='Nova tarefa… ex.: "Consulta dentista amanhã 14h #saude !alta @nome"  (N)'
              onKeyDown={(ev) => { if (ev.key === "Enter") criar(); if (ev.key === "Escape") setRapida(""); }} />
            <Button size="sm" variant="primary" disabled={!previa?.titulo} onClick={criar}>Criar</Button>
          </div>
          <div className="tarefas-previa" aria-live="polite">
            {previa?.titulo ? (
              <>
                <span>Vai criar:</span>
                <strong className="text-text-primary">{previa.titulo}</strong>
                {previa.data && <span className="ficha is-hoje">{rotuloDoDia(previa.data, hoje)}{previa.hora ? ` · ${previa.hora}` : ""}</span>}
                {previa.categoria !== "outro" && <span className="ficha">{categoria(previa.categoria).emoji} {categoria(previa.categoria).rotulo}</span>}
                {previa.prioridade !== "media" && <span className={`ficha prioridade-${previa.prioridade}`}>{ROTULO_DA_PRIORIDADE[previa.prioridade]}</span>}
                {previa.recorrencia !== "nunca" && <span className="ficha">↻ {ROTULO_DA_RECORRENCIA[previa.recorrencia]}</span>}
                {previa.responsavel !== ajustes.eu.id && <span className="ficha">para {pessoas.find((p) => p.id === previa.responsavel)?.nome}</span>}
              </>
            ) : (
              <span>Dica: escreva data (hoje, amanhã, sexta, 12/11), hora (14h, 9:30), #categoria, !prioridade, @pessoa ou "todo dia".</span>
            )}
          </div>

          <div className="tarefas-linha">
            <button type="button" className={`ficha ficha-filtro${ajustes.filtroPessoa === "todas" ? " is-ativa" : ""}`} onClick={() => salvarAjustes({ filtroPessoa: "todas" })}>Todas</button>
            <button type="button" className={`ficha ficha-filtro${ajustes.filtroPessoa === "minhas" ? " is-ativa" : ""}`} onClick={() => salvarAjustes({ filtroPessoa: "minhas" })}>Minhas</button>
            {pessoas.filter((p) => p.id !== ajustes.eu.id).map((p) => (
              <button key={p.id} type="button" className={`ficha ficha-filtro${ajustes.filtroPessoa === p.id ? " is-ativa" : ""}`} onClick={() => salvarAjustes({ filtroPessoa: p.id })}>
                <Avatar pessoa={p} /> {p.nome.split(" ")[0]}
              </button>
            ))}
            <span className="mx-1 h-4 w-px bg-[rgb(var(--c-border-subtle))]" />
            {CATEGORIAS.map((c) => (
              <button key={c.id} type="button" title={c.rotulo} className={`ficha ficha-filtro${ajustes.filtroCategoria === c.id ? " is-ativa" : ""}`}
                onClick={() => salvarAjustes({ filtroCategoria: ajustes.filtroCategoria === c.id ? "" : c.id })}>
                {c.emoji}<span className="ficha-rotulo"> {c.rotulo.split(" ")[0]}</span>
              </button>
            ))}
            <span className="flex-1" />
            {ajustes.visao === "semana" && (
              <span className="flex items-center gap-1">
                <Button size="sm" variant="ghost" aria-label="Semana anterior" onClick={() => setSemana((s) => somarDias(s, -7))}>‹</Button>
                <Button size="sm" variant="ghost" onClick={() => setSemana(inicioDaSemana(hoje))}>Esta semana</Button>
                <Button size="sm" variant="ghost" aria-label="Próxima semana" onClick={() => setSemana((s) => somarDias(s, 7))}>›</Button>
              </span>
            )}
            <Input size="sm" type="search" value={busca} onChange={(ev) => setBusca(ev.target.value)} placeholder="Buscar…" aria-label="Buscar tarefas" className="w-44" />
          </div>
          {e.erro && <p role="alert" className="tarefas-aviso is-erro">{e.erro}</p>}
          {desfazer && (
            <p className="tarefas-aviso" role="status">
              <span className="flex-1">"{desfazer.titulo}" apagada.</span>
              <Button size="sm" variant="ghost" onClick={() => { void desfazerApagar(desfazer.id); setDesfazer(null); }}>Desfazer</Button>
            </p>
          )}
        </header>

        <div className="tarefas-corpo">
          {!e.carregado ? <p className="text-sm text-text-muted">Carregando tarefas…</p>
            : ajustes.visao === "quadro" ? <Quadro {...props} />
            : ajustes.visao === "semana" ? <Semana {...props} inicio={semana} />
            : ajustes.visao === "lista" ? <Lista {...props} />
            : <MeuDia {...props} />}
        </div>
      </div>
      {aberta ? (
        <DetalheDaTarefa key={aberta.id} t={aberta} todas={e.tarefas} pessoas={pessoas} eu={ajustes.eu} hoje={hoje}
          onMudar={(t) => void salvar(t).catch(() => undefined)} onApagar={tirar} onAbrir={setAbertaId} onFechar={() => setAbertaId(null)} />
      ) : lateral}
    </div>
  );
}

function BoasVindas({ onClose, onAbrirAjustes }: { onClose: () => void; onAbrirAjustes: () => void }) {
  const [nome, setNome] = useState("");
  const [cor, setCor] = useState<string>(CORES_DE_PESSOA[0]);
  return (
    <div className="tarefas h-full overflow-auto">
      <div className="tarefas-boas-vindas glass">
        <div className="flex items-center gap-2">
          <h1 className="tarefas-titulo flex-1">Suas tarefas, num lugar só</h1>
          <Button size="sm" variant="ghost" aria-label="Fechar" onClick={onClose}>×</Button>
        </div>
        <p className="text-sm text-text-secondary">
          Médico, faculdade, trabalho, casa: crie digitando uma frase, veja o seu dia, arraste no quadro.
          Para dividir com outra pessoa, escolha depois uma pasta compartilhada (Google Drive, iCloud, Dropbox) em Ajustes.
        </p>
        <label className="text-sm text-text-primary" htmlFor="tarefas-nome">Como você se chama?</label>
        <Input id="tarefas-nome" autoFocus value={nome} onChange={(e) => setNome(e.target.value)} placeholder="Seu nome"
          onKeyDown={(e) => { if (e.key === "Enter" && nome.trim()) salvarAjustes({ eu: { id: "", nome: nome.trim(), cor, atualizadaEm: "" } }); }} />
        <div className="flex items-center gap-2">
          <span className="text-sm text-text-secondary">Sua cor:</span>
          {CORES_DE_PESSOA.map((c) => (
            <button key={c} type="button" aria-label={`Cor ${c}`} aria-pressed={cor === c} className="tarefa-cor" style={{ background: c, outlineColor: cor === c ? c : "transparent" }} onClick={() => setCor(c)} />
          ))}
        </div>
        <div className="flex gap-2">
          <Button variant="primary" disabled={!nome.trim()} onClick={() => salvarAjustes({ eu: { id: "", nome: nome.trim(), cor, atualizadaEm: "" } })}>Começar</Button>
          <Button variant="ghost" onClick={onAbrirAjustes}>Já tenho uma pasta compartilhada</Button>
        </div>
      </div>
    </div>
  );
}
