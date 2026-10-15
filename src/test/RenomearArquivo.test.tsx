import { selecionarNome } from "../components/FileTree";

function campo(valor: string) {
  const input = document.createElement("input");
  input.value = valor;
  document.body.appendChild(input);
  return input;
}

it("renomear arquivo pré-seleciona só o nome, sem a extensão", () => {
  const md = campo("relatorio.final.md");
  selecionarNome(md, false);
  expect([md.selectionStart, md.selectionEnd]).toEqual([0, "relatorio.final".length]);

  const oculto = campo(".gitignore");
  selecionarNome(oculto, false);
  expect([oculto.selectionStart, oculto.selectionEnd]).toEqual([0, ".gitignore".length]);

  const pasta = campo("versao.1");
  selecionarNome(pasta, true);
  expect([pasta.selectionStart, pasta.selectionEnd]).toEqual([0, "versao.1".length]);
});
