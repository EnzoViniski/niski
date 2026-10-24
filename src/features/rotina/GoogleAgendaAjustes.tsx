// Fork (Google Agenda): cartão dos Ajustes da Rotina para ligar o Google Agenda pessoal. O login é no
// navegador (a página do Google); o ID e a chave do cliente vão direto para o engine e não voltam.
import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Button, Card } from "../../components/ui";
import { conectarGoogleAgenda, desconectarGoogleAgenda, lerGoogleAgenda, sincronizarGoogleAgenda, type GoogleAgenda } from "./googleAgendaService";

function quando(ms: number): string {
  if (!ms) return "ainda não";
  return new Date(ms).toLocaleString("pt-BR", { day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" });
}

export function GoogleAgendaCartao() {
  const [status, setStatus] = useState<GoogleAgenda | null>(null);
  const [id, setId] = useState("");
  const [segredo, setSegredo] = useState("");
  const [url, setUrl] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [erro, setErro] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);

  useEffect(() => { void lerGoogleAgenda().then(setStatus, (e: Error) => setErro(e.message)); }, []);
  // Enquanto o login está aberto no navegador, confere a cada 2 s.
  useEffect(() => {
    if (!status?.conectando) return;
    const t = window.setInterval(() => void lerGoogleAgenda().then(setStatus).catch(() => undefined), 2_000);
    return () => window.clearInterval(t);
  }, [status?.conectando]);

  // Se o navegador não abrir, o link fica na tela para copiar (0.31.0 não abria o login do Google).
  const [semNavegador, setSemNavegador] = useState<string | null>(null);
  const abrir = (endereco: string) => void openUrl(endereco).then(() => setSemNavegador(null), () => {
    setSemNavegador(endereco);
    setErro("Não consegui abrir o navegador. Copie o link abaixo e cole no navegador.");
  });
  const executar = async (acao: () => Promise<GoogleAgenda | null>, mensagem?: string) => {
    setOcupado(true); setErro(null); setAviso(null);
    try { const s = await acao(); if (s) setStatus(s); if (mensagem) setAviso(mensagem); }
    catch (e) { setErro(e instanceof Error ? e.message : "Não consegui concluir."); } finally { setOcupado(false); }
  };
  const conectar = () => executar(async () => {
    const s = await conectarGoogleAgenda(id, segredo);
    setSegredo("");
    if (s.url) { setUrl(s.url); abrir(s.url); }
    return s;
  });

  return (
    <Card title="Google Agenda">
      <div className="space-y-3 text-xs">
        <p className="text-text-secondary">Os compromissos de todas as agendas que estão visíveis no seu Google Agenda (inclusive as de outros e-mails compartilhadas com você e os aniversários) aparecem em Hoje e na Semana (atualiza a cada 5 minutos), e você cria
          compromissos no Google pelo botão “Novo compromisso”. O agente da rotina também enxerga e cria.</p>

        {status && !status.conectado && !status.conectando && <>
          <ol className="list-decimal space-y-1 pl-4 text-text-secondary">
            <li>Abra o <button className="text-accent" onClick={() => abrir("https://console.cloud.google.com/projectcreate")}>Google Cloud</button> com
              a sua conta pessoal e crie um projeto (ex.: “Niski”). É gratuito.</li>
            <li>Ative a <button className="text-accent" onClick={() => abrir("https://console.cloud.google.com/apis/library/calendar-json.googleapis.com")}>Google Calendar API</button> nesse projeto.</li>
            <li>Em <button className="text-accent" onClick={() => abrir("https://console.cloud.google.com/auth/overview")}>Google Auth Platform</button>,
              configure: nome “Niski”, público <strong>Externo</strong>, seu e-mail. Depois, em “Público-alvo”, clique em <strong>Publicar app</strong>
              (em modo de teste o Google desconecta a cada 7 dias).</li>
            <li>Em “Clientes”, crie um cliente do tipo <strong>App para computador</strong> e copie o ID e a chave secreta.</li>
            <li>Cole abaixo e clique em Conectar. No navegador, entre com a conta pessoal; como o app é só seu, o Google avisa que não é verificado:
              clique em “Avançado” e “Acessar Niski”.</li>
          </ol>
          <label className="block">ID do cliente
            <input autoComplete="off" spellCheck={false} aria-label="ID do cliente" placeholder="…apps.googleusercontent.com"
              className="mt-1 w-full rounded border border-border-subtle bg-bg-elevated p-2" value={id} onChange={(e) => setId(e.target.value)} disabled={ocupado} />
          </label>
          <label className="block">Chave secreta do cliente
            <input type="password" autoComplete="off" spellCheck={false} aria-label="Chave secreta do cliente"
              className="mt-1 w-full rounded border border-border-subtle bg-bg-elevated p-2" value={segredo} onChange={(e) => setSegredo(e.target.value)} disabled={ocupado} />
          </label>
          <Button size="sm" disabled={ocupado || !id.trim() || !segredo.trim()} onClick={() => void conectar()}>{ocupado ? "Abrindo…" : "Conectar"}</Button>
        </>}

        {status?.conectando && <div className="space-y-2">
          <p role="status">Abri a página do Google no navegador. Entre com a sua conta pessoal e autorize o acesso à agenda…</p>
          <div className="flex gap-2">
            {url && <Button size="sm" onClick={() => abrir(url)}>Abrir de novo</Button>}
            <Button size="sm" variant="ghost" disabled={ocupado} onClick={() => void executar(desconectarGoogleAgenda)}>Cancelar</Button>
          </div>
        </div>}

        {status?.conectado && <>
          <p className="text-success">Conectado{status.conta ? ` a ${status.conta}` : ""}</p>
          <p className="text-text-muted">{status.eventos} compromissos nos próximos meses · última atualização {quando(status.sincronizado_ms)}</p>
          <div className="flex gap-2">
            <Button size="sm" disabled={ocupado} onClick={() => void executar(async () => {
              const r = await sincronizarGoogleAgenda();
              setAviso(`Atualizado: ${r.eventos} compromissos.`);
              return lerGoogleAgenda();
            })}>Atualizar agora</Button>
            <Button size="sm" variant="ghost" disabled={ocupado} onClick={() => void executar(desconectarGoogleAgenda, "Google Agenda desconectado.")}>Desconectar</Button>
          </div>
        </>}

        {semNavegador && <div className="space-y-1">
          <input readOnly aria-label="Link para abrir no navegador" value={semNavegador} onFocus={(e) => e.currentTarget.select()}
            className="w-full rounded border border-border-subtle bg-bg-elevated p-2" />
          <Button size="sm" variant="ghost" onClick={() => void navigator.clipboard.writeText(semNavegador).then(() => setAviso("Link copiado."), () => undefined)}>Copiar link</Button>
        </div>}
        {(erro || status?.erro) && <p className="text-danger" role="alert">{erro || status?.erro}</p>}
        {aviso && <p className="text-text-secondary" role="status">{aviso}</p>}
      </div>
    </Card>
  );
}
