import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Button, Card } from "../../components/ui";
import { ROTULOS_DA_ATIVIDADE, TIPOS_DE_ATIVIDADE } from "../atividade/tipos";
import { telegram, type TelegramStatus } from "./telegramService";

/** Fork: notificações opt-in por bot privado; link para responder pela conversa já autenticada. */
export function TelegramSettings() {
  const [status, setStatus] = useState<TelegramStatus | null>(null);
  const [token, setToken] = useState("");
  const [ocupado, setOcupado] = useState(false);
  const [erro, setErro] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  useEffect(() => {
    let saiu = false;
    void telegram().then(s => { if (!saiu) setStatus(s); }).catch(e => { if (!saiu) setErro(e.message); });
    return () => { saiu = true; };
  }, []);
  useEffect(() => {
    if (!status?.parear_url && !status?.conectado) return;
    let saiu = false;
    const timer = window.setInterval(() => { void telegram().then(s => {
      if (!saiu) { setStatus(s); if (s.conectado && !status.conectado) setAviso("Telegram conectado. Use Enviar teste para conferir."); }
    }).catch(e => { if (!saiu) setErro(e.message); }); }, 3_000);
    return () => { saiu = true; window.clearInterval(timer); };
  }, [status?.parear_url, status?.conectado]);

  async function executar(acao: () => Promise<TelegramStatus>, mensagem?: string) {
    setOcupado(true); setErro(null); setAviso(null);
    try { setStatus(await acao()); if (mensagem) setAviso(mensagem); }
    catch (e) { setErro(e instanceof Error ? e.message : "Não consegui concluir a configuração."); }
    finally { setOcupado(false); }
  }
  const salvar = (mudanca: Partial<TelegramStatus>) => executar(() => telegram("", {
    ativo: status!.ativo, tipos: status!.tipos, incluir_titulos: status!.incluir_titulos, ...mudanca,
  }));
  return <Card title="Telegram">
    <div className="space-y-3 text-xs">
      <p className="text-text-secondary">Receba avisos quando um agente terminar, encontrar um erro, atingir o limite ou precisar de você. Funciona com a janela fechada enquanto o Mac e o engine estiverem ligados e com internet.</p>
      <p className="text-text-muted">Para responder, toque em Abrir conversa no Niski no aviso, com o Tailscale conectado e o celular pareado em Acesso remoto.</p>
      {!status?.conectado && !status?.parear_url && <>
        <p>Crie um bot exclusivo no <button className="text-accent" onClick={() => void openUrl("https://t.me/BotFather").catch(() => setErro("Não consegui abrir o Telegram."))}>@BotFather</button> com /newbot e cole o token abaixo.</p>
        <label className="block">Token do bot
          <input type="password" autoComplete="off" spellCheck={false} aria-label="Token do bot" className="mt-1 w-full rounded border border-border-subtle bg-bg-elevated p-2"
            value={token} onChange={e => setToken(e.target.value)} disabled={ocupado} />
        </label>
        <Button size="sm" disabled={ocupado || !token.trim() || !status} onClick={() => {
          const valor = token; setToken(""); void executar(() => telegram("/conectar", { token: valor }));
        }}>{ocupado ? "Conectando…" : "Conectar bot"}</Button>
      </>}
      {status?.parear_url && <div className="space-y-2">
        <p>Abra @{status.bot} pelo botão abaixo e toque em Iniciar no Telegram. O link vale por 10 minutos e conecta somente sua conversa privada.</p>
        <Button size="sm" onClick={() => void openUrl(status.parear_url!).catch(() => setErro("Não consegui abrir o Telegram."))}>Abrir Telegram para conectar</Button>
        <p className="text-text-muted" role="status">Aguardando você tocar em Iniciar…</p>
      </div>}
      {status?.conectado && <>
        <p className="text-success">Conectado a @{status.bot}</p>
        <label className="flex gap-2"><input type="checkbox" checked={status.ativo} disabled={ocupado} onChange={e => void salvar({ ativo: e.target.checked })} />Enviar avisos pelo Telegram</label>
        <fieldset className="space-y-2" disabled={ocupado}>
          <legend className="mb-2 text-text-muted">Quais avisos receber</legend>
          {TIPOS_DE_ATIVIDADE.map(tipo => <label key={tipo} className="flex gap-2"><input type="checkbox" checked={status.tipos.includes(tipo)}
            onChange={e => void salvar({ tipos: e.target.checked ? [...status.tipos, tipo] : status.tipos.filter(t => t !== tipo) })} />{ROTULOS_DA_ATIVIDADE[tipo]}</label>)}
          <label className="flex gap-2"><input type="checkbox" checked={status.incluir_titulos} onChange={e => void salvar({ incluir_titulos: e.target.checked })} />Incluir nome do projeto e título da conversa nos avisos</label>
        </fieldset>
        <p className="text-text-muted">Por padrão, o Telegram recebe apenas o agente, o tipo de aviso e o link. Textos das conversas e pedidos de permissão ficam no Niski.</p>
        <Button size="sm" disabled={ocupado} onClick={() => void executar(async () => { await telegram("/testar", {}); return telegram(); }, "Teste enviado ao Telegram.")}>Enviar teste</Button>
        {/* Fork: um exemplo de cada caso (erro, limite, pergunta…) para ver como chega no Telegram. */}
        <Button size="sm" variant="ghost" disabled={ocupado} onClick={() => void executar(async () => { await telegram("/exemplos", {}); return telegram(); }, "Exemplos enviados: um de cada aviso.")}>Enviar exemplo de cada aviso</Button>
      </>}
      {(status?.conectado || status?.parear_url) && <Button size="sm" variant="ghost" disabled={ocupado} onClick={() => void executar(() => telegram("/desconectar", {}), "Telegram desconectado.")}>Desconectar Telegram</Button>}
      {(erro || status?.erro) && <p className="text-danger" role="alert">{erro || status?.erro}</p>}
      {aviso && <p className="text-text-secondary" role="status">{aviso}</p>}
    </div>
  </Card>;
}
