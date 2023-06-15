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
   * 字幕・文字スーパーを表示すべき再生位置。
   */
  pos;
  /**
   * 字幕・文字スーパーのデータ。
   */
  caption;

  constructor(type, options) {
    super(type, options);
    this.pos = options.pos;
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

  find(predicate) {
    return this.#services.find(predicate);
  }

  findIndex(predicate) {
    return this.#services.findIndex(predicate);
  }

  [Symbol.iterator]() {
    return this.#services[Symbol.iterator]();
  }
}

/**
 * 動画を操作するためのオブジェクト。
 */
export const gController = new class Controller extends EventTarget {
  constructor() {
    super();

    window.chrome.webview.addEventListener("message", e => {
      this.#handleNotification(e.data);
    });
  }

  #handleNotification(noti) {
    switch (noti.notification) {
      case "source":
        // ファイルが開かれた、または閉じられた
        this.#source = noti.path;
        this.#lastPos = 0;
        this.#lastPosTime = 0;
        this.#lastTimestamp = null;
        this.#lastTimestampTime = 0;
        this.#duration = NaN;
        this.#services._clear();
        this.#currentServiceId = 0;
        this.#activeVideoTag = null;
        this.#activeAudioTag = null;
        this.#videoWidth = 0;
        this.#videoHeight = 0;
        this.#dualMonoMode = null;
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
        // pause -> playingの場合に一時停止していた時間だけ再生位置が進んでしまうのを防止
        this.#lastPosTime = performance.now();
        this.#lastTimestampTime = performance.now();
        this.dispatchEvent(new PlayerEvent("state"));
        break;

      case "position":
        // 再生位置が更新された
        if (this.#lastTimestamp != null) {
          const diff = noti.position - this.#lastPos;
          this.#lastTimestamp += diff;
          this.#lastTimestampTime = performance.now();
        }
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
        this.#lastTimestamp = this.timestamp?.getTime();
        this.#lastTimestampTime = performance.now();
        this.dispatchEvent(new PlayerEvent("rate"));
        break;

      case "video-size":
        // 映像の解像度が更新された
        if (this.#videoWidth !== noti.width || this.#videoHeight !== noti.height) {
          this.#videoWidth = noti.width;
          this.#videoHeight = noti.height;
          this.dispatchEvent(new PlayerEvent("resize"));
        }
        break;

      case "dual-mono-mode":
        // デュアルモノラルの再生方法が更新された
        this.#dualMonoMode = noti.mode;
        this.dispatchEvent(new PlayerEvent("dual-mono-mode"));
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

      case "switching-started":
        // 現在位置を記録
        this.#lastPos = this.currentTime;
        this.#lastPosTime = performance.now();
        this.#lastTimestamp = this.timestamp?.getTime();
        this.#lastTimestampTime = performance.now();
        this.#isSwitching = true;
        break;

      case "switching-ended":
        this.#isSwitching = false;
        break;

      case "caption":
        // 字幕
        this.dispatchEvent(new CaptionEvent("caption", { pos: noti.pos, caption: noti.caption }));
        break;

      case "superimpose":
        // 文字スーパー
        this.dispatchEvent(new CaptionEvent("superimpose", { pos: noti.pos, caption: noti.caption }));
        break;

      case "timestamp":
        this.#lastTimestamp = noti.timestamp;
        this.#lastTimestampTime = performance.now();
        this.dispatchEvent(new PlayerEvent("timestamp"));
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

  setVideoBounds(left, top, right, bottom) {
    window.chrome.webview.postMessage({
      command: "set-video-bounds",
      left,
      top,
      right,
      bottom,
    });
  }

  openDevTools() {
    // 開発者ツールからgControllerを使えるようにする
    window.gController = gController;
    window.chrome.webview.postMessage({ command: "open-dev-tools" });
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
   * ストリームを切り替え中かどうか。
   */
  #isSwitching = false;

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
   * ホストから通知された再生位置。
   */
  #lastPos = 0;
  /**
   * 再生位置が通知された時刻。
   */
  #lastPosTime = 0;

  /**
   * 秒単位の再生位置。
   */
  get currentTime() {
    if (!this.#isSwitching && this.#state === "playing") {
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

  /**
   * ホストから通知された日付時刻。
   */
  #lastTimestamp = null;
  /**
   * 日付時刻が通知された時刻。
   */
  #lastTimestampTime = 0;

  /**
   * 日付時刻。
   */
  get timestamp() {
    if (this.#lastTimestamp == null) {
      return null;
    }
    if (!this.#isSwitching && this.#state === "playing") {
      return new Date(this.#lastTimestamp + (performance.now() - this.#lastTimestampTime) * this.#playbackRate);
    }
    return new Date(this.#lastTimestamp);
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
    slowest: 0.25,
    fastest: 3.0,
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

  #videoWidth = 0;
  #videoHeight = 0;

  /**
   * 映像の幅。
   */
  get videoWidth() {
    return this.#videoWidth;
  }
  /**
   * 映像の高さ。
   */
  get videoHeight() {
    return this.#videoHeight;
  }

  #dualMonoMode = null;

  /**
   * デュアルモノラルの再生方法。
   */
  get dualMonoMode() {
    return this.#dualMonoMode;
  }

  set dualMonoMode(value) {
    if (!["left", "right", "stereo", "mix"].includes(value)) {
      throw new Error("不正なデュアルモノラルの再生方法");
    }

    window.chrome.webview.postMessage({
      command: "set-dual-mono-mode",
      mode: value,
    });
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
