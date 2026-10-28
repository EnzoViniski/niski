// Fork (Onda 2.0): o rodapé da conversa — avisos, campo de mensagem, ajustes, renomear, modo,
// bifurcar e ocultar — saiu do `ConversationDetail` (antes no `MobileApp.tsx`) sem mudança de lógica.
import { useCallback, useState } from "react";
import { ArchiveIcon, EditIcon, GitBranchIcon } from "../../components/ui/PixelIcon";
import type { Conversation, Timeline } from "../api";
import { Composer } from "../Composer";
import { ListaDeTarefas } from "../ListaDeTarefas";
import { modoVigente, SeloDoModo } from "../SeloDoModo";
import { AjustesDaConversa, type Ajustes } from "../AjustesDaConversa";
import type { Anexo } from "../anexos";
import type { ItemDaBarra } from "./comandosDaBarra"; // Lista do `/`.
import { ControlesDaConversa, type ComandoDaConversa, type ModoDePermissao } from "./ControlesDaConversa";
import { TelaDeAgendamentos } from "../../features/agendamentos/TelaDeAgendamentos"; // Agenda da conversa (3D).
import { RenomearConversa } from "./RenomearConversa";
import { TrocarAgente } from "./TrocarAgente"; // Continuar a mesma conversa com outro agente.

export interface RodapeDaConversaProps {
  conversation: Conversation;
  timeline: Timeline["timeline"] | undefined;
  falhaEnvio: string | null;
  error: string | null;
  anexoGrande: boolean;
  /** Agente trabalhando ou mensagem enviada ainda sem resposta. */
  ativo: boolean;
  trabalhando: boolean;
  /** Sessão aberta e agente parado: dá para mandar mesmo com a tela não reconhecida. */
  destravavel: boolean;
  texto: string;
  onTexto: (texto: string) => void;
  anexos: Anexo[];
  onAdicionar: (arquivos: File[]) => void;
  onRemover: (id: string) => void;
  onEnviar: () => void;
  podeEnviar: boolean;
  busy: boolean;
  onRenomear: (nome: string) => Promise<void>;
  onAjustar: (ajustes: Ajustes) => Promise<void>;
  onRetomar: (ajustes: Ajustes) => Promise<void>;
  onModo: (alvo: ModoDePermissao) => Promise<void>;
  onComando: (comando: ComandoDaConversa, argumento?: string) => Promise<void>;
  bifurcando: boolean;
  onBifurcar: () => Promise<void>;
  onOcultar: (oculta: boolean) => Promise<void>;
  /** Só na web e no celular: no PC a lateral já bifurca e oculta. */
  abrirConversa?: (id: string) => void;
  onParar?: () => void;
  parando: boolean;
  buscarArquivos: (busca: string) => Promise<string[]>;
  /** Lista do `/` no campo. */
  buscarComandos?: () => Promise<ItemDaBarra[]>;
}

