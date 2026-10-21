import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { RoadmapsView } from "../features/roadmaps/RoadmapsView";
import { gravacoesPendentes } from "../features/roadmaps/armazenamento";
import { limparLoja } from "../features/roadmaps/loja";
import { animarConclusao } from "../features/roadmaps/animacoes";
import { setMotionLevel } from "../lib/motion";
import { definirPausaDeEnergia } from "../lib/energia";
import {
  SISTEMA_DA_ENTREVISTA, SISTEMA_DA_ETAPA, SISTEMA_DAS_TAREFAS, SISTEMA_DO_PAINEL, SISTEMA_DO_PLANO, SISTEMA_DOS_CARDS,
} from "../features/roadmaps/prompts";

const mockedInvoke = vi.mocked(invoke);

/** Disco falso com as mesmas regras dos comandos `roadmaps_*` do Rust (o que importa para a tela). */
function discoFalso() {
  const arquivos = new Map<string, string>();
  let ordem: string[] = [];
  let n = 0;
  return {
    arquivos,
    comando(cmd: string, args: Record<string, unknown>) {
      switch (cmd) {
        case "roadmaps_listar":
          return ordem.filter((id) => arquivos.has(`${id}/meta.json`)).map((id) => ({ id, meta: JSON.parse(arquivos.get(`${id}/meta.json`) as string) }));
        case "roadmaps_criar": {
          const id = `r-teste-${++n}`;
          arquivos.set(`${id}/meta.json`, JSON.stringify({ ...(args.meta as object), id }));
          ordem = [id, ...ordem];
          return id;
        }
        case "roadmaps_ler": return arquivos.get(`${args.id as string}/${args.arquivo as string}`) ?? null;
        case "roadmaps_gravar": {
          const arquivo = args.arquivo as string;
          if (arquivo.endsWith(".json")) JSON.parse(args.conteudo as string);
          arquivos.set(`${args.id as string}/${arquivo}`, args.conteudo as string);
          return null;
        }
        case "roadmaps_anexar_conversa": {
          const chave = `${args.id as string}/conversa.jsonl`;
          arquivos.set(chave, `${arquivos.get(chave) ?? ""}${args.linha as string}\n`);
          return null;
        }
        case "roadmaps_ordenar": ordem = args.ordem as string[]; return null;
        case "roadmaps_backup_antes_de_migrar": return "/tmp/backup";
        default: return undefined;
      }
    },
  };
}

const PLANO = {
  objetivo: "Fechar mais vendas B2B", visao: "Do rapport ao fechamento",
  modulos: [
    { id: "m-base", titulo: "Fundamentos", etapas: [
      { id: "e-rapport", titulo: "Rapport", minutos: 20, tarefa: "Ligue para 2 clientes", fontes: [{ titulo: "Guia de rapport", url: "https://exemplo.com/rapport", tipo: "artigo", gratis: true }] },
      { id: "e-spin", titulo: "Perguntas SPIN", minutos: 25, tarefa: "Escreva 5 perguntas", fontes: [{ titulo: "Livro SPIN", url: "https://exemplo.com/spin", tipo: "livro", gratis: false }] },
    ] },
    { id: "m-fim", titulo: "Fechamento", etapas: [{ id: "e-objecoes", titulo: "Objeções", minutos: 30, tarefa: "Liste objeções", fontes: [] }] },
  ],
};

