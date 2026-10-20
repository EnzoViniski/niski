import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { readTextFile } from "@tauri-apps/plugin-fs";
import { Button } from "../../components/ui";
import { engineApi } from "../terminal/terminalService";

interface Memoria { ativa: boolean; texto: string }

export async function memoriaApi(method: "GET" | "POST", memoria?: Memoria): Promise<Memoria> {
  const resposta = await engineApi(method, "/memoria", memoria ? JSON.stringify(memoria) : null);
  if (resposta.status === 404) throw new Error("Atualize o app e o engine para usar a memória compartilhada.");
  if (resposta.status >= 400) {
    let mensagem = "Não foi possível acessar a memória compartilhada.";
    try { mensagem = (JSON.parse(resposta.body) as { error?: string }).error ?? mensagem; } catch { /* erro sem JSON */ }
    throw new Error(mensagem);
  }
  return JSON.parse(resposta.body) as Memoria;
}

export function MemoriaCompartilhada() {
  const [memoria, setMemoria] = useState<Memoria | null>(null);
  const [salva, setSalva] = useState<Memoria | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [erro, setErro] = useState<string | null>(null);
  const [aviso, setAviso] = useState("");
  useEffect(() => {
    let viva = true;
    memoriaApi("GET").then((valor) => { if (viva) { setMemoria(valor); setSalva(valor); } })
      .catch((motivo: unknown) => { if (viva) setErro(String(motivo instanceof Error ? motivo.message : motivo)); });
    return () => { viva = false; };
  }, []);

  async function salvar() {
    if (!memoria) return;
    setOcupado(true); setErro(null); setAviso("");
    try {
      const valor = await memoriaApi("POST", memoria);
      setSalva(valor);
      setAviso("Memória salva. Vale ao abrir ou reiniciar agentes; conversas existentes mantêm o contexto já carregado pela CLI.");
    } catch (motivo) { setErro(motivo instanceof Error ? motivo.message : String(motivo)); }
    finally { setOcupado(false); }
  }

  async function importar() {
    try {
      const caminho = await open({ multiple: false, filters: [{ name: "Texto ou Markdown", extensions: ["md", "txt"] }] });
      if (typeof caminho !== "string") return;
      const texto = await readTextFile(caminho);
      if (Array.from(texto).length > 12_000) throw new Error("O arquivo ultrapassa 12000 caracteres. Reduza o texto antes de importar.");
      setMemoria({ ativa: true, texto }); setAviso("Arquivo carregado. Confira o conteúdo e salve a memória."); setErro(null);
    } catch (motivo) { setErro(motivo instanceof Error ? motivo.message : String(motivo)); }
  }

  return (
    <section className="glass p-5 space-y-4">
      <div>
        <h3 className="text-sm font-medium text-text-primary">O que seus agentes devem saber</h3>
        <p className="mt-2 max-w-prose text-xs leading-relaxed text-text-secondary">
          Seu jeito de trabalhar, preferências e contexto da empresa em um só lugar.
          Compartilhado com Claude e Codex de todas as contas deste app, inclusive nos fluxos.
        </p>
      </div>
      {erro && <p role="alert" className="text-xs text-danger">{erro}</p>}
      {!memoria && !erro && <p className="text-xs text-text-muted">Carregando memória…</p>}
      {memoria && <>
        <label className="flex items-center gap-2 text-xs text-text-secondary">
          <input type="checkbox" checked={memoria.ativa} disabled={ocupado}
            onChange={(event) => { setMemoria({ ...memoria, ativa: event.target.checked }); setAviso(""); }} />
          Compartilhar com meus agentes
        </label>
        <div>
          <label htmlFor="memoria-compartilhada" className="block mb-2 text-xs text-text-secondary">Sobre mim e meu trabalho</label>
          <textarea id="memoria-compartilhada" rows={14} value={memoria.texto} disabled={ocupado}
            onChange={(event) => { setMemoria({ ...memoria, texto: event.target.value }); setAviso(""); }}
            placeholder="Como quer ser chamado, o que sua empresa faz, como prefere receber respostas e quais decisões os agentes podem tomar…"
            className="w-full resize-y border border-border-subtle bg-bg-surface p-3 text-sm leading-relaxed text-text-primary focus:outline-none focus:border-accent" />
          <p className="mt-1 text-[11px] text-text-muted">{Array.from(memoria.texto).length.toLocaleString("pt-BR")} de 12.000 caracteres. O texto é enviado aos provedores dos agentes que você abrir.</p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button size="sm" disabled={ocupado || Array.from(memoria.texto).length > 12_000 || JSON.stringify(salva) === JSON.stringify(memoria)} onClick={() => void salvar()}>
            {ocupado ? "Salvando…" : "Salvar memória"}
          </Button>
          <Button size="sm" variant="ghost" disabled={ocupado} onClick={() => void importar()}>Importar arquivo…</Button>
        </div>
        {aviso && <p role="status" className="text-xs leading-relaxed text-text-secondary">{aviso}</p>}
        <p className="max-w-prose text-[11px] leading-relaxed text-text-muted">
          A memória guarda apenas o texto que você escolher. O histórico de cada conversa continua separado.
          Comandos personalizados com instruções próprias e outros agentes seguem as configurações da CLI.
        </p>
      </>}
    </section>
  );
}
