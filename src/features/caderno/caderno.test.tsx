import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { montarPedido } from "./ia";
import {
  caderno, cadernos, carregarCadernos, criarCaderno, digitarNaPagina, gravacoesPendentes, gravarPagina, limparCadernos,
  mandarPaginaParaALixeira, moverPagina, novaPagina, usarTransporteDosCadernos, type TransporteDosCadernos,
} from "./loja";
import { CadernoView } from "./CadernoView";
import { lerConversa, normalizarMeta, type MetaDoCaderno } from "./tipos";

/** Disco de mentira: `<id>/<arquivo>` → texto. */
function discoFalso() {
  const arquivos = new Map<string, string>();
  const t: TransporteDosCadernos & { arquivos: Map<string, string> } = {
    arquivos,
    listar: async () => [...arquivos.entries()].filter(([k]) => k.endsWith("/meta.json")).map(([, v]) => JSON.parse(v)),
    ler: async (id, a) => arquivos.get(`${id}/${a}`) ?? null,
    gravar: async (id, a, c) => { arquivos.set(`${id}/${a}`, c); },
    anexar: async (id, l) => { arquivos.set(`${id}/conversa.jsonl`, `${arquivos.get(`${id}/conversa.jsonl`) ?? ""}${l}\n`); },
    lixeira: vi.fn(async (id: string, pagina?: string) => {
      const chave = pagina ? `${id}/paginas/${pagina}.md` : `${id}/meta.json`;
      if (!arquivos.has(chave)) throw new Error("Não encontrei o que apagar.");
      arquivos.delete(chave);
    }),
  };
  return t;
}

const dados = { nome: "O Poder do Hábito", tipo: "livro" as const, autor: "Charles Duhigg", capa: "#2f4b7c", papel: "pautado" as const, letra: "manuscrita" as const, ia: { cli: "claude" as const } };

let disco: ReturnType<typeof discoFalso>;
beforeEach(() => { localStorage.clear(); limparCadernos(); disco = discoFalso(); usarTransporteDosCadernos(disco); });

describe("Caderno: arquivos", () => {
  it("meta estranho vira padrão e id inválido fica de fora", () => {
    expect(normalizarMeta({ id: "../x" })).toBeNull();
    const m = normalizarMeta({ id: "c-1", tipo: "inventado", papel: 3, paginas: [{ id: "p-1", titulo: "A" }, { id: "p-1" }, { id: "Ruim" }] }) as MetaDoCaderno;
    expect(m.tipo).toBe("livre");
    expect(m.papel).toBe("pautado");
    expect(m.paginas.map((p) => p.id)).toEqual(["p-1"]);
    expect(lerConversa('{"de":"eu","texto":"oi"}\nquebrado\n{"de":"x","texto":"y"}')).toHaveLength(1);
  });

  it("cria, escreve, reordena e apaga páginas (a lixeira guarda)", async () => {
    const meta = await criarCaderno(dados);
    expect(disco.arquivos.has(`${meta.id}/meta.json`)).toBe(true);
    const primeira = meta.paginas[0].id;
    digitarNaPagina(meta.id, primeira, "# Hábito\n\nDeixa, rotina, recompensa.");
    await gravarPagina(meta.id, primeira);
    expect(disco.arquivos.get(`${meta.id}/paginas/${primeira}.md`)).toContain("rotina");
    const segunda = await novaPagina(meta.id, "Capítulo 2");
    await moverPagina(meta.id, segunda!.id, primeira);
    expect(caderno(meta.id)?.paginas.map((p) => p.id)).toEqual([segunda!.id, primeira]);
    await mandarPaginaParaALixeira(meta.id, primeira);
    expect(disco.lixeira).toHaveBeenCalledWith(meta.id, primeira);
    expect(caderno(meta.id)?.paginas.map((p) => p.id)).toEqual([segunda!.id]);
    // Reabrir o app: lê do disco.
    await gravacoesPendentes();
    limparCadernos();
    await carregarCadernos();
    expect(cadernos()?.[0].paginas.map((p) => p.titulo)).toEqual(["Capítulo 2"]);
  });

  it("o pedido à IA leva o caderno, a página aberta e a conversa", async () => {
    const meta = await criarCaderno(dados);
    const p = meta.paginas[0].id;
    const pedido = montarPedido(meta, p, { [p]: "Hábitos são laços de deixa e recompensa." }, [{ de: "eu", texto: "oi", em: "" }], "Resuma");
    expect(pedido).toContain("O Poder do Hábito");
    expect(pedido).toContain("Charles Duhigg");
    expect(pedido).toContain("laços de deixa");
    expect(pedido).toContain("Pessoa: oi");
    expect(pedido.trim().endsWith("Resuma")).toBe(true);
    const longo = montarPedido(meta, p, { [p]: "x".repeat(50_000) }, [], "?");
    expect(longo).toContain("cortado");
  });
});

describe("Caderno: tela", () => {
  it("cria um caderno de livro, escreve na folha e grava sozinho", async () => {
    render(<CadernoView continuar={false} />);
    await userEvent.click(await screen.findByRole("button", { name: "Crie o primeiro" }));
    await userEvent.type(screen.getByLabelText("Nome do livro"), "Confissões");
    await userEvent.click(screen.getByRole("button", { name: "Criar caderno" }));
    const texto = await screen.findByLabelText("Texto da página");
    await userEvent.type(texto, "Tarde te amei");
    await waitFor(() => expect([...disco.arquivos.entries()].some(([k, v]) => k.includes("/paginas/") && v === "Tarde te amei")).toBe(true), { timeout: 2000 });
    expect(screen.getByLabelText("Título da página")).toHaveValue("Primeiras impressões");
    expect(screen.getByText("IA do caderno")).toBeInTheDocument();
  });
});