/** IA falsa: responde pelo prompt de sistema; conta os pedidos de cada tipo. */
function iaFalsa() {
  const pedidos: { sistema: string; texto: string; web: boolean; cli: string }[] = [];
  let perguntas = 0;
  return {
    pedidos,
    responder(pedido: { sistema: string; texto: string; web: boolean; cli: string }) {
      pedidos.push(pedido);
      if (pedido.sistema === SISTEMA_DA_ENTREVISTA) {
        perguntas++;
        return perguntas === 1
          ? JSON.stringify({ tipo: "pergunta", pergunta: "Qual é o seu nível hoje?", opcoes: ["Iniciante", "Intermediário"] })
          : "```json\n" + JSON.stringify({ tipo: "pronto", resumo: "Você é iniciante e quer fechar mais." }) + "\n```";
      }
      if (pedido.sistema === SISTEMA_DO_PLANO) return `Aqui está:\n${JSON.stringify(PLANO)}`;
      if (pedido.sistema === SISTEMA_DA_ETAPA) return JSON.stringify({ resumo: "## Rapport\n\nRapport é criar sintonia com o cliente antes de vender.", tarefa: "", fontes: [{ titulo: "Vídeo sobre rapport", url: "https://exemplo.com/video", tipo: "video", gratis: true }] });
      if (pedido.sistema === SISTEMA_DOS_CARDS) return JSON.stringify({ cards: [{ frente: "O que é rapport?", verso: "Sintonia com o cliente" }, { frente: "Para que serve?", verso: "Gerar confiança" }] });
      if (pedido.sistema === SISTEMA_DAS_TAREFAS) return JSON.stringify({ tarefas: [{ texto: "Grave um pitch de 1 minuto" }] });
      if (pedido.sistema === SISTEMA_DO_PAINEL) {
        const novo = { ...PLANO, modulos: [...PLANO.modulos, { id: "m-extra", titulo: "Prática extra", etapas: [{ id: "e-role", titulo: "Role-play", minutos: 20 }] }] };
        return `Claro! Rapport vem do francês.\n\`\`\`plano\n${JSON.stringify(novo)}\n\`\`\``;
      }
      throw new Error(`prompt inesperado: ${pedido.sistema.slice(0, 40)}`);
    },
  };
}

let disco: ReturnType<typeof discoFalso>;
let ia: ReturnType<typeof iaFalsa>;

beforeEach(() => {
  limparLoja();
  localStorage.clear();
  disco = discoFalso();
  ia = iaFalsa();
  mockedInvoke.mockReset();
  mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
    const a = (args ?? {}) as Record<string, unknown>;
    if (cmd === "roadmaps_ia") return ia.responder(a.pedido as { sistema: string; texto: string; web: boolean; cli: string });
    if (cmd === "roadmaps_ia_cancelar") return null;
    if (cmd === "list_profiles") return [{ id: "claude-padrao", provider: "claude", name: "Padrão", config_dir: "", builtin: true, created_at_ms: 0, last_used_at_ms: null, authenticated: true }];
    if (cmd === "agentes_modelos") return { claude: [{ id: "sonnet", label: "Sonnet", esforcos: [] }] };
    return disco.comando(cmd, a);
  });
});

afterEach(async () => { await gravacoesPendentes(); });

/** "Fecha e reabre o app": esquece tudo da memória; só o disco falso continua. */
function reabrir(desmontar: () => void) {
  desmontar();
  limparLoja();
  return render(<RoadmapsView onClose={() => undefined} continuar />);
}

async function criarEAceitar() {
  const tela = render(<RoadmapsView onClose={() => undefined} />);
  fireEvent.click(await screen.findByRole("button", { name: "Novo roadmap" }));
  fireEvent.change(screen.getByPlaceholderText(/melhorar em vendas/), { target: { value: "melhorar em vendas" } });
  fireEvent.change(screen.getByPlaceholderText("Vendas"), { target: { value: "Vendas" } });
  fireEvent.click(screen.getByRole("button", { name: "Começar entrevista" }));
  // Primeira pergunta, com resposta rápida.
  fireEvent.click(await screen.findByRole("button", { name: "Iniciante" }));
  // "Pronto" → monta o plano (com web) → revisão do plano.
  const proposta = await screen.findByRole("region", { name: "Plano proposto" });
  expect(within(proposta).getByText("Fechar mais vendas B2B")).toBeInTheDocument();
  fireEvent.click(within(proposta).getByRole("button", { name: "Aceitar e começar" }));
  await screen.findByRole("button", { name: /Rapport: Disponível \(hoje\)/ });
  return tela;
}

