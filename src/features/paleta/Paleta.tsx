import { useEffect, useMemo, useRef, useState } from "react";
import type { WorkspaceState } from "../../types/workspace";
import type { Conversation } from "../../mobile/api";
import { listAllFiles, resolveIgnoreList, type FileEntry } from "../files/filesService";
import { matchesShortcut, shortcutLabel } from "../../lib/shortcuts";
import { useListaDeAcoes } from "./acoes";
import { filtrarItens, moverSelecao, type ItemDaPaleta } from "./filtro";
import { buscarNoHistorico, listarConversas, type BuscaNoHistorico } from "./servico";

interface Props {
  workspaces: WorkspaceState[];
  projectPath: string | null;
  onProjeto: (workspaceId: string, projectId: string) => void;
  onConversa: (conversa: Conversation) => void;
  onArquivo: (path: string) => void;
}

export function Paleta({ workspaces, projectPath, onProjeto, onConversa, onArquivo }: Props) {
  const acoes = useListaDeAcoes();
  const [aberta, setAberta] = useState(false);
  const [busca, setBusca] = useState("");
  const [selecionado, setSelecionado] = useState(0);
  const [conversas, setConversas] = useState<Conversation[]>([]);
  const [arquivos, setArquivos] = useState<FileEntry[]>([]);
  const [historico, setHistorico] = useState<BuscaNoHistorico | null>(null);
  const [carregando, setCarregando] = useState(false);
  const [erro, setErro] = useState("");
  const [erroIndice, setErroIndice] = useState("");
  const lista = useRef<HTMLDivElement>(null);
  const campo = useRef<HTMLInputElement>(null);
  const filaDeBusca = useRef<Promise<unknown>>(Promise.resolve());
  const focoAnterior = useRef<HTMLElement | null>(null);
  const noHistorico = busca.startsWith("?");
  const texto = busca.slice(1).trim();

  useEffect(() => {
    function tecla(evento: KeyboardEvent) {
      if (!matchesShortcut(evento, "paleta")) return;
      evento.preventDefault(); evento.stopImmediatePropagation();
      setAberta((atual) => {
        if (!atual) focoAnterior.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        return !atual;
      });
    }
    window.addEventListener("keydown", tecla, true);
    return () => window.removeEventListener("keydown", tecla, true);
  }, []);

  useEffect(() => {
    if (!aberta) return;
    setBusca(""); setSelecionado(0); setErro(""); setErroIndice(""); setArquivos([]); setConversas([]);
    let cancelado = false;
    void listarConversas().then((dados) => { if (!cancelado) setConversas(dados); })
      .catch(() => { if (!cancelado) setErroIndice("Conversas indisponíveis. Tente abrir a paleta novamente."); });
    if (projectPath) void resolveIgnoreList(projectPath).then((ignorar) => listAllFiles(projectPath, ignorar))
      .then((dados) => { if (!cancelado) setArquivos(dados); })
      .catch(() => { if (!cancelado) setErroIndice("Arquivos indisponíveis. Tente abrir a paleta novamente."); });
    return () => { cancelado = true; focoAnterior.current?.focus(); };
  }, [aberta, projectPath]);

  useEffect(() => {
    setHistorico(null); setErro("");
    if (!aberta || !noHistorico || !texto) { setCarregando(false); return; }
    let cancelado = false;
    setCarregando(true);
    const timer = window.setTimeout(() => {
      // O transporte IPC não aborta o worker: serializa e descarta consultas já substituídas.
      filaDeBusca.current = filaDeBusca.current.catch(() => undefined).then(async () => {
        if (cancelado) return;
        try { const dados = await buscarNoHistorico(texto); if (!cancelado) setHistorico(dados); }
        catch { if (!cancelado) setErro("Não foi possível buscar no histórico."); }
        finally { if (!cancelado) setCarregando(false); }
      });
    }, 220);
    return () => { cancelado = true; window.clearTimeout(timer); };
  }, [aberta, noHistorico, texto]);

  const itens = useMemo<ItemDaPaleta[]>(() => {
    if (noHistorico) return (historico?.achados ?? []).flatMap((achado, indice) => {
      const conversa = conversas.find((item) => item.id === achado.conversa_id);
      return conversa ? [{ id: `achado-${indice}`, secao: "No histórico" as const, titulo: conversa.title,
        detalhe: `${achado.papel === "user" ? "Você" : "Agente"} · ${achado.data ? new Date(achado.data).toLocaleString("pt-BR") : "Sem data"} · ${achado.trecho}`,
        executar: () => onConversa(conversa) }] : [];
    });
    return filtrarItens([
      ...acoes.map((acao) => ({ id: `acao-${acao.id}`, secao: "Ações" as const, titulo: acao.titulo, executar: acao.executar })),
      ...workspaces.flatMap((workspace) => workspace.projects.map((projeto) => ({ id: `projeto-${projeto.id}`, secao: "Projetos" as const,
        titulo: projeto.name, detalhe: `${workspace.name} · ${projeto.path}`, executar: () => onProjeto(workspace.id, projeto.id) }))),
      ...conversas.map((conversa) => ({ id: `conversa-${conversa.id}`, secao: "Conversas" as const,
        titulo: conversa.title, detalhe: `${conversa.provider ?? ""} · ${workspaces.flatMap((w) => w.projects).find((p) => p.id === conversa.project_id)?.name ?? "Histórico"}`,
        executar: () => onConversa(conversa) })),
      ...arquivos.map((arquivo) => ({ id: `arquivo-${arquivo.path}`, secao: "Arquivos" as const, titulo: arquivo.name,
        detalhe: arquivo.path.slice(projectPath?.length ?? 0).replace(/^[\\/]/, ""), executar: () => onArquivo(arquivo.path) })),
    ], busca);
  }, [acoes, workspaces, conversas, arquivos, historico, noHistorico, busca, projectPath, onProjeto, onConversa, onArquivo]);

  useEffect(() => { setSelecionado((atual) => Math.max(0, Math.min(atual, itens.length - 1))); }, [itens.length]);
  useEffect(() => { lista.current?.querySelector('[aria-selected="true"]')?.scrollIntoView?.({ block: "nearest" }); }, [selecionado]);
  if (!aberta) return null;
  function fechar() { setAberta(false); }
  function escolher(item: ItemDaPaleta | undefined) {
    if (!item) return;
    try {
      const resultado = item.executar();
      if (resultado instanceof Promise) void resultado.then(fechar).catch(() => setErro("Não foi possível executar esta ação."));
      else fechar();
    } catch { setErro("Não foi possível executar esta ação."); }
  }

  return <div className="fixed inset-0 z-[80] flex justify-center bg-black/40 px-4 pt-[12vh]" onPointerDown={fechar}>
    <div role="dialog" aria-modal="true" aria-label="Paleta de comandos" className="popover flex h-fit max-h-[70vh] w-full max-w-2xl flex-col p-2"
      onPointerDown={(evento) => evento.stopPropagation()} onKeyDown={(evento) => {
        if (evento.key === "Escape") fechar();
        else if (evento.key === "Tab") { evento.preventDefault(); return; }
        else return;
        evento.preventDefault(); evento.stopPropagation();
      }}>
      <input ref={campo} autoFocus role="combobox" aria-expanded="true" aria-controls="paleta-resultados" aria-activedescendant={itens[selecionado] ? `paleta-${selecionado}` : undefined}
        aria-label="Buscar comandos, projetos, conversas e arquivos" className="field px-3 py-2 text-sm" value={busca}
        placeholder="Buscar ações, projetos, conversas, arquivos · ? texto no histórico"
        onChange={(evento) => { setBusca(evento.target.value); setSelecionado(0); }}
        onKeyDown={(evento) => {
          if (evento.key === "Enter") escolher(itens[selecionado]);
          else if (evento.key === "ArrowDown" || evento.key === "ArrowUp") setSelecionado((atual) => moverSelecao(atual, evento.key, itens.length));
          else return;
          evento.preventDefault(); evento.stopPropagation();
        }} />
      <div className="flex justify-between px-2 py-2 text-[11px] text-text-muted">
        <span>{shortcutLabel("paleta")} · ↑↓ navegar · Enter abrir · Esc fechar</span>
        <button type="button" tabIndex={-1} onClick={() => { setBusca(noHistorico ? "" : "?"); setSelecionado(0); campo.current?.focus(); }} className="text-accent">{noHistorico ? "Todos" : "No histórico"}</button>
      </div>
      {(erro || erroIndice) && <p role="alert" className="px-2 py-2 text-xs text-danger">{erro || erroIndice}</p>}
      {carregando && <p role="status" className="px-2 py-2 text-xs text-text-muted">Buscando no histórico…</p>}
      {!carregando && itens.length === 0 && <p className="px-2 py-3 text-xs text-text-muted">{noHistorico && !texto ? "Digite ? seguido do texto que procura." : "Nenhum resultado encontrado."}</p>}
      <div id="paleta-resultados" role="listbox" ref={lista} className="min-h-0 overflow-y-auto">
        {itens.map((item, indice) => <div key={item.id}>
          {itens[indice - 1]?.secao !== item.secao && <p className="px-2 pb-1 pt-3 text-[10px] uppercase tracking-wider text-text-muted">{item.secao}</p>}
          <div id={`paleta-${indice}`} role="option" aria-selected={indice === selecionado} onPointerMove={() => setSelecionado(indice)} onClick={() => escolher(item)}
            className={`cursor-default px-3 py-2 text-xs ${indice === selecionado ? "bg-accent-muted text-text-primary" : "text-text-secondary"}`}>
            <p className="truncate">{item.titulo}</p>{item.detalhe && <p className="mt-1 line-clamp-2 break-words text-[11px] text-text-muted">{item.detalhe}</p>}
          </div>
        </div>)}
      </div>
      {historico?.parcial && <p className="px-2 pt-2 text-xs text-text-muted">Busca parcial: limite de tempo, leitura ou resultados atingido. Refine o texto.</p>}
    </div>
  </div>;
}
