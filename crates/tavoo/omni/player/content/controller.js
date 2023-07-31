// @ts-check

/**
 * @typedef {import("./message.d.ts").Caption} Caption
 * @typedef {import("./message.d.ts").Command} Command
 * @typedef {import("./message.d.ts").DualMonoMode} DualMonoMode
 * @typedef {import("./message.d.ts").Notification} Notification
 * @typedef {import("./message.d.ts").Service} Service
 */

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
   *
   * @type {number}
   */
  serviceId;

  /**
   * @param {string} type
   * @param {EventInit & { serviceId: number }} options
   */
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
   *
   * @type {number}
   */
  serviceId;

  /**
   * 更新されたイベントが現在のもの（`true`）か次のもの（`false`）かを示す。
   *
   * @type {boolean}
   */
  isPresent;

  /**
   * @param {string} type
   * @param {EventInit & { serviceId: number; isPresent: boolean }} options
   */
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
   *
   * @type {number}
   */
  pos;

  /**
   * 字幕・文字スーパーのデータ。
   *
   * @type {Caption}
   */
  caption;

  /**
   * @param {string} type
   * @param {EventInit & { pos: number; caption: Caption }} options
   */
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
  /** @type {Service[]} */
  #services = [];

  /** @type {Record<number, number | undefined>} */
  #indices = {};

  _clear() {
    this.#services = [];
    this.#indices = {};
  }

  /** @param {Service[]} services */
  _update(services) {
    this.#services = services;

    /** @type {Record<number, number | undefined>} */
    const indices = {};
    for (let i = 0; i < services.length; i++) {
      indices[services[i].serviceId] = i;
    }
    this.#indices = indices;
  }

  /** @param {Service} service */
  _updateOne(service) {
    const index = this.#indices[service.serviceId];
    if (index === undefined) {
      return;
    }

    this.#services[index] = service;
  }

  /**
   * 添え字によりサービスを取得する。
   *
   * @param {number} index
   * @returns {Service | undefined}
   */
  get(index) {
    return this.#services[index];
  }

  /**
   * サービス識別によりサービスを取得する。
   *
   * @param {number} id
   * @returns {Service | undefined}
   */
  getById(id) {
    const index = this.#indices[id];
    return index !== undefined ? this.#services[index] : undefined;
  }

  /**
   * サービスの個数。
   *
   * @type {number}
   */
  get length() {
    return this.#services.length;
  }

  /**
   * @param {(service: Service, index: number) => boolean} predicate
   *
   * @returns {Service | undefined}
   */
  find(predicate) {
    return this.#services.find(predicate);
  }

  /**
   * @param {(service: Service, index: number) => boolean} predicate
   *
   * @returns {number}
   */
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

    // @ts-ignore: WebView2専用API
    window.chrome.webview.addEventListener("message", e => {
      this.#handleNotification(e.data);
    });
  }

  /** @param {Command} command */
  #postCommand(command) {
    // @ts-ignore: WebView2専用API
    window.chrome.webview.postMessage(command);
  }

  /** @param {Notification} noti */
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
        this.#lastTimestamp = this.timestamp?.getTime() ?? null;
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

      case "audio-channels":
        // 音声のチャンネル数が更新された
        this.#audioChannels = noti.numChannels;
        this.dispatchEvent(new PlayerEvent("audio-channels"));
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
        this.dispatchEvent(new ServiceEvent("service", { serviceId: noti.service.serviceId }));
        break;
      }

      case "event": {
        // サービスのイベント情報が更新された
        const service = this.#services.getById(noti.serviceId);
        if (service) {
          if (noti.isPresent) {
            service.presentEvent = noti.event;
          } else {
            service.followingEvent = noti.event;
          }
          this.dispatchEvent(new EventEvent("event", { serviceId: noti.serviceId, isPresent: noti.isPresent }));
        }
        break;
      }

      case "service-changed":
        // サービスが選択し直された
        this.#currentServiceId = noti.newServiceId;
        this.#activeVideoTag = noti.videoComponentTag;
        this.#activeAudioTag = noti.audioComponentTag;
        this.dispatchEvent(new PlayerEvent("service-changed"));
        break;

      case "stream-changed":
        this.#activeVideoTag = noti.videoComponentTag;
        this.#activeAudioTag = noti.audioComponentTag;
        this.dispatchEvent(new PlayerEvent("stream-changed"));
        break;

      case "switching-started":
        // 現在位置を記録
        this.#lastPos = this.currentTime;
        this.#lastPosTime = performance.now();
        this.#lastTimestamp = this.timestamp?.getTime() ?? null;
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
        // @ts-ignore: 型定義上はあり得ない
        console.error(`不明な通知：${noti.notification}`);
        break;
    }
  }

  /**
   * @param {number} left
   * @param {number} top
   * @param {number} right
   * @param {number} bottom
   */
  setVideoBounds(left, top, right, bottom) {
    this.#postCommand({
      command: "set-video-bounds",
      left,
      top,
      right,
      bottom,
    });
  }

  openDevTools() {
    // 開発者ツールからgControllerを使えるようにする
    // @ts-ignore: 型定義に無いがデバッグ用に無視
    window.gController = gController;
    this.#postCommand({ command: "open-dev-tools" });
  }

  /**
   * サービスの一覧。
   *
   * @type {Services}
   */
  #services = new Services();

  /**
   * サービスの一覧。
   *
   * @type {Services}
   */
  get services() {
    return this.#services;
  }

  /**
   * 現在選択されているサービスのサービス識別。
   *
   * `0`では未選択。
   *
   * @type {number}
   */
  #currentServiceId = 0;

  /**
   * 現在選択されているサービスのサービス識別。
   *
   * `0`では未選択。
   *
   * @type {number}
   */
  get currentServiceId() {
    return this.#currentServiceId;
  }

  /**
   * 現在選択されているサービス。
   *
   * @type {Service | undefined}
   */
  get currentService() {
    return this.#currentServiceId !== 0 ? this.#services.getById(this.#currentServiceId) : undefined;
  }

  /**
   * @type {number | null}
   */
  #activeVideoTag = null;

  /**
   * アクティブな映像コンポーネントのタグ。
   *
   * TSを開いていない状態、または映像ストリームにコンポーネントタグがない場合には`null`となる。
   *
   * @type {number | null}
   */
  get activeVideoTag() {
    return this.#activeVideoTag;
  }

  /**
   * @type {number | null}
   */
  #activeAudioTag = null;

  /**
   * アクティブな音声コンポーネントのタグ。
   *
   * TSを開いていない状態、または音声ストリームにコンポーネントタグがない場合には`null`となる。
   *
   * @type {number | null}
   */
  get activeAudioTag() {
    return this.#activeAudioTag;
  }

  /**
   * @type {"open-pending" | "playing" | "paused" | "stopped" | "closed"}
   */
  #state = "closed";

  /**
   * 現在の再生状態。
   *
   * @type {"open-pending" | "playing" | "paused" | "stopped" | "closed"}
   */
  get state() {
    return this.#state;
  }

  /**
   * ストリームを切り替え中かどうか。
   *
   * @type {boolean}
   */
  #isSwitching = false;

  /**
   * @type {string | null}
   */
  #source = null;

  /**
   * 現在開かれているファイルのパス。
   *
   * @type {string | null}
   */
  get source() {
    return this.#source;
  }

  /**
   * 動画の長さ。
   *
   * @type {number}
   */
  #duration = NaN;

  /**
   * 動画の秒単位での長さ。
   *
   * 再生していない状態では`NaN`、リアルタイム視聴などで長さが不明な場合は`+Infinity`となる。
   *
   * @type {number}
   */
  get duration() {
    return this.#duration;
  }

  /**
   * ホストから通知された再生位置。
   *
   * @type {number}
   */
  #lastPos = 0;
  /**
   * 再生位置が通知された時刻。
   *
   * @type {number}
   */
  #lastPosTime = 0;

  /**
   * 秒単位の再生位置。
   *
   * @type {number}
   */
  get currentTime() {
    if (!this.#isSwitching && this.#state === "playing") {
      return this.#lastPos + (performance.now() - this.#lastPosTime) / 1000 * this.#playbackRate;
    }
    return this.#lastPos;
  }

  set currentTime(value) {
    this.#postCommand({
      command: "set-position",
      position: value,
    });
  }

  /**
   * ホストから通知された日付時刻。
   *
   * @type {number | null}
   */
  #lastTimestamp = null;
  /**
   * 日付時刻が通知された時刻。
   *
   * @type {number}
   */
  #lastTimestampTime = 0;

  /**
   * 日付時刻。
   *
   * @type {Date | null}
   */
  get timestamp() {
    if (this.#lastTimestamp === null) {
      return null;
    }
    if (!this.#isSwitching && this.#state === "playing") {
      return new Date(this.#lastTimestamp + (performance.now() - this.#lastTimestampTime) * this.#playbackRate);
    }
    return new Date(this.#lastTimestamp);
  }

  /**
   * @type {number}
   */
  #volume = 1.0;

  /**
   * 音量。
   *
   * @type {number}
   */
  get volume() {
    return this.#volume;
  }

  set volume(value) {
    this.#volume = value;
    this.#postCommand({
      command: "set-volume",
      volume: value,
    });
  }

  /**
   * @type {boolean}
   */
  #muted = false;

  /**
   * ミュート状態。
   *
   * @type {boolean}
   */
  get muted() {
    return this.#muted;
  }

  set muted(value) {
    this.#muted = value;
    this.#postCommand({
      command: "set-muted",
      muted: value,
    });
  }

  /**
   * @type {number}
   */
  #playbackRate = 1.0;

  /**
   * @type {{ slowest: number, fastest: number }}
   */
  #playbackRateRange = {
    slowest: 0.25,
    fastest: 3.0,
  }

  /**
   * 再生速度。
   *
   * @type {number}
   */
  get playbackRate() {
    return this.#playbackRate;
  }

  set playbackRate(value) {
    if (value < this.#playbackRateRange.slowest || value > this.#playbackRateRange.fastest) {
      throw new Error("再生速度の範囲外");
    }

    this.#postCommand({
      command: "set-rate",
      rate: value,
    });
  }

  /**
   * 再生速度の範囲。
   *
   * @type {{ slowest: number, fastest: number }}
   */
  get playbackRateRange() {
    return {
      slowest: this.#playbackRateRange.slowest,
      fastest: this.#playbackRateRange.fastest,
    };
  }

  /**
   * @type {number}
   */
  #videoWidth = 0;

  /**
   * @type {number}
   */
  #videoHeight = 0;

  /**
   * 映像の幅。
   *
   * @type {number}
   */
  get videoWidth() {
    return this.#videoWidth;
  }
  /**
   * 映像の高さ。
   *
   * @type {number}
   */
  get videoHeight() {
    return this.#videoHeight;
  }

  /**
   * @type {number | null}
   */
  #audioChannels = null;

  /**
   * 音声のチャンネル数。
   *
   * @type {number | null}
   */
  get audioChannels() {
    return this.#audioChannels;
  }

  /**
   * @type {DualMonoMode | null}
   */
  #dualMonoMode = null;

  /**
   * デュアルモノラルの再生方法。
   *
   * @type {DualMonoMode | null}
   */
  get dualMonoMode() {
    return this.#dualMonoMode;
  }

  set dualMonoMode(value) {
    if (!value || !["left", "right", "stereo", "mix"].includes(value)) {
      throw new Error("不正なデュアルモノラルの再生方法");
    }

    this.#postCommand({
      command: "set-dual-mono-mode",
      mode: value,
    });
  }

  /**
   * 再生を開始する。
   */
  play() {
    this.#postCommand({ command: "play" });
  }

  /**
   * 再生を一時停止する。
   */
  pause() {
    this.#postCommand({ command: "pause" });
  }

  /**
   * 再生を停止する。
   */
  stop() {
    this.#postCommand({ command: "stop" });
  }

  /**
   * ファイルを閉じる。
   */
  close() {
    this.#postCommand({ command: "close" });
  }

  /**
   * サービスを選択する。
   *
   * @param {number} serviceId
   */
  selectService(serviceId) {
    this.#postCommand({
      command: "select-service",
      serviceId,
    });
  }

  /**
   * 映像ストリームを選択する。
   *
   * @param {number} componentTag
   */
  selectVideoStream(componentTag) {
    this.#postCommand({
      command: "select-video-stream",
      componentTag,
    });
  }

  /**
   * 音声ストリームを選択する。
   *
   * @param {number} componentTag
   */
  selectAudioStream(componentTag) {
    this.#postCommand({
      command: "select-audio-stream",
      componentTag,
    });
  }
};
