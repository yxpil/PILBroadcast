import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import QRCode from "qrcode";
import "./style.css";

// ── Window controls ──
const win = getCurrentWindow();
document.querySelector(".titlebar")?.addEventListener("mousedown", (e) => {
  if ((e.target as HTMLElement).closest(".window-controls, .win-btn")) return;
  win.startDragging();
});
document.getElementById("btn-minimize")?.addEventListener("click", () => win.hide());
document.getElementById("btn-maximize")?.addEventListener("click", async () => {
  (await win.isMaximized()) ? win.unmaximize() : win.maximize();
});
document.getElementById("btn-close")?.addEventListener("click", () => win.hide());

// ── DOM refs ──
const btnToggleServer = document.getElementById("btn-toggle-server")!;
const serverStatus = document.getElementById("server-status")!;
const captureStatus = document.getElementById("capture-status")!;
const urlDisplay = document.getElementById("url-display")!;
const qrCanvas = document.getElementById("qr-canvas") as HTMLCanvasElement;
const btnToggleLog = document.getElementById("btn-toggle-log")!;
const logPanel = document.getElementById("log-panel")!;
const logEntries = document.getElementById("log-entries")!;
const btnSettings = document.getElementById("btn-settings")!;
const settingsPopup = document.getElementById("settings-popup")!;
const inputPassword = document.getElementById("input-password") as HTMLInputElement;
const btnSetPwd = document.getElementById("btn-set-pwd")!;
const sliderQuality = document.getElementById("slider-quality") as HTMLInputElement;
const sliderFps = document.getElementById("slider-fps") as HTMLInputElement;
const qualityVal = document.getElementById("quality-val")!;
const fpsVal = document.getElementById("fps-val")!;
const previewWrapper = document.getElementById("preview-wrapper")!;
const previewImg = document.getElementById("preview-img") as HTMLImageElement;
const previewWaiting = document.getElementById("preview-waiting")!;

// ── State ──
let serverRunning = false;
let captureRunning = false;
let pollSeq = 0;
let pollTimer: ReturnType<typeof setTimeout> | null = null;

// ── Settings gear ──
let settingsOpen = false;
btnSettings.addEventListener("click", () => {
  settingsOpen = !settingsOpen;
  settingsPopup.style.display = settingsOpen ? "flex" : "none";
});

btnSetPwd.addEventListener("click", async () => {
  try { await invoke("set_password", { password: inputPassword.value.trim() }); } catch (e) { console.error(e); }
});

// ── Quality slider ──
sliderQuality.addEventListener("input", () => { qualityVal.textContent = sliderQuality.value + "%"; });
sliderQuality.addEventListener("change", async () => {
  try { await invoke("set_quality", { quality: parseInt(sliderQuality.value) }); } catch (e) { console.error(e); }
});

// ── FPS slider ──
sliderFps.addEventListener("input", () => { fpsVal.textContent = sliderFps.value + " FPS"; });
sliderFps.addEventListener("change", async () => {
  try { await invoke("set_fps", { fps: parseInt(sliderFps.value) }); } catch (e) { console.error(e); }
  if (captureRunning) { stopPolling(); startPolling(); }
});

// ── Toggle server (auto-manages capture) ──
btnToggleServer.addEventListener("click", async () => {
  try {
    if (serverRunning) {
      stopPolling();
      await invoke("stop_server");
      serverRunning = false;
      captureRunning = false;
      updateCaptureUI();
    } else {
      await invoke("start_server");
      serverRunning = true;
      captureRunning = true;
      updateCaptureUI();
      startPolling();
    }
    updateServerUI();
  } catch (e) { console.error(e); }
});

// ── Frame polling ──
function startPolling() {
  stopPolling();
  pollSeq = 0;

  previewWrapper.style.display = "flex";
  previewWaiting.style.display = "block";

  const fps = parseInt(sliderFps.value) || 30;
  const POLL_INTERVAL = Math.max(10, Math.floor(1000 / (fps * 2)));

  async function poll() {
    try {
      const result = await invoke<{ seq: number; data: string } | null>("poll_frame", { seq: pollSeq });
      if (result && result.data) {
        pollSeq = result.seq;
        const old = previewImg.src;
        previewImg.src = `data:image/jpeg;base64,${result.data}`;
        if (previewWaiting.style.display !== "none") {
          previewWaiting.style.display = "none";
        }
        if (old && old.startsWith("blob:")) URL.revokeObjectURL(old);
      }
    } catch (_) {}
    pollTimer = setTimeout(poll, POLL_INTERVAL);
  }

  poll();
}

function stopPolling() {
  if (pollTimer !== null) {
    clearTimeout(pollTimer);
    pollTimer = null;
  }
  previewWrapper.style.display = "none";
  previewImg.src = "";
  previewWaiting.style.display = "block";
}

function updateServerUI() {
  if (serverRunning) {
    btnToggleServer.textContent = "停止服务器";
    btnToggleServer.classList.add("running");
    serverStatus.textContent = "● 运行中 :8726";
    serverStatus.className = "status-on";
  } else {
    btnToggleServer.textContent = "启动服务器";
    btnToggleServer.classList.remove("running");
    serverStatus.textContent = "● 未启动";
    serverStatus.className = "status-off";
  }
}

function updateCaptureUI() {
  if (captureRunning) {
    captureStatus.textContent = "◉ 直播中";
    captureStatus.className = "status-capture-on";
  } else {
    captureStatus.textContent = "◉ 未直播";
    captureStatus.className = "status-capture-off";
  }
}

// ── QR code ──
async function updateQR() {
  try {
    const ip = await invoke<string>("get_local_ip");
    const url = `http://${ip}:8726`;
    urlDisplay.textContent = url;
    await QRCode.toCanvas(qrCanvas, url, { width: 200, margin: 2, color: { dark: "#1b4332", light: "#ffffff" } });
  } catch {
    urlDisplay.textContent = "http://0.0.0.0:8726";
    await QRCode.toCanvas(qrCanvas, "http://0.0.0.0:8726", { width: 200, margin: 2, color: { dark: "#1b4332", light: "#ffffff" } });
  }
}

// ── Log toggle ──
let logVisible = false;
btnToggleLog.addEventListener("click", async () => {
  logVisible = !logVisible;
  if (logVisible) {
    btnToggleLog.textContent = "隐藏日志";
    logPanel.style.display = "block";
    const logs = await invoke<string[]>("get_logs");
    logEntries.textContent = logs.join("\n") || "暂无日志";
  } else {
    btnToggleLog.textContent = "显示日志";
    logPanel.style.display = "none";
  }
});

// ── Init ──
updateQR();
(async () => {
  try {
    serverRunning = await invoke<boolean>("is_server_running");
    updateServerUI();

    captureRunning = await invoke<boolean>("is_capture_running");
    updateCaptureUI();

    const pwd = await invoke<string | null>("get_password");
    if (pwd) inputPassword.value = pwd;

    const q = await invoke<number>("get_quality");
    sliderQuality.value = String(q);
    qualityVal.textContent = q + "%";

    const f = await invoke<number>("get_fps");
    sliderFps.value = String(f);
    fpsVal.textContent = f + " FPS";

    if (captureRunning) startPolling();
  } catch (_) {}
})();
