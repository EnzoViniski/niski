import { useEffect, useRef, useState, type ClipboardEvent, type FormEvent, type KeyboardEvent, type ReactNode } from "react";
import { AttachmentIcon, CloseIcon, LoaderIcon, SendIcon, StopIcon, WarningIcon } from "../components/ui/PixelIcon";
import { iconForPath } from "../features/files/fileIcons";
import { tamanhoLegivel, type Anexo } from "./anexos";

interface ComposerProps {
  texto: string;
  onTexto: (valor: string) => void;
  anexos: Anexo[];
  onAdicionar: (arquivos: File[]) => void;
  onRemover: (id: string) => void;
  onEnviar: () => void;
  podeEnviar: boolean;
  ocupado: boolean;
  placeholder: string;
  /** Acima do cartão (renomear a conversa, por exemplo). */
  toolbar?: ReactNode;
  /** Na barra de baixo, depois do anexo: conta, modelo, esforço. */
  chips?: ReactNode;
  /** Na barra de baixo, antes do enviar: o modo (plano/auto). */
  direita?: ReactNode;
  /** Agente trabalhando: o botão principal vira "Parar" (o Esc do terminal). */
  onParar?: () => void;
  parando?: boolean;
  /** Arquivos do projeto para o `@` (como no terminal). Sem isto, `@` é só texto. */
  buscarArquivos?: (busca: string) => Promise<string[]>;
}

