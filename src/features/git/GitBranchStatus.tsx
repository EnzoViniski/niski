// Fork: a paleta abre o mesmo seletor de branch usado pelo atalho.
import { useAcoesDaPaleta } from "../paleta/acoes";
import { useCallback, useEffect, useRef, useState } from "react";
import { matchesShortcut } from "../../lib/shortcuts";
import { GitBranchIcon, GithubIcon } from "../../components/ui/PixelIcon";
import { isRemoteProject } from "../projects/targetState";
import { gitBranches, gitCheckoutBranch, gitConta, gitPull, type GitBranch, type GitConta } from "./gitService";
import { gitStatusCompartilhado } from "./statusCompartilhado";

/** Releitura da conta: muda quase nunca (login/logout), e o `gh auth status` vai à rede. */
const CONTA_RITMO_MS = 120_000;

/** Conta do git no rodapé, à esquerda da branch: o login do GitHub (ou o `user.name`) e um ponto
 *  verde quando está tudo certo para commitar e dar push — amarelo com o motivo quando não. */
export function GitContaStatus({ projectPath }: { projectPath: string | null }) {
  const [conta, setConta] = useState<GitConta | null>(null);
  useEffect(() => {
    if (!projectPath) { setConta(null); return; }
    let vivo = true;
    const ler = () => gitConta(projectPath).then((lida) => { if (vivo) setConta(lida); }).catch(() => { if (vivo) setConta(null); });
    ler();
    const timer = window.setInterval(ler, CONTA_RITMO_MS);
    return () => { vivo = false; window.clearInterval(timer); };
  }, [projectPath]);

  if (!conta) return null;
  const quem = conta.github ?? conta.nome ?? "sem conta";
  const detalhes = [
    conta.nome ? `Commits como: ${conta.nome}` : "Commits sem nome de autor",
    conta.email_configurado ? "E-mail do git configurado" : "E-mail do git não configurado",
    conta.gh_instalado ? (conta.github ? `GitHub: ${conta.github}` : "GitHub CLI sem login") : "GitHub CLI não instalado",
    conta.problema ?? "Tudo certo para commitar e dar push",
  ].join("\n");
  return (
    <span className="flex min-w-0 shrink items-center gap-1.5" title={detalhes}>
      <span aria-hidden className={`h-1.5 w-1.5 shrink-0 ${conta.problema ? "bg-warning" : "bg-success"}`} />
      <span className="sr-only">{conta.problema ? `Git com problema: ${conta.problema}.` : "Git ok."}</span>
      <span className={`min-w-0 truncate ${conta.problema ? "text-warning" : ""}`}>{quem}</span>
      {conta.github
        ? <GithubIcon aria-hidden className={`h-3 w-3 shrink-0 ${conta.problema ? "text-warning" : "text-success"}`} />
        : <GitBranchIcon aria-hidden className={`h-3 w-3 shrink-0 ${conta.problema ? "text-warning" : "text-success"}`} />}
    </span>
  );
}

/** Branch do projeto em foco no rodapé, com pull, troca de branch e criação ao lado. O status vem
 *  do mesmo `git_status` do painel lateral; relê sozinho porque quem troca de branch costuma ser o
 *  agente, no terminal. */
