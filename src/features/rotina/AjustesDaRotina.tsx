// Fork (Rotina): Ajustes da Rotina — o agente (plataforma, conta, modelo, conversa por semana ou
// contínua), os lembretes (o que lembrar, janela acordado, domingo à noite), lembretes próprios, a
// memória da rotina e o bot do Telegram da rotina.
import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Button, Card } from "../../components/ui";
import { listProfiles, type Profile } from "../terminal/terminalService";
import { lerAjustes, lerLembretes, lerSemana, operar, telegramDaRotina } from "./rotinaService";
import { NOMES_CURTOS_DOS_DIAS, type AjustesDaRotina as Ajustes, type Lembretes, type TelegramDaRotina } from "./tipos";

const DIAS_COMPLETOS = ["Segunda", "Terça", "Quarta", "Quinta", "Sexta", "Sábado", "Domingo"];

function Agente({ onErro }: { onErro: (e: string) => void }) {
  const [a, setA] = useState<Ajustes | null>(null);
  const [perfis, setPerfis] = useState<Profile[]>([]);
  useEffect(() => {
    void lerAjustes().then(setA, (e: Error) => onErro(e.message));
    void listProfiles().then(setPerfis).catch(() => setPerfis([]));
  }, []); // eslint-disable-line react-hooks/exhaustive-deps
  if (!a) return null;
  const salvar = (m: Partial<Ajustes>) => void operar<Ajustes>("salvar_ajustes", m).then((n) => setA({ ...a, ...n }), (e: Error) => onErro(e.message));
  const contas = perfis.filter((p) => p.provider === a.plataforma);
  return (
    <Card title="Agente da rotina">
      <div className="space-y-3 text-xs">
        <p className="text-text-secondary">A conversa da rotina é uma conversa normal do app: aparece no Histórico e volta de onde parou. Só Claude Code e Codex,
          que aceitam as ferramentas da rotina só nesta conversa.</p>
        <div className="rotina-form">
          <label>Plataforma<select value={a.plataforma} onChange={(e) => salvar({ plataforma: e.target.value, conta: "" })}>
            <option value="">A do copiloto</option><option value="claude">Claude Code</option><option value="codex">Codex</option></select></label>
          <label>Conta<select value={a.conta} disabled={!a.plataforma} onChange={(e) => salvar({ conta: e.target.value })}>
            <option value="">Padrão</option>{contas.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}</select></label>
          <label>Modelo<input value={a.modelo} disabled={!a.plataforma} placeholder="padrão da CLI" onChange={(e) => setA({ ...a, modelo: e.target.value })}
            onBlur={() => salvar({ modelo: a.modelo })} /></label>
          <label>Esforço<select value={a.esforco} disabled={!a.plataforma} onChange={(e) => salvar({ esforco: e.target.value })}>
            <option value="">Padrão</option><option value="low">Baixo</option><option value="medium">Médio</option><option value="high">Alto</option></select></label>
        </div>
        <fieldset className="space-y-1"><legend className="mb-1 text-text-muted">Conversa</legend>
          <label className="flex gap-2"><input type="radio" name="modo-conversa" checked={a.modo_conversa !== "continua"} onChange={() => salvar({ modo_conversa: "semanal" })} />
            Uma conversa por semana (recomendado): começa limpa no domingo e lê o plano, o histórico e a memória da rotina.</label>
          <label className="flex gap-2"><input type="radio" name="modo-conversa" checked={a.modo_conversa === "continua"} onChange={() => salvar({ modo_conversa: "continua" })} />
            Sempre a mesma conversa: guarda tudo o que foi dito, mas fica mais lenta e cara com o tempo.</label>
        </fieldset>
        <p className="text-text-muted">Mudanças valem na próxima conversa nova.</p>
      </div>
    </Card>
  );
}

