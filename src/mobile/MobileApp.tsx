// Fork (Onda 2.0): este arquivo ficou só com a navegação do celular (topo, rotas, início e projeto).
// Listas em `listas/`, a conversa em `conversa/`. Os nomes que outros arquivos importam daqui
// continuam exportados por este módulo.
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Input } from "../components/ui/Input";
import { SegmentedControl } from "../components/ui/SegmentedControl";
import { MarcaDoProduto } from "../components/ui/MarcaDoProduto";
import { ArrowLeftIcon, ReloadIcon } from "../components/ui/PixelIcon";
import { PairingScreen } from "./Chat";
import { useRota, type Rota } from "./rota";
import { useDados } from "./useDados";
import { NOME_DO_PRODUTO } from "../lib/identidade";
import { haQuanto } from "../lib/tempo";
import { lerSessao, gravarSessao } from "./sessaoDaAba";
import { filterConversations } from "./listas/filtro";
import { Secao, Vazio } from "./listas/Secao";
import { ProjectList } from "./listas/ProjectList";
import { ConversationList, ConversasPorEstado } from "./listas/ConversationList";
import { Ocultas } from "./listas/Ocultas";
import { NovaSessao } from "./listas/NovaSessao";
import { ConversationDetail } from "./conversa/ConversationDetail";
import { CabecalhoDaConversa } from "./conversa/CabecalhoDaConversa";

export { haQuanto, filterConversations, Secao, ProjectList, ConversationList, ConversasPorEstado, Ocultas, NovaSessao, ConversationDetail };
export { juntarPaginas, agruparMensagens } from "./conversa/useTimeline";
export { ModoDoAgente } from "./conversa/ModoDoAgente";

/** Quanto tempo faz que o desktop publicou. O celular mostra a idade e quem julga se a lista ainda
 *  vale é quem está lendo — nada expira sozinho. */
function idadeDaLista(publishedAtMs: number): string | null {
  if (!publishedAtMs) return "Lista montada a partir das conversas: o desktop ainda não publicou.";
  const minutos = Math.floor((Date.now() - publishedAtMs) / 60_000);
  if (minutos < 5) return null;
  if (minutos < 60) return `Lista publicada há ${minutos} min.`;
  return `Lista publicada há ${Math.floor(minutos / 60)} h.`;
}

/* ─────────────────────────────────────────────────────────────── estrutura */

function TopBar({ titulo, subtitulo, onVoltar, onAtualizar }: {
  titulo: ReactNode; subtitulo?: ReactNode; onVoltar?: () => void; onAtualizar: () => Promise<void>;
}) {
  const [girando, setGirando] = useState(false);
  return (
    <header className="m-topo glass glass-strong">
      {onVoltar
        ? <button type="button" className="btn btn-ghost m-topo-icone" aria-label="Voltar" onClick={onVoltar}>
            <ArrowLeftIcon aria-hidden className="h-5 w-5" />
          </button>
        : <MarcaDoProduto className="m-topo-logo" />}
      <div className="m-topo-texto">
        <div className="m-topo-titulo">{titulo}</div>
        {subtitulo && <div className="m-topo-sub">{subtitulo}</div>}
      </div>
      <button type="button" className="btn btn-ghost m-topo-icone" aria-label="Atualizar" disabled={girando}
        onClick={() => { setGirando(true); void onAtualizar().finally(() => setGirando(false)); }}>
        <ReloadIcon aria-hidden className={`h-5 w-5${girando ? " anexo-girando" : ""}`} />
      </button>
    </header>
  );
}

/* ─────────────────────────────────────────────────────────────── app */

const CHAVE_FILTRO = "omni-filtro";

