import { gController } from "tavoo://player/content/controller.js";

export class Skin extends HTMLElement {
  constructor() {
    super();

    const parser = new DOMParser();
    const doc = parser.parseFromString(`
      <root xmlns="http://www.w3.org/1999/xhtml">
        <link rel="stylesheet" href="tavoo://player/skin/default.css" />

        <tavoo-screen id="screen"></tavoo-screen>
      </root>
    `, "application/xml");

    const shadow = this.attachShadow({ mode: "open" });
    shadow.append(...doc.documentElement.children);
  }

  connectedCallback() {
    gController.addEventListener("source", this);
    gController.addEventListener("volume", this);
    gController.addEventListener("rate-range", this);
    gController.addEventListener("duration", this);
    gController.addEventListener("state", this);
    gController.addEventListener("position", this);
    gController.addEventListener("seek-completed", this);
    gController.addEventListener("rate", this);
    gController.addEventListener("resize", this);
    gController.addEventListener("dual-mono-mode", this);
    gController.addEventListener("services", this);
    gController.addEventListener("service", this);
    gController.addEventListener("event", this);
    gController.addEventListener("service-changed", this);
    gController.addEventListener("stream-changed", this);
    gController.addEventListener("caption", this);
    gController.addEventListener("superimpose", this);
    gController.addEventListener("timestamp", this);
  }

  disconnectedCallback() {
    gController.removeEventListener("source", this);
    gController.removeEventListener("volume", this);
    gController.removeEventListener("rate-range", this);
    gController.removeEventListener("duration", this);
    gController.removeEventListener("state", this);
    gController.removeEventListener("position", this);
    gController.removeEventListener("seek-completed", this);
    gController.removeEventListener("rate", this);
    gController.removeEventListener("resize", this);
    gController.removeEventListener("dual-mono-mode", this);
    gController.removeEventListener("services", this);
    gController.removeEventListener("service", this);
    gController.removeEventListener("event", this);
    gController.removeEventListener("service-changed", this);
    gController.removeEventListener("stream-changed", this);
    gController.removeEventListener("caption", this);
    gController.removeEventListener("superimpose", this);
    gController.removeEventListener("timestamp", this);
  }

  handleEvent(e) {
    switch (e.target) {
      case gController:
        switch (e.type) {
          case "source":
            console.log(`ファイル：${gController.source}`);
            break;

          case "volume":
            console.log(`音量：${gController.volume}`);
            break;

          case "rate-range": {
            const { slowest, fastest } = gController.playbackRateRange;
            console.log(`速度範囲：${slowest}..=${fastest}`);
            break;
          }

          case "duration":
            //
            break;

          case "state":
            console.log(`再生状態：${gController.state}`);
            break;

          case "position":
            console.log(`再生位置：${gController.currentTime}`);
            break;

          case "seek-completed":
            console.log("全シーク完了");
            break;

          case "rate":
            console.log(`再生速度：${gController.playbackRate}`);
            break;

          case "resize":
            console.log(`解像度：${gController.videoWidth}x${gController.videoHeight}`);
            break;

          case "dual-mono-mode":
            console.log(`デュアルモノラル：${gController.dualMonoMode}`);
            break;

          case "services":
            console.log("全サービス更新", [...gController.services]);
            break;

          case "service":
            console.log("サービス更新", gController.services.getById(e.serviceId));
            break;

          case "event": {
            const service = gController.services.getById(e.serviceId);
            const event = e.isPresent ? service.present_event : service.following_event;
            console.log(`イベント（${e.serviceId}、${e.isPresent}）`, event);
            break;
          }

          case "service-changed":
            console.log(`新サービスID：${gController.currentServiceId}`);
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
            console.log(`日付時刻：${gController.timestamp}`);
            break;
        }
        break;
    }
  }
}