function AjustesDosLembretes({ onErro }: { onErro: (e: string) => void }) {
  const [l, setL] = useState<Lembretes | null>(null);
  const [novo, setNovo] = useState({ texto: "", hora: "", tipo: "meta", dias: [] as number[] });
  const recarregar = () => lerLembretes().then(setL, (e: Error) => onErro(e.message));
  useEffect(() => { void recarregar(); }, []); // eslint-disable-line react-hooks/exhaustive-deps
  if (!l) return null;
  const salvar = (m: Record<string, unknown>) => void operar<Lembretes>("salvar_lembretes", m).then(setL, (e: Error) => onErro(e.message));
  const caixa = (campo: "ativo" | "tarefas" | "blocos" | "refeicoes" | "treino", rotulo: string) => (
    <label className="flex gap-2"><input type="checkbox" checked={l[campo]} disabled={campo !== "ativo" && !l.ativo} onChange={(e) => salvar({ [campo]: e.target.checked })} />{rotulo}</label>
  );
  return (
    <Card title="Lembretes">
      <div className="space-y-3 text-xs">
        <p className="text-text-secondary">Saem com a janela fechada, enquanto o Mac e o Niski estiverem ligados. No app aparecem num aviso discreto no canto;
          no Telegram, só com o bot da rotina conectado.</p>
        {caixa("ativo", "Lembretes ligados")}
        {caixa("tarefas", "Tarefas com hora")}
        {caixa("blocos", "Blocos da rotina (aulas, trabalho…)")}
        {caixa("treino", "Treino")}
        {caixa("refeicoes", "Refeições do plano")}
        <div className="rotina-form">
          <label>Avisar antes (min)<input type="number" min={0} max={240} className="rotina-numero" defaultValue={l.antecedencia_min}
            onBlur={(e) => salvar({ antecedencia_min: Number(e.target.value) })} /></label>
          <label>Acordo às<input type="time" defaultValue={l.acordar} onBlur={(e) => salvar({ acordar: e.target.value })} /></label>
          <label>Durmo às<input type="time" defaultValue={l.dormir} onBlur={(e) => salvar({ dormir: e.target.value })} /></label>
        </div>
        <div className="rotina-form">
          <label className="flex gap-2"><input type="checkbox" checked={l.agua.ativo} disabled={!l.ativo} onChange={(e) => salvar({ agua: { ativo: e.target.checked } })} />Água</label>
          <label>A cada (min)<input type="number" min={15} max={600} step={15} className="rotina-numero" defaultValue={l.agua.intervalo_min}
            onBlur={(e) => salvar({ agua: { intervalo_min: Number(e.target.value) } })} /></label>
          <span className="text-text-muted">Só entre acordar e dormir, e para quando a meta do dia é batida.</span>
        </div>
        <div className="rotina-form">
          <label className="flex gap-2"><input type="checkbox" checked={l.planejar.ativo} disabled={!l.ativo} onChange={(e) => salvar({ planejar: { ativo: e.target.checked } })} />
            Lembrar de planejar a semana</label>
          <label>Dia<select value={l.planejar.dia} onChange={(e) => salvar({ planejar: { dia: Number(e.target.value) } })}>
            {DIAS_COMPLETOS.map((d, i) => <option key={d} value={i}>{d}</option>)}</select></label>
          <label>Hora<input type="time" defaultValue={l.planejar.hora} onBlur={(e) => salvar({ planejar: { hora: e.target.value } })} /></label>
        </div>
        <h4 className="text-text-muted">Lembretes seus (metas, remédio…)</h4>
        <ul className="rotina-lista">
          {l.personalizados.map((p) => (
            <li key={p.id} className="rotina-linha-botoes">
              <span className="flex-1">{p.hora} · {p.texto} <small className="text-text-muted">{p.data || (p.dias.length ? p.dias.map((d) => NOMES_CURTOS_DOS_DIAS[d]).join(", ") : "todo dia")}</small></span>
              <button type="button" className="rotina-mini" onClick={() => void operar("remover_lembrete", { id: p.id }).then(recarregar, (e: Error) => onErro(e.message))}>Apagar</button>
            </li>
          ))}
        </ul>
        <form className="rotina-form" onSubmit={(e) => {
          e.preventDefault();
          void operar("salvar_lembrete", novo).then(() => { setNovo({ ...novo, texto: "", dias: [] }); void recarregar(); }, (er: Error) => onErro(er.message));
        }}>
          <label className="rotina-form-largo">Lembrete<input value={novo.texto} onChange={(e) => setNovo({ ...novo, texto: e.target.value })} placeholder="Ler 20 páginas" /></label>
          <label>Hora<input type="time" value={novo.hora} onChange={(e) => setNovo({ ...novo, hora: e.target.value })} /></label>
          <label>Tipo<select value={novo.tipo} onChange={(e) => setNovo({ ...novo, tipo: e.target.value })}>
            <option value="meta">Meta</option><option value="remedio">Remédio</option><option value="livre">Outro</option></select></label>
          <fieldset className="rotina-dias"><legend>Dias (nenhum = todo dia)</legend>
            {NOMES_CURTOS_DOS_DIAS.map((nome, i) => (
              <label key={nome} className={novo.dias.includes(i) ? "is-on" : ""}><input type="checkbox" checked={novo.dias.includes(i)}
                onChange={(e) => setNovo({ ...novo, dias: e.target.checked ? [...novo.dias, i].sort() : novo.dias.filter((d) => d !== i) })} />{nome}</label>
            ))}
          </fieldset>
          <Button size="sm" type="submit" disabled={!novo.texto.trim() || !novo.hora}>Criar lembrete</Button>
        </form>
      </div>
    </Card>
  );
}

