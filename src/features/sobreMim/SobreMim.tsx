// Fork (Sobre mim): a tela onde o usuário conta de si para os agentes. Usada nas Configurações do
// desktop e no celular (`src/mobile/telas/SobreMim.tsx`); sem `style` inline (o celular manda CSP).
import { useState } from "react";
import { Button, Textarea } from "../../components/ui";
import { FolderIcon, SparkleIcon } from "../../components/ui/PixelIcon";
import { contarCaracteres, promptParaOrganizar, type ChamarSobreMim, type EstadoDoSobreMim, type PropostaDoAgente } from "./apiDoSobreMim";
import { useSobreMim, type SituacaoDaArea } from "./useSobreMim";
import "./sobreMim.css";

interface Props {
  chamar: ChamarSobreMim;
  /** Só no desktop: mostra a pasta no Finder. */
  abrirPasta?: (pasta: string) => void;
  atrasoMs?: number;
}

function Situacao({ situacao }: { situacao?: SituacaoDaArea }) {
  if (!situacao) return null;
  if (situacao.fase === "salvando") return <span className="text-text-muted">Salvando…</span>;
  if (situacao.fase === "salvo") return <span className="text-text-muted">Salvo</span>;
  return <span role="alert" className="text-danger">Não salvou: {situacao.mensagem}</span>;
}

function Sugestao({ proposta, estado, ocupado, onAceitar, onDescartar }: {
  proposta: PropostaDoAgente; estado: EstadoDoSobreMim; ocupado: boolean;
  onAceitar: (area: string, texto: string) => void; onDescartar: () => void;
}) {
  const [area, setArea] = useState(proposta.area);
  const [texto, setTexto] = useState(proposta.texto);
  return (
    <li className="sobre-mim-sugestao">
      <Textarea size="sm" rows={2} value={texto} aria-label="Texto da sugestão" disabled={ocupado}
        onChange={(evento) => setTexto(evento.target.value)} />
      <div className="sobre-mim-sugestao-acoes">
        <label className="flex items-center gap-2 text-[11px] text-text-secondary">
          Vai para
          <select className="field px-2 py-1 text-xs" value={area} disabled={ocupado} aria-label="Área da sugestão"
            onChange={(evento) => setArea(evento.target.value)}>
            {estado.areas.map((item) => <option key={item.id} value={item.id}>{item.titulo}</option>)}
          </select>
        </label>
        {proposta.origem && <span className="text-[11px] text-text-muted">Sugerido por {proposta.origem}</span>}
        <span className="flex-1" />
        <Button size="sm" variant="ghost" disabled={ocupado} onClick={onDescartar}>Descartar</Button>
        <Button size="sm" disabled={ocupado || !texto.trim()} onClick={() => onAceitar(area, texto)}>Aceitar</Button>
      </div>
    </li>
  );
}

