// Fork (energia): o reset de variáveis do Tailwind não alcança as linhas do terminal e do editor.
import { describe, expect, it } from "vitest";
import postcss from "postcss";
import { variaveisForaDoTerminal } from "../../scripts/postcss/variaveisForaDoTerminal";

async function processar(css: string) {
  return (await postcss([variaveisForaDoTerminal()]).process(css, { from: undefined })).css;
}

describe("variáveis do Tailwind fora do terminal", () => {
  it("reescreve só o reset universal de --tw-*, com especificidade zero", async () => {
    const saida = await processar("*, ::before, ::after { --tw-rotate: 0; --tw-shadow: 0 0 #0000 }\n*, ::before, ::after { box-sizing: border-box }\n.rotate-3 { --tw-rotate: 3deg }");
    expect(saida).toContain(":where(*:not(.xterm-rows *, .view-lines *)), :where(*:not(.xterm-rows *, .view-lines *))::before, :where(*:not(.xterm-rows *, .view-lines *))::after { --tw-rotate: 0");
    // Preflight (box-sizing) e utilitários continuam iguais.
    expect(saida).toContain("*, ::before, ::after { box-sizing: border-box }");
    expect(saida).toContain(".rotate-3 { --tw-rotate: 3deg }");
  });

  it("aceita a forma de um dois-pontos que o minificador gera", async () => {
    const saida = await processar("*,:before,:after{--tw-blur: }");
    expect(saida).toContain(":where(*:not(.xterm-rows *, .view-lines *))::before");
  });
});
