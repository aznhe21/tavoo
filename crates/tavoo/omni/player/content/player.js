import { Skin as SkinDefault } from "tavoo://player/content/skin-default.js";

/**
 * プレイヤーで発生するイベント。
 */
export class PlayerEvent extends Event { }

/**
 * サービスが更新された際に発生するイベント。
 */
export class ServiceEvent extends PlayerEvent {
  /**
   * 更新されたサービスのサービス識別。
   */
  serviceId;

  constructor(type, options) {
    super(type, options);
    this.serviceId = options.serviceId;
  }
}

/**
 * イベントが更新された際に発生するイベント。
 */
export class EventEvent extends PlayerEvent {
  /**
   * 更新されたイベントが属するサービスのサービス識別。
   */
  serviceId;

  /**
   * 更新されたイベントが現在のもの（`true`）か次のもの（`false`）かを示す。
   */
  isPresent;

  constructor(type, options) {
    super(type, options);
    this.serviceId = options.serviceId;
    this.isPresent = options.isPresent;
  }
}

/**
 * 字幕・文字スーパーを受信した際に発生するイベント。
 */
export class CaptionEvent extends PlayerEvent {
  /**
   * 字幕・文字スーパーのデータ。
   */
  caption;

  constructor(type, options) {
    super(type, options);
    this.caption = options.caption;
  }
}

/**
 * 全サービスを保持するクラス。
 *
 * `get`により添え字で、また`getById`によりサービス識別でサービスを取得することができる。
 */
export class Services {
  #services = [];
  #indices = {};

  _clear() {
    this.#services = [];
    this.#indices = {};
  }

  _update(services) {
    this.#services = services;

    const indices = {};
    for (let i = 0; i < services.length; i++) {
      indices[services[i].service_id] = i;
    }
    this.#indices = indices;
  }

  _updateOne(service) {
    const index = this.#indices[service.service_id];
    if (index === undefined) {
      return;
    }

    this.#services[index] = service;
  }

  /**
   * 添え字によりサービスを取得する。
   */
  get(index) {
    return this.#services[index];
  }

  /**
   * サービス識別によりサービスを取得する。
   */
  getById(id) {
    const index = this.#indices[id];
    return index !== undefined ? this.#services[index] : undefined;
  }

  /**
   * サービスの個数。
   */
  get length() {
    return this.#services.length;
  }

  [Symbol.iterator]() {
    return this.#services[Symbol.iterator]();
  }
}

function openDevTools() {
  window.chrome.webview.postMessage({ command: "open-dev-tools" });
}

/**
 * 動画を操作、また映像用領域を指示するためのカスタム要素。
 *
 * 複数の`tavoo-player`をHTMLに配置した際の挙動は未定義である。
 */
export class Player extends HTMLElement {
  static register() {
    customElements.define("tavoo-player", Player);
  }

  #resizeObserver;

