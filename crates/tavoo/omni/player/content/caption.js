// @ts-check
import { gController } from "tavoo://player/content/controller.js";

/** 過去の字幕と見做す時間差。シーク後に保留中の字幕からこれ以上古いものは削除する。 */
const DT_PAST = 10;
/** 未来の字幕と見做す時間差。シーク後に保留中の字幕からこれ以上新しいものは削除する。 */
const DT_FUTURE = 10;

/**
 * @typedef {"drcs0" | "drcs1" | "drcs2" | "drcs3" | "drcs4" | "drcs5" | "drcs6" | "drcs7" |
 *   "drcs8" | "drcs9" | "drcs10" | "drcs11" | "drcs12" | "drcs13" | "drcs14" | "drcs15"} DrcsType
 * @typedef {"small" | "medium" | "normal" | "height-w" | "width-w" | "size-w"} CharSize
 * @typedef {"qhd-horz" | "qhd-vert" | "sd-horz" | "sd-vert"} DisplayFormat
 * @typedef {"auto-display" | "selectable"} DisplayMode
 *
 * @typedef {{
 *   type: "char-size";
 *   charSize: CharSize;
 * } | {
 *   type: "string";
 *   string: string;
 * } | {
 *   type: "drcs0";
 *   code1: number;
 *   code2: number;
 * } | {
 *   type: Exclude<DrcsType, "drcs0">;
 *   code: number;
 * } | {
 *   type: "null";
 * } | {
 *   type: "active-position-backward";
 * } | {
 *   type: "active-position-forward";
 * } | {
 *   type: "active-position-down";
 * } | {
 *   type: "active-position-up";
 * } | {
 *   type: "active-position-return";
 * } | {
 *   type: "parameterized-active-position-forward";
 *   p1: number;
 * } | {
 *   type: "active-position-set";
 *   p1: number;
 *   p2: number;
 * } | {
 *   type: "clear-screen";
 * } | {
 *   type: "unit-separator";
 * } | {
 *   type: "space";
 * } | {
 *   type: "delete";
 * } | {
 *   type: "color-foreground";
 *   p1: number;
 * } | {
 *   type: "color-background";
 *   p1: number;
 * } | {
 *   type: "color-half-foreground";
 *   p1: number;
 * } | {
 *   type: "color-half-background";
 *   p1: number;
 * } | {
 *   type: "color-palette";
 *   p1: number;
 * } | {
 *   type: "pattern-polarity-normal";
 * } | {
 *   type: "pattern-polarity-inverted-1";
 * } | {
 *   type: "flushing-control-start-normal";
 * } | {
 *   type: "flushing-control-start-inverted";
 * } | {
 *   type: "flushing-control-stop";
 * } | {
 *   type: "wait-for-process";
 *   p1: number;
 * } | {
 *   type: "repeat-character";
 *   p1: number;
 * } | {
 *   type: "stop-lining";
 * } | {
 *   type: "start-lining";
 * } | {
 *   type: "highlight-block";
 *   p1: number;
 * } | {
 *   type: "set-writing-format-init";
 *   p1: number;
 * } | {
 *   type: "raster-color-command";
 *   p1: number;
 * } | {
 *   type: "active-coordinate-position-set";
 *   p1: number;
 *   p2: number;
 * } | {
 *   type: "set-display-format";
 *   p1: number;
 *   p2: number;
 * } | {
 *   type: "set-display-position";
 *   p1: number;
 *   p2: number;
 * } | {
 *   type: "character-composition-dot-designation";
 *   p1: number;
 *   p2: number;
 * } | {
 *   type: "set-horizontal-spacing";
 *   p1: number;
 * } | {
 *   type: "set-vertical-spacing";
 *   p1: number;
 * } | {
 *   type: "ornament-control-clear";
 * } | {
 *   type: "ornament-control-hemming";
 *   p1: number;
 * } | {
 *   type: "builtin-sound-replay";
 *   p1: number;
 * } | {
 *   type: "scroll-designation";
 *   p1: number;
 *   p2: number;
 * }} AribChar
 *
 * @typedef {{
 *   type: "drcs0";
 *   code1: number;
 *   code2: number;
 * } | {
 *   type: Exclude<DrcsType, "drcs0">;
 *   code: number;
 * }} DrcsCharCode
 *
 * @typedef {{
 *   depth: number;
 *   width: number;
 *   height: number;
 *   patternData: string;
 * }} DrcsData
 *
 * @typedef {{
 *   characterCode: DrcsCharCode;
 *   fonts: DrcsData[];
 * }} DrcsCode
 *
 * @typedef {{
 *   depth: number;
 *   width: number;
 *   height: number;
 *   patternData: string;
 * }} DrcsFont
 *
 * @typedef {{
 *   languageTag: number;
 *   dmfRecv: DisplayMode;
 *   dmfPlayback: DisplayMode;
 *   format: DisplayFormat | null;
 *   langCode: string;
 *   rollupMode: "non-rollup" | "rollup" | "reserved";
 * }} CaptionLanguage
 *
 * @typedef {{
 *   type: "statement-body";
 *   statement: AribChar[];
 * } | {
 *   type: "drcs";
 *   drcs: DrcsCode[];
 * } | {
 *   type: "bitmap";
 *   xPosition: number;
 *   yPosition: number;
 *   colorIndices: string;
 *   pngData: string;
 * }} CaptionDataUnit
 *
 * @typedef {{
 *   type: "management-data";
 *   group: "A" | "B";
 *   tmd: "free" | "real-time";
 *   languages: CaptionLanguage[];
 *   dataUnits: CaptionDataUnit[];
 * } | {
 *   type: "data";
 *   group: "A" | "B";
 *   languageTag: number;
 *   tmd: "free" | "real-time";
 *   stm: number | null;
 *   dataUnits: CaptionDataUnit[];
 * } | {
 *   type: "postponed";
 *   postponed: AribChar[];
 * }} Caption
 *
 * @typedef {DisplayFormat | "profile-c"} ExtendedDisplayFormat
 */