export function SobreMim({ chamar, abrirPasta, atrasoMs }: Props) {
  const tela = useSobreMim(chamar, atrasoMs);
  const { estado, erro, aviso, ocupado } = tela;

  async function copiarPrompt() {
    if (!estado) return;
    try {
      await navigator.clipboard.writeText(promptParaOrganizar(estado.pasta));
      tela.setAviso("Pedido copiado. Cole numa conversa com qualquer agente: ele só propõe, quem grava aqui é você.");
    } catch {
      tela.setAviso("Não consegui copiar. Peça a um agente para ler a pasta e organizar cada área sem gravar nada.");
    }
  }

  if (!estado) {
    return erro
      ? <p role="alert" className="glass p-5 text-xs text-danger">{erro}</p>
      : <p className="glass p-5 text-xs text-text-muted">Carregando…</p>;
  }

  const preenchidas = estado.areas.filter((area) => area.texto.trim()).length;
  return (
    <div className="sobre-mim">
      <section className="glass p-5 space-y-3">
        <div>
          <h3 className="text-sm font-medium text-text-primary">Tudo o que seus agentes devem saber sobre você</h3>
          <p className="mt-2 max-w-prose text-xs leading-relaxed text-text-secondary">
            Escreva do seu jeito, em tópicos ou texto corrido. Salva sozinho enquanto você digita.
            Os agentes que você abrir pelo app (Claude e Codex, inclusive nos fluxos) recebem este texto
            junto da conversa, e ele vai para o provedor de cada um. Conversas já abertas veem as mudanças quando reiniciam.
          </p>
          <p className="mt-2 text-[11px] text-text-muted">
            {preenchidas === 0 ? "Nenhuma área preenchida ainda." : `${preenchidas} de ${estado.areas.length} áreas preenchidas.`}
            {" "}Fica só neste computador, em <span className="break-all">{estado.pasta}</span>.
          </p>
        </div>
        <div className="flex flex-col gap-2">
          <label className="flex items-center gap-2 text-xs text-text-secondary">
            <input type="checkbox" checked={estado.ajustes.ativo} disabled={ocupado}
              onChange={(evento) => void tela.mudarAjustes({ ativo: evento.target.checked })} />
            Compartilhar com meus agentes
          </label>
          <label className="flex items-center gap-2 text-xs text-text-secondary">
            <input type="checkbox" checked={estado.ajustes.sugestoes} disabled={ocupado || !estado.ajustes.ativo}
              onChange={(evento) => void tela.mudarAjustes({ sugestoes: evento.target.checked })} />
            Deixar os agentes sugerirem o que aprenderem sobre mim
          </label>
        </div>
        <div className="flex flex-wrap gap-2">
          {abrirPasta && (
            <Button size="sm" variant="ghost" onClick={() => abrirPasta(estado.pasta)}>
              <FolderIcon className="h-3.5 w-3.5" aria-hidden /> Abrir pasta
            </Button>
          )}
          <Button size="sm" variant="ghost" onClick={() => void copiarPrompt()}>
            <SparkleIcon className="h-3.5 w-3.5" aria-hidden /> Pedir para um agente organizar
          </Button>
        </div>
        {erro && <p role="alert" className="text-xs text-danger">{erro}</p>}
        {aviso && <p role="status" className="text-xs text-text-secondary">{aviso}</p>}
      </section>

      <section className="glass p-5 space-y-3" aria-labelledby="sobre-mim-sugestoes">
        <div className="flex items-center gap-2">
          <h3 id="sobre-mim-sugestoes" className="text-sm font-medium text-text-primary">Sugestões dos agentes</h3>
          {estado.propostas.length > 0 && <span className="sobre-mim-contador">{estado.propostas.length}</span>}
          <span className="flex-1" />
          <Button size="sm" variant="ghost" onClick={() => void tela.recarregar()}>Atualizar</Button>
        </div>
        {estado.propostas.length === 0
          ? <p className="text-xs text-text-muted">
              Quando um agente aprender algo novo sobre você numa conversa, a sugestão aparece aqui.
              Nada entra no seu Sobre mim sem você aceitar.
            </p>
          : <ul className="space-y-3">
              {estado.propostas.map((proposta, indice) => (
                <Sugestao key={`${indice}-${proposta.linha}`} proposta={proposta} estado={estado} ocupado={ocupado}
                  onAceitar={(area, texto) => void tela.aceitar(proposta, area, texto)}
                  onDescartar={() => void tela.descartar(proposta)} />
              ))}
            </ul>}
      </section>

      <div className="sobre-mim-areas">
        {estado.areas.map((area) => {
          const texto = tela.textoDa(area.id);
          const total = contarCaracteres(texto);
          const campo = `sobre-mim-${area.id}`;
          return (
            <section key={area.id} className="glass p-5 space-y-2 sobre-mim-area">
              <label htmlFor={campo} className="block text-sm font-medium text-text-primary">{area.titulo}</label>
              <p className="text-[11px] leading-relaxed text-text-muted">{area.descricao}</p>
              <Textarea id={campo} rows={7} value={texto} spellCheck
                onChange={(evento) => tela.editar(area.id, evento.target.value)}
                onBlur={() => void tela.salvar(area.id)} />
              <p className="flex flex-wrap justify-between gap-2 text-[11px]">
                <span className={total > estado.max_por_area ? "text-danger" : "text-text-muted"}>
                  {total.toLocaleString("pt-BR")} de {estado.max_por_area.toLocaleString("pt-BR")} caracteres
                </span>
                <Situacao situacao={tela.situacao[area.id]} />
              </p>
            </section>
          );
        })}
      </div>
    </div>
  );
}
