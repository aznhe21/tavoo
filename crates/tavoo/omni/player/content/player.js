const gPlayer = new class Player {
  constructor() {
    window.chrome.webview.addEventListener("message", e => {
      this.#handleNotification(e.data);
    });
  }

  #handleNotification(noti) {
    switch (noti.notification) {
      case "source":
        // ファイルが開かれた、または閉じられた
        console.log(`ファイル：${noti.path}`);
        this.#source = noti.path;
        break;

      case "rate-range":
        // 再生速度の範囲
        console.log(`速度範囲：${noti.slowest}..=${noti.fastest}`);
        this.#playbackRateRange.slowest = noti.slowest;
        this.#playbackRateRange.fastest = noti.fastest;
        break;

      case "state":
        // 再生状態が更新された
        console.log(`再生状態：${noti.state}`);
        this.#state = noti.state;
        break;

      case "position":
        // 再生位置が更新された
        console.log(`再生位置：${noti.position}`);
        this.#lastPos = noti.position;
        this.#lastPosTime = performance.now();
        break;

      case "rate":
        // 再生速度が更新された
        console.log(`再生速度：${noti.rate}`);
        this.#playbackRate = noti.rate;
        break;

      case "services":
        // 全サービスが更新された
        console.log("全サービス更新", noti.services);
        this.services = noti.services;
        break;

      case "service": {
        // 特定のサービスが更新された
        console.log("サービス更新", noti.service);

        const idx = this.services.findIndex(svc => svc.service_id == noti.service.service_id);
        if (idx >= 0) {
          this.services[idx] = noti.service;
        }
        break;
      }

      case "event": {
        // サービスのイベント情報が更新された
        console.log(`イベント（${noti.service_id}、${noti.is_present}）`, noti.event);

        const idx = this.services.findIndex(svc => svc.service_id == noti.service_id);
        if (idx >= 0) {
          if (noti.is_present) {
            this.services[idx].present_event = noti.event;
          } else {
            this.services[idx].following_event = noti.event;
          }
        }
        break;
      }

      case "service-changed":
        // サービスが選択し直された
        console.log(`新サービスID：${noti.new_service_id}`);
        break;

      case "caption":
        // 字幕
        console.log("字幕", noti.caption);
        break;

      case "superimpose":
        // 文字スーパー
        console.log("文字スーパー", noti.superimpose);
        break;

      case "error":
        // エラーが発生した
        alert(noti.message);
        break;

      default:
        console.error(`不明な通知：${noti.notification}`);
        break;
    }
  }

  /**
   * サービスの一覧。
   */
  services = [];

  /**
   * 現在の再生状態。
   *
   * 有効な値："open-pending", "playing", "paused", "stopped", "closed"
   */
  #state = "closed";

  /**
   * 現在開かれているファイルのパス。
   */
  #source = null;

  /**
   * 現在開かれているファイルのパス。
   */
  get source() {
    return this.#source;
  }

  /**
   * ホストから通知された再生位置
   */
  #lastPos = 0;
  /**
   * 再生位置が通知された時刻
   */
  #lastPosTime = performance.now();

  /**
   * 秒単位の再生位置。
   */
  get currentTime() {
    if (this.#state === "playing") {
      return this.#lastPos + (performance.now() - this.#lastPosTime) / 1000;
    }
    return this.#lastPos;
  }

  set currentTime(value) {
    window.chrome.webview.postMessage({
      command: "set-position",
      position: value,
    });
  }

  #volume = 1.0;

  /**
   * 音量。
   */
  get volume() {
    return this.#volume;
  }

  set volume(value) {
    this.#volume = value;
    window.chrome.webview.postMessage({
      command: "set-volume",
      volume: value,
    });
  }

  #muted = false;

  /**
   * ミュート状態。
   */
  get muted() {
    return this.#muted;
  }

  set muted(value) {
    this.#muted = value;
    window.chrome.webview.postMessage({
      command: "set-muted",
      muted: value,
    });
  }

  #playbackRate = 1.0;
  #playbackRateRange = {
    slowest: 1.0,
    fastest: 1.0,
  }

  /**
   * 再生速度。
   */
  get playbackRate() {
    return this.#playbackRate;
  }

  set playbackRate(value) {
    if (value < this.#playbackRateRange.slowest || value > this.#playbackRateRange.fastest) {
      throw new Error("再生速度の範囲外");
    }

    window.chrome.webview.postMessage({
      command: "set-rate",
      rate: value,
    });
  }

  /**
   * 再生速度の範囲。
   */
  get playbackRateRange() {
    return {
      slowest: this.#playbackRateRange.slowest,
      fastest: this.#playbackRateRange.fastest,
    };
  }

  /**
   * 再生を開始する。
   */
  play() {
    window.chrome.webview.postMessage({ command: "play" });
  }

  /**
   * 再生を一時停止する。
   */
  pause() {
    window.chrome.webview.postMessage({ command: "pause" });
  }

  /**
   * 再生を停止する。
   */
  stop() {
    window.chrome.webview.postMessage({ command: "stop" });
  }

  /**
   * ファイルを閉じる。
   */
  close() {
    window.chrome.webview.postMessage({ command: "close" });
  }

  /**
   * サービスを選択する。
   */
  selectService(serviceId) {
    window.chrome.webview.postMessage({
      command: "select-service",
      service_id: serviceId,
    });
  }

  /**
   * 映像ストリームを選択する。
   */
  selectVideoStream(componentTag) {
    window.chrome.webview.postMessage({
      command: "select-video-stream",
      component_tag: componentTag,
    });
  }

  /**
   * 音声ストリームを選択する。
   */
  selectAudioStream(componentTag) {
    window.chrome.webview.postMessage({
      command: "select-audio-stream",
      component_tag: componentTag,
    });
  }
};
window.gPlayer = gPlayer;

document.body.addEventListener("keydown", e => {
  // keyは押されたキー全部を表す文字列であり、制御キーはアルファベット順で付く
  // 例："a"、"C-a"、"C-Shift"、"A-C-M-S-a"
  let key = e.key;
  if (e.shiftKey && e.key.length === 1) {
    key = e.key.toLowerCase();
  }
  if (e.shiftKey && e.key !== "Shift") {
    key = "S-" + key;
  }
  if (e.metaKey && e.key !== "Meta") {
    key = "M-" + key;
  }
  if (e.ctrlKey && e.key !== "Control") {
    key = "C-" + key;
  }
  if (e.altKey && e.key !== "Alt") {
    key = "A-" + key;
  }

  switch (key) {
    case "F3":
    case "F5":
    case "F7":
    case "C-r":
    case "C-F5":
    case "BrowserRefresh":
      e.preventDefault();
      break;

    case "F12":
      e.preventDefault();
      // TODO: そのうちメニューか何かに移す
      window.chrome.webview.postMessage({ command: "open-dev-tools" });
      break;

    default:
      if (e.target !== document.body) {
        return;
      }

      // TODO: ショートカットキーとして処理
      console.log(key, e);
      break;
  }
});
