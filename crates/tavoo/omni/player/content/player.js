import { gController } from "tavoo://player/content/controller.js";
import { Slider } from "tavoo://player/content/slider.js";
import { Skin as SkinDefault } from "tavoo://player/content/skin-default.js";

/**
 * 映像用領域を指示するためのカスタム要素。
 *
 * 複数の`tavoo-screen`をHTMLに配置した際の挙動は未定義である。
 */
class Screen extends HTMLElement {
  static register() {
    customElements.define("tavoo-screen", Screen);
  }

  #resizeObserver;

  constructor() {
    super();

    this.#resizeObserver = new ResizeObserver(() => {
      this.#onResized();
    });
  }

  connectedCallback() {
    this.#resizeObserver.observe(this);
  }

  disconnectedCallback() {
    this.#resizeObserver.unobserve(this);
  }

  #onResized() {
    const { offsetWidth, offsetHeight } = document.body;

    gController.setVideoBounds(
      this.offsetLeft / offsetWidth,
      this.offsetTop / offsetHeight,
      (this.offsetLeft + this.offsetWidth) / offsetHeight,
      (this.offsetTop + this.offsetHeight) / offsetHeight,
    );
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
      gController.openDevTools();
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
  Screen.register();
  Slider.register();

  document.body.addEventListener("keydown", handleKeyDown);

  customElements.define("skin-default", SkinDefault);
  document.body.replaceChildren(document.createElement("skin-default"));
}
