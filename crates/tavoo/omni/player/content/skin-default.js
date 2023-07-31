import { gController } from "tavoo://player/content/controller.js";

function formatTime(time) {
  time = Math.round(time);
  const hours = Math.floor(time / 60 / 60).toString();
  const minutes = Math.floor(time / 60 % 60).toString().padStart(2, "0");
  const seconds = Math.floor(time % 60).toString().padStart(2, "0");

  return `${hours}:${minutes}:${seconds}`;
}

function formatTimestamp(date) {
  const hours = date.getHours().toString().padStart(2, "0");
  const minutes = date.getMinutes().toString().padStart(2, "0");
  const seconds = date.getSeconds().toString().padStart(2, "0");
  return `${hours}:${minutes}:${seconds}`;
}

// 再生速度の最小（0.01単位）
const RATE_MIN = 25;
// 再生速度の最大（0.01単位）
const RATE_MAX = 500;
// 再生速度の間隔（0.01単位）
const RATE_STEP = 25;

export class Skin extends HTMLElement {
  #playButton;
  #stopButton;
  #muteButton;
  #volumeSlider;
  #captionCheckbox;
  #playbackRate;
  #seekbar;
  #videoStreams;
  #audioStreams;
  #services;
  #positionLabel;
  #durationLabel;

  #positionTimer = undefined;

  #volume = 1.0;

  #prompter;

  #seeking = false;
  /**
   * シークバーのつまみドラッグ状態。
   *
   * - `none`：操作していない
   * - `dragging`：ドラッグ中
   * - `completing`：ドラッグ終了後のシーク完了待ち
   */
  #scrubberDraggingState = "none";
  /**
   * つまみのドラッグを開始する前のプレイヤーの状態。
   * この値は、ドラッグ開始後からシークが完了するまでの間設定される。
   */
  #scrubberPlayerState = undefined;

  constructor() {
    super();

    const parser = new DOMParser();
    const doc = parser.parseFromString(`
      <root xmlns="http://www.w3.org/1999/xhtml">
        <link rel="stylesheet" href="tavoo://player/skin/default.css" />

        <div id="left"></div>
        <div id="top"></div>
        <div id="right"></div>
        <div id="bottom">
          <div id="seekbar-stack">
            <tavoo-slider id="seekbar" value="0" isolate="true"></tavoo-slider>
            <div id="position"></div>
            <div id="duration"></div>
          </div>

          <div id="controls">
            <button id="play">▶</button>
            <button id="stop">⏹</button>
            <button id="mute">🔊</button>
            <tavoo-slider id="volume" value="1" title="音量"></tavoo-slider>
            <label id="caption-display">
              <input id="caption-display-checkbox" type="checkbox" />
              字幕
            </label>
            <select id="playback-rate" title="再生速度"></select>

            <select id="video-streams"></select>
            <select id="audio-streams"></select>
            <select id="services"></select>
          </div>
        </div>
        <tavoo-screen id="screen"></tavoo-screen>
        <tavoo-prompter id="prompter"></tavoo-prompter>
      </root>
    `, "application/xml");

    const shadow = this.attachShadow({ mode: "open" });
    shadow.append(...doc.documentElement.children);

    this.#playButton = shadow.getElementById("play");
    this.#playButton.addEventListener("click", this);

    this.#stopButton = shadow.getElementById("stop");
    this.#stopButton.addEventListener("click", this);

    this.#muteButton = shadow.getElementById("mute");
    this.#muteButton.addEventListener("click", this);

    this.#volumeSlider = shadow.getElementById("volume");
    this.#volumeSlider.addEventListener("input", this);
    this.#volumeSlider.addEventListener("change", this);

    this.#captionCheckbox = shadow.getElementById("caption-display-checkbox");
    this.#captionCheckbox.addEventListener("input", this);

    this.#playbackRate = shadow.getElementById("playback-rate");
    this.#playbackRate.addEventListener("input", this);
    for (let rate = RATE_MIN; rate <= RATE_MAX; rate += RATE_STEP) {
      const option = document.createElement("option");
      option.value = rate.toString();

      if (rate === 100) {
        option.textContent = "等速";
        option.selected = true;
      } else {
        let s = `×${rate / 100 | 0}`;
        let n = rate % 100;
        if (n !== 0) {
          s += `.${n / 10 | 0}`;
          n = n % 10;
          if (n !== 0) {
            s += n.toString();
          }
        }

        option.textContent = s;
      }
      this.#playbackRate.append(option);
    }

    this.#seekbar = shadow.getElementById("seekbar");
    this.#seekbar.addEventListener("input", this);
    this.#seekbar.addEventListener("change", this);

    this.#videoStreams = shadow.getElementById("video-streams");
    this.#videoStreams.addEventListener("input", this);

    this.#audioStreams = shadow.getElementById("audio-streams");
    this.#audioStreams.addEventListener("input", this);

    this.#services = shadow.getElementById("services");
    this.#services.addEventListener("input", this);

    this.#positionLabel = shadow.getElementById("position");

    this.#durationLabel = shadow.getElementById("duration");

    this.#prompter = shadow.getElementById("prompter");

    customElements.upgrade(this.shadowRoot);
    this.updateSource();
    this.updatePosition();
    this.updateState();
    this.updateMuteButton();
    this.updateVolumeSlider();
    this.updatePlaybackRate();
    this.updateServices();
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
    gController.removeEventListener("timestamp", this);
  }