export function GitBranchStatus({ projectPath, abrirPara = "cima", atalho = true }: {
  projectPath: string | null;
  /** Fork: na faixa do agente (topo do painel) o menu abre para baixo. */
  abrirPara?: "cima" | "baixo";
  /** Fork: só o do rodapé responde ao atalho de branch nova — senão cada agente abriria o seu. */
  atalho?: boolean;
}) {
  const [branch, setBranch] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  useAcoesDaPaleta("branch", atalho && projectPath ? [{ id: "branch", titulo: "Criar ou trocar branch", executar: () => setOpen(true) }] : []);
  const [branches, setBranches] = useState<GitBranch[] | null>(null);
  const [nome, setNome] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const botaoRef = useRef<HTMLButtonElement>(null);
  /** Posição do popover em coordenadas de tela. O rodapé é `h-7` com `overflow-hidden`: um popover
   *  `absolute` nasce dentro dele e é recortado até sumir — `fixed` escapa do recorte. */
  const [ancora, setAncora] = useState<{ left: number; bottom?: number; top?: number }>({ left: 0, bottom: 0 });

  // Fork (desempenho): o tique de fundo divide o `git status` com o rodapé, o painel e as outras abas.
  const refresh = useCallback((fresco = true) => {
    if (!projectPath) {
      setBranch(null);
      return;
    }
    gitStatusCompartilhado(projectPath, { fresco })
      .then((status) => setBranch(status?.branch ?? null))
      .catch(() => setBranch(null));
  }, [projectPath]);

  useEffect(() => {
    refresh(false);
    // Em projeto remoto cada leitura é um `wsl`/`ssh` — 3s viraria tráfego constante à toa.
    const ritmo = isRemoteProject(projectPath) ? 10_000 : 3_000;
    // Fork: com a janela oculta não roda `git status` (o próximo tique depois de voltar relê).
    const timer = window.setInterval(() => { if (!document.hidden) refresh(false); }, ritmo);
    return () => window.clearInterval(timer);
  }, [refresh, projectPath]);

  // Em captura, como o Ctrl+P: sem isso o atalho não funcionaria com o foco no terminal, que é
  // justamente onde o usuário está quando decide abrir uma branch nova.
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (!atalho || !matchesShortcut(event, "newBranch") || !projectPath) return;
      event.preventDefault();
      event.stopPropagation();
      setOpen(true);
    }
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [projectPath, atalho]);

  // A lista é relida a cada abertura: branch criada no terminal aparece sem precisar reabrir o app.
  useEffect(() => {
    if (!open || !projectPath) return;
    setNome("");
    setBranches(null);
    const rect = botaoRef.current?.getBoundingClientRect();
    if (rect) setAncora(abrirPara === "baixo" ? { left: rect.left, top: rect.bottom + 4 } : { left: rect.left, bottom: window.innerHeight - rect.top + 4 });
    gitBranches(projectPath)
      .then(setBranches)
      .catch(() => setBranches([]));
    inputRef.current?.focus();
  }, [open, projectPath, abrirPara]);

  if (!projectPath || !branch) return null;

  async function correr(acao: () => Promise<string | void>) {
    if (!projectPath) return;
    setBusy(true);
    setMessage(null);
    try {
      const saida = await acao();
      setMessage((typeof saida === "string" && saida.trim()) || null);
      setOpen(false);
      refresh();
    } catch (reason) {
      setMessage(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }

  const alvo = nome.trim();
  const existe = (branches ?? []).some((item) => item.name === alvo);
  const listadas = (branches ?? []).filter((item) => item.name.toLowerCase().includes(alvo.toLowerCase()));

  return (
    <span className="relative flex min-w-0 items-center gap-1.5">
      <button
        ref={botaoRef}
        type="button"
        onClick={() => setOpen((value) => !value)}
        aria-expanded={open}
        aria-haspopup="dialog"
        title={`Branch atual: ${branch} — clique para trocar ou criar`}
        className="min-w-0 truncate hover:text-text-primary"
      >
        ⎇ {branch}
      </button>
      <button
        type="button"
        disabled={busy}
        onClick={() => void correr(() => gitPull(projectPath))}
        title="git pull --ff-only"
        className="shrink-0 px-1 uppercase hover:text-text-primary disabled:opacity-40"
      >
        {busy ? "pull…" : "pull"}
      </button>
      {message && (
        // Clicar limpa: é recado de uma ação só, e no rodapé não cabe banner.
        <button
          type="button"
          onClick={() => setMessage(null)}
          title={message}
          className="hidden min-w-0 max-w-[16rem] truncate text-left hover:text-text-primary lg:inline"
        >
          {message.split("\n")[0]}
        </button>
      )}

      {open && (
        <>
          {/* Camada de fundo: clicar fora fecha, sem listener global no document. */}
          <span className="fixed inset-0 z-40" onPointerDown={() => setOpen(false)} />
          <div
            role="dialog"
            aria-label="Branches"
            style={ancora}
            className="popover fixed z-50 flex w-64 flex-col gap-1 p-1"
          >
            <input
              ref={inputRef}
              autoFocus
              value={nome}
              onChange={(event) => setNome(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Escape") setOpen(false);
                if (event.key !== "Enter" || !alvo || busy) return;
                void correr(() => gitCheckoutBranch(projectPath, alvo, !existe));
              }}
              placeholder="Filtrar ou nome da branch nova"
              aria-label="Filtrar ou nome da branch nova"
              className="w-full border border-border-subtle bg-bg-elevated px-2 py-1 text-xs text-text-primary outline-none focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent"
            />
            {alvo && !existe && (
              <button
                type="button"
                disabled={busy}
                onClick={() => void correr(() => gitCheckoutBranch(projectPath, alvo, true))}
                className="border border-border-subtle px-2 py-1 text-left text-xs text-accent hover:bg-overlay/[0.07] disabled:opacity-40"
              >
                + Criar “{alvo}” a partir de {branch}
              </button>
            )}
            <ul className="max-h-48 overflow-y-auto">
              {branches === null && <li className="px-2 py-1 text-xs text-text-muted">Lendo branches…</li>}
              {branches !== null && listadas.length === 0 && (
                <li className="px-2 py-1 text-xs text-text-muted">Nenhuma branch com esse nome.</li>
              )}
              {listadas.map((item) => (
                <li key={item.name}>
                  <button
                    type="button"
                    disabled={busy || item.current}
                    onClick={() => void correr(() => gitCheckoutBranch(projectPath, item.name, false))}
                    className="w-full truncate px-2 py-1 text-left text-xs text-text-secondary hover:bg-overlay/[0.07] disabled:opacity-40"
                    title={item.current ? `${item.name} (atual)` : `Trocar para ${item.name}`}
                  >
                    {item.current ? "● " : "○ "}
                    {item.name}
                  </button>
                </li>
              ))}
            </ul>
          </div>
        </>
      )}
    </span>
  );
}