function Memoria({ onErro }: { onErro: (e: string) => void }) {
  const [texto, setTexto] = useState<string | null>(null);
  const [salvo, setSalvo] = useState(false);
  useEffect(() => { void lerSemana().then((s) => setTexto(s.memoria), (e: Error) => onErro(e.message)); }, []); // eslint-disable-line react-hooks/exhaustive-deps
  if (texto === null) return null;
  return (
    <Card title="Memória da rotina">
      <div className="space-y-2 text-xs">
        <p className="text-text-secondary">O que o agente aprendeu sobre a sua rotina e lê toda semana. Pode corrigir aqui.</p>
        <textarea className="rotina-entrada w-full" rows={5} value={texto} onChange={(e) => { setTexto(e.target.value); setSalvo(false); }} aria-label="Memória da rotina" />
        <Button size="sm" onClick={() => void operar("anotar_memoria", { texto }).then(() => setSalvo(true), (e: Error) => onErro(e.message))}>Salvar memória</Button>
        {salvo && <span className="text-text-muted" role="status"> Salva.</span>}
      </div>
    </Card>
  );
}

export function TelegramDaRotinaCartao() {
  const [status, setStatus] = useState<TelegramDaRotina | null>(null);
  const [ajustes, setAjustes] = useState<Ajustes | null>(null);
  const [token, setToken] = useState("");
  const [ocupado, setOcupado] = useState(false);
  const [erro, setErro] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  useEffect(() => {
    void telegramDaRotina().then(setStatus, (e: Error) => setErro(e.message));
    void lerAjustes().then(setAjustes).catch(() => undefined);
  }, []);
  useEffect(() => {
    if (!status?.parear_url) return;
    const timer = window.setInterval(() => void telegramDaRotina().then(setStatus).catch(() => undefined), 3_000);
    return () => window.clearInterval(timer);
  }, [status?.parear_url]);
  const executar = async (acao: () => Promise<TelegramDaRotina>, mensagem?: string) => {
    setOcupado(true); setErro(null); setAviso(null);
    try { setStatus(await acao()); if (mensagem) setAviso(mensagem); }
    catch (e) { setErro(e instanceof Error ? e.message : "Não consegui concluir."); } finally { setOcupado(false); }
  };
  const mudarAjuste = (m: Partial<Ajustes>) => void operar<Ajustes>("salvar_ajustes", m).then((n) => setAjustes((a) => a ? { ...a, ...n } : a), (e: Error) => setErro(e.message));
  return (
    <Card title="Telegram da rotina">
      <div className="space-y-3 text-xs">
        <p className="text-text-secondary">Um bot só da rotina, separado do bot dos avisos dos agentes. <strong>Os lembretes vão com o texto</strong> (nome da tarefa,
          da refeição, da meta) e com botões Feito, Adiar 15 min e Pular. O bot dos avisos continua mandando só o tipo de aviso.</p>
        {!status?.conectado && !status?.parear_url && <>
          <p>Crie outro bot no <button className="text-accent" onClick={() => void openUrl("https://t.me/BotFather").catch(() => setErro("Não consegui abrir o Telegram."))}>@BotFather</button> com /newbot (não use o bot dos avisos) e cole o token.</p>
          <label className="block">Token do bot da rotina
            <input type="password" autoComplete="off" spellCheck={false} aria-label="Token do bot da rotina" className="mt-1 w-full rounded border border-border-subtle bg-bg-elevated p-2"
              value={token} onChange={(e) => setToken(e.target.value)} disabled={ocupado} />
          </label>
          <Button size="sm" disabled={ocupado || !token.trim() || !status} onClick={() => {
            const valor = token; setToken(""); void executar(() => telegramDaRotina("/conectar", { token: valor }));
          }}>{ocupado ? "Conectando…" : "Conectar bot da rotina"}</Button>
        </>}
        {status?.parear_url && <div className="space-y-2">
          <p>Abra @{status.bot} e toque em Iniciar. O link vale por 10 minutos e conecta só a sua conversa privada.</p>
          <Button size="sm" onClick={() => void openUrl(status.parear_url!).catch(() => setErro("Não consegui abrir o Telegram."))}>Abrir Telegram para conectar</Button>
          <p className="text-text-muted" role="status">Aguardando você tocar em Iniciar…</p>
        </div>}
        {status?.conectado && <>
          <p className="text-success">Conectado a @{status.bot}</p>
          <label className="flex gap-2"><input type="checkbox" checked={status.ativo} disabled={ocupado}
            onChange={(e) => void executar(() => telegramDaRotina("", { ativo: e.target.checked }))} />Bot da rotina ligado</label>
          {ajustes && <>
            <label className="flex gap-2"><input type="checkbox" checked={ajustes.telegram_lembretes} onChange={(e) => mudarAjuste({ telegram_lembretes: e.target.checked })} />
              Mandar os lembretes por aqui</label>
            <label className="flex gap-2"><input type="checkbox" checked={ajustes.telegram_conversa} onChange={(e) => mudarAjuste({ telegram_conversa: e.target.checked })} />
              Conversar com o agente da rotina pelo Telegram</label>
            <p className="text-text-muted">Ligado, o que você escrever ao bot vai para a conversa da rotina (no seu Claude Code ou Codex, no Mac) e a resposta volta
              para o Telegram. O texto passa pelos servidores do Telegram. Comandos: /hoje e /agua.</p>
          </>}
          <Button size="sm" disabled={ocupado} onClick={() => void executar(async () => { await telegramDaRotina("/testar", {}); return telegramDaRotina(); }, "Teste enviado.")}>Enviar teste</Button>
        </>}
        {(status?.conectado || status?.parear_url) && <Button size="sm" variant="ghost" disabled={ocupado}
          onClick={() => void executar(() => telegramDaRotina("/desconectar", {}), "Bot da rotina desconectado.")}>Desconectar</Button>}
        {(erro || status?.erro) && <p className="text-danger" role="alert">{erro || status?.erro}</p>}
        {aviso && <p className="text-text-secondary" role="status">{aviso}</p>}
      </div>
    </Card>
  );
}

export function AjustesDaRotina({ onErro }: { onErro: (e: string) => void }) {
  return (
    <div className="rotina-ajustes">
      <Agente onErro={onErro} />
      <AjustesDosLembretes onErro={onErro} />
      <TelegramDaRotinaCartao />
      <Memoria onErro={onErro} />
    </div>
  );
}