/**
 * DRCSのフォントから各要素が`0.0`～`1.0`からなる`Float32Array`によるビットマップを生成する。
 *
 * @param {DrcsFont} font
 * @returns {Float32Array|undefined}
 */
export function createDrcsBitmap(font) {
  if (font.depth !== 0 && font.depth !== 2) {
    return undefined;
  }
  const bpp = font.depth === 0 ? 1 : 2;
  const size = font.width * font.height;
  const bits = size * bpp;
  const pattern = Uint8Array.from(atob(font.patternData), c => c.charCodeAt(0));
  if (pattern.length * 8 !== bits) {
    return undefined;
  }
  const mask = (1 << bpp) - 1;

  const bitmap = new Float32Array(size);
  for (let p = 0, i = 0; p < bits; p += bpp, i++) {
    const pos = p >>> 3; // floor(p / 3)
    const shift = 8 - bpp - p & 7; // 8 - bpp - p % 8
    const bits = (pattern[pos] >>> shift) & mask;

    bitmap[i] = bits / mask;
  }
  return bitmap;
}

/**
 * DRCSのフォントから白で描画済みの`HTMLCanvasElement`を生成する。
 *
 * @param {DrcsFont} font
 * @returns {HTMLCanvasElement|undefined}
 */
export function createDrcsCanvas(font) {
  const bitmap = createDrcsBitmap(font);
  if (!bitmap) {
    return undefined;
  }

  const canvas = document.createElement("canvas");
  canvas.width = font.width;
  canvas.height = font.height;
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    return undefined;
  }

  const imageData = ctx.createImageData(font.width, font.height);
  for (let i = 0; i < bitmap.length; i++) {
    imageData.data[i * 4 + 0] = 255;
    imageData.data[i * 4 + 1] = 255;
    imageData.data[i * 4 + 2] = 255;
    imageData.data[i * 4 + 3] = bitmap[i] * 255;
  }
  ctx.putImageData(imageData, 0, 0);

  return canvas;
}

class CaptionDrcsFont {
  /**
   * @type {HTMLCanvasElement}
   */
  #canvas;
  /**
   * @type {string | undefined}
   */
  #url;

  /**
   * @param {HTMLCanvasElement} canvas
   */
  constructor(canvas) {
    this.#canvas = canvas;
  }

  /**
   * @type {number}
   */
  get width() {
    return this.#canvas.width;
  }

  /**
   * @type {number}
   */
  get height() {
    return this.#canvas.height;
  }

  /**
   * @type {string}
   */
  get url() {
    if (this.#url === undefined) {
      this.#url = this.#canvas.toDataURL();
    }
    return this.#url;
  }
}

/**
 * DRCS符号。
 */
class CaptionDrcsCode {
  /**
   * @type {CaptionDrcsFont[]}
   */
  fonts = [];

  /**
   * `HTMLCanvasElement`をフォントとして追加する。
   *
   * @param {HTMLCanvasElement} canvas
   */
  add(canvas) {
    this.fonts.push(new CaptionDrcsFont(canvas));
  }

  /**
   * 指定されたフォントサイズと同じ大きさのDRCSフォントを返す。
   *
   * @param {number} width
   * @param {number} height
   * @returns {CaptionDrcsFont | undefined}
   */
  get(width, height) {
    for (const font of this.fonts) {
      if (font.width === width && font.height === height) {
        return font;
      }
    }
    return this.fonts.length > 0 ? this.fonts[this.fonts.length - 1] : undefined;
  }

  /**
   * 符号を消去する。
   */
  clear() {
    this.fonts = [];
  }
}

/**
 * DRCSの符号集合。
 */
class DrcsSets {
  /**
   * @type {Record<DrcsType, Map<number, CaptionDrcsCode>>}
   */
  #sets = {
    drcs0: new Map(),
    drcs1: new Map(),
    drcs2: new Map(),
    drcs3: new Map(),
    drcs4: new Map(),
    drcs5: new Map(),
    drcs6: new Map(),
    drcs7: new Map(),
    drcs8: new Map(),
    drcs9: new Map(),
    drcs10: new Map(),
    drcs11: new Map(),
    drcs12: new Map(),
    drcs13: new Map(),
    drcs14: new Map(),
    drcs15: new Map(),
  };

  /**
   * 符号集合を消去する。
   */
  clear() {
    for (const codes of Object.values(this.#sets)) {
      for (const code of codes.values()) {
        code.clear();
      }
      codes.clear();
    }
  }

  /**
   * DRCSの符号コードから符号を得る。
   *
   * @param {DrcsCharCode} dcc
   * @returns {CaptionDrcsCode}
   */
  get(dcc) {
    const map = this.#sets[dcc.type];
    const key = dcc.type === "drcs0" ? (dcc.code1 << 8) | dcc.code2 : dcc.code;
    let code = map.get(key);
    if (!code) {
      code = new CaptionDrcsCode();
      map.set(key, code);
    }
    return code;
  }
}

/**
 * @typedef {object} SectionConfig 表示区画の設定
 * @property {number} fontWidthFactor  字方向のフォントサイズのための、文字幅に対する因数
 * @property {number} fontHeightFactor 行方向のフォントサイズのための、文字高さに対する因数
 * @property {number} horzSpaceFactor  左右を合わせた字間隔のための、字間隔に対する因数
 * @property {number} vertSpaceFactor  上下を合わせた行間隔のための、行間隔に対する因数
 * @property {number} leftSpaceFactor  左側字間隔のための、字間隔に対する因数
 * @property {number} rightSpaceFactor 右側字間隔のための、字間隔に対する因数
 * @property {number} upperSpaceFactor 上側行間隔のための、行間隔に対する因数
 * @property {number} lowerSpaceFactor 下側行間隔のための、行間隔に対する因数
 */
