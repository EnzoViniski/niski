import { useEffect, useRef, type ClipboardEvent, type FormEvent, type KeyboardEvent, type ReactNode } from "react";
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
export function Composer({ texto, onTexto, anexos, onAdicionar, onRemover, onEnviar, podeEnviar, ocupado, placeholder, toolbar, chips, direita, onParar, parando }: ComposerProps) {
  const campo = useRef<HTMLTextAreaElement>(null);

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
    // `isComposing`: Enter que confirma acento/IME não pode disparar o envio.
    if (event.key !== "Enter" || event.shiftKey || event.nativeEvent.isComposing || !enterEnvia()) return;
    event.preventDefault();
    if (podeEnviar) onEnviar();
  }

  // Com texto escrito, o principal continua sendo enviar (a mensagem entra na fila do agente).
  const parar = onParar && !texto.trim();

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
          className="composer-campo" onPaste={colar} onKeyDown={tecla} onChange={(event) => onTexto(event.target.value)} />
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
            {parar ? (
              <button type="button" className="composer-enviar composer-parar" aria-label="Parar" title="Parar o agente (Esc no terminal)"
                disabled={parando} onClick={onParar}>
                {parando ? <LoaderIcon aria-hidden className="anexo-girando h-4 w-4" /> : <StopIcon aria-hidden className="h-4 w-4" />}
              </button>
            ) : (
              <button type="submit" className="composer-enviar" aria-label="Enviar resposta" title="Enviar (Enter)" disabled={!podeEnviar}>
                {ocupado ? <LoaderIcon aria-hidden className="anexo-girando h-4 w-4" /> : <SendIcon aria-hidden className="h-4 w-4" />}
              </button>
            )}
          </div>
        </div>
      </form>
    </div>
  );
}
