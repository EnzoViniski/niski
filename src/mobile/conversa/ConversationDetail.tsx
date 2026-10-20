// Fork: controles oficiais aguardam a execução da ação antes de confirmar a troca.
import { aguardarAcao } from "./aguardarAcao";
import type { ComandoDaConversa, ModoDePermissao } from "./ControlesDaConversa";
// Fork (Onda 2.0): a conversa aberta (celular, web e aba Conversa do PC) saiu do `MobileApp.tsx` sem
// mudança de lógica. Timeline e rolagem em `useTimeline.ts`, envio em `useEnvio.ts`, `@` em
// `useMencoes.ts`, aprovação em `CartaoDePermissao.tsx` e o campo com os botões em `RodapeDaConversa.tsx`.
import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "../../components/ui/Button";
import { AttachmentIcon } from "../../components/ui/PixelIcon";
import { request, requestSeMudou, esquecerVersao, enviarTeclas, actionKey, ApiError, type Conversation, type Timeline } from "../api";
import { AgentWorking, BlocoDeAcoes, LinhaDeAviso, LinhaDeComando, MessageBubble } from "../Chat";
import { CartaoDePergunta } from "../CartaoDePergunta";
// Fork: conversa rica — plano do plan mode.
import { CartaoDoPlano } from "../CartaoDoPlano";
import type { Ajustes } from "../AjustesDaConversa";
import {
  ANEXO_MAX_BYTES,
  criarAnexo,
  descartarAnexo,
  enviarAnexo,
  montarPrompt,
  type Anexo, EVENTO_ANEXAR, EVENTO_ARRASTANDO } from "../anexos";
import { usePoll } from "../usePoll";
import { lerSessao, gravarSessao } from "../sessaoDaAba";
import { Vazio } from "../listas/Secao";
import { indiceDaMensagem, POLL_ATIVO_MS, POLL_PARADO_MS, useBlocos, useMensagensAntigas, useRolagem, type Bloco } from "./useTimeline";
import { useAcoesDaConversa, useConfirmacaoDoEnvio, type Enviado } from "./useEnvio";
import { useMencoes } from "./useMencoes";
import { CartaoDePermissao } from "./CartaoDePermissao";
// Fork (pergunta-da-cli): diálogo da CLI que a conversa não interpreta, e o terminal para responder.
import { CartaoDeEspera } from "./CartaoDeEspera";
import { TerminalFlutuante } from "./TerminalFlutuante";
import { RodapeDaConversa } from "./RodapeDaConversa";