/**
 * @type {Record<CharSize, SectionConfig>}
 */
const SECTION_CONFIGS = {
  small: {
    fontWidthFactor: 0.5,
    fontHeightFactor: 0.5,
    horzSpaceFactor: 0.5,
    vertSpaceFactor: 0.5,
    leftSpaceFactor: 0.25,
    rightSpaceFactor: 0.25,
    upperSpaceFactor: 0.25,
    lowerSpaceFactor: 0.25,
  },
  medium: {
    fontWidthFactor: 0.5,
    fontHeightFactor: 1,
    horzSpaceFactor: 0.5,
    vertSpaceFactor: 1,
    leftSpaceFactor: 0.25,
    rightSpaceFactor: 0.25,
    upperSpaceFactor: 0.5,
    lowerSpaceFactor: 0.5,
  },
  normal: {
    fontWidthFactor: 1,
    fontHeightFactor: 1,
    horzSpaceFactor: 1,
    vertSpaceFactor: 1,
    leftSpaceFactor: 0.5,
    rightSpaceFactor: 0.5,
    upperSpaceFactor: 0.5,
    lowerSpaceFactor: 0.5,
  },
  "height-w": {
    fontWidthFactor: 1,
    fontHeightFactor: 2,
    horzSpaceFactor: 1,
    vertSpaceFactor: 2,
    leftSpaceFactor: 0.5,
    rightSpaceFactor: 0.5,
    upperSpaceFactor: 1.5,
    lowerSpaceFactor: 0.5,
  },
  "width-w": {
    fontWidthFactor: 2,
    fontHeightFactor: 1,
    horzSpaceFactor: 2,
    vertSpaceFactor: 1,
    leftSpaceFactor: 1,
    rightSpaceFactor: 1,
    upperSpaceFactor: 0.5,
    lowerSpaceFactor: 0.5,
  },
  "size-w": {
    fontWidthFactor: 2,
    fontHeightFactor: 2,
    horzSpaceFactor: 2,
    vertSpaceFactor: 2,
    leftSpaceFactor: 1,
    rightSpaceFactor: 1,
    upperSpaceFactor: 1.5,
    lowerSpaceFactor: 0.5,
  },
};

/**
 * カラーマップアドレスからCSSの変数参照を生成する。
 *
 * @param {number} index
 */
function captionColor(index) {
  return `var(--caption-color-${index})`;
}

/**
 * 囲み制御・アンダーライン制御用に`<polygon>`の点集合を1つ以上生成する。
 *
 * @param {number} hlc
 * @param {boolean} underline
 * @param {number} posX
 * @param {number} posY
 * @param {number} cw
 * @param {number} ch
 * @param {number} width
 * @returns {string[]}
 */
function highlightPolygons(hlc, underline, posX, posY, cw, ch, width) {
  if (underline) {
    hlc |= 0b0001;
  }

  const l = posX;
  const lw = l + width;
  const t = posY - ch;
  const tw = t + width;
  const r = posX + cw;
  const rw = r - width;
  const b = posY;
  const bw = b - width;
  switch (hlc) {
    // 下
    case 0b0001: return [`${l},${b} ${r},${b} ${r},${bw} ${l},${bw}`];
    // 右
    case 0b0010: return [`${r},${t} ${r},${b} ${rw},${b} ${rw},${t} ${r},${t}`];
    // ┛
    case 0b0011: return [`${r},${t} ${r},${b} ${l},${b} ${l},${bw} ${rw},${bw} ${rw},${t} ${r},${t}`];
    // 上
    case 0b0100: return [`${l},${t} ${r},${t} ${r},${tw} ${l},${tw} ${l},${t}`];
    // 上と下
    case 0b0101: return [`${l},${t} ${r},${t} ${r},${tw} ${l},${tw} ${l},${t}`,
                         `${l},${b} ${r},${b} ${r},${bw} ${l},${bw}`];
    // ┓
    case 0b0110: return [`${l},${t} ${r},${t} ${r},${b} ${rw},${b} ${rw},${tw} ${l},${tw} ${l},${t}`];
    // コ
    case 0b0111: return [`${l},${t} ${r},${t} ${r},${b} ${l},${b} ${l},${bw} ${rw},${bw} ${rw},${tw} ${l},${tw} ${l},${t}`];
    // 左
    case 0b1000: return [`${l},${t} ${l},${b} ${lw},${b} ${lw},${t} ${l},${t}`];
    // ┗
    case 0b1001: return [`${l},${t} ${l},${b} ${r},${b} ${r},${bw} ${lw},${bw} ${lw},${t} ${l},${t}`];
    // 左と右
    case 0b1010: return [`${l},${t} ${l},${b} ${lw},${b} ${lw},${t} ${l},${t}`,
                         `${r},${t} ${r},${b} ${rw},${b} ${rw},${t} ${r},${t}`];
    // 凵
    case 0b1011: return [`${l},${t} ${l},${b} ${r},${b} ${r},${t} ${rw},${t} ${rw},${bw} ${lw},${bw} ${lw},${t} ${l},${t}`];
    // ┏
    case 0b1100: return [`${l},${b} ${l},${t} ${r},${t} ${r},${tw} ${lw},${tw} ${lw},${b} ${l},${b}`];
    // 匚
    case 0b1101: return [`${r},${t} ${l},${t} ${l},${b} ${r},${b} ${r},${bw} ${lw},${bw} ${lw},${tw} ${r},${tw} ${r},${t}`];
    // 冂
    case 0b1110: return [`${l},${b} ${l},${t} ${r},${t} ${r},${b} ${rw},${b} ${rw},${tw} ${lw},${tw} ${lw},${b} ${l},${b}`];
    // 囗
    case 0b1111: return [`${r},${t} ${r},${b} ${l},${b} ${l},${bw} ${rw},${bw} ${rw},${t} ${r},${t}`,
                         `${l},${b} ${l},${t} ${r},${t} ${r},${tw} ${lw},${tw} ${lw},${b} ${l},${b}`];
    default: throw new Error("不正なHLC");
  }
}

