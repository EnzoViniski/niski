import { realcar } from "../features/revisao/realce";

const monaco = vi.hoisted(() => ({
  colorize: vi.fn().mockResolvedValue("HTML ignorado"),
  tokenize: vi.fn().mockReturnValue([[{ offset: 0, type: "keyword.ts" }, { offset: 5, type: "identifier.ts" }, { offset: 11, type: "number.ts" }]]),
}));
vi.mock("monaco-editor", () => ({ editor: monaco }));

it("carrega o tokenizer existente e preserva texto nos tokens sem depender do tema global", async () => {
  const resultado = await realcar("exemplo.TS", ["const x = 123"]);
  expect(monaco.colorize).toHaveBeenCalledWith("const x = 123", "typescript", { tabSize: 4 });
  expect(resultado[0].map(t => t.texto).join("")).toBe("const x = 123");
  expect(resultado[0].map(t => t.tipo)).toEqual(["keyword", "texto", "number"]);
});

it("usa texto simples para extensões desconhecidas", async () => {
  await realcar("arquivo.desconhecido", ["<script>conteúdo sintético</script>"]);
  expect(monaco.colorize).toHaveBeenLastCalledWith("<script>conteúdo sintético</script>", "plaintext", { tabSize: 4 });
});
