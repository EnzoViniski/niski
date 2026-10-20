import { useEffect, useState } from "react";
import { haQuantoTempo } from "../../lib/tempo";
import { nomeDaPlataforma } from "../../lib/plataformasDeAgente";
import { ROTULOS_DA_ATIVIDADE, type ItemDeAtividade, type TipoDeAtividade } from "./tipos";

const FILTROS: { id: "tudo" | TipoDeAtividade; rotulo: string }[] = [
  { id: "tudo", rotulo: "Tudo" }, { id: "precisa_de_voce", rotulo: "Precisa de você" },
  { id: "terminou", rotulo: "Terminou" }, { id: "erro", rotulo: "Erro" },
];
export function ListaDeAtividade({ itens, carregado, erro, marcar, abrir }: {
  itens: ItemDeAtividade[]; carregado: boolean; erro: string | null;
  marcar: (ids: string[]) => Promise<void>; abrir: (item: ItemDeAtividade) => void;
}) {
  const [filtro, setFiltro] = useState("tudo");
  const [falha, setFalha] = useState<string | null>(null);
  const [salvando, setSalvando] = useState(false);
  const [agora, setAgora] = useState(Date.now);
  useEffect(() => {
    const timer = window.setInterval(() => setAgora(Date.now()), 60_000);
    return () => window.clearInterval(timer);
  }, []);
  const lista = itens.filter(item => filtro === "tudo" || item.tipo === filtro
    || filtro === "terminou" && item.tipo === "subagente_terminou" || filtro === "erro" && item.tipo === "limite");
  const naoVistos = itens.filter(item => !item.visto);
  async function ver(ids: string[], item?: ItemDeAtividade) {
    setSalvando(true); setFalha(null);
    try { await marcar(ids); if (item) abrir(item); }
    catch { setFalha("Não foi possível marcar como lido. Tente novamente."); }
    finally { setSalvando(false); }
  }
  return <div className="flex min-h-0 flex-1 flex-col gap-3 p-3">
    <div className="flex flex-wrap gap-1" aria-label="Filtrar atividade">
      {FILTROS.map(({ id, rotulo }) => <button key={id} type="button" aria-pressed={filtro === id}
        className={`rounded px-2 py-1 text-xs ${filtro === id ? "bg-accent-muted text-accent" : "text-text-muted hover:text-text-primary"}`}
        onClick={() => setFiltro(id)}>{rotulo}</button>)}
    </div>
    <button type="button" className="self-end text-xs text-accent disabled:opacity-40" disabled={salvando || !naoVistos.length}
      onClick={() => void ver(naoVistos.map(item => item.id))}>Marcar tudo como lido</button>
    {(erro || falha) && <p role="alert" className="text-xs text-danger">{falha ?? erro}</p>}
    {!carregado && !erro && <p role="status" className="text-xs text-text-muted">Carregando atividade…</p>}
    {carregado && !lista.length && <p className="py-6 text-center text-sm text-text-muted">Nenhuma atividade neste filtro.</p>}
    <ul className="min-h-0 flex-1 space-y-1 overflow-y-auto" aria-label="Atividade dos agentes">
      {lista.map(item => <li key={item.id}>
        <button type="button" disabled={salvando} onClick={() => void ver(itens.filter(outro => outro.conversa === item.conversa && !outro.visto).map(outro => outro.id), item)}
          className={`w-full rounded border p-3 text-left hover:bg-overlay/5 ${item.visto ? "border-border-subtle" : "border-accent/40 bg-accent-muted"}`}>
          <span className="flex items-start justify-between gap-2 text-xs">
            <span className={item.tipo === "erro" || item.tipo === "limite" ? "text-danger" : "text-accent"}>{ROTULOS_DA_ATIVIDADE[item.tipo]}</span>
            <span className="shrink-0 text-text-muted">{haQuantoTempo(item.desde_ms, agora, "longo")}</span>
          </span>
          <span className="mt-1 block truncate text-sm text-text-primary">{item.titulo}</span>
          <span className="mt-1 block text-xs text-text-muted">{item.projeto} · {nomeDaPlataforma(item.plataforma)} · {item.conta}</span>
          <span className="sr-only">{item.visto ? "Lido" : "Não lido"}</span>
        </button>
      </li>)}
    </ul>
  </div>;
}