export function RodapeDaConversa({ conversation, timeline, falhaEnvio, error, anexoGrande, ativo, trabalhando, destravavel,
  texto, onTexto, anexos, onAdicionar, onRemover, onEnviar, podeEnviar, busy, onRenomear, onAjustar, onRetomar, onModo, onComando,
  bifurcando, onBifurcar, onOcultar, abrirConversa, onParar, parando, buscarArquivos, buscarComandos }: RodapeDaConversaProps) {
  const [editandoNome, setEditandoNome] = useState(false);
  const [agendando, setAgendando] = useState(false);
  const fecharAgenda = useCallback(() => setAgendando(false), []);
  const capability = conversation.capabilities;

  return <>
    {agendando && <TelaDeAgendamentos inicial={{ conversa: conversation.id, titulo: conversation.title, prompt: texto }} onFechar={fecharAgenda} />}
    {(falhaEnvio || error) && <p role="alert" className="m-alerta">{falhaEnvio ?? error}</p>}
    {anexoGrande && <p className="m-aviso">Tire o anexo acima de 25 MB para enviar.</p>}
    {/* O motivo genérico só aparece com o agente parado: trabalhando, o "ocupado" é esperado e o
        indicador acima já explica. */}
    {!ativo && !destravavel && capability?.reason && <p className="m-dica">{capability.reason}</p>}

    <Composer texto={texto} onTexto={onTexto} anexos={anexos} onAdicionar={onAdicionar} onRemover={onRemover}
      onEnviar={onEnviar} podeEnviar={podeEnviar} ocupado={busy}
      placeholder={!capability?.prompt ? (capability?.escolha ? "Responda a pergunta acima para continuar"
        : capability?.enfileirar ? "Agente trabalhando… sua mensagem entra na fila"
        : trabalhando ? "Agente trabalhando… escreva a próxima mensagem"
        : destravavel ? "Mensagem para o agente…"
        : conversation.capabilities ? "Sem sessão ativa" : "Conversa fechada — escolha conta, modelo e esforço e retome abaixo") : "Mensagem para o agente…"}
      toolbar={<>
        {/* Lista de tarefas do agente, fixa acima do campo. */}
        {timeline?.tarefas && <ListaDeTarefas tarefas={timeline.tarefas} conversa={conversation.id} ativo={ativo} />}
        {editandoNome ? <RenomearConversa nome={conversation.title} onFechar={() => setEditandoNome(false)}
          onSalvar={(nome) => { setEditandoNome(false); void onRenomear(nome); }} /> : null}
      </>}
      chips={<>
        <button type="button" className="composer-botao" title="Renomear conversa" aria-label="Renomear conversa"
          onClick={() => setEditandoNome(true)}>
          <EditIcon aria-hidden className="h-4 w-4" />
        </button>
        {/* Modo de permissão em que o agente está (tela → transcript). */}
        {conversation.capabilities && <SeloDoModo modo={modoVigente(conversation.mode, timeline?.modo_de_permissao)} />}
        {/* Bifurcar e ocultar só na web e no celular (quem passa `abrirConversa`): no PC a lateral já faz. */}
        {abrirConversa && conversation.live && (conversation.provider === "claude" || conversation.provider === "codex") && (
          <button type="button" className="composer-botao" disabled={bifurcando}
            title="Bifurcar: abre uma conversa nova com todo o histórico desta, que segue independente" aria-label="Bifurcar conversa"
            onClick={() => void onBifurcar()}>
            <GitBranchIcon aria-hidden className={`h-4 w-4${bifurcando ? " anexo-girando" : ""}`} />
          </button>
        )}
        {abrirConversa && (
          <button type="button" className={`composer-botao${conversation.hidden ? " is-ativo" : ""}`}
            title={conversation.hidden ? "Reabrir: a conversa volta para a lista" : "Ocultar da lista — fica em Ocultas, nada é apagado"}
            aria-label={conversation.hidden ? "Reabrir conversa na lista" : "Ocultar conversa da lista"}
            onClick={() => void onOcultar(!conversation.hidden)}>
            <ArchiveIcon aria-hidden className="h-4 w-4" />
          </button>
        )}
      </>}
      // Agendar também funciona com a conversa fechada ou durante turno.
      direita={<>
        <ControlesDaConversa conversaId={conversation.id} provider={conversation.provider}
          modo={modoVigente(conversation.mode, timeline?.modo_de_permissao)} fast={conversation.fast}
          pronto={Boolean(conversation.capabilities?.prompt) && !busy} onModo={onModo} onComando={onComando} onAgendar={() => setAgendando(true)} />
        {/* "Continuar com…" outro agente na mesma conversa (passagem de bastão, `TrocarAgente.tsx`). */}
        <TrocarAgente conversaId={conversation.id} provider={conversation.provider} profileId={conversation.profile_id}
          pronto={!busy && (Boolean(conversation.capabilities?.prompt) || !conversation.live)}
          mensagem={texto} titulo={conversation.title}
          onTrocado={(resultado) => { if (resultado.mensagem_enviada) onTexto(""); }} />
        {/* Modelo e esforço no canto direito, ao lado do Parar/Enviar, como no Codex. */}
        {(conversation.live || conversation.capabilities) ? (
          <AjustesDaConversa conversaId={conversation.id} pronto={Boolean(conversation.capabilities?.prompt) && !busy}
            onAplicar={onAjustar} />
        ) : (
          <AjustesDaConversa key="retomar" conversaId={conversation.id} pronto={!busy} onAplicar={onAjustar} onRetomar={onRetomar} />
        )}
      </>}
      onParar={onParar}
      parando={parando}
      buscarArquivos={buscarArquivos} buscarComandos={buscarComandos} />
  </>;
}
