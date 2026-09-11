const el = (id) => document.getElementById(id);
const names = { 1: "清晰", 2: "明显模糊", 3: "暂不判断" };
let state, index = 0, busy = false, ready = false, generation = 0;
const item = () => state.items[index];
const error = (message = "") => { el("error").textContent = message; el("error").hidden = !message; };

function controls() {
  document.querySelectorAll("button").forEach((button) => {
    button.disabled = busy || !ready || !state;
    if (state && button.dataset.label) {
      button.disabled ||= item().bbox === null && button.dataset.label !== "3";
      button.classList.toggle("active", item().label === Number(button.dataset.label));
    }
  });
}

function zoom() {
  const canvas = el("detail");
  const factor = Number(el("zoom").value) / window.devicePixelRatio;
  canvas.style.width = canvas.width * factor + "px";
  canvas.style.height = canvas.height * factor + "px";
}

async function render() {
  const token = ++generation;
  ready = false;
  controls();
  const current = item();
  el("progress").textContent = `已保存 ${state.completed} / ${state.total}`;
  el("position").textContent = `第 ${index + 1} / ${state.total} 张 · ${current.image_ref}`;
  el("complete").hidden = state.completed !== state.total;
  el("status").textContent = current.label === null ? "等待本张判断；选择后自动保存并进入下一张。" : `本张已保存：${names[current.label]}，可以修改。`;
  el("overview").getContext("2d").clearRect(0, 0, el("overview").width, el("overview").height);
  el("detail").getContext("2d").clearRect(0, 0, el("detail").width, el("detail").height);
  try {
    const image = new Image();
    image.src = "/image/" + encodeURIComponent(current.sample_id);
    await image.decode();
    if (token !== generation) return;
    if (image.naturalWidth !== current.dimensions[0] || image.naturalHeight !== current.dimensions[1]) {
      throw new Error("图片显示尺寸与测量尺寸不一致，请联系 AI，不要继续标注。");
    }
    const overview = el("overview");
    overview.width = image.naturalWidth;
    overview.height = image.naturalHeight;
    const context = overview.getContext("2d");
    context.drawImage(image, 0, 0);
    el("no-face").hidden = current.bbox !== null;
    el("detail").hidden = current.bbox === null;
    if (current.bbox) {
      const [x, y, w, h] = current.bbox;
      context.strokeStyle = "#63ffc2";
      context.lineWidth = Math.max(3, image.naturalWidth / 350);
      context.strokeRect(x, y, w, h);
      const detail = el("detail");
      // Integer crop at native resolution. Only the display zoom changes; no resaved image.
      const left = Math.floor(x), top = Math.floor(y);
      detail.width = Math.ceil(x + w) - left;
      detail.height = Math.ceil(y + h) - top;
      detail.getContext("2d").drawImage(image, left, top, detail.width, detail.height, 0, 0, detail.width, detail.height);
      zoom();
    }
    ready = true;
  } catch (failure) {
    error(failure.message || "图片无法显示，请联系 AI。");
  }
  controls();
}

async function save(label) {
  if (busy || !ready || !state) return;
  busy = true;
  controls();
  error();
  try {
    const response = await fetch(label === null ? "/api/clear" : "/api/label", {
      method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ sample_id: item().sample_id, label }),
    });
    const next = await response.json();
    if (!response.ok) throw new Error(next.error || "保存失败，未进入下一张。");
    state = next;
    if (label !== null) {
      for (let offset = 1; offset <= state.total; offset += 1) {
        const candidate = (index + offset) % state.total;
        if (state.items[candidate].label === null) { index = candidate; break; }
      }
    }
    await render();
  } catch (failure) {
    error(failure.message || "连接中断，请重试；不要关闭页面。");
  } finally {
    busy = false;
    controls();
  }
}

function navigate(delta) {
  if (busy || !ready || !state) return;
  index = (index + delta + state.total) % state.total;
  error();
  render();
}

document.querySelectorAll("[data-label]").forEach((button) => button.addEventListener("click", () => save(Number(button.dataset.label))));
el("previous").addEventListener("click", () => navigate(-1));
el("next").addEventListener("click", () => navigate(1));
el("clear").addEventListener("click", () => save(null));
el("zoom").addEventListener("change", zoom);
window.addEventListener("resize", zoom);
window.addEventListener("keydown", (event) => {
  if (event.repeat || event.metaKey || event.ctrlKey || event.altKey || event.target.tagName === "SELECT") return;
  if (/^[123]$/.test(event.key)) { event.preventDefault(); save(Number(event.key)); }
  if (event.key === "ArrowLeft") { event.preventDefault(); navigate(-1); }
  if (event.key === "ArrowRight") { event.preventDefault(); navigate(1); }
});

fetch("/api/state", { cache: "no-store" }).then(async (response) => {
  if (!response.ok) throw new Error("批次无法载入，请联系 AI。");
  state = await response.json();
  index = Math.max(0, state.items.findIndex((entry) => entry.label === null));
  await render();
}).catch((failure) => error(failure.message));
