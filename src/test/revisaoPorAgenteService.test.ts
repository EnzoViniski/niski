import { usarTransporte } from "../mobile/api";
import { listTerminalSessions, type TerminalSession } from "../features/terminal/terminalService";
import { revisarComAgente, mandarAchados } from "../features/revisao/revisaoPorAgenteService";

vi.mock("../features/terminal/terminalService",() => ({ listTerminalSessions:vi.fn() }));
const transporte = vi.fn();
beforeEach(() => {
  vi.clearAllMocks(); usarTransporte(transporte);
  vi.mocked(listTerminalSessions).mockResolvedValue([{id:"s",conversation_id:"origem/a"} as TerminalSession]);
});
afterEach(() => usarTransporte((p,i) => fetch(p,i)));
it("usa o id da conversa de origem e não aceita pasta no pedido", async () => {
  transporte.mockResolvedValue(new Response(JSON.stringify({id:"nova",session_id:"n",name:"Revisão"}),{status:201}));
  await revisarComAgente("s",{provider:"codex",escopo:"branch",base:"main"},"k");
  expect(transporte).toHaveBeenCalledWith("/conversas/origem%2Fa/revisao",expect.objectContaining({method:"POST",headers:expect.objectContaining({"Idempotency-Key":"k"}),body:JSON.stringify({provider:"codex",escopo:"branch",base:"main"})}));
});
it("sem origem não envia o pedido", async () => {
  vi.mocked(listTerminalSessions).mockResolvedValue([]);
  await expect(revisarComAgente("s",{provider:"claude",escopo:"local"})).rejects.toThrow("origem");
  expect(transporte).not.toHaveBeenCalled();
});
it("manda só falas do revisor usando revisão e fila da origem", async () => {
  transporte.mockImplementation(async (p:string) => new Response(JSON.stringify(p.includes("filha") ? {timeline:{messages:[{role:"assistant",kind:"text",text:"Corrigir validação"},{role:"assistant",kind:"tool",text:"segredo de ferramenta"},{role:"user",kind:"text",text:"pedido"}]}} : p.endsWith("/timeline") ? {conversation:{capabilities:{prompt:false,enfileirar:true,revision:"r"}}} : {id:"acao"})));
  await mandarAchados("s",{id:"filha",session_id:"n",name:"Revisão",origem:"origem/a"},"achados");
  const envio = transporte.mock.calls.at(-1)!;
  expect(envio[0]).toBe("/conversas/origem%2Fa/prompt");
  expect(envio[1].headers).toMatchObject({"If-Match":"r","Idempotency-Key":"achados"});
  expect(JSON.parse(envio[1].body)).toMatchObject({fila:true,texto:expect.stringContaining("Corrigir validação")});
  expect(envio[1].body).not.toContain("segredo");
});
it("não envia quando ainda não há achados", async () => {
  transporte.mockResolvedValue(new Response(JSON.stringify({timeline:{messages:[]}})));
  await expect(mandarAchados("s",{id:"filha",session_id:"n",name:"Revisão",origem:"origem/a"},"k")).rejects.toThrow("ainda não");
  expect(transporte).toHaveBeenCalledTimes(1);
});
