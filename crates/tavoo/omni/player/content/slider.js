function clamp(value, min, max) {
  return value < min ? min : value > max ? max : value;
}

export class Slider extends HTMLElement {
  static register() {
    customElements.define("tavoo-slider", Slider);
  }

  static get observedAttributes() {
    return ["disabled", "isolate", "value"];
  }

  #dragging = false;
  #disabled = false;
  #isolate = false;
  #value = 0;
  #progressValue = 0;

  #container;
  #slider;
  #progress;
  #scrubber;

  constructor() {
    super();

    const parser = new DOMParser();
    const doc = parser.parseFromString(`
      <div id="container" xmlns="http://www.w3.org/1999/xhtml">
        <link rel="stylesheet" href="tavoo://player/skin/slider.css" />

        <div id="slider" part="slider">
          <div id="progress" part="progress"></div>
          <div id="scrubber" part="scrubber"></div>
        </div>
        <div id="slot-container">
          <slot></slot>
        </div>
      </div>
    `, "application/xml");

    const shadow = this.attachShadow({ mode: "open" });
    shadow.append(doc.documentElement);

    this.#container = shadow.firstChild;
    this.#slider = shadow.getElementById("slider");
    this.#progress = shadow.getElementById("progress");
    this.#scrubber = shadow.getElementById("scrubber");

    this.#container.addEventListener("pointerdown", this, { capture: true });
    this.#container.addEventListener("pointermove", this, { capture: true });
    this.#container.addEventListener("pointerup", this, { capture: true });

    this.#updateStyles();
  }

  #updateStyles() {
    let percent = this.#value * 100;
    this.#scrubber.style.left = `${percent}%`;

    if (this.#isolate) {
      percent = this.#progressValue * 100;
    }
    this.#progress.style.width = `${percent}%`;
  }

  #pointerMoved(e) {
    if (this.#disabled) {
      return;
    }

    const newValue = clamp((e.offsetX - this.#slider.offsetLeft) / this.#slider.offsetWidth, 0, 1);
    // 4Kで1px動かしたときの差
    if (Math.abs(this.#value - newValue) < 0.00026) {
      return;
    }

    this.value = newValue;
    this.dispatchEvent(new Event("input"));
  }

  handleEvent(e) {
    switch (e.type) {
      case "pointerdown":
        if (e.button !== 0) {
          return;
        }

        this.#dragging = true;
        this.#container.setPointerCapture(e.pointerId);
        e.preventDefault();
        this.#pointerMoved(e);
        break;

      case "pointermove":
        if (!this.#dragging) {
          return;
        }

        this.#pointerMoved(e);
        break;

      case "pointerup":
        if (!this.#dragging) {
          return;
        }

        this.#dragging = false;
        this.#container.releasePointerCapture(e.pointerId);
        e.preventDefault();
        this.#pointerMoved(e);
        this.dispatchEvent(new Event("change"));
        break;
    }
  }

  attributeChangedCallback(name, _, value) {
    switch (name) {
      case "disabled":
        this.disabled = value !== null;
        break;

      case "isolate":
        this.isolate = value !== null;
        break;

      case "value":
        this.value = value;
        break;
    }
  }

  /**
   * 無効化状態。
   *
   * 無効化状態ではユーザーによる操作を受け付けなくなる。
   */
  get disabled() {
    return this.#disabled;
  }

  set disabled(value) {
    this.#disabled = value;
  }

  /**
   * 分離モード。
   *
   * 真の場合はつまみと進行バーが分離され、位置設定時にはつまみだけが更新されるようになる。
   * このとき進行バーを設定するには`progressValue`を使用する。
   */
  get isolate() {
    return this.#isolate;
  }

  set isolate(value) {
    this.#isolate = value;
  }

  /**
   * スライダーの位置。
   *
   * 分離モードが有効の場合はつまみの位置を示す。
   */
  get value() {
    return this.#value;
  }

  set value(value) {
    if (typeof value === "string") {
      value = Number.parseFloat(value);
    }

    this.#value = clamp(value, 0, 1);
    this.#updateStyles();
  }

  /**
   * 進行バーの位置。
   *
   * 分離モードが無効の場合は`value`を参照・設定するのと変わらない。
   */
  get progressValue() {
    return this.#isolate ? this.#progressValue : this.#value;
  }

  set progressValue(value) {
    if (this.#isolate) {
      if (typeof value === "string") {
        value = Number.parseFloat(value);
      }

      this.#progressValue = clamp(value, 0, 1);
      this.#updateStyles();
    } else {
      this.value = value;
    }
  }
}