/**
 * 字幕・文字スーパーで別々に使用されるSVG生成器。
 */
class Renderer {
  /**
   * 2番目の言語を使うかどうか。
   *
   * @type {boolean}
   */
  useSubLang = false;

  /**
   * 描画対象のサービスがワンセグかどうか。
   *
   * @type {boolean}
   */
  isOneseg = false;

  /**
   * @type {Array<{ pos: number; caption: Caption; }>}
   */
  #pending = [];

  /**
   * @type {SVGSVGElement}
   */
  #svg;
  /**
   * @type {SVGGElement}
   */
  #bg;
  /**
   * @type {SVGGElement}
   */
  #fg;

  /**
   * 最後に字幕管理データを受信した再生位置。
   *
   * @type {number | undefined}
   */
  #lastMdPos;
  /**
   * 字幕管理データで指定される表示書式。
   *
   * @type {ExtendedDisplayFormat | undefined}
   */
  #displayFormat;
  /**
   * 字幕管理データで指定される表示モード。
   *
   * @type {DisplayMode | undefined}
   */
  #displayMode;
  /**
   * 字幕管理データで指定されるデータグループ。
   *
   * @type {"A" | "B" | undefined}
   */
  #dataGroup;
  /**
   * 字幕管理データで指定される言語識別。
   *
   * @type {number | undefined}
   */
  #languageTag;

  #drcsSets = new DrcsSets();

  /**
   * @param {SVGSVGElement} svg
   * @param {SVGGElement} bg
   * @param {SVGGElement} fg
   */
  constructor(svg, bg, fg) {
    this.#svg = svg;
    this.#bg = bg;
    this.#fg = fg;
  }

  /**
   * 管理しているすべてのデータを消去し、オブジェクトを初期化する。
   */
  resetAll() {
    this.reset();
    this.#pending = [];
  }

  /**
   * 字幕および字幕管理用データをすべて消去する。
   */
  reset() {
    if (this.isOneseg) {
      // ワンセグでは管理データがあまり来ないので初期値を与えておく
      this.#displayFormat = "profile-c"; // 固定
      this.#displayMode = "selectable"; // 固定
      this.#dataGroup = "A";
      this.#languageTag = 0;
      this.#svg.setAttribute("caption-display-format", this.#displayFormat);
      this.#svg.setAttribute("caption-display-mode", this.#displayMode);
    } else {
      this.#displayFormat = undefined;
      this.#displayMode = undefined;
      this.#dataGroup = undefined;
      this.#languageTag = undefined;
    }

    this.#lastMdPos = undefined;
    this.#drcsSets.clear();
    this.#svg.removeAttribute("viewBox");
    this.clear();
  }

  /**
   * SVGの内容を消去する。
   */
  clear() {
    this.#bg.replaceChildren();
    this.#fg.replaceChildren();
  }

