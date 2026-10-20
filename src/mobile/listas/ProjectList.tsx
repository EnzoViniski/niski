// Fork (Onda 2.0): saiu do `MobileApp.tsx` sem mudança de lógica.
import { AgentIcon } from "../../components/ui/AgentIcon";
import { FolderIcon } from "../../components/ui/PixelIcon";
import { haQuanto } from "../../lib/tempo";
import type { Conversation, Projects } from "../api";
import { Vazio } from "./Secao";
import { NOME_CURTO } from "../../lib/identidade";

export function ProjectList({ projects, conversations, abrir }: {
  projects: Projects["projects"]; conversations: Conversation[]; abrir: (id: string) => void;
}) {
  if (projects.length === 0) return <Vazio>Nenhum projeto conhecido ainda. Abra um projeto no {NOME_CURTO} do PC.</Vazio>;
  // Projeto mexido por último primeiro, como a lista de conversas.
  const ultimaAtividade = (id: string) => Math.max(0, ...conversations.filter(c => c.project_id === id).map(c => c.updated_at_ms ?? 0));
  const ordenados = [...projects].sort((a, b) => ultimaAtividade(b.id) - ultimaAtividade(a.id));
  return <ul className="m-lista">{ordenados.map(project => {
    const doProjeto = conversations.filter(c => c.project_id === project.id);
    const aguardando = doProjeto.filter(c => c.capabilities?.approve).length;
    const trabalhando = doProjeto.filter(c => c.state === "working").length;
    const ultima = haQuanto(Math.max(0, ...doProjeto.map(c => c.updated_at_ms ?? 0)) || undefined);
    const agentes = [...new Set(doProjeto.map(c => c.provider).filter((p): p is string => Boolean(p)))];
    return <li key={project.id}>
      <button type="button" className="m-card glass glass-hover" onClick={() => abrir(project.id)} title={project.path}
        aria-label={`Abrir projeto ${project.name}, ${doProjeto.length} conversa(s)`}>
        <span className="m-card-icone"><FolderIcon aria-hidden className="h-5 w-5" /></span>
        <span className="m-card-corpo">
          <span className="m-card-titulo">{project.name}</span>
          <span className="m-card-sub">
            {agentes.map(p => <AgentIcon key={p} provider={p} size={11} className="shrink-0" />)}
            <span className="truncate">{doProjeto.length === 1 ? "1 conversa" : `${doProjeto.length} conversas`}{ultima ? ` · ${ultima}` : ""}</span>
          </span>
        </span>
        <span className="m-card-lado">
          {aguardando > 0 && <span className="m-badge m-badge-alerta">{aguardando} aprovar</span>}
          {trabalhando > 0 && <span className="m-badge m-badge-ativo">{trabalhando} ativo</span>}
          <span className="m-badge">{doProjeto.length}</span>
        </span>
      </button>
    </li>;
  })}</ul>;
}