export function MobileApp() {
  const { rota, ir, voltar } = useRota();
  const { conversations, attention, catalog, carregado, error, semAcesso, refresh, atualizar, parear } = useDados(rota.tela === "conversa" ? rota.conversa : null);
  const naoVistas = useMemo(() => new Set(attention.map((item) => item.id)), [attention]);
  const [onlyAttention, setOnlyAttention] = useState(() => lerSessao(CHAVE_FILTRO) === "atencao");
  const [query, setQuery] = useState("");

  useEffect(() => { gravarSessao(CHAVE_FILTRO, onlyAttention ? "atencao" : null); }, [onlyAttention]);


  if (semAcesso) return <main className="m-shell"><PairingScreen parear={parear} /></main>;

  const conversa = rota.tela === "conversa" ? conversations.find(c => c.id === rota.conversa) ?? null : null;
  // As ocultas saem das listas; continuam em `conversations` para abrir pela seção Ocultas.
  const visiveis = conversations.filter(c => !c.hidden);
  const projetoId = rota.tela === "projeto" ? rota.projeto : conversa?.project_id ?? null;
  const projeto = catalog?.projects.find(p => p.id === projetoId) ?? null;
  const perfis = catalog?.profiles ?? [];
  const aviso = catalog ? idadeDaLista(catalog.published_at_ms) : null;

  const pai: Rota = rota.tela === "conversa" && conversa ? { tela: "projeto", projeto: conversa.project_id } : { tela: "inicio" };
  const abrirConversa = (id: string) => ir({ tela: "conversa", conversa: id });

  let titulo: ReactNode = <span className="pixel-text">{NOME_DO_PRODUTO}</span>;
  let subtitulo: ReactNode = null;
  let conteudo: ReactNode;

  if (rota.tela === "conversa") {
    if (conversa) {
      titulo = conversa.title;
      subtitulo = <CabecalhoDaConversa conversa={conversa} projeto={projeto} />;
      conteudo = <ConversationDetail key={conversa.id} conversation={conversa} refresh={refresh} abrirConversa={abrirConversa} />;
    } else {
      titulo = "Conversa";
      conteudo = <Vazio>{carregado ? "Essa conversa não existe mais no PC." : "Carregando conversa…"}</Vazio>;
    }
  } else if (rota.tela === "projeto") {
    if (projeto) {
      const doProjeto = (onlyAttention ? attention : visiveis).filter(c => c.project_id === projeto.id);
      const daLista = filterConversations(doProjeto, query, perfis, catalog?.projects);
      const aguardando = attention.filter(c => c.project_id === projeto.id).length;
      titulo = projeto.name;
      subtitulo = <span className="truncate">{projeto.path}</span>;
      conteudo = <>
        {catalog && projeto.path && <NovaSessao project={projeto} catalog={catalog} onCriada={refresh} />}
        <Secao titulo="Conversas">
          <SegmentedControl<"todas" | "atencao"> value={onlyAttention ? "atencao" : "todas"}
            onChange={valor => setOnlyAttention(valor === "atencao")}
            options={[{ value: "todas", label: "Todas" }, { value: "atencao", label: `Atenção (${aguardando})` }]} />
          <Input type="search" value={query} onChange={event => setQuery(event.target.value)}
            placeholder="Pesquisar conversas" aria-label="Pesquisar conversas" className="m-busca" />
          {onlyAttention
            ? <ConversationList conversations={daLista} perfis={perfis} abrir={abrirConversa} naoVistas={naoVistas}
                vazio="Nada esperando por você neste projeto." />
            : <ConversasPorEstado conversations={daLista} perfis={perfis} abrir={abrirConversa} naoVistas={naoVistas}
                vazio={query ? "Nenhuma conversa encontrada neste projeto." : "Nenhuma conversa neste projeto ainda."} />}
          {!query && !onlyAttention && (
            <Ocultas conversas={conversations.filter(c => c.hidden && c.project_id === projeto.id)} perfis={perfis} abrir={abrirConversa} />
          )}
        </Secao>
      </>;
    } else {
      titulo = "Projeto";
      conteudo = <Vazio>{carregado ? "Esse projeto não está mais aberto no PC." : "Carregando projeto…"}</Vazio>;
    }
  } else {
    const baseDaBusca = onlyAttention ? attention : visiveis;
    const resultadoDaBusca = filterConversations(baseDaBusca, query, perfis, catalog?.projects);
    const trabalhandoAgora = visiveis.filter(c => c.state === "working").length;
    const projetos = catalog?.projects ?? [];
    conteudo = <>
      {aviso && <p className="m-aviso">{aviso}</p>}
      {/* Painel do início: o que está acontecendo agora, em três números. "Precisa de você" filtra. */}
      <div className="m-resumo">
        <button type="button" className="m-resumo-item glass" data-tom="ativo" onClick={() => setOnlyAttention(false)}>
          <span className="m-resumo-num">{trabalhandoAgora}</span>
          <span className="m-resumo-rotulo">Trabalhando</span>
        </button>
        <button type="button" className="m-resumo-item glass" data-tom="alerta" aria-pressed={onlyAttention}
          onClick={() => setOnlyAttention(valor => !valor)}>
          <span className="m-resumo-num">{attention.length}</span>
          <span className="m-resumo-rotulo">Aguardando</span>
        </button>
        <button type="button" className="m-resumo-item glass" data-tom="neutro" onClick={() => setOnlyAttention(false)}>
          <span className="m-resumo-num">{projetos.length}</span>
          <span className="m-resumo-rotulo">Projetos</span>
        </button>
      </div>
      <Input type="search" value={query} onChange={event => setQuery(event.target.value)}
        placeholder="Pesquisar conversas" aria-label="Pesquisar conversas" className="m-busca" />
      {query
        ? <ConversationList conversations={resultadoDaBusca} perfis={perfis} projetos={projetos}
            mostrarProjeto abrir={abrirConversa} naoVistas={naoVistas} vazio="Nenhuma conversa encontrada." />
        : onlyAttention
        ? <Secao titulo="Precisa de você">
            <ConversationList conversations={attention} perfis={perfis} projetos={projetos} mostrarProjeto abrir={abrirConversa}
              naoVistas={naoVistas} vazio="Nada esperando por você agora." />
          </Secao>
        : <>
            {visiveis.length > 0 && (
              <Secao titulo="Recentes">
                <ConversationList conversations={visiveis.slice(0, 3)} perfis={perfis} projetos={projetos} mostrarProjeto
                  abrir={abrirConversa} naoVistas={naoVistas} vazio="" />
              </Secao>
            )}
            <Secao titulo="Projetos">
              <ProjectList projects={projetos} conversations={visiveis} abrir={id => ir({ tela: "projeto", projeto: id })} />
            </Secao>
          </>}
    </>;
  }

  return <div className="m-shell">
    <TopBar titulo={titulo} subtitulo={subtitulo} onAtualizar={atualizar}
      onVoltar={rota.tela === "inicio" ? undefined : () => voltar(pai)} />
    <main className="m-conteudo">
      {error && <p role="alert" className="m-alerta">Não foi possível conectar: {error}</p>}
      {conteudo}
    </main>
  </div>;
}