describe("Roadmaps de estudo", () => {
  it("cria por entrevista, revisa o plano e abre a trilha na etapa de hoje", async () => {
    await criarEAceitar();
    expect(screen.getByRole("button", { name: /Perguntas SPIN: Bloqueada/ })).toBeInTheDocument();
    const entrevista = ia.pedidos.filter((p) => p.sistema === SISTEMA_DA_ENTREVISTA);
    expect(entrevista).toHaveLength(2);
    expect(entrevista.every((p) => !p.web)).toBe(true);
    expect(entrevista[1].texto).toContain("Pessoa: Iniciante");
    expect(ia.pedidos.find((p) => p.sistema === SISTEMA_DO_PLANO)?.web).toBe(true);
    await gravacoesPendentes();
    const id = [...disco.arquivos.keys()][0].split("/")[0];
    expect(JSON.parse(disco.arquivos.get(`${id}/meta.json`) as string)).toMatchObject({ nome: "Vendas", fase: "ativo", resumo: { etapas: 3, concluidas: 0 } });
    expect(JSON.parse(disco.arquivos.get(`${id}/plano.json`) as string).modulos).toHaveLength(2);
    expect(JSON.parse(disco.arquivos.get(`${id}/entrevista.json`) as string).proposta).toBeUndefined();
  });

  it("etapa: prepara o resumo, gera cards e tarefas, conclui; revisão FSRS; tudo continua depois de reabrir", async () => {
    const tela = await criarEAceitar();
    fireEvent.click(screen.getByRole("button", { name: /Rapport: Disponível/ }));
    const painel = await screen.findByRole("complementary", { name: "Etapa" });
    // Abrir a etapa disponível é começar.
    await screen.findByRole("button", { name: /Rapport: Em andamento/ });
    expect(within(painel).getByText("Grátis")).toBeInTheDocument();
    fireEvent.click(within(painel).getByRole("button", { name: "Preparar etapa" }));
    expect(await within(painel).findByText(/criar sintonia com o cliente/)).toBeInTheDocument();
    expect(within(painel).getByRole("link", { name: "Vídeo sobre rapport" })).toHaveAttribute("href", "https://exemplo.com/video");
    fireEvent.click(within(painel).getByRole("button", { name: /Gerar cards de revisão/ }));
    expect(await within(painel).findByText("2 cards entraram na revisão de hoje.")).toBeInTheDocument();
    fireEvent.click(within(painel).getByRole("button", { name: "Gerar tarefas" }));
    expect(await within(painel).findByText("Grave um pitch de 1 minuto")).toBeInTheDocument();
    fireEvent.click(within(painel).getByRole("checkbox"));
    fireEvent.click(within(painel).getByRole("button", { name: "Concluir etapa" }));
    await screen.findByRole("button", { name: /Rapport: Concluída/ });
    expect(screen.getByRole("button", { name: /Perguntas SPIN: Disponível \(hoje\)/ })).toBeInTheDocument();
    expect(screen.getByText("🔥 1")).toBeInTheDocument();

    // Revisão: 2 cards novos na fila; "Bom" e "Fácil" tiram da fila de agora.
    fireEvent.click(screen.getByRole("button", { name: "Revisar (2)" }));
    const revisao = screen.getByRole("dialog", { name: "Revisão" });
    fireEvent.click(within(revisao).getByRole("button", { name: "Mostrar resposta" }));
    fireEvent.click(within(revisao).getByRole("button", { name: /Fácil/ }));
    await within(revisao).findByText("1 feitas");
    fireEvent.click(within(revisao).getByRole("button", { name: "Mostrar resposta" }));
    fireEvent.click(within(revisao).getByRole("button", { name: /Fácil/ }));
    await within(revisao).findByText(/Pronto por agora/);
    fireEvent.click(within(revisao).getByRole("button", { name: "Voltar à trilha" }));
    await gravacoesPendentes();

    // Fecha e reabre: o botão Roadmaps volta direto ao mesmo roadmap, com tudo lá.
    const segunda = reabrir(tela.unmount);
    expect(await screen.findByRole("button", { name: /Rapport: Concluída/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Perguntas SPIN: Disponível \(hoje\)/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Revisar" })).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: /Rapport: Concluída/ }));
    const reaberto = await screen.findByRole("complementary", { name: "Etapa" });
    expect(await within(reaberto).findByText(/criar sintonia com o cliente/)).toBeInTheDocument();
    expect(within(reaberto).getByRole("checkbox")).toBeChecked();
    expect(within(reaberto).getByRole("button", { name: /Gerar cards de revisão \(2\)/ })).toBeInTheDocument();
    segunda.unmount();
  });

  it("painel de IA: conversa gravada em arquivo e reajuste só com confirmação", async () => {
    const tela = await criarEAceitar();
    fireEvent.click(screen.getByRole("button", { name: "IA" }));
    const painel = await screen.findByRole("complementary", { name: "IA ao lado" });
    fireEvent.change(within(painel).getByLabelText("Mensagem para a IA"), { target: { value: "Estou atrasado" } });
    fireEvent.click(within(painel).getByRole("button", { name: "Enviar" }));
    expect(within(painel).getByText("Estou atrasado")).toBeInTheDocument();
    expect(await within(painel).findByText(/Rapport vem do francês/)).toBeInTheDocument();
    const proposta = within(painel).getByRole("region", { name: "Reajuste proposto" });
    // Ainda não mudou nada.
    expect(screen.queryByRole("region", { name: "Prática extra" })).toBeNull();
    fireEvent.click(within(proposta).getByRole("button", { name: "Aplicar" }));
    expect(await screen.findByRole("region", { name: "Prática extra" })).toBeInTheDocument();
    await gravacoesPendentes();
    const id = [...disco.arquivos.keys()][0].split("/")[0];
    const linhas = (disco.arquivos.get(`${id}/conversa.jsonl`) as string).trim().split("\n").map((l) => JSON.parse(l));
    expect(linhas.map((l) => l.de)).toEqual(["eu", "ia"]);
    expect(linhas[1].texto).not.toContain("```plano");

    reabrir(tela.unmount);
    fireEvent.click(await screen.findByRole("button", { name: "IA" }));
    expect(await screen.findByText("Estou atrasado")).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Prática extra" })).toBeInTheDocument();
  });

  it("erro da IA aparece com 'Tentar de novo' e a resposta da pessoa não se perde", async () => {
    render(<RoadmapsView onClose={() => undefined} />);
    fireEvent.click(await screen.findByRole("button", { name: "Novo roadmap" }));
    fireEvent.change(screen.getByPlaceholderText(/melhorar em vendas/), { target: { value: "inglês" } });
    fireEvent.click(screen.getByRole("button", { name: "Começar entrevista" }));
    await screen.findByRole("button", { name: "Iniciante" });
    const original = ia.responder;
    ia.responder = () => { throw new Error("Nenhuma conta do Claude com login"); };
    fireEvent.click(screen.getByRole("button", { name: "Iniciante" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Nenhuma conta do Claude com login");
    await gravacoesPendentes();
    const id = [...disco.arquivos.keys()][0].split("/")[0];
    expect(JSON.parse(disco.arquivos.get(`${id}/entrevista.json`) as string).mensagens.map((m: { texto: string }) => m.texto)).toEqual(["Qual é o seu nível hoje?", "Iniciante"]);
    ia.responder = original;
    fireEvent.click(screen.getByRole("button", { name: "Tentar de novo" }));
    expect(await screen.findByRole("region", { name: "Plano proposto" })).toBeInTheDocument();
  });

  it("lista: renomear, arquivar e reordenar gravam no disco", async () => {
    const tela = render(<RoadmapsView onClose={() => undefined} />);
    await screen.findByText(/Nenhum roadmap ainda/);
    for (const nome of ["Inglês", "Cálculo"]) {
      fireEvent.click(screen.getByRole("button", { name: "Novo roadmap" }));
      fireEvent.change(screen.getByPlaceholderText(/melhorar em vendas/), { target: { value: nome } });
      fireEvent.click(screen.getByRole("button", { name: "Começar entrevista" }));
      fireEvent.click(await screen.findByRole("button", { name: "← Roadmaps" }));
      await screen.findByRole("button", { name: "Novo roadmap" });
    }
    const nomes = () => screen.getAllByRole("button", { name: /^Abrir / }).map((b) => b.getAttribute("aria-label"));
    expect(nomes()).toEqual(["Abrir Cálculo", "Abrir Inglês"]);
    fireEvent.click(screen.getByRole("button", { name: "Mover Cálculo para depois" }));
    await waitFor(() => expect(nomes()).toEqual(["Abrir Inglês", "Abrir Cálculo"]));
    fireEvent.click(screen.getByRole("button", { name: "Editar Inglês" }));
    fireEvent.change(screen.getByLabelText("Nome do roadmap"), { target: { value: "Inglês para reuniões" } });
    fireEvent.click(screen.getByRole("button", { name: "Salvar" }));
    await screen.findByRole("button", { name: "Abrir Inglês para reuniões" });
    fireEvent.click(screen.getByRole("button", { name: "Arquivar Cálculo" }));
    await waitFor(() => expect(nomes()).toEqual(["Abrir Inglês para reuniões"]));
    await gravacoesPendentes();

    reabrir(tela.unmount);
    // Reaberto com `continuar`: o último aberto foi o Cálculo, que está arquivado mas existe.
    fireEvent.click(await screen.findByRole("button", { name: "← Roadmaps" }));
    await act(async () => { await gravacoesPendentes(); });
    expect(nomes()).toEqual(["Abrir Inglês para reuniões"]);
    fireEvent.click(screen.getByRole("button", { name: /Arquivados \(1\)/ }));
    expect(nomes()).toEqual(["Abrir Cálculo"]);
  });

  it("trilha aberta não liga timer nem poll", async () => {
    await criarEAceitar();
    // Só os cliques e o que eles disparam (o `findBy` do testing-library usa setInterval).
    const intervalo = vi.spyOn(window, "setInterval");
    const espera = vi.spyOn(window, "setTimeout");
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /Rapport/ }));
      fireEvent.click(screen.getByRole("button", { name: "IA" }));
      for (let i = 0; i < 5; i++) await Promise.resolve();
    });
    expect(screen.getByText(/Pergunte qualquer coisa/)).toBeInTheDocument();
    expect(intervalo).not.toHaveBeenCalled();
    // Nenhum setTimeout recorrente nosso (o React/testing-library pode usar um ou outro).
    expect(espera.mock.calls.filter(([, ms]) => typeof ms === "number" && ms >= 1000)).toEqual([]);
    intervalo.mockRestore(); espera.mockRestore();
  });

  it("animação de conclusão: só opacity/transform, nada com movimento desligado ou pausa de energia", () => {
    const no = document.createElement("button");
    const quadros: Keyframe[][] = [];
    no.animate = ((k: Keyframe[]) => { quadros.push(k); return { cancel() {}, onfinish: null, oncancel: null } as unknown as Animation; }) as typeof no.animate;
    const span = HTMLElement.prototype.animate;
    HTMLElement.prototype.animate = no.animate;
    try {
      setMotionLevel("completas");
      expect(animarConclusao(no)).toHaveLength(2);
      for (const k of quadros.flat()) expect(Object.keys(k).every((p) => p === "opacity" || p === "transform" || p === "offset")).toBe(true);
      expect(no.querySelector(".roadmap-anel")).not.toBeNull();
      setMotionLevel("desligadas");
      expect(animarConclusao(no)).toEqual([]);
      setMotionLevel("completas");
      definirPausaDeEnergia(true);
      expect(animarConclusao(no)).toEqual([]);
    } finally {
      definirPausaDeEnergia(false);
      setMotionLevel("completas");
      HTMLElement.prototype.animate = span;
    }
  });
});