  constructor() {
    super();

    window.chrome.webview.addEventListener("message", e => {
      this.#handleNotification(e.data);
    });

    this.#resizeObserver = new ResizeObserver(() => {
      this.#onResized();
    });
  }

  #handleNotification(noti) {
    switch (noti.notification) {
      case "source":
        // ファイルが開かれた、または閉じられた
        this.#source = noti.path;
        this.#lastPos = 0;
        this.#lastPosTime = 0;
        this.#duration = NaN;
        this.#services._clear();
        this.#currentServiceId = 0;
        this.#activeVideoTag = null;
        this.#activeAudioTag = null;
        this.dispatchEvent(new PlayerEvent("source"));
        break;

      case "volume":
        this.#volume = noti.volume;
        this.#muted = noti.muted;
        this.dispatchEvent(new PlayerEvent("volume"));
        break;

      case "rate-range":
        // 再生速度の範囲
        this.#playbackRateRange.slowest = noti.slowest;
        this.#playbackRateRange.fastest = noti.fastest;
        this.dispatchEvent(new PlayerEvent("rate-range"));
        break;

      case "duration":
        // 動画の長さ
        this.#duration = noti.duration ?? +Infinity;
        this.dispatchEvent(new PlayerEvent("duration"));
        break;

      case "state":
        // 再生状態が更新された
        this.#state = noti.state;
        this.dispatchEvent(new PlayerEvent("state"));
        break;

      case "position":
        // 再生位置が更新された
        this.#lastPos = noti.position;
        this.#lastPosTime = performance.now();
        this.dispatchEvent(new PlayerEvent("position"));
        break;

      case "seek-completed":
        this.dispatchEvent(new PlayerEvent("seek-completed"));
        break;

      case "rate":
        // 再生速度が更新された
        this.#playbackRate = noti.rate;
        this.#lastPos = this.currentTime;
        this.#lastPosTime = performance.now();
        this.dispatchEvent(new PlayerEvent("rate"));
        break;

      case "services":
        // 全サービスが更新された
        this.#services._update(noti.services);
        this.dispatchEvent(new PlayerEvent("services"));
        break;

      case "service": {
        // 特定のサービスが更新された
        this.#services._updateOne(noti.service);
        this.dispatchEvent(new ServiceEvent("service", { serviceId: noti.service.service_id }));
        break;
      }

      case "event": {
        // サービスのイベント情報が更新された
        const service = this.#services.getById(noti.service_id);
        if (service) {
          if (noti.is_present) {
            service.present_event = noti.event;
          } else {
            service.following_event = noti.event;
          }
          this.dispatchEvent(new EventEvent("event", { serviceId: noti.service_id, isPresent: noti.is_present }));
        }
        break;
      }

      case "service-changed":
        // サービスが選択し直された
        this.#currentServiceId = noti.new_service_id;
        this.#activeVideoTag = noti.video_component_tag;
        this.#activeAudioTag = noti.audio_component_tag;
        this.dispatchEvent(new PlayerEvent("service-changed"));
        break;

      case "stream-changed":
        this.#activeVideoTag = noti.video_component_tag;
        this.#activeAudioTag = noti.audio_component_tag;
        this.dispatchEvent(new PlayerEvent("stream-changed"));
        break;

      case "caption":
        // 字幕
        this.dispatchEvent(new CaptionEvent("caption", { caption: noti.caption }));
        break;

      case "superimpose":
        // 文字スーパー
        this.dispatchEvent(new CaptionEvent("superimpose", { caption: noti.caption }));
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

  connectedCallback() {
    this.#resizeObserver.observe(this);
  }

  disconnectedCallback() {
    this.#resizeObserver.unobserve(this);
  }

  #onResized() {
    const { offsetWidth, offsetHeight } = document.body;

    window.chrome.webview.postMessage({
      command: "set-video-bounds",
      left: this.offsetLeft / offsetWidth,
      top: this.offsetTop / offsetHeight,
      right: (this.offsetLeft + this.offsetWidth) / offsetHeight,
      bottom: (this.offsetTop + this.offsetHeight) / offsetHeight,
    });
  }

  openDevTools() {
    openDevTools();
  }

  /**
   * サービスの一覧。
   */
  #services = new Services();

  /**
   * サービスの一覧。
   */
  get services() {
    return this.#services;
  }

  /**
   * 現在選択されているサービスのサービス識別。
   *
   * `0`では未選択。
   */
  #currentServiceId = 0;

  /**
   * 現在選択されているサービスのサービス識別。
   *
   * `0`では未選択。
   */
  get currentServiceId() {
    return this.#currentServiceId;
  }

  /**
   * 現在選択されているサービス。
   */
  get currentService() {
    return this.#currentServiceId !== 0 ? this.#services.getById(this.#currentServiceId) : null;
  }

  #activeVideoTag = null;

  /**
   * アクティブな映像コンポーネントのタグ。
   *
   * TSを開いていない状態、または映像ストリームにコンポーネントタグがない場合には`null`となる。
   */
  get activeVideoTag() {
    return this.#activeVideoTag;
  }

  #activeAudioTag = null;

  /**
   * アクティブな音声コンポーネントのタグ。
   *
   * TSを開いていない状態、または音声ストリームにコンポーネントタグがない場合には`null`となる。
   */
  get activeAudioTag() {
    return this.#activeAudioTag;
  }

  /**
   * 現在の再生状態。
   *
   * 有効な値："open-pending", "playing", "paused", "stopped", "closed"
   */
  #state = "closed";

  /**
   * 現在の再生状態。
   *
   * 有効な値："open-pending", "playing", "paused", "stopped", "closed"
   */
  get state() {
    return this.#state;
  }

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
   * 動画の長さ。
   */
  #duration = NaN;

  /**
   * 動画の秒単位での長さ。
   *
   * 再生していない状態では`NaN`、リアルタイム視聴などで長さが不明な場合は`+Infinity`となる。
   */
  get duration() {
    return this.#duration;
  }

  /**
   * ホストから通知された再生位置
   */
  #lastPos = 0;
  /**
   * 再生位置が通知された時刻
   */
  #lastPosTime = 0;

  /**
   * 秒単位の再生位置。
   */
  get currentTime() {
    if (this.#state === "playing") {
      return this.#lastPos + (performance.now() - this.#lastPosTime) / 1000 * this.#playbackRate;
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

function handleKeyDown(e) {
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
      openDevTools();
      break;

    default:
      if (e.target !== document.body) {
        return;
      }

      // TODO: ショートカットキーとして処理
      console.log(key, e);
      break;
  }
}

export function startup() {
  Player.register();

  document.body.addEventListener("keydown", handleKeyDown);

  customElements.define("skin-default", SkinDefault);
  document.body.replaceChildren(document.createElement("skin-default"));
}