  /**
   * 保留中の字幕から過去のものや未来のものを削除する。
   *
   * @param {number} currentTime
   */
  validate(currentTime) {
    this.#pending = this.#pending.filter(({ pos }) => {
      const diff = pos - currentTime;
      return diff <= DT_PAST || diff >= DT_FUTURE;
    });
  }

  /**
   * 保留中の字幕を処理する。
   *
   * @param {number} currentTime
   */
  tick(currentTime) {
    if (this.#pending.length === 0) {
      this.#checkExpiration(currentTime);
      return;
    }

    while (this.#pending.length > 0 && this.#pending[0].pos <= currentTime) {
      const { pos, caption } = /** @type {{ pos: number; caption: Caption }} */(this.#pending.shift());
      this.render(pos, caption);
    }
  }

  /**
   * 字幕表示を保留。
   *
   * @param {number} pos
   * @param {Caption} caption
   */
  defer(pos, caption) {
    this.#pending.push({ pos: pos, caption: caption });
    this.#pending.sort((a, b) => a.pos - b.pos);
  }

  /**
   * 字幕データ`caption`を処理してSVGに描画する。
   *
   * @param {number} pos
   * @param {Caption} caption
   */
  render(pos, caption) {
    this.#checkExpiration(pos);

    switch (caption.type) {
      case "management-data": {
        // rollupModeには対応しない
        this.#lastMdPos = pos;

        if (this.#dataGroup !== caption.group) {
          this.reset();
          this.#dataGroup = caption.group;
        }

        const langIndex = !this.useSubLang || caption.languages.length < 2 ? 0 : 1;
        const lang = caption.languages[langIndex];
        this.#languageTag = lang.languageTag;

        // ワンセグでは固定値または運用されない
        if (!this.isOneseg && lang.format !== null) {
          this.#displayFormat = lang.format;
          // TODO: リアルタイム視聴時はdmfRecv
          this.#displayMode = lang.dmfPlayback;
          this.#svg.setAttribute("caption-display-format", this.#displayFormat);
          this.#svg.setAttribute("caption-display-mode", this.#displayMode);

          this.#processDataUnits(pos, caption.dataUnits);
        }
        break;
      }

      case "data":
        if (caption.group === this.#dataGroup && caption.languageTag === this.#languageTag) {
          this.#processDataUnits(pos, caption.dataUnits);
        }
        break;

      case "postponed":
        // wait-for-process（TIME）で処理を待たされていた字幕文
        this.#processStatement(pos, caption.postponed);
        break;
    }
  }

  /**
   * 字幕管理データの再生時間を確認し、一定時間データが来ていなければリセットする。
   *
   * @param {number} pos
   */
  #checkExpiration(pos) {
    if (this.#lastMdPos === undefined) {
      return;
    }

    if (pos < this.#lastMdPos || pos >= this.#lastMdPos + 3 * 60 * 1000) {
      // 巻き戻し、または最後の字幕管理データから3分以上経過したので初期化
      this.reset();
    }
  }

  /**
   * @param {number} pos
   * @param {CaptionDataUnit[]} dataUnits
   */
  #processDataUnits(pos, dataUnits) {
    for (const unit of dataUnits) {
      switch (unit.type) {
        case "drcs":
          // TODO: TTF化
          for (const code of unit.drcs) {
            const drcsCode = this.#drcsSets.get(code.characterCode);
            drcsCode.clear();

            for (const font of code.fonts) {
              // TTF化で直るので今のところは前景色を無視する
              const canvas = createDrcsCanvas(font);
              if (canvas) {
                drcsCode.add(canvas);
              }
            }
          }
          break;

        case "bitmap":
          // TODO: TSを手に入れたら実装
          break;

        case "statement-body":
          this.#processStatement(pos, unit.statement);
          break;
      }
    }
  }

  /**
   * @param {number} pos
   * @param {AribChar[]} statement
   */
  #processStatement(pos, statement) {
    if (!this.#displayFormat) {
      // 管理データ到着前は何もしない
      return;
    }

    /** 水平方向の動作位置。 */
    let posX = 0;
    /** 垂直方向の動作位置。 */
    let posY = 0;
    /**
     * RPCで指定される文字繰り返し。
     * @type {number | undefined}
     */
    let repeatCharacter = undefined;

    /** NSZ・SZX等で設定された文字サイズの、表示区画の設定。 */
    let sectionConfig = SECTION_CONFIGS.normal;
    /** COLで指定されるパレット番号。 */
    let paletteIndex = 0;
    /** BKFやCOLなどで指定される前景色。 */
    let foregroundColorValue = "";
    /** COLで指定される背景色。 */
    let backgroundColorValue = "";
    /**
     * ORNで指定される縁取りの色。
     * @type {string | undefined}
     */
    let hemmingColorValue = undefined;
    /**
     * FLCで指定されるフラッシング方法。
     * @type {"none" | "normal" | "inverted"}
     */
    let flushingMode = "none";
    /** STL・SPLで指定されるアンダーライン制御 */
    let underline = false;
    /** HLCで指定される囲み制御 */
    let highlightBlock = 0;
    /**
     * POLで指定される極性制御。
     * @type {"normal" | "inverted-1"}
     * */
    let polarity = "normal";
    /** SDFで指定される表示領域の横幅。 */
    let displayWidth = 0;
    /** SDFで指定される表示領域の高さ。 */
    let displayHeight = 0;
    /** SDPで指定される水平方向の表示位置。 */
    let displayLeft = 0;
    /** SDPで指定される垂直方向の表示位置。 */
    let displayTop = 0;
    /** SSMで指定される文字幅。 */
    let charCompWidth = 0;
    /** SSMで指定される文字高さ。 */
    let charCompHeight = 0;
    /** SHSで指定される字間隔。 */
    let horizontalSpacing = 0;
    /** SVSで指定される行間隔。 */
    let verticalSpacing = 0;
    /** 行末まで進んだ事による動作行前進が発生したか。 */
    let wrapped = false;

    /** 前景色。 */
    const foregroundColor = () => {
      return polarity === "normal" ? foregroundColorValue : backgroundColorValue;
    };
    /** 背景色。 */
    const backgroundColor = () => {
      return polarity === "normal" ? backgroundColorValue : foregroundColorValue;
    };
    /** 縁取りの色。ORNがない場合は背景色で縁取る。 */
    const hemmingColor = () => {
      return hemmingColorValue ?? backgroundColor();
    };
    /** 字方向のフォントサイズ。 */
    const fontWidth = () => charCompWidth * sectionConfig.fontWidthFactor | 0;
    /** 行方向のフォントサイズ。 */
    const fontHeight = () => charCompHeight * sectionConfig.fontHeightFactor | 0;
    /** 左右を合わせた字間隔。 */
    const horzSpace = () => horizontalSpacing * sectionConfig.horzSpaceFactor | 0;
    /** 左側の字間隔。 */
    const leftSpace = () => horizontalSpacing * sectionConfig.leftSpaceFactor | 0;
    /** 上下を合わせた行間隔。 */
    const vertSpace = () => verticalSpacing * sectionConfig.vertSpaceFactor | 0;
    /** 下側の行間隔。 */
    const lowerSpace = () => verticalSpacing * sectionConfig.lowerSpaceFactor | 0;
    /** 表示区画字方向サイズ。 */
    const charWidth = () => fontWidth() + horzSpace();
    /** 表示区画行方向サイズ。 */
    const charHeight = () => fontHeight() + vertSpace();
    /** 動作位置を1文字進ませる。 */
    const forwardChar = () => {
      posX += charWidth();
      if (posX >= displayLeft + displayWidth) {
        posY += charHeight();
        posX = displayLeft;
        wrapped = true;
      } else {
        wrapped = false;
      }
    };

    /**
     * @param {ExtendedDisplayFormat} displayFormat
     */
    const reset = displayFormat => {
      repeatCharacter = undefined;

      sectionConfig = SECTION_CONFIGS.normal;
      paletteIndex = 0;
      foregroundColorValue = captionColor(7);
      backgroundColorValue = captionColor(8);
      hemmingColorValue = undefined;
      flushingMode = "none";
      underline = false;
      highlightBlock = 0;
      polarity = "normal";
      displayLeft = 0;
      displayTop = 0;
      charCompWidth = 36;
      charCompHeight = 36;
      wrapped = false;

      switch (displayFormat) {
        case "qhd-horz":
          this.#svg.setAttribute("viewBox", `0 0 960 540`);
          displayWidth = 960;
          displayHeight = 540;
          horizontalSpacing = 4;
          verticalSpacing = 24;
          posX = displayLeft;
          posY = displayTop + charHeight();
          break;

        case "qhd-vert":
          this.#svg.setAttribute("viewBox", `0 0 960 540`);
          displayWidth = 960;
          displayHeight = 540;
          horizontalSpacing = 12;
          verticalSpacing = 24;
          posX = displayLeft + displayWidth - charWidth();
          posY = displayTop + charHeight();
          break;

        case "sd-horz":
          this.#svg.setAttribute("viewBox", `0 0 960 480`);
          displayWidth = 720;
          displayHeight = 480;
          horizontalSpacing = 4;
          verticalSpacing = 16;
          posX = displayLeft;
          posY = displayTop + charHeight();
          break;

        case "sd-vert":
          this.#svg.setAttribute("viewBox", `0 0 720 480`);
          displayWidth = 720;
          displayHeight = 480;
          horizontalSpacing = 8;
          verticalSpacing = 24;
          posX = displayLeft + displayWidth - charWidth();
          posY = displayTop + charHeight();
          break;

        case "profile-c":
          // 表示区画を20x24とし、16文字×3行の表示領域を確保する
          // ただし文字がはみ出すことがあるので右側に余裕を設ける
          // 参考：https://github.com/xtne6f/TVCaptionMod2/blob/710ed28f0fc19e7d88ad867f863c863b8ee1bcf2/Caption_src/ARIB8CharDecode.cpp#L157-L164
          this.#svg.setAttribute("viewBox", `0 0 330 180`);
          displayWidth = 320;
          displayHeight = 180;
          charCompWidth = 18;
          charCompHeight = 18;
          horizontalSpacing = 2;
          verticalSpacing = 6;
          posX = displayLeft;
          // 下三行で表示
          posY = displayTop + displayHeight - charHeight() * 2;
          // 背景もORNも運用されないので勝手に縁取り
          hemmingColorValue = captionColor(0);
          break;
      }
    };
    reset(this.#displayFormat);

    /**
     * 背景用矩形を追加する。
     * @param {string} [forcedColor]
     * */
    const addBackground = forcedColor => {
      const cw = charWidth();
      const ch = charHeight();

      const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
      rect.setAttribute("x", posX.toString());
      rect.setAttribute("y", (posY - ch).toString());
      rect.setAttribute("width", cw.toString());
      rect.setAttribute("height", ch.toString());
      rect.style.fill = forcedColor ?? backgroundColor();
      this.#bg.append(rect);

      // 囲み制御・アンダーライン制御
      if (highlightBlock !== 0 || underline) {
        // 表示区画内に収めるためpolygonで線の領域を指示する
        for (const points of highlightPolygons(highlightBlock, underline, posX, posY, cw, ch, 1)) {
          const polygon = document.createElementNS("http://www.w3.org/2000/svg", "polygon");
          polygon.setAttribute("points", points);
          polygon.style.fill = foregroundColor();
          this.#bg.append(polygon);
        }
      }
    };
    /**
     * 文字用要素を追加する。
     * @param {string} c
     */
    const addChar = c => {
      const widthFactor = sectionConfig.fontWidthFactor;
      const heightFactor = sectionConfig.fontHeightFactor;
      const x = posX + leftSpace();
      const y = posY - lowerSpace() - fontHeight();

      const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
      text.setAttribute("x", "0");
      text.setAttribute("y", "0");
      text.setAttribute("transform", `translate(${x} ${y}) scale(${widthFactor} ${heightFactor})`);
      text.setAttribute("font-size", charCompWidth.toString());
      text.setAttribute("caption-flushing", flushingMode);
      text.style.fill = foregroundColor();
      text.style.stroke = hemmingColor();
      text.textContent = c;
      this.#fg.append(text);
    };
    /**
     * DRCSを1文字追加する。
     * @param {CaptionDrcsFont} font
     */
    const addDrcs = font => {
      const fw = fontWidth();
      const fh = fontHeight();

      const image = document.createElementNS("http://www.w3.org/2000/svg", "image");
      image.setAttribute("href", font.url);
      image.setAttribute("x", (posX + leftSpace()).toString());
      image.setAttribute("y", (posY - lowerSpace() - fh).toString());
      image.setAttribute("width", fw.toString());
      image.setAttribute("height", fh.toString());
      image.setAttribute("caption-flushing", flushingMode);
      this.#fg.append(image);
    };

    for (let i = 0; i < statement.length; i++) {
      const char = statement[i];

      switch (char.type) {
        case "char-size":
          sectionConfig = SECTION_CONFIGS[char.charSize];
          break;

        case "string": {
          let string = char.string;

          if (repeatCharacter !== undefined) {
            const c = string[0];
            string = string.slice(1);

            if (repeatCharacter === 0) {
              // 動作行前進が発生するまで文字追加
              while (!wrapped) {
                addChar(c);
                addBackground();
                forwardChar();
              }
            } else {
              for (let j = 0; j < repeatCharacter; j++)  {
                addChar(c);
                addBackground();
                forwardChar();
              }
            }

            repeatCharacter = undefined;
          }

          for (const c of string) {
            addChar(c);
            addBackground();
            forwardChar();
          }
          break;
      }

        case "drcs0":
        case "drcs1":
        case "drcs2":
        case "drcs3":
        case "drcs4":
        case "drcs5":
        case "drcs6":
        case "drcs7":
        case "drcs8":
        case "drcs9":
        case "drcs10":
        case "drcs11":
        case "drcs12":
        case "drcs13":
        case "drcs14":
        case "drcs15": {
          const font = this.#drcsSets.get(char).get(fontWidth(), fontHeight());
          if (repeatCharacter === 0) {
            // 動作行前進が発生するまで文字追加
            while (!wrapped) {
              if (font) {
                addDrcs(font);
              }
              addBackground();
              forwardChar();
            }
          } else {
            const count = repeatCharacter ?? 1;
            for (let j = 0; j < count; j++) {
              if (font) {
                addDrcs(font);
              }
              addBackground();
              forwardChar();
            }
          }

          repeatCharacter = undefined;
          break;
        }

        // NUL
        case "null":
          // 無視
          break;

        // APB
        case "active-position-backward":
          wrapped = false;
          posX -= charWidth();
          if (posX < displayLeft) {
            posY -= charHeight();
            posX = displayLeft + displayWidth - charWidth();
          }
          break;

        // APF
        case "active-position-forward":
          forwardChar();
          break;

        // PAPF
        case "parameterized-active-position-forward":
          for (let i = 0; i < char.p1; i++) {
            forwardChar();
          }
          break;

        // APD
        case "active-position-down":
          wrapped = false;
          posY += charHeight();
          if (posY >= displayTop + displayHeight) {
            posY = displayTop;
          }
          break;

        // APU
        case "active-position-up":
          wrapped = false;
          posY -= charHeight();
          if (posY < displayTop) {
            posY = displayTop + displayHeight - charHeight();
          }
          break;

        // APR
        case "active-position-return":
          // 折り返し直後のAPRには反応しない
          if (!wrapped) {
            posX = displayLeft;
            posY += charHeight();
          }
          break;

        // APS
        case "active-position-set":
          wrapped = false;
          posX = displayLeft + char.p2 * charWidth();
          posY = displayTop + (char.p1 + 1) * charHeight();
          break;

        // CSIのACPS
        case "active-coordinate-position-set":
          wrapped = false;
          posX = char.p1;
          posY = char.p2;
          break;

        // CS
        case "clear-screen":
          this.clear();
          reset(this.#displayFormat);
          break;

        // US
        case "unit-separator":
          // 無視
          break;

        // SP / DEL
        case "space":
        case "delete": {
          const color = char.type === "space" ? backgroundColor() : foregroundColor();

          if (repeatCharacter === 0) {
            while (!wrapped) {
              addBackground(color);
              forwardChar();
            }
          } else {
            const count = repeatCharacter ?? 1;
            for (let j = 0; j < count; j++) {
              addBackground(color);
              forwardChar();
            }
          }

          repeatCharacter = undefined;
          break;
        }

        // COLほか
        case "color-foreground":
          foregroundColorValue = captionColor((paletteIndex << 4) | char.p1);
          break;

        // COL
        case "color-background":
          backgroundColorValue = captionColor((paletteIndex << 4) | char.p1);
          break;

        // COL
        case "color-half-foreground":
          // 前中間色には対応しない
          break;

        // COL
        case "color-half-background":
          // 背中間色には対応しない
          break;

        // COL
        case "color-palette":
          paletteIndex = char.p1;
          break;

        // POL
        case "pattern-polarity-normal":
          polarity = "normal";
          break;

        // POL
        case "pattern-polarity-inverted-1":
          polarity = "inverted-1";
          break;

        // FLC
        case "flushing-control-start-normal":
          flushingMode = "normal";
          break;

        // FLC
        case "flushing-control-start-inverted":
          flushingMode = "inverted";
          break;

        // FLC
        case "flushing-control-stop":
          flushingMode = "none";
          break;

        // TIME
        case "wait-for-process": {
          this.defer(
            pos + char.p1 / 10,
            {
              type: "postponed",
              postponed: statement.slice(i + 1),
            },
          );
          return;
        }

        // RPC
        case "repeat-character":
          repeatCharacter = char.p1;
          break;

        // SPL
        case "stop-lining":
          underline = false;
          break;

        // STL
        case "start-lining":
          underline = true;
          break;

        // HLC
        case "highlight-block":
          highlightBlock = char.p1;
          break;

        // CSIのSWF
        case "set-writing-format-init":
          switch (char.p1) {
            case 7:
              reset("qhd-horz");
              break;

            case 8:
              reset("qhd-vert");
              break;

            case 9:
              reset("sd-horz");
              break;

            case 10:
              reset("sd-vert");
              break;

            default:
              reset(this.#displayFormat);
              break;
          }
          break;

        // CSIのRCS
        case "raster-color-command":
          // ラスタ色制御には対応しない
          break;

        // CSIのSDF
        case "set-display-format":
          displayWidth = char.p1;
          displayHeight = char.p2;
          break;

        // CSIのSDP
        case "set-display-position":
          displayLeft = char.p1;
          displayTop = char.p2;
          break;

        // CSIのSSM
        case "character-composition-dot-designation":
          charCompWidth = char.p1;
          charCompHeight = char.p2;
          break;

        // CSIのSHS
        case "set-horizontal-spacing":
          horizontalSpacing = char.p1;
          break;

        // CSIのSVS
        case "set-vertical-spacing":
          verticalSpacing = char.p1;
          break;

        // CSIのORN
        case "ornament-control-clear":
          hemmingColorValue = undefined;
          break;

        // CSIのORN
        case "ornament-control-hemming":
          hemmingColorValue = captionColor(char.p1);
          break;

        // CSIのPRA
        case "builtin-sound-replay":
          // TODO: 内蔵音再生
          break;

        // CSIのSCR
        case "scroll-designation":
          // TODO: スクロール指定
          break;

        default:
          console.error("不明な字幕文字", char);
          break;
      }
    }
  }
}

/**
 * 字幕・文字スーパーを表示する領域。
 */
export class Prompter extends HTMLElement {
  static register() {
    customElements.define("tavoo-prompter", Prompter);
  }

  static get observedAttributes() {
    return ["display"];
  }

  /**
   * @type {"auto" | "always"}
   */
  #display = "auto";

  /**
   * @type {number | undefined}
   */
  #raf = undefined;

  /**
   * @type {SVGSVGElement}
   */
  #root;

  /**
   * @type {Renderer}
   */
  #rendererCaption;
  /**
   * @type {Renderer}
   */
  #rendererSuperimpose;

  constructor() {
    super();

    const parser = new DOMParser();
    const doc = parser.parseFromString(`
      <root xmlns="http://www.w3.org/1999/xhtml">
        <link rel="stylesheet" href="tavoo://player/skin/caption.css" />
        <svg id="root" xmlns="http://www.w3.org/2000/svg" caption-display="${this.#display}">
          <svg id="caption" class="screen">
            <g class="background"></g>
            <g class="foreground"></g>
          </svg>
          <svg id="superimpose" class="screen">
            <g class="background"></g>
            <g class="foreground"></g>
          </svg>
        </svg>
      </root>
    `, "application/xml");

    const shadow = this.attachShadow({ mode: "open" });
    shadow.append(...doc.documentElement.children);

    this.#root = /** @type {SVGSVGElement} */(shadow.querySelector("#root"));

    this.#rendererCaption = new Renderer(
      /** @type {SVGSVGElement} */(shadow.querySelector("#caption")),
      /** @type {SVGGElement} */(shadow.querySelector("#caption > .background")),
      /** @type {SVGGElement} */(shadow.querySelector("#caption > .foreground")),
    );
    this.#rendererSuperimpose = new Renderer(
      /** @type {SVGSVGElement} */(shadow.querySelector("#superimpose")),
      /** @type {SVGGElement} */(shadow.querySelector("#superimpose > .background")),
      /** @type {SVGGElement} */(shadow.querySelector("#superimpose > .foreground")),
    );
  }

  /**
   * 字幕の表示方法。
   *
   * `"auto"`では字幕データ次第で自動表示され、`"always"`では全字幕を表示する。
   *
   * @type {"auto" | "always"}
   */
  get display() {
    return this.#display;
  }

  set display(value) {
    this.#display = value === "always" ? "always" : "auto";
    this.#root.setAttribute("caption-display", this.#display);
  }

  connectedCallback() {
    gController.addEventListener("caption", this);
    gController.addEventListener("superimpose", this);
    gController.addEventListener("source", this);
    gController.addEventListener("service-changed", this);
    gController.addEventListener("state", this);
    gController.addEventListener("seek-completed", this);
  }

  disconnectedCallback() {
    gController.removeEventListener("caption", this);
    gController.removeEventListener("superimpose", this);
    gController.removeEventListener("source", this);
    gController.removeEventListener("service-changed", this);
    gController.removeEventListener("state", this);
    gController.removeEventListener("seek-completed", this);
  }

  /**
   * @param {"display"} name
   * @param {string | null} _oldValue
   * @param {string | null} newValue
   */
  attributeChangedCallback(name, _oldValue, newValue) {
    switch (name) {
      case "display":
        this.display = /** @type {any} */(newValue);
        break;
    }
  }

  /**
   * @param {Event & ({
   *   type: "caption" | "superimpose";
   *   pos: number | null;
   *   caption: Caption;
   * } | {
   *   type: "source" | "service-changed" | "state" | "seek-completed";
   * })} e
   */
  handleEvent(e) {
    switch (e.type) {
      case "caption": {
        if (e.pos === null) {
          // PTSのない字幕は無視
          return;
        }

        if (gController.state === "playing") {
          if (e.pos - gController.currentTime <= 0) {
            // 過去の字幕はすぐに描画
            this.#rendererCaption.render(e.pos, e.caption);
            return;
          }
        } else {
          if (e.pos === null) {
            // 再生中でない（＝シーク中の）場合におけるPTSのない字幕は無視
            return;
          }
        }

        this.#rendererCaption.defer(e.pos, e.caption);
        break;
      }

      case "superimpose":
        // 文字スーパーは現在の再生位置のものとしてすぐに描画
        this.#rendererSuperimpose.render(gController.currentTime, e.caption);
        break;

      case "source":
        this.#rendererCaption.isOneseg = false;
        this.#rendererSuperimpose.isOneseg = false;

        this.#rendererCaption.resetAll();
        this.#rendererSuperimpose.resetAll();
        break;

      case "service-changed":
        this.#rendererCaption.isOneseg = gController.currentService?.isOneseg ?? false;
        this.#rendererSuperimpose.isOneseg = gController.currentService?.isOneseg ?? false;

        this.#rendererCaption.resetAll();
        this.#rendererSuperimpose.resetAll();
        break;

      case "state":
        if (this.#raf !== undefined) {
          cancelAnimationFrame(this.#raf);
          this.#raf = undefined;
        }

        switch (gController.state) {
          case "playing":
            this.#raf = requestAnimationFrame(() => this.#onAnimationFrame());
            break;

          case "stopped":
            this.#rendererCaption.resetAll();
            this.#rendererSuperimpose.resetAll();
            break;
        }
        break;

      case "seek-completed": {
        // 保留中の字幕から過去のものや未来のものを削除した上で処理
        const currentTime = gController.currentTime;

        this.#rendererCaption.validate(currentTime);
        this.#rendererCaption.tick(currentTime);

        this.#rendererSuperimpose.validate(currentTime);
        this.#rendererSuperimpose.tick(currentTime);
        break;
      }
    }
  }

  #onAnimationFrame() {
    this.#raf = requestAnimationFrame(() => this.#onAnimationFrame());

    const currentTime = gController.currentTime;
    this.#rendererCaption.tick(currentTime);
    this.#rendererSuperimpose.tick(currentTime);
  }
}