/** O `@algo` que está sendo digitado logo antes do cursor (precisa vir no começo ou após espaço). */
export function mencaoNoCursor(texto: string, cursor: number): { inicio: number; busca: string } | null {
  const antes = texto.slice(0, cursor);
  const achado = /(^|\s)@([^\s@"]*)$/.exec(antes);
  if (!achado) return null;
  return { inicio: antes.length - achado[2].length - 1, busca: achado[2] };
}

/** Caminho com espaço vai entre aspas, como a CLI aceita. */
export function textoDaMencao(caminho: string): string {
  return /\s/.test(caminho) ? `@"${caminho}" ` : `@${caminho} `;
}

const ALTURA_MAXIMA_PX = 160;

/** Teclado físico e mouse: Enter envia, como no terminal do PC. No celular o Enter do teclado
 *  virtual continua quebrando linha — lá o botão de enviar está sempre à mão. */
function enterEnvia(): boolean {
  return typeof window !== "undefined" && Boolean(window.matchMedia?.("(pointer: fine)").matches);
}

function ChipAnexo({ anexo, onRemover, travado }: { anexo: Anexo; onRemover: () => void; travado: boolean }) {
  const Icone = iconForPath(anexo.arquivo.name);
  const meta = anexo.estado === "erro" ? anexo.erro ?? "Falhou"
    : anexo.estado === "enviando" ? "Enviando…"
    : anexo.estado === "enviado" ? "No PC"
    : tamanhoLegivel(anexo.arquivo.size);
  return (
    <li className="anexo-chip" data-estado={anexo.estado}>
      {anexo.previa
        ? <img src={anexo.previa} alt="" className="anexo-previa" />
        : <span className="anexo-tipo" aria-hidden><Icone className="h-5 w-5" /></span>}
      <span className="anexo-info">
        <span className="anexo-nome">{anexo.arquivo.name || "anexo"}</span>
        <span className="anexo-meta">
          {anexo.estado === "enviando" && <LoaderIcon aria-hidden className="anexo-girando h-3 w-3" />}
          {anexo.estado === "erro" && <WarningIcon aria-hidden className="h-3 w-3" />}
          {meta}
        </span>
      </span>
      <button type="button" className="anexo-remover" aria-label={`Remover ${anexo.arquivo.name || "anexo"}`}
        disabled={travado} onClick={onRemover}>
        <CloseIcon aria-hidden className="h-4 w-4" />
      </button>
    </li>
  );
}

/**
 * Barra de digitação do chat: texto, anexos por botão ou colados, e enviar.
 *
 * O botão de anexo é um `<label>` em volta do `<input type="file">`, não um `.click()` por código:
 * é o único jeito que abre o seletor em todo navegador de celular, inclusive o Safari antigo, que
 * ignora clique programático em input escondido. Sem `accept`: qualquer tipo de arquivo vale.
 */
export function Composer({ texto, onTexto, anexos, onAdicionar, onRemover, onEnviar, podeEnviar, ocupado, placeholder, toolbar, chips, direita, onParar, parando, buscarArquivos }: ComposerProps) {
  const campo = useRef<HTMLTextAreaElement>(null);
  // Menção aberta: onde começa o `@`, o que foi digitado depois dele e o que achou.
  const [mencao, setMencao] = useState<{ inicio: number; busca: string } | null>(null);
  const [sugestoes, setSugestoes] = useState<string[]>([]);
  const [escolhida, setEscolhida] = useState(0);
  const ultimaLista = useRef<string[]>([]);

  // Busca com um respiro curto: digitar "@src/comp" não dispara uma consulta por letra.
  useEffect(() => {
    if (!mencao || !buscarArquivos) { ultimaLista.current = []; setSugestoes([]); return; }
    let cancelado = false;
    const espera = window.setTimeout(() => {
      buscarArquivos(mencao.busca).then((lista) => {
        if (cancelado) return;
        // Mesma lista de antes (resposta repetida ou atrasada): mantém a escolha feita nas setas.
        const atual = ultimaLista.current;
        if (atual.length === lista.length && atual.every((item, i) => item === lista[i])) return;
        ultimaLista.current = lista;
        setSugestoes(lista);
        setEscolhida(0);
      }).catch(() => { if (!cancelado) setSugestoes([]); });
    }, 120);
    return () => { cancelado = true; window.clearTimeout(espera); };
  }, [mencao, buscarArquivos]);

  function acompanharMencao(valor: string, cursor: number) {
    const achada = buscarArquivos ? mencaoNoCursor(valor, cursor) : null;
    setMencao((atual) => (atual?.inicio === achada?.inicio && atual?.busca === achada?.busca ? atual : achada));
  }

  function escolher(caminho: string) {
    if (!mencao) return;
    const fim = mencao.inicio + 1 + mencao.busca.length;
    const inserido = textoDaMencao(caminho);
    const novo = texto.slice(0, mencao.inicio) + inserido + texto.slice(fim);
    onTexto(novo);
    setMencao(null);
    setSugestoes([]);
    const cursor = mencao.inicio + inserido.length;
    requestAnimationFrame(() => {
      campo.current?.focus();
      campo.current?.setSelectionRange(cursor, cursor);
    });
  }

  // Cresce com o texto até um limite, como nos apps de mensagem.
  useEffect(() => {
    const el = campo.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, ALTURA_MAXIMA_PX)}px`;
    // Barra de rolagem só quando passa do limite; antes aparecia até com o campo vazio.
    el.style.overflowY = el.scrollHeight > ALTURA_MAXIMA_PX ? "auto" : "hidden";
  }, [texto]);

  function colar(event: ClipboardEvent<HTMLTextAreaElement>) {
    const arquivos = Array.from(event.clipboardData?.files ?? []);
    // Texto comum segue o caminho normal do campo; só arquivo (print, foto, documento) vira anexo.
    if (arquivos.length === 0) return;
    event.preventDefault();
    onAdicionar(arquivos);
  }

  function enviar(event: FormEvent) {
    event.preventDefault();
    if (podeEnviar) onEnviar();
  }

  function tecla(event: KeyboardEvent<HTMLTextAreaElement>) {
    // Lista de arquivos aberta: setas escolhem, Enter/Tab inserem, Esc fecha — nada disso envia.
    if (mencao && sugestoes.length > 0) {
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        const passo = event.key === "ArrowDown" ? 1 : -1;
        setEscolhida((atual) => (atual + passo + sugestoes.length) % sugestoes.length);
        return;
      }
      if (event.key === "Enter" || event.key === "Tab") {
        event.preventDefault();
        escolher(sugestoes[escolhida]);
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        setMencao(null);
        return;
      }
    }
    // `isComposing`: Enter que confirma acento/IME não pode disparar o envio.
    if (event.key !== "Enter" || event.shiftKey || event.nativeEvent.isComposing || !enterEnvia()) return;
    event.preventDefault();
    if (podeEnviar) onEnviar();
  }

  // Com texto escrito, o principal continua sendo enviar (a mensagem entra na fila do agente).
  // Agente trabalhando: o Parar fica sempre à mão, e o Enviar aparece ao lado assim que há algo
  // escrito — a mensagem entra na fila da CLI, como digitar no terminal durante o turno.
  const parar = Boolean(onParar);
  const enviarJunto = !parar || texto.trim().length > 0 || anexos.length > 0;

  return (
    <div className="composer-faixa">
      {toolbar}
      <form onSubmit={enviar} className="composer">
        {anexos.length > 0 && (
          <ul className="anexos" aria-label="Anexos">
            {anexos.map((anexo) => (
              <ChipAnexo key={anexo.id} anexo={anexo} travado={ocupado} onRemover={() => onRemover(anexo.id)} />
            ))}
          </ul>
        )}
        <label htmlFor="reply" className="sr-only">Sua resposta</label>
        <textarea id="reply" ref={campo} rows={1} value={texto} maxLength={16000} placeholder={placeholder}
          className="composer-campo" onPaste={colar} onKeyDown={tecla}
          onChange={(event) => { onTexto(event.target.value); acompanharMencao(event.target.value, event.target.selectionStart ?? event.target.value.length); }}
          onClick={(event) => acompanharMencao(event.currentTarget.value, event.currentTarget.selectionStart ?? 0)}
          onBlur={() => window.setTimeout(() => setMencao(null), 150)}
          aria-autocomplete="list" aria-expanded={Boolean(mencao && sugestoes.length)} aria-controls="mencoes" />
        {mencao && sugestoes.length > 0 && (
          <ul id="mencoes" role="listbox" aria-label="Arquivos do projeto" className="composer-mencoes">
            {sugestoes.map((caminho, indice) => {
              const barra = caminho.lastIndexOf("/");
              const Icone = iconForPath(caminho);
              return (
                <li key={caminho} role="option" aria-selected={indice === escolhida}
                  className={`composer-mencao${indice === escolhida ? " is-ativa" : ""}`}
                  onMouseDown={(event) => { event.preventDefault(); escolher(caminho); }}
                  onMouseEnter={() => setEscolhida(indice)}>
                  <Icone aria-hidden className="h-3.5 w-3.5 shrink-0" />
                  <span className="composer-mencao-nome">{caminho.slice(barra + 1)}</span>
                  {barra > 0 && <span className="composer-mencao-pasta">{caminho.slice(0, barra)}</span>}
                </li>
              );
            })}
          </ul>
        )}
        <div className="composer-rodape">
          <label className={`composer-botao${ocupado ? " is-disabled" : ""}`} title="Anexar arquivo">
            <AttachmentIcon aria-hidden className="h-4 w-4" />
            <input type="file" multiple className="sr-only" aria-label="Anexar arquivo" disabled={ocupado}
              onChange={(event) => {
                onAdicionar(Array.from(event.target.files ?? []));
                // Zera para escolher o mesmo arquivo de novo depois de removê-lo disparar `change`.
                event.target.value = "";
              }} />
          </label>
          <div className="composer-chips">{chips}</div>
          <div className="composer-direita">
            {direita}
            {parar && (
              <button type="button" className="composer-parar" aria-label="Parar" title="Parar o agente (Esc no terminal)"
                disabled={parando} onClick={onParar}>
                <span className="composer-parar-icone" aria-hidden>
                  {parando ? <LoaderIcon className="anexo-girando h-3.5 w-3.5" /> : <StopIcon className="h-3.5 w-3.5" />}
                </span>
                <span className="composer-parar-texto">{parando ? "Parando…" : "Parar"}</span>
              </button>
            )}
            {enviarJunto && (
              <button type="submit" className="composer-enviar" aria-label={parar ? "Enviar para a fila" : "Enviar resposta"}
                title={parar ? "Entra na fila e o agente lê quando terminar (Enter)" : "Enviar (Enter)"} disabled={!podeEnviar}>
                {ocupado ? <LoaderIcon aria-hidden className="anexo-girando h-4 w-4" /> : <SendIcon aria-hidden className="h-4 w-4" />}
              </button>
            )}
          </div>
        </div>
      </form>
    </div>
  );
}
