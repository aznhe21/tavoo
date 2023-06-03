export class Skin extends HTMLElement {
  #player;

  constructor() {
    super();

    const parser = new DOMParser();
    const doc = parser.parseFromString(`
      <root xmlns="http://www.w3.org/1999/xhtml">
        <link rel="stylesheet" href="tavoo://player/skin/default.css" />

        <tavoo-player id="player"></tavoo-player>
      </root>
    `, "application/xml");

    const shadow = this.attachShadow({ mode: "open" });
    shadow.append(...doc.documentElement.children);

    this.#player = shadow.getElementById("player");

    this.#player.addEventListener("source", this);
    this.#player.addEventListener("volume", this);
    this.#player.addEventListener("rate-range", this);
    this.#player.addEventListener("duration", this);
    this.#player.addEventListener("state", this);
    this.#player.addEventListener("position", this);
    this.#player.addEventListener("seek-completed", this);
    this.#player.addEventListener("rate", this);
    this.#player.addEventListener("dual-mono-mode", this);
    this.#player.addEventListener("services", this);
    this.#player.addEventListener("service", this);
    this.#player.addEventListener("event", this);
    this.#player.addEventListener("service-changed", this);
    this.#player.addEventListener("stream-changed", this);
    this.#player.addEventListener("caption", this);
    this.#player.addEventListener("superimpose", this);
    this.#player.addEventListener("timestamp", this);
  }

  handleEvent(e) {
    switch (e.target) {
      case this.#player:
        switch (e.type) {
          case "source":
            console.log(`ファイル：${this.#player.source}`);
            break;

          case "volume":
            console.log(`音量：${this.#player.volume}`);
            break;

          case "rate-range": {
            const { slowest, fastest } = this.#player.playbackRateRange;
            console.log(`速度範囲：${slowest}..=${fastest}`);
            break;
          }

          case "duration":
            //
            break;

          case "state":
            console.log(`再生状態：${this.#player.state}`);
            break;

          case "position":
            console.log(`再生位置：${this.#player.currentTime}`);
            break;

          case "seek-completed":
            console.log("全シーク完了");
            break;

          case "rate":
            console.log(`再生速度：${this.#player.playbackRate}`);
            break;

          case "dual-mono-mode":
            console.log(`デュアルモノラル：${this.#player.dualMonoMode}`);
            break;

          case "services":
            console.log("全サービス更新", [...this.#player.services]);
            break;

          case "service":
            console.log("サービス更新", this.#player.services.getById(e.serviceId));
            break;

          case "event": {
            const service = this.#player.services.getById(e.serviceId);
            const event = e.isPresent ? service.present_event : service.following_event;
            console.log(`イベント（${e.serviceId}、${e.isPresent}）`, event);
            break;
          }

          case "service-changed":
            console.log(`新サービスID：${this.#player.currentServiceId}`);
            break;

          case "stream-changed":
            console.log("ストリーム更新");
            break;

          case "caption":
            console.log("字幕", e.caption);
            break;

          case "superimpose":
            console.log("文字スーパー", e.caption);
            break;

          case "timestamp":
            console.log(`日付時刻：${this.#player.timestamp}`);
            break;
        }
        break;
    }
  }
}