export function ConversationDetail({ conversation: daLista, refresh, abrirConversa }: {
  conversation: Conversation;
  refresh: () => Promise<void>;
  /** Retomar uma conversa do histórico cria a conversa do OMNI com outro id: a tela vai para ela. */
  abrirConversa?: (id: string) => void;
}) {
  const chaveRascunho = `omni-rascunho:${daLista.id}`;
  const [data, setData] = useState<Timeline | null>(null);
  // O rascunho sobrevive ao refresh da página: atualizar no meio da digitação não apaga nada.
  const [text, setText] = useState(() => lerSessao(chaveRascunho) ?? "");
  const [anexos, setAnexos] = useState<Anexo[]>([]);
  const [arrastando, setArrastando] = useState(false);
  const telaRef = useRef<HTMLElement>(null);
  const [error, setError] = useState<string | null>(null);
  // Separado de `error`: o polling limpa `error` a cada ciclo bem-sucedido, e uma recusa da fila
  // aparecia e sumia no mesmo instante. Este só limpa no próximo envio.
  const [falhaEnvio, setFalhaEnvio] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [parando, setParando] = useState(false);
  const [bifurcando, setBifurcando] = useState(false);
  const [terminalDaPergunta, setTerminalDaPergunta] = useState(false);
  const [enviado, setEnviado] = useState<Enviado | null>(null);
  // Mensagens mandadas com o agente trabalhando: estão na fila da CLI e entram quando o turno acabar.
  const [fila, setFila] = useState<Enviado[]>([]);
  const trabalhandoDesde = useRef<number | null>(null);
  const fim = useRef<HTMLDivElement>(null);
  const urlCarregada = useRef<string | null>(null);
  const anexosVivos = useRef<Anexo[]>([]);
  anexosVivos.current = anexos;
  const enviadoVivo = useRef<Enviado | null>(null);
  enviadoVivo.current = enviado;

  // O estado que veio com a timeline é mais novo que o da lista (que anda a cada 5 s). Conversa só
  // do histórico não tem estado vivo; fica o da lista.
  const conversation = data?.conversation && data.conversation.id === daLista.id ? { ...daLista, ...data.conversation } : daLista;
  // Fork (Onda 2.0): Idempotency-Key e If-Match das ações (`useEnvio.ts`).
  const { post, falhou } = useAcoesDaConversa(conversation, setError);

  useEffect(() => { gravarSessao(chaveRascunho, text || null); }, [chaveRascunho, text]);
  // Prévia local não pode vazar memória ao sair da conversa.
  useEffect(() => () => {
    anexosVivos.current.forEach(descartarAnexo);
    enviadoVivo.current?.anexos.forEach(a => a.previa && URL.revokeObjectURL(a.previa));
  }, []);

  const trabalhando = conversation.state === "working";
  const ativo = trabalhando || enviado !== null;
  if (ativo && trabalhandoDesde.current === null) trabalhandoDesde.current = enviado?.em ?? Date.now();
  if (!ativo) trabalhandoDesde.current = null;

  const base = `/conversas/${encodeURIComponent(conversation.id)}/timeline`;
  const url = base;

  /** Lê a timeline. Página nova (ou componente recém-montado) vem inteira; depois só se mudou. */
  const ler = useCallback(async () => {
    if (urlCarregada.current !== url) esquecerVersao(url);
    const resultado = await requestSeMudou<Timeline>(url);
    urlCarregada.current = url;
    if (resultado) setData(resultado);
  }, [url]);

  usePoll(async () => { await ler(); setError(null); }, reason => setError(String(reason)), [ler],
    ativo ? POLL_ATIVO_MS : POLL_PARADO_MS);

  // O agente terminou (ou começou): a lista ao lado também precisa saber, sem esperar o ciclo dela.
  const estadoAnterior = useRef(conversation.state);
  useEffect(() => {
    if (estadoAnterior.current !== conversation.state) {
      estadoAnterior.current = conversation.state;
      void refresh().catch(() => undefined);
    }
  }, [conversation.state, refresh]);

  // Fork (Onda 2.0): páginas antigas, blocos e rolagem moram em `useTimeline.ts`.
  const { mensagens, anteriorA, buscandoAntigas, buscarAntigas, buscarAntigasRef } = useMensagensAntigas({ data, base, fim, setError });
  const { blocos, separadores, continuacoes } = useBlocos(mensagens);
  const ultimaId = mensagens[mensagens.length - 1]?.id ?? "";

  useConfirmacaoDoEnvio({ data, mensagens, enviado, setEnviado, fila, setFila, setFalhaEnvio });

  const { grudado, novidades, irParaOFim } = useRolagem({ fim, carregado: data !== null, buscarAntigasRef, ultimaId, ativo });

  function adicionar(arquivos: File[]) {
    if (arquivos.length === 0) return;
    setFalhaEnvio(null);
    setAnexos(lista => [...lista, ...arquivos.map(criarAnexo)]);
  }

  // Arquivos entregues por fora dos eventos do navegador (o arrastar do app do PC) e o colar sem foco
  // em lugar nenhum (Ctrl+V com a conversa na tela).
  const adicionarRef = useRef(adicionar);
  adicionarRef.current = adicionar;
  useEffect(() => {
    const tela = telaRef.current;
    if (!tela) return;
    const anexar = (evento: Event) => adicionarRef.current((evento as CustomEvent<File[]>).detail ?? []);
    const arrastar = (evento: Event) => setArrastando(Boolean((evento as CustomEvent<boolean>).detail));
    const colarSolto = (evento: ClipboardEvent) => {
      // Já tratado (o campo de texto ou a própria conversa colaram): não anexa duas vezes.
      if (evento.defaultPrevented) return;
      const alvo = evento.target as HTMLElement | null;
      if (alvo && alvo !== document.body && alvo.closest("input, textarea, [contenteditable], .xterm")) return;
      if (!tela.isConnected || tela.closest(".invisible")) return;
      const arquivos = Array.from(evento.clipboardData?.files ?? []);
      if (arquivos.length === 0) return;
      evento.preventDefault();
      adicionarRef.current(arquivos);
    };
    tela.addEventListener(EVENTO_ANEXAR, anexar);
    tela.addEventListener(EVENTO_ARRASTANDO, arrastar);
    document.addEventListener("paste", colarSolto);
    return () => {
      tela.removeEventListener(EVENTO_ANEXAR, anexar);
      tela.removeEventListener(EVENTO_ARRASTANDO, arrastar);
      document.removeEventListener("paste", colarSolto);
    };
  }, []);

  function remover(id: string) {
    setAnexos(lista => {
      const saindo = lista.find(a => a.id === id);
      if (saindo) descartarAnexo(saindo);
      return lista.filter(a => a.id !== id);
    });
  }

  const marcar = (id: string, mudanca: Partial<Anexo>) =>
    setAnexos(lista => lista.map(a => (a.id === id ? { ...a, ...mudanca } : a)));

  async function aprovar(permitir: boolean) {
    if (!conversation.capabilities || busy) return;
    setBusy(true); setError(null); setFalhaEnvio(null);
    try { await post("aprovar", { permitir }); await Promise.all([ler(), refresh()]); }
    catch (reason) { falhou(reason); }
    finally { setBusy(false); }
  }

  /** Responde um menu da CLI pelo número da opção; `null` fecha o menu (Esc); `"avancar"` é o Tab
   *  do marcar-várias. */
  async function escolher(opcao: number | null | "avancar") {
    if (!conversation.capabilities?.escolha || busy) return;
    setBusy(true); setError(null); setFalhaEnvio(null);
    try { await post("escolher", opcao === "avancar" ? { avancar: true } : { opcao }); await Promise.all([ler(), refresh()]); }
    catch (reason) { falhou(reason); }
    finally { setBusy(false); }
  }

  /** Alterna entre plano e automático — o que o shift+tab faz no terminal do PC. */
  async function trocarModo(alvo: ModoDePermissao) {
    if (!conversation.capabilities?.prompt || busy) return;
    setBusy(true); setError(null); setFalhaEnvio(null);
    try { const acao = await post("modo", { modo: alvo }); await aguardarAcao(conversation.id, acao.id); await Promise.all([ler(), refresh()]); }
    catch (reason) { falhou(reason); throw reason; }
    finally { setBusy(false); }
  }

  async function comandar(comando: ComandoDaConversa, argumento?: string) {
    if (!conversation.capabilities?.prompt || busy) throw new Error("Espere o agente ficar pronto");
    setBusy(true); setError(null);
    try {
      const acao = await post("comando", { comando, ...(argumento ? { argumento } : {}) });
      await aguardarAcao(conversation.id, acao.id);
      await Promise.all([ler(), refresh()]);
    } catch (reason) { falhou(reason); throw reason; }
    finally { setBusy(false); }
  }

  /** Conta, modelo e esforço: o engine reabre a conversa com `--resume` e os ajustes novos. */
  async function ajustar(ajustes: Ajustes) {
    if (!conversation.capabilities?.prompt || busy) return;
    setBusy(true); setError(null); setFalhaEnvio(null);
    try { await post("ajustes", { ...ajustes }); await Promise.all([ler(), refresh()]); }
    catch (reason) { falhou(reason); throw reason; }
    finally { setBusy(false); }
  }

  /** Conversa sem sessão: abre uma no PC com `--resume`, já com conta, modelo e esforço. A fila
   *  abre em segundo plano, então espera a sessão aparecer antes de trocar de tela. */
  async function retomar(ajustes: Ajustes) {
    setBusy(true); setError(null); setFalhaEnvio(null);
    try {
      const acao = await request<{ conversation_id?: string }>(`/conversas/${encodeURIComponent(conversation.id)}/retomar`, {
        method: "POST",
        headers: { "Content-Type": "application/json", "Idempotency-Key": actionKey() },
        body: JSON.stringify({ ...ajustes }),
      });
      const destino = acao.conversation_id || conversation.id;
      for (let tentativa = 0; tentativa < 30; tentativa += 1) {
        await new Promise((resolve) => setTimeout(resolve, 700));
        const lido = await request<Timeline>(`/conversas/${encodeURIComponent(destino)}/timeline`).catch(() => null);
        if (lido?.conversation?.capabilities) break;
      }
      await refresh();
      if (destino !== conversation.id && abrirConversa) abrirConversa(destino);
      else await ler();
    } catch (reason) { setError(String(reason)); throw reason; }
    finally { setBusy(false); }
  }

  // `@` no campo: arquivos da pasta do projeto, filtrados pelo engine.
  const buscarArquivos = useMencoes(conversation.id);

  /** O Esc do terminal: para o agente no meio do turno, como no PC. */
  async function interromper() {
    setError(null);
    try { await enviarTeclas(conversation.id, "\u001b"); await ler(); }
    catch (reason) { setError(String(reason)); }
  }

  /** O "Bifurcar" da lateral do PC: conversa nova com todo o histórico desta, e a tela vai para ela. */
  async function bifurcar() {
    setBifurcando(true);
    try {
      const nova = await request<{ id: string | null }>(`/conversas/${encodeURIComponent(conversation.id)}/bifurcar`, { method: "POST" });
      await refresh();
      if (nova.id) abrirConversa?.(nova.id);
    } catch (reason) { falhou(reason); } finally { setBifurcando(false); }
  }

  /** Tira da lista (ou devolve). Não apaga nada: a conversa vai para a seção Ocultas. */
  async function ocultar(oculta: boolean) {
    try {
      await request(`/conversas/${encodeURIComponent(conversation.id)}/ocultar`, {
        method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ oculta }),
      });
      await refresh();
    } catch (reason) { falhou(reason); }
  }

  async function renomear(nome: string) {
    const limpo = nome.trim();
    if (!limpo || limpo === conversation.title) return;
    try {
      await request(`/conversas/${encodeURIComponent(conversation.id)}/nome`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ nome: limpo }),
      });
      await Promise.all([ler(), refresh()]);
    } catch (reason) { falhou(reason); }
  }

  async function enviarPrompt() {
    if (!conversation.capabilities || busy) return;
    // Agente no meio do turno: a mensagem vai para a fila da CLI, como digitar no terminal.
    const paraFila = !conversation.capabilities.prompt && Boolean(conversation.capabilities.enfileirar);
    setBusy(true); setError(null); setFalhaEnvio(null);
    const lista = anexosVivos.current;
    try {
      // Fork: tela não reconhecida ("Entrada não reconhecida"): o engine limpa o campo ou reabre o
      // agente e devolve a versão da tela já pronta. Antes o campo travava e não havia o que fazer.
      let revisao: string | undefined;
      if (!conversation.capabilities.prompt && !paraFila) {
        try {
          const pronta = await request<{ revision?: string }>(`/conversas/${encodeURIComponent(conversation.id)}/destravar`, { method: "POST" });
          revisao = pronta.revision;
        } catch (motivo) {
          // Engine antigo ainda rodando (troca adiada pela atualização): sem a rota, tenta enviar direto.
          if (!(motivo instanceof ApiError && motivo.status === 404)) throw motivo;
        }
      }
      // Sobe o que ainda não subiu. O caminho fica guardado no anexo: se o prompt der timeout, o
      // reenvio monta o mesmo texto — e a mesma chave de idempotência — sem duplicar arquivo.
      const caminhos: string[] = [];
      for (const anexo of lista) {
        if (anexo.caminho) { caminhos.push(anexo.caminho); continue; }
        marcar(anexo.id, { estado: "enviando", erro: undefined });
        try {
          const caminho = await enviarAnexo(conversation.id, anexo.arquivo);
          // Um reenvio lê a lista do render seguinte, que já traz o caminho.
          marcar(anexo.id, { estado: "enviado", caminho });
          caminhos.push(caminho);
        } catch (reason) {
          marcar(anexo.id, { estado: "erro", erro: reason instanceof Error ? reason.message : String(reason) });
          throw new Error(`Não consegui mandar "${anexo.arquivo.name}" para o PC.`);
        }
      }
      const texto = montarPrompt(text, caminhos);
      const acao = await post("prompt", paraFila ? { texto, fila: true } : { texto }, revisao);
      const item: Enviado = {
        texto, em: Date.now(), acao: acao?.id ?? null,
        base: Math.max(-1, ...mensagens.map(m => indiceDaMensagem(m.id))),
        // As prévias passam para o balão otimista; quem revoga é ele, quando o transcript chegar.
        anexos: lista.map(a => ({ nome: a.arquivo.name || "anexo", previa: a.previa })),
      };
      if (paraFila) setFila((atual) => [...atual, item]); else setEnviado(item);
      setText("");
      setAnexos([]);
      grudado.current = true;
      await Promise.all([ler(), refresh()]);
    } catch (reason) {
      falhou(reason);
    } finally { setBusy(false); }
  }

  const capability = conversation.capabilities;
  const temConteudo = text.trim().length > 0 || anexos.length > 0;
  const anexoGrande = anexos.some(a => a.arquivo.size > ANEXO_MAX_BYTES);
  // Fork: com a sessão aberta e o agente parado, dá para mandar mesmo com a tela não reconhecida — o
  // envio destrava antes (`enviarPrompt`). Só pergunta/aprovação na tela continua bloqueando.
  const destravavel = Boolean(capability && conversation.live && !trabalhando && !capability.approve && !capability.escolha && !capability.espera);
  const abrirTerminal = conversation.live ? () => setTerminalDaPergunta(true) : undefined;
  const podeEnviar = Boolean(capability?.prompt || capability?.enfileirar || destravavel) && !busy && temConteudo && !anexoGrande;
  const timeline = data?.timeline;
  const ultimoBloco = blocos[blocos.length - 1];
  // Fork: menu da CLI logo depois do plano é a aprovação dele (aprovar / continuar planejando).
  const planoPendente = ultimoBloco?.tipo === "plano" && Boolean(capability?.escolha);

  function renderizarBloco(bloco: Bloco, indice: number) {
    if (bloco.tipo === "acoes") {
      return <BlocoDeAcoes key={bloco.id} acoes={bloco.acoes} provider={bloco.acoes[0]?.provider}
        aoVivo={ativo && bloco === ultimoBloco} conversa={conversation.id} />;
    }
    if (bloco.tipo === "comando") return <LinhaDeComando key={bloco.mensagem.id} texto={bloco.mensagem.text} />;
    if (bloco.tipo === "aviso") return <LinhaDeAviso key={bloco.mensagem.id} texto={bloco.mensagem.text} />;
    if (bloco.tipo === "plano") {
      // Fork: a CLI esperando a aprovação do plano — a pergunta dela vem dentro do cartão.
      return <CartaoDoPlano key={bloco.mensagem.id} plano={bloco.mensagem.plano ?? ""} provider={bloco.mensagem.provider}
        horario={bloco.mensagem.timestamp}
        pergunta={planoPendente && bloco === ultimoBloco && capability?.escolha
          ? <CartaoDePergunta escolha={capability.escolha} provider={conversation.provider} busy={busy}
              onEscolher={(opcao) => void escolher(opcao)} onAvancar={() => void escolher("avancar")} onAbrirTerminal={abrirTerminal} />
          : undefined} />;
    }
    const message = bloco.mensagem;
    return <MessageBubble key={message.id} lado={message.role === "user" ? "user" : "agent"}
      provider={message.provider} texto={message.text} horario={message.timestamp} continua={continuacoes.has(indice)}
      imagens={message.imagens ? { conversa: conversation.id, n: indiceDaMensagem(message.id), total: message.imagens } : undefined}
      arquivos={message.arquivos?.length ? { conversa: conversation.id, n: indiceDaMensagem(message.id), caminhos: message.arquivos } : undefined} />;
  }

  return <section className="chat-tela" ref={telaRef}
    onDragEnter={(evento) => { if (temArquivos(evento.dataTransfer)) { evento.preventDefault(); setArrastando(true); } }}
    onDragOver={(evento) => { if (temArquivos(evento.dataTransfer)) { evento.preventDefault(); evento.dataTransfer.dropEffect = "copy"; } }}
    onDragLeave={(evento) => { if (!evento.currentTarget.contains(evento.relatedTarget as Node | null)) setArrastando(false); }}
    onDrop={(evento) => {
      const arquivos = Array.from(evento.dataTransfer?.files ?? []);
      setArrastando(false);
      if (arquivos.length === 0) return;
      evento.preventDefault();
      adicionar(arquivos);
    }}
    onPaste={(evento) => {
      // O campo de texto já trata o próprio colar; aqui é o colar com o foco em outro ponto da conversa.
      if ((evento.target as HTMLElement).id === "reply") return;
      const arquivos = Array.from(evento.clipboardData?.files ?? []);
      if (arquivos.length === 0) return;
      evento.preventDefault();
      adicionar(arquivos);
    }}>
    {arrastando && (
      <div className="chat-soltar" aria-hidden>
        <span><AttachmentIcon className="h-5 w-5" />Solte para anexar à conversa</span>
      </div>
    )}
    {anteriorA !== null && (
      <Button variant="ghost" size="sm" className="m-paginar" disabled={buscandoAntigas} onClick={() => void buscarAntigas()}>
        {buscandoAntigas ? "Carregando…" : "Mensagens anteriores"}
      </Button>
    )}
    {!!timeline?.unavailable_segments.length && <p className="m-aviso">Parte do histórico está indisponível; nenhum transcript foi associado por aproximação.</p>}

    <div className="chat" aria-label="Mensagens">
      {!data && <Vazio>Carregando conversa…</Vazio>}
      {data && mensagens.length === 0 && !enviado && !ativo && !timeline?.unavailable_segments.length &&
        <Vazio>Nenhuma mensagem ainda. Mande o primeiro prompt abaixo.</Vazio>}
      {blocos.map((bloco, indice) => {
        const dia = separadores.get(indice);
        const conteudo = renderizarBloco(bloco, indice);
        // Chave pelo bloco, não pela posição: com as antigas entrando em cima, a posição muda.
        const chave = bloco.tipo === "acoes" ? bloco.id : bloco.mensagem.id;
        return dia ? <div key={`dia-${chave}`} className="chat-grupo"><p className="chat-dia"><span>{dia}</span></p>{conteudo}</div> : conteudo;
      })}
      {enviado && <MessageBubble lado="user" pendente texto={enviado.texto}
        anexosLocais={enviado.anexos.length ? enviado.anexos : undefined} />}
      {capability?.approve && <CartaoDePermissao texto={capability.approval_text} busy={busy} onResponder={(permitir) => void aprovar(permitir)}
        pedido={capability.pedido} conversaId={conversation.id} onRespondido={() => void Promise.all([ler(), refresh()])} /* Fork (Onda 2) */ />}
      {capability?.escolha && !planoPendente && <CartaoDePergunta escolha={capability.escolha} provider={conversation.provider} busy={busy}
        onEscolher={(opcao) => void escolher(opcao)} onAvancar={() => void escolher("avancar")} onAbrirTerminal={abrirTerminal} />}
      {capability?.espera && !capability.approve && !capability.escolha && abrirTerminal &&
        <CartaoDeEspera espera={capability.espera} provider={conversation.provider} onAbrirTerminal={abrirTerminal} />}
      {terminalDaPergunta && <TerminalFlutuante conversaId={conversation.id} tipo="pergunta" onFechar={() => setTerminalDaPergunta(false)} />}
      {ativo && !capability?.approve &&
        <AgentWorking desde={trabalhandoDesde.current ?? Date.now()} provider={conversation.provider} tokens={conversation.tokens ?? null} />}
      {fila.length > 0 && (
        <div className="chat-fila" aria-label="Mensagens na fila">
          <p className="chat-fila-rotulo">Na fila · entram quando o agente terminar</p>
          {fila.map((item) => (
            <MessageBubble key={item.em} lado="user" pendente="na fila" texto={item.texto}
              anexosLocais={item.anexos.length ? item.anexos : undefined} />
          ))}
        </div>
      )}
      <div ref={fim} />
    </div>

    {novidades && (
      <button type="button" className="chat-novidades" onClick={() => irParaOFim(true)}>Novas mensagens ↓</button>
    )}

    <RodapeDaConversa conversation={conversation} timeline={timeline} falhaEnvio={falhaEnvio} error={error}
      anexoGrande={anexoGrande} ativo={ativo} trabalhando={trabalhando} destravavel={destravavel}
      texto={text} onTexto={setText} anexos={anexos} onAdicionar={adicionar} onRemover={remover}
      onEnviar={() => void enviarPrompt()} podeEnviar={podeEnviar} busy={busy}
      onRenomear={renomear} onAjustar={ajustar} onRetomar={retomar} onModo={trocarModo} onComando={comandar}
      bifurcando={bifurcando} onBifurcar={bifurcar} onOcultar={ocultar} abrirConversa={abrirConversa}
      onParar={trabalhando && conversation.live ? () => { setParando(true); void interromper().finally(() => setParando(false)); } : undefined}
      parando={parando}
      buscarArquivos={buscarArquivos} />
  </section>;
}

/** Arrasto que traz arquivos (e não texto ou uma aba sendo movida). */
function temArquivos(dados: DataTransfer | null): boolean {
  return Boolean(dados && Array.from(dados.types ?? []).includes("Files"));
}
