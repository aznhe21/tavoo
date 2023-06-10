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
  #playbackRate;
  #seekbar;
  #videoStreams;
  #audioStreams;
  #services;
  #positionLabel;
  #durationLabel;

  #positionTimer = undefined;

  #volume = 1.0;

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
            <select id="playback-rate" title="再生速度"></select>

            <select id="video-streams"></select>
            <select id="audio-streams"></select>
            <select id="services"></select>
          </div>
        </div>
        <tavoo-screen id="screen"></tavoo-screen>
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
              // ドラッグ終了、シーク中ならシーク完了待ち
              this.#scrubberDraggingState = this.#seeking ? "completing" : "none";
              if (this.#scrubberPlayerState === "playing") {
                gController.play();
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
            console.log(`ファイル：${gController.source}`);
            this.updateSource();
            break;

          case "volume":
            console.log(`音量：${gController.volume}`);
            // 外部からの音量変更時は音量を記録しない
            this.updateVolumeSlider();
            break;

          case "rate-range": {
            const { slowest, fastest } = gController.playbackRateRange;
            console.log(`速度範囲：${slowest}..=${fastest}`);
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
            console.log(`再生位置：${gController.currentTime}`);
            this.updatePosition();
            break;

          case "seek-completed":
            console.log("全シーク完了");
            this.#seeking = false;
            if (this.#scrubberDraggingState === "completing") {
              // つまみドラッグ後のシークが完了
              this.#scrubberDraggingState = "none";
            }
            break;

          case "rate":
            console.log(`再生速度：${gController.playbackRate}`);
            this.updatePlaybackRate();
            break;

          case "resize":
            console.log(`解像度：${gController.videoWidth}x${gController.videoHeight}`);
            // this.updateActiveVideoStream();
            break;

          case "dual-mono-mode":
            console.log(`デュアルモノラル：${gController.dualMonoMode}`);
            this.updateActiveAudioStream();
            break;

          case "services":
            console.log("全サービス更新", [...gController.services]);
            this.updateServices();
            break;

          case "service":
            console.log("サービス更新", gController.services.getById(e.serviceId));
            this.updateService(e.serviceId);
            break;

          case "event": {
            const service = gController.services.getById(e.serviceId);
            const event = e.isPresent ? service.present_event : service.following_event;
            console.log(`イベント（${e.serviceId}、${e.isPresent}）`, event);
            if (e.isPresent) {
              this.updateService(e.serviceId);
            }
            break;
          }

          case "service-changed":
            console.log(`新サービスID：${gController.currentServiceId}`);
            this.updateSelectedService();
            break;

          case "stream-changed":
            console.log("ストリーム更新");
            this.updateActiveStream();
            break;

          case "caption":
            console.log("字幕", e.pos, e.caption);
            break;

          case "superimpose":
            console.log("文字スーパー", e.pos, e.caption);
            break;

          case "timestamp":
            console.log(`日付時刻：${gController.timestamp}`);
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
    if (state === "paused" && this.#scrubberDraggingState === "dragging") {
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
        this.#seekbar.disabled = true;
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
        value: base.stream.component_tag,
        text,
      };
    }

    for (let i = 0; i < service.audio_streams.length; i++) {
      const stream = service.audio_streams[i];
      const component = service.present_event?.audio_components.find(s => s.component_tag == stream.component_tag);
      const selected = stream.component_tag === gController.activeAudioTag;
      const base = {
        stream,
        component,
        selected,
      };

      if (component) {
        if (component.component_type === 0x02 && component.lang_code !== component.lang_code_2) {
          const [text1, text2] = component.text
            ? component.text.split("\n", 2)
            : [Skin.getLanguageText(component.lang_code), Skin.getLanguageText(component.lang_code_2)];
          yield* genDualMono(base, text1, text2, `${text1}+${text2}`);
        } else {
          const text = component.text
            ? component.text.split("\n", 1)[0]
            : Skin.getLanguageText(component.lang_code);
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
    const index = gController.services.findIndex(svc => svc.service_id === service.service_id);
    let text = service.present_event?.name ?? `サービス${index + 1}`;
    if (service.service_name) {
      text = `${service.service_name} ${text}`;
    }

    return {
      value: service.service_id,
      selected: service.service_id === gController.currentServiceId,
      disabled: service.video_streams.length === 0 || service.audio_streams.length === 0,
      text,
    };
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
  }

  /**
   * `serviceId`で指定されたサービスの情報を更新する。
   *
   * 指定サービスが選択中サービスの場合、ストリーム情報も更新する。
   */
  updateService(serviceId) {
    const index = gController.services.findIndex(svc => svc.service_id === serviceId);
    const option = this.#services.options[index];
    if (option) {
      const service = gController.services.getById(serviceId);
      const info = service ? this.getServiceInfo(service) : { text: "", disabled: true };
      option.textContent = info.text;
      option.disabled = info.disabled;
    }

    if (serviceId === gController.currentServiceId) {
      this.updateActiveStream();
    }
  }

  /**
   * サービスの選択状態を更新する。
   *
   * ストリーム情報も更新する。
   */
  updateSelectedService() {
    const index = gController.services.findIndex(svc => svc.service_id === gController.currentServiceId);
    this.#services.selectedIndex = index;

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

    this.#videoStreams.replaceChildren(...service.video_streams.map((stream, i) => {
      const option = document.createElement("option");
      option.value = stream.component_tag;
      // FIXME: 解像度？
      option.textContent = `動画${i + 1}`;
      option.selected = stream.component_tag === gController.activeAudioTag;
      return option;
    }));
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
