import { useEffect, useRef, useState } from "react";
import { CloseIcon } from "../../components/ui/PixelIcon";
import { matchesShortcut, shortcutLabel } from "../../lib/shortcuts";
import { useAcoesDaPaleta } from "../paleta/acoes";
import { useAtividade } from "./atividadeService";
import { EVENTO_AVISO_INTERNO } from "./avisos";
import { ListaDeAtividade } from "./ListaDeAtividade";
import { pedirAbrirConversaDaAtividade } from "./navegacao";
import type { ItemDeAtividade } from "./tipos";

export function CaixaDeAtividade() {
  const [aberta, setAberta] = useState(false);
  const [aviso, setAviso] = useState<ItemDeAtividade[]>([]);
  const dados = useAtividade();
  const painel = useRef<HTMLElement>(null);
  const botao = useRef<HTMLButtonElement>(null);
  const contador = dados.itens.filter(item => !item.visto).length;
  useAcoesDaPaleta("atividade", [{ id: "abrir-atividade", titulo: "Abrir Atividade", executar: () => setAberta(true) }]);
  useEffect(() => {
    const tecla = (evento: KeyboardEvent) => {
      if (evento.defaultPrevented || !matchesShortcut(evento, "atividade")) return;
      evento.preventDefault(); evento.stopImmediatePropagation(); setAberta(atual => !atual);
    };
    // Captura antes do textarea do xterm: o atalho do app não pode escrever na PTY.
    window.addEventListener("keydown", tecla, true);
    return () => window.removeEventListener("keydown", tecla, true);
  }, []);
  useEffect(() => {
    if (!aberta) return;
    const anterior = document.activeElement as HTMLElement | null;
    painel.current?.querySelector<HTMLButtonElement>("button")?.focus();
    const tecla = (evento: KeyboardEvent) => {
      if (evento.key === "Escape") { evento.preventDefault(); evento.stopImmediatePropagation(); setAberta(false); }
      if (evento.key !== "Tab") return;
      const focaveis = painel.current?.querySelectorAll<HTMLElement>("button:not(:disabled), select, [tabindex='0']");
      const primeiro = focaveis?.[0], ultimo = focaveis?.[focaveis.length - 1];
      if (evento.shiftKey && document.activeElement === primeiro) { evento.preventDefault(); ultimo?.focus(); }
      else if (!evento.shiftKey && document.activeElement === ultimo) { evento.preventDefault(); primeiro?.focus(); }
    };
    window.addEventListener("keydown", tecla, true);
    return () => { window.removeEventListener("keydown", tecla, true); (anterior ?? botao.current)?.focus(); };
  }, [aberta]);
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const ouvir = (evento: Event) => {
      setAviso((evento as CustomEvent<ItemDeAtividade[]>).detail);
      clearTimeout(timer); timer = setTimeout(() => setAviso([]), 6_000);
    };
    window.addEventListener(EVENTO_AVISO_INTERNO, ouvir);
    return () => { clearTimeout(timer); window.removeEventListener(EVENTO_AVISO_INTERNO, ouvir); };
  }, []);
  const abrir = (item: ItemDeAtividade) => { pedirAbrirConversaDaAtividade(item); setAberta(false); setAviso([]); };
  return <>
    <button ref={botao} type="button" className={`titlebar-acao${aberta ? " text-accent" : ""}`} aria-label={`Atividade (${contador} não lidos)`}
      title={`Atividade (${shortcutLabel("atividade")})`} aria-expanded={aberta} aria-controls="gaveta-atividade" onClick={() => setAberta(atual => !atual)}>
      <span>Atividade</span>{contador > 0 && <span className="rounded bg-accent-muted px-1 text-accent">{contador}</span>}
    </button>
    <div className={`gaveta-uso-fundo${aberta ? " is-aberta" : ""}`} onClick={() => setAberta(false)} aria-hidden />
    <aside ref={painel} id="gaveta-atividade" role="dialog" aria-modal={aberta || undefined} aria-label="Atividade" aria-hidden={!aberta}
      className={`gaveta-uso${aberta ? " is-aberta" : ""}`}>
      <div className="flex items-center justify-between border-b border-border-subtle p-3">
        <h2 className="text-sm text-text-primary">Atividade</h2>
        <button type="button" aria-label="Fechar atividade" className="text-text-muted hover:text-text-primary" onClick={() => setAberta(false)}><CloseIcon className="h-4 w-4" /></button>
      </div>
      <ListaDeAtividade {...dados} abrir={abrir} />
    </aside>
    {aviso.length > 0 && <div role="status" className="fixed right-4 top-14 z-[100] max-w-sm rounded border border-border-subtle bg-bg-elevated p-3 text-sm text-text-primary shadow-lg">
      <button type="button" className="float-right ml-3 text-text-muted" aria-label="Dispensar aviso" onClick={() => setAviso([])}>×</button>
      <button type="button" onClick={() => aviso.length > 1 ? (setAberta(true), setAviso([])) : abrir(aviso[0])}>
        {aviso.length > 1 ? `${aviso.length} agentes com atividade nova` : `${aviso[0].projeto} · ${aviso[0].titulo}: ${aviso[0].resumo}`}
      </button>
    </div>}
  </>;
}
