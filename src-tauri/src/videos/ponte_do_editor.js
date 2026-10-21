/* Fork (Editor de vídeo): ponte da edição na mão. Só existe na cópia servida ao preview (o arquivo do
   projeto e o render não têm). Conversa com o app por postMessage: o app nunca toca no DOM daqui. */
(function () {
  if (window.__medzaPonte) return;
  window.__medzaPonte = true;
  var ativo = true;
  var quadro = 0;
  function escapar(v) { return window.CSS && CSS.escape ? CSS.escape(v) : String(v).replace(/["\\]/g, "\\$&"); }
  function alvo(el) {
    while (el && el.nodeType === 1) {
      if (el.hasAttribute("data-composition-id")) return null;
      if (el.hasAttribute("data-medza-o")) return el;
      el = el.parentElement;
    }
    return null;
  }
  function porRef(ref) { return ref ? document.querySelector('[data-medza-o="' + escapar(ref) + '"]') : null; }
  function info(el) {
    if (!el) return null;
    var r = el.getBoundingClientRect();
    var cs = getComputedStyle(el);
    return {
      ref: el.getAttribute("data-medza-o"), id: el.id || null, tag: el.tagName.toLowerCase(),
      rect: { x: r.left, y: r.top, w: r.width, h: r.height },
      texto: el.children.length === 0 ? el.textContent : null,
      fonte: parseFloat(cs.fontSize) || null, peso: cs.fontWeight, cor: cs.color, fundo: cs.backgroundColor,
      translate: el.style.translate || "", scale: el.style.scale || "", rotate: el.style.rotate || "",
      visivel: cs.visibility !== "hidden" && cs.display !== "none" && parseFloat(cs.opacity) > 0.01,
    };
  }
  function enviar(m) { m.medza = "ponte"; try { parent.postMessage(m, "*"); } catch (e) { /* sem app */ } }
  document.addEventListener("pointerdown", function (e) {
    if (!ativo || e.button > 0) return;
    e.preventDefault();
    enviar({ tipo: "selecionar", el: info(alvo(e.target)) });
  }, true);
  document.addEventListener("dblclick", function (e) {
    if (!ativo) return;
    var el = alvo(e.target);
    if (el) enviar({ tipo: "editarTexto", el: info(el) });
  }, true);
  document.addEventListener("pointermove", function (e) {
    if (!ativo || quadro) return;
    var t = e.target;
    quadro = requestAnimationFrame(function () { quadro = 0; enviar({ tipo: "passar", el: info(alvo(t)) }); });
  }, true);
  document.addEventListener("pointerleave", function () { if (ativo) enviar({ tipo: "passar", el: null }); }, true);
  window.addEventListener("message", function (e) {
    if (e.source !== parent) return;
    var d = e.data;
    if (!d || d.medza !== "editor") return;
    if (d.tipo === "ativo") { ativo = !!d.valor; return; }
    var el = porRef(d.ref);
    if (d.tipo === "estilo" && el && d.props) {
      for (var k in d.props) { if (d.props[k] === null || d.props[k] === "") el.style.removeProperty(k); else el.style.setProperty(k, String(d.props[k])); }
    } else if (d.tipo === "texto" && el && el.children.length === 0 && typeof d.texto === "string") {
      el.textContent = d.texto;
    }
    enviar({ tipo: "medida", ref: d.ref || null, el: info(el) });
  });
  enviar({ tipo: "pronta" });
})();