  handleEvent(e) {
    switch (e.target) {
      case this.#seekbar:
        switch (e.type) {
          case "input":
            if (this.#scrubberDraggingState !== "dragging") {
              this.#scrubberDraggingState = "dragging";
              this.#scrubberPlayerState = gController.state;
              if (gController.state === "playing") {
                gController.pause();
              }
            }

            this.#seeking = true;
            gController.currentTime = this.#seekbar.value * gController.duration;
            this.updatePosition(true);
            break;

          case "change":
            if (this.#scrubberDraggingState === "dragging") {
              // ドラッグ終了
              if (this.#scrubberPlayerState === "playing" && gController.state === "paused") {
                gController.play();
              }

              if (this.#seeking) {
                // シーク完了待ち
                this.#scrubberDraggingState = "completing";
              } else {
                this.#scrubberDraggingState = "none";
                this.#scrubberPlayerState = undefined;
              }
            }
            break;
        }
        break;

      case this.#playButton:
        switch (e.type) {
          case "click":
            if (e.button !== 0) {
              return;
            }

            switch (gController.state) {
              case "playing":
                gController.pause();
                break;

              case "paused":
              case "stopped":
                gController.play();
                break;
            }
            break;
        }
        break;

      case this.#stopButton:
        switch (e.type) {
          case "click":
            if (e.button !== 0) {
              return;
            }

            if (["open-pending", "playing", "paused"].includes(gController.state)) {
              gController.stop();
            }
            break;
        }
        break;

      case this.#muteButton:
        switch (e.type) {
          case "click":
            if (e.button !== 0) {
              return;
            }

            if (gController.muted || gController.volume <= 0) {
              // ミュート解除時は非ミュート時の音量を復元
              gController.volume = this.#volume;
              gController.muted = false;
            } else {
              gController.muted = true;
            }
            this.updateVolumeSlider();
            break;
        }
        break;

      case this.#volumeSlider:
        switch (e.type) {
          case "input":
            if (this.#volumeSlider.value > 0) {
              gController.muted = false;
              gController.volume = this.#volumeSlider.value;
            } else {
              // 変化後がミュートの場合は音量を復元
              gController.muted = true;
              gController.volume = this.#volume;
            }
            this.updateMuteButton();
            break;

          case "change":
            if (this.#volumeSlider.value > 0) {
              // 変化後がミュートじゃない場合は音量を記録
              this.#volume = this.#volumeSlider.value;
            }
            this.updateMuteButton();
            break;
        }
        break;

      case this.#captionCheckbox:
        this.#prompter.display = this.#captionCheckbox.checked ? "always" : "auto";
        break;

      case this.#playbackRate:
        switch (e.type) {
          case "input":
            gController.playbackRate = Number.parseInt(this.#playbackRate.value, 10) / 100;
            break;
        }
        break;

      case this.#videoStreams:
        switch (e.type) {
          case "input": {
            const value = this.#videoStreams.value;
            if (value) {
              gController.selectVideoStream(Number.parseInt(value, 10));
            }
            break;
          }
        }
        break;

      case this.#audioStreams:
        switch (e.type) {
          case "input": {
            const value = this.#audioStreams.value;
            if (value) {
              if (gController.dualMonoMode != null) {
                gController.dualMonoMode = value;
              } else {
                gController.selectAudioStream(Number.parseInt(value, 10));
              }
            }
            break;
          }
        }
        break;

      case this.#services:
        switch (e.type) {
          case "input": {
            const value = this.#services.value;
            if (value) {
              gController.selectService(Number.parseInt(value, 10));
            }
            break;
          }
        }
        break;

      case gController:
        switch (e.type) {
          case "source":
            this.updateSource();
            break;

          case "volume":
            // 外部からの音量変更時は音量を記録しない
            this.updateVolumeSlider();
            break;

          case "rate-range": {
            const { slowest, fastest } = gController.playbackRateRange;
            for (const option of this.#playbackRate.options) {
              const rate = Number.parseInt(option.value, 10) / 100;
              option.disabled = rate < slowest || rate > fastest;
            }
            break;
          }

          case "duration":
            this.updatePosition();
            break;

          case "state":
            console.log(`再生状態：${gController.state}`);
            this.updateState();
            break;

          case "position":
            this.updatePosition();
            break;

          case "seek-completed":
            console.log("全シーク完了");
            this.#seeking = false;
            if (this.#scrubberDraggingState === "completing") {
              // つまみドラッグ後のシークが完了
              this.#scrubberDraggingState = "none";
              this.#scrubberPlayerState = undefined;
            }
            break;

          case "rate":
            this.updatePlaybackRate();
            break;

          case "resize":
            this.updateActiveVideoStream();
            break;

          case "dual-mono-mode":
            this.updateActiveAudioStream();
            break;

          case "services":
            this.updateServices();
            break;

          case "service":
            this.updateService(e.serviceId);
            break;

          case "event": {
            if (e.isPresent) {
              this.updateService(e.serviceId);
            }
            break;
          }

          case "service-changed":
            this.updateSelectedService();
            break;

          case "stream-changed":
            this.updateActiveStream();
            break;

          case "timestamp":
            this.updatePosition();
            break;
        }
        break;
    }
  }

  updateSource() {
    document.title = gController.source ? `${gController.source} - TaVoo` : "TaVoo";
  }

  updatePosition(noSeekbar = false) {
    const { currentTime, duration } = gController;
    if (Number.isFinite(duration)) {
      if (!noSeekbar) {
        const pos = currentTime / duration;
        this.#seekbar.progressValue = pos;
        if (this.#scrubberDraggingState === "none") {
          // つまみを操作していない場合だけつまみを移動
          this.#seekbar.value = pos;
        }
      }

      let durText = ` / ${formatTime(duration)}`;
      const timestamp = gController.timestamp;
      if (timestamp !== null) {
        durText += ` (${formatTimestamp(timestamp)})`;
      }

      this.#positionLabel.textContent = formatTime(duration * this.#seekbar.value);
      this.#durationLabel.textContent = durText;
    } else {
      if (!noSeekbar) {
        this.#seekbar.progressValue = 0;
        this.#seekbar.value = 0;
      }

      this.#positionLabel.textContent = "";
      this.#durationLabel.textContent = "";
    }
  }

  updateState() {
    this.setPositionTimer();

    let state = gController.state;
    if (this.#scrubberPlayerState === "playing") {
      // つまみドラッグ中は動画を一時停止するが画面上は再生中にする
      state = "playing";
    }

    switch (state) {
      case "open-pending":
      case "closed":
        this.#playButton.textContent = "▶";
        this.#playButton.disabled = true;
        this.#stopButton.disabled = true;
        this.#seekbar.disabled = true;
        break;

      case "playing":
        this.#playButton.textContent = "⏸";
        this.#playButton.disabled = false;
        this.#stopButton.disabled = false;
        this.#seekbar.disabled = false;
        break;

      case "paused":
        this.#playButton.textContent = "▶";
        this.#playButton.disabled = false;
        this.#stopButton.disabled = false;
        this.#seekbar.disabled = false;
        break;

      case "stopped":
        this.#playButton.textContent = "▶";
        this.#playButton.disabled = false;
        this.#stopButton.disabled = false;
        this.#seekbar.disabled = false;
        break;
    }
  }

  updateMuteButton() {
    this.#muteButton.textContent = !gController.muted && gController.volume > 0 ? "🔊" : "🔇";
  }

  updateVolumeSlider() {
    this.#volumeSlider.value = gController.muted ? 0 : gController.volume;
    this.updateMuteButton();
  }

  updatePlaybackRate() {
    this.#playbackRate.value = ((gController.playbackRate * 100 / 25 | 0) * 25).toString();
  }

  static LANG_CODES = {
    "jpn": "日本語",
    "eng": "英語",
  };

  static getLanguageText(code) {
    return Skin.LANG_CODES[code] ?? code.toUpperCase();
  }

  static LANG_SHORT_CODES = {
    "jpn": "日",
    "eng": "英",
  };

  static getLanguageShortText(code) {
    return Skin.LANG_SHORT_CODES[code] ?? code.toUpperCase();
  }

  // https://github.com/DBCTRADO/LibISDB/blob/066ec430b83338085accbf7600e74dec69e98296/LibISDB/TS/TSInformation.cpp#L157-L195
  static getVideoComponentTypeText(componentType) {
    switch (componentType) {
      case 0x01: return "480i[4:3]";
      case 0x02: return "480i[16:9] パンベクトルあり";
      case 0x03: return "480i[16:9]";
      case 0x04: return "480i[>16:9]";
      case 0x91: return "2160p[4:3]";
      case 0x92: return "2160p[16:9] パンベクトルあり";
      case 0x93: return "2160p[16:9]";
      case 0x94: return "2160p[>16:9]";
      case 0xA1: return "480p[4:3]";
      case 0xA2: return "480p[16:9] パンベクトルあり";
      case 0xA3: return "480p[16:9]";
      case 0xA4: return "480p[>16:9]";
      case 0xB1: return "1080i[4:3]";
      case 0xB2: return "1080i[16:9] パンベクトルあり";
      case 0xB3: return "1080i[16:9]";
      case 0xB4: return "1080i[>16:9]";
      case 0xC1: return "720p[4:3]";
      case 0xC2: return "720p[16:9] パンベクトルあり";
      case 0xC3: return "720p[16:9]";
      case 0xC4: return "720p[>16:9]";
      case 0xD1: return "240p[4:3]";
      case 0xD2: return "240p[16:9] パンベクトルあり";
      case 0xD3: return "240p[16:9]";
      case 0xD4: return "240p[>16:9]";
      case 0xE1: return "1080p[4:3]";
      case 0xE2: return "1080p[16:9] パンベクトルあり";
      case 0xE3: return "1080p[16:9]";
      case 0xE4: return "1080p[>16:9]";
      case 0xF1: return "180p[4:3]";
      case 0xF2: return "180p[16:9] パンベクトルあり";
      case 0xF3: return "180p[16:9]";
      case 0xF4: return "180p[>16:9]";
      default: return undefined;
    }
  }

  // https://github.com/DBCTRADO/LibISDB/blob/066ec430b83338085accbf7600e74dec69e98296/LibISDB/TS/TSInformation.cpp#L198-L223
  static getAudioComponentTypeText(componentType) {
    switch (componentType) {
      case 0x01: return "Mono";
      case 0x02: return "Dual mono";
      case 0x03: return "Stereo";
      case 0x04: return "3ch[2/1]";
      case 0x05: return "3ch[3/0]";
      case 0x06: return "4ch[2/2]";
      case 0x07: return "4ch[3/1]";
      case 0x08: return "5ch";
      case 0x09: return "5.1ch";
      case 0x0A: return "6.1ch[3/3.1]";
      case 0x0B: return "6.1ch[2/0/0-2/0/2-0.1]";
      case 0x0C: return "7.1ch[5/2.1]";
      case 0x0D: return "7.1ch[3/2/2.1]";
      case 0x0E: return "7.1ch[2/0/0-3/0/2-0.1]";
      case 0x0F: return "7.1ch[0/2/0-3/0/2-0.1]";
      case 0x10: return "10.2ch";
      case 0x11: return "22.2ch";
      case 0x40: return "視覚障害者用音声解説";
      case 0x41: return "聴覚障害者用音声";
      default: return undefined;
    }
  }

  /**
   * 選択中サービスにおける音声ストリームを列挙する。
   */
  *currentAudioStreams() {
    const service = gController.currentService;
    if (!service) {
      return;
    }

    const dualMonoMode = gController.dualMonoMode;
    function* genDualMono(base, text1, text2, textBoth) {
      yield {
        ...base,
        value: "left",
        text: text1,
        selected: base.selected && dualMonoMode === "left",
      };
      yield {
        ...base,
        value: "right",
        text: text2,
        selected: base.selected && dualMonoMode === "right",
      };
      yield {
        ...base,
        value: "mix",
        text: `${textBoth}（混合）`,
        selected: base.selected && dualMonoMode === "mix",
      };
      yield {
        ...base,
        value: "stereo",
        text: `${textBoth}（ステレオ）`,
        selected: base.selected && dualMonoMode === "stereo",
      };
    }
    function* genNormal(base, text) {
      yield {
        ...base,
        value: base.stream.componentTag,
        text,
      };
    }

    for (let i = 0; i < service.audioStreams.length; i++) {
      const stream = service.audioStreams[i];
      const component = service.presentEvent?.audioComponents.find(s => s.componentTag == stream.componentTag);
      const selected = stream.componentTag === gController.activeAudioTag;
      const base = {
        stream,
        component,
        selected,
      };

      if (component) {
        if (component.componentType === 0x02 && component.langCode !== component.langCode2) {
          const [text1, text2] = component.text
            ? component.text.split("\n", 2)
            : [Skin.getLanguageText(component.langCode), Skin.getLanguageText(component.langCode2)];
          yield* genDualMono(base, text1, text2, `${text1}+${text2}`);
        } else {
          const text = component.text
            ? component.text.split("\n", 1)[0]
            : Skin.getLanguageText(component.langCode);
          yield* genNormal(base, text);
        }
      } else {
        // EITに情報が無い（EIT未受信またはワンセグ）場合は音声自体の情報を参照
        if (gController.dualMonoMode != null) {
          yield* genDualMono(base, "主音声", "副音声", "主+副音声");
        } else {
          yield* genNormal(base, `音声${i + 1}`);
        }
      }
    }
  }

  getServiceInfo(service) {
    const index = gController.services.findIndex(svc => svc.serviceId === service.serviceId);
    let text = service.presentEvent?.name ?? `サービス${index + 1}`;
    if (service.serviceName) {
      text = `${service.serviceName} ${text}`;
    }

    return {
      value: service.serviceId,
      selected: service.serviceId === gController.currentServiceId,
      disabled: service.videoStreams.length === 0 || service.audioStreams.length === 0,
      text,
    };
  }

  getPresentEventText(service) {
    const event = service.presentEvent;
    if (!event) {
      return "";
    }

    // https://github.com/DBCTRADO/TVTest/blob/41ce0bcfb39ccd98cfd5721cd197961020a60293/src/EventInfoPopup.cpp#L114-L199

    const startTime = new Date(event.startTime * 1000);
    const endTime = new Date((event.startTime + event.duration) * 1000);
    let text = `${startTime.toLocaleString()}～${endTime.toLocaleTimeString()}\n`;
    if (event.name) {
      text += `${event.name}\n`;
    }

    const eventText = event.text.trimEnd();
    if (eventText) {
      text += `\n${event.text}\n`;
    }

    if (event.extendedItems.length > 0) {
      for (const item of event.extendedItems) {
        text += `\n${item.description}\n${item.item.trimEnd()}`;
      }

      text += "\n";
    }

    if (event.videoComponents.length > 0) {
      const video = Skin.getVideoComponentTypeText(event.videoComponents[0].componentType);
      if (video !== undefined) {
        text += `\n■映像：${video}`;
      }
    }
    if (event.audioComponents.length > 0) {
      function format(component) {
        let text = "";
        let bilingual = false;
        if (component.componentType === 0x02 && component.langCode2 && component.langCode !== component.langCode2) {
          text += "Mono 二カ国語";
          bilingual = true;
        } else {
          text += Skin.getAudioComponentTypeText(component.componentType) ?? "?";
        }

        if (component.text) {
          text += ` [${component.text.replaceAll("\n", "/")}]`;
        } else if (bilingual) {
          const lang1 = Skin.getLanguageText(component.langCode);
          const lang2 = Skin.getLanguageText(component.langCode2);
          text += ` [${lang1}/${lang2}]`;
        } else {
          const lang = Skin.getLanguageText(component.langCode);
          text += ` [${lang}]`;
        }

        return text;
      }

      text += "\n■音声：";
      if (event.audioComponents.length === 1) {
        text += format(event.audioComponents[0]);
      } else {
        for (let i = 0; i < event.audioComponents.length; i++) {
          if (i === 0) {
            text += "主：";
          } else {
            text += " / 副：";
          }

          text += format(event.audioComponents[i]);
        }
      }
    }

    // TODO: ジャンル（event.genres）

    return text;
  }

  updateServices() {
    this.#services.replaceChildren(...Array.from(gController.services, service => {
      const info = this.getServiceInfo(service);

      const option = document.createElement("option");
      option.value = info.value;
      option.textContent = info.text;
      option.selected = info.selected;
      option.disabled = info.disabled;
      return option;
    }));

    const service = gController.currentService;
    if (service) {
      this.#services.title = this.getPresentEventText(service);
    }
  }

  /**
   * `serviceId`で指定されたサービスの情報を更新する。
   *
   * 指定サービスが選択中サービスの場合、ストリーム情報も更新する。
   */
  updateService(serviceId) {
    const index = gController.services.findIndex(svc => svc.serviceId === serviceId);
    const service = gController.services.get(index);

    const option = this.#services.options[index];
    if (option) {
      const info = service ? this.getServiceInfo(service) : { text: "", disabled: true };
      option.textContent = info.text;
      option.disabled = info.disabled;
    }

    if (serviceId === gController.currentServiceId) {
      this.#services.title = this.getPresentEventText(service);
      this.updateActiveStream();
    }
  }

  /**
   * サービスの選択状態を更新する。
   *
   * ストリーム情報も更新する。
   */
  updateSelectedService() {
    const index = gController.services.findIndex(svc => svc.serviceId === gController.currentServiceId);
    this.#services.selectedIndex = index;

    const service = gController.currentService;
    if (service) {
      this.#services.title = this.getPresentEventText(service);
    }

    this.updateActiveStream();
  }

  /**
   * 映像・音声のストリーム情報を更新する。
   */
  updateActiveStream() {
    this.updateActiveVideoStream();
    this.updateActiveAudioStream();
  }

  /**
   * 映像のストリーム情報を更新する。
   */
  updateActiveVideoStream() {
    const service = gController.currentService;
    if (!service) {
      return;
    }

    this.#videoStreams.replaceChildren(...service.videoStreams.map((stream, i) => {
      const option = document.createElement("option");
      option.value = stream.componentTag;
      option.textContent = `動画${i + 1}`;
      option.selected = stream.componentTag === gController.activeAudioTag;
      return option;
    }));
    this.#videoStreams.title = `${gController.videoWidth}x${gController.videoHeight}`;
  }

  /**
   * 音声のストリーム情報を更新する。
   */
  updateActiveAudioStream() {
    this.#audioStreams.replaceChildren(...Array.from(this.currentAudioStreams(), (as, index) => {
      const option = document.createElement("option");
      option.value = as.value;
      option.textContent = `${index + 1}：${as.text}`;
      option.selected = as.selected;
      return option;
    }));

    let text = "";
    const service = gController.currentService;
    if (service) {
      const stream = service.audioStreams.find(s => s.componentTag === gController.activeAudioTag);
      const component = service.presentEvent?.audioComponents.find(s => s.componentTag === stream.componentTag);

      // https://github.com/DBCTRADO/TVTest/blob/ace93932082f1d64ea6bd87913036701ae206dc5/src/UICore.cpp#L591-L725

      const dualMonoMode = gController.dualMonoMode;
      if (dualMonoMode) {
        if (component && component.componentType === 0x02 && component.langCode2 &&
          component.langCode !== component.langCode2)
        {
          switch (dualMonoMode) {
            case "left":
              text += Skin.getLanguageText(component.langCode);
              break;

            case "right":
              text += Skin.getLanguageText(component.langCode2);
              break;

            case "mix": {
              const lang1 = Skin.getLanguageShortText(component.langCode);
              const lang2 = Skin.getLanguageShortText(component.langCode2);
              text += `${lang1}+${lang2} [混]`;
              break;
            }

            case "stereo": {
              const lang1 = Skin.getLanguageShortText(component.langCode);
              const lang2 = Skin.getLanguageShortText(component.langCode2);
              text += `${lang1}+${lang2} [ス]`;
              break;
            }
          }
        } else {
          switch (dualMonoMode) {
            case "left":
              text += "主音声";
              break;

            case "right":
              text += "副音声";
              break;

            case "mix":
              text += "主+副音声 [混]";
              break;

            case "stereo":
              text += "主+副音声 [ス]";
              break;
          }
        }
      } else if (service.audioStreams.length > 1) {
        let format;
        switch (gController.audioChannels) {
          case 1:
            format = "[M]";
            break;

          case 2:
            format = "[S]";
            break;

          case 6:
            format = "[5.1]";
            break;

          default:
            format = `[${gController.audioChannels}ch]`;
            break;
        }
        text += `${format} `;

        const audio = component.text
          .replaceAll("\n", "/")
          // [S]などがあれば除去する
          .replace(new RegExp(` ?${format.replaceAll("[", "\\[")} ?`), "");
        if (audio) {
          text += audio;
        } else {
          text += Skin.getLanguageText(component.langCode);
        }
      } else {
        switch (gController.audioChannels) {
          case 1:
            text += "Mono";
            break;

          case 2:
            text += "Stereo";
            break;

          case 6:
            text += "5.1ch";
            break;

          default:
            text += `${gController.audioChannels}ch`;
            break;
        }
      }

      Skin.getAudioComponentTypeText();
    }
    this.#audioStreams.title = text;
  }

  setPositionTimer() {
    if (this.#positionTimer !== undefined) {
      clearTimeout(this.#positionTimer);
    }

    this.updatePosition();

    // 再生中だけ定期更新
    if (gController.state === "playing") {
      this.#positionTimer = setTimeout(() => this.setPositionTimer(), 50);
    } else {
      this.#positionTimer = undefined;
    }
  }
}
