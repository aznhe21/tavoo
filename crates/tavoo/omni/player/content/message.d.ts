// mod.rs

export type PlaybackState = "open-pending" | "playing" | "paused" | "stopped" | "closed";
export type DualMonoMode = "left" | "right" | "stereo" | "mix";

/** ホストからの通知。 */
export type Notification = {
  /** ファイルが開かれた、または閉じられた。 */
  notification: "source";
  /** 開かれたファイルへのパスだが、ファイルが閉じられた場合は`null`。 */
  path: string | null;
} | {
  /** 音量。 */
  notification: "volume";
  volume: number;
  muted: boolean;
} | {
  /** 再生速度の範囲。 */
  notification: "rate-range";
  slowest: number;
  fastest: number;
} | {
  /** 動画の長さ。 */
  notification: "duration";
  /**
   * 秒単位の長さ。
   *
   * 再生していない状態やリアルタイム視聴などで長さが不明な場合は`null`となる。
   */
  duration: number | null;
} | {
  /** 再生状態が更新された。 */
  notification: "state";
  state: PlaybackState;
} | {
  /** 再生位置が更新された。 */
  notification: "position";
  position: number;
} | {
  /** すべてのシークが完了した。 */
  notification: "seek-completed";
} | {
  /** 再生速度が更新された。 */
  notification: "rate";
  rate: number;
} | {
  /** 映像の解像度が変更された。 */
  notification: "video-size";
  width: number;
  height: number;
} | {
  /** 音声のチャンネル数が変更された。 */
  notification: "audio-channels";
  numChannels: number;
} | {
  /** デュアルモノラルの再生方法が更新された。 */
  notification: "dual-mono-mode";
  mode: DualMonoMode | null;
} | {
  /** ストリームの切り替えが開始した。 */
  notification: "switching-started";
} | {
  /** ストリームの切り替えが終了した。 */
  notification: "switching-ended";
} | {
  /** 全サービスが更新された。 */
  notification: "services";
  services: Service[];
} | {
  /** 特定のサービスが更新された。 */
  notification: "service";
  service: Service;
} | {
  /** サービスのイベント情報が更新された。 */
  notification: "event";
  serviceId: number;
  isPresent: boolean;
  event: TvEvent;
} | {
  /** サービスが選択し直された。 */
  notification: "service-changed";
  newServiceId: number;
  videoComponentTag: number;
  audioComponentTag: number;
} | {
  /** ストリームが変更された。 */
  notification: "stream-changed";
  videoComponentTag: number;
  audioComponentTag: number;
} | {
  /** 字幕。 */
  notification: "caption";
  /** 字幕を表示すべき再生位置。 */
  pos: number;
  /** 字幕データ。 */
  caption: Caption;
} | {
  /** 文字スーパー。 */
  notification: "superimpose";
  /** 字幕を表示すべき再生位置。 */
  pos: number;
  /** 文字スーパーのデータ。 */
  caption: Caption;
} | {
  /** TSの日付時刻。 */
  notification: "timestamp";
  timestamp: Timestamp;
} | {
  /** エラーが発生した。 */
  notification: "error";
  message: string;
};

/** ホストへの要求。 */
export type Command = {
  /** 開発者ツールを開く。 */
  command: "open-dev-tools";
} | {
  /**
   * 映像の位置を変更。
   *
   * 各値は相対値として`0.0`～`1.0`で指定する。
   */
  command: "set-video-bounds";
  left: number;
  top: number;
  right: number;
  bottom: number;
} | {
  /** 再生。 */
  command: "play";
} | {
  /** 一時停止。 */
  command: "pause";
} | {
  /** 停止。 */
  command: "stop";
} | {
  /** 再生終了。 */
  command: "close";
} | {
  /** 再生位置の変更。 */
  command: "set-position";
  position: number;
} | {
  /** 音量の変更。 */
  command: "set-volume";
  volume: number;
} | {
  /** ミュート状態の変更。 */
  command: "set-muted";
  muted: boolean;
} | {
  /** 再生速度の変更。 */
  command: "set-rate";
  rate: number;
} | {
  /** デュアルモノラルの再生方法の変更。 */
  command: "set-dual-mono-mode";
  mode: DualMonoMode;
} | {
  /** サービスの選択。 */
  command: "select-service";
  /** `null`や`0`の場合は既定のサービスが選択される。 */
  serviceId: number | null;
} | {
  /** 映像ストリームの選択。 */
  command: "select-video-stream";
  componentTag: number;
} | {
  /** 音声ストリームの選択。 */
  command: "select-audio-stream";
  componentTag: number;
};

// bin.rs

/** Base64でシリアライズされたバイナリデータ。 */
export type Binary = string;

// caption.rs

export type DrcsData = {
  depth: number;
  width: number;
  height: number;
  patternData: Binary;
};

export type DrcsType = "drcs0" | "drcs1" | "drcs2" | "drcs3" | "drcs4" | "drcs5" | "drcs6" | "drcs7" | "drcs8" |
  "drcs9" | "drcs10" | "drcs11" | "drcs12" | "drcs13" | "drcs14" | "drcs15";

export type DrcsCharCode = {
  type: "drcs0";
  code1: number;
  code2: number;
} | {
  type: Exclude<DrcsType, "drcs0">;
  code: number;
};

export type DrcsCode = {
  characterCode: DrcsCharCode;
  fonts: DrcsData[];
};

export type Drcs = DrcsCode[];

export type Bitmap = {
  xPosition: number;
  yPosition: number;
  colorIndices: Binary;
  pngData: Binary;
};

export type CaptionDataUnit = {
  type: "statement-body";
  statement: AribString;
} | {
  type: "drcs";
  drcs: Drcs;
} | {
  type: "bitmap";
  bitmap: Bitmap;
};

export type TimeControlMode = "free" | "real-time";

export type DisplayMode = "auto-display" | "selectable";

export type CaptionFormat = "qhd-horz" | "qhd-vert" | "sd-horz" | "sd-vert";

export type CaptionRollupMode = "non-rollup" | "rollup" | "reserved";

export type CaptionLanguage = {
  languageTag: number;
  dmfRecv: DisplayMode;
  dmfPlayback: DisplayMode;
  format: CaptionFormat | null;
  langCode: string;
  rollupMode: CaptionRollupMode;
};

export type CaptionGroup = "A" | "B";

export type Caption = {
  type: "management-data";
  group: CaptionGroup;
  tmd: TimeControlMode;
  languages: CaptionLanguage[];
  dataUnits: CaptionDataUnit[];
} | {
  type: "data";
  group: CaptionGroup;
  languageTag: number;
  tmd: TimeControlMode;
  stm: number | null;
  dataUnits: CaptionDataUnit[];
};

// service.rs

export type Stream = {
  streamType: number;
  componentTag: number | null;
};

export type ExtendedEventItem = {
  item: string;
  description: string;
};

export type VideoComponent = {
  streamContent: number;
  componentType: number;
  componentTag: number;
  langCode: string;
  text: string;
};

export type AudioComponent = {
  streamContent: number;
  componentType: number;
  componentTag: number;
  streamType: number;
  simulcastGroupTag: number;
  mainComponentFlag: boolean;
  qualityIndicator: number;
  samplingRate: number;
  langCode: string;
  langCode2: string | null;
  text: string;
};

export type ContentGenre = {
  largeGenreClassification: number;
  middleGenreClassification: number;
  userGenre1: number;
  userGenre2: number;
};

export type TvEvent = {
  eventId: number;
  startTime: UnixTime;
  duration: number;
  name: string | null;
  text: string | null;
  extendedItems: ExtendedEventItem[];
  videoComponents: VideoComponent[];
  audioComponents: AudioComponent[];
  genres: ContentGenre[] | null;
};

export type Service = {
  serviceId: number;
  isOneseg: boolean;
  videoStreams: Stream[];
  audioStreams: Stream[];
  providerName: string;
  serviceName: string;
  presentEvent: TvEvent | null;
  followingEvent: TvEvent | null;
};

// str.rs

export type CharSize = "small" | "medium" | "normal" | "height-w" | "width-w" | "size-w";

/** ARIB TR-B14で使用可とされている符号。 */
export type AribChar = {
  type: "char-size";
  charSize: CharSize;
} | {
  type: "string";
  string: string;
} | {
  type: "drcs0";
  code1: number;
  code2: number;
} | {
  type: Exclude<DrcsType, "drcs0">;
  code: number;
} | {
  type: "null";
} | {
  type: "active-position-backward";
} | {
  type: "active-position-forward";
} | {
  type: "active-position-down";
} | {
  type: "active-position-up";
} | {
  type: "active-position-return";
} | {
  type: "parameterized-active-position-forward";
  p1: number;
} | {
  type: "active-position-set";
  p1: number;
  p2: number;
} | {
  type: "clear-screen";
} | {
  type: "unit-separator";
} | {
  type: "space";
} | {
  type: "delete";
} | {
  type: "color-foreground";
  p1: number;
} | {
  type: "color-background";
  p1: number;
} | {
  type: "color-half-foreground";
  p1: number;
} | {
  type: "color-half-background";
  p1: number;
} | {
  type: "color-palette";
  p1: number;
} | {
  type: "pattern-polarity-normal";
} | {
  type: "pattern-polarity-inverted-1";
} | {
  type: "flushing-control-start-normal";
} | {
  type: "flushing-control-start-inverted";
} | {
  type: "flushing-control-stop";
} | {
  type: "wait-for-process";
  p1: number;
} | {
  type: "repeat-character";
  p1: number;
} | {
  type: "stop-lining";
} | {
  type: "start-lining";
} | {
  type: "highlight-block";
  p1: number;
} | {
  type: "set-writing-format-init";
  p1: number;
} | {
  type: "raster-color-command";
  p1: number;
} | {
  type: "active-coordinate-position-set";
  p1: number;
  p2: number;
} | {
  type: "set-display-format";
  p1: number;
  p2: number;
} | {
  type: "set-display-position";
  p1: number;
  p2: number;
} | {
  type: "character-composition-dot-designation";
  p1: number;
  p2: number;
} | {
  type: "set-horizontal-spacing";
  p1: number;
} | {
  type: "set-vertical-spacing";
  p1: number;
} | {
  type: "ornament-control-clear";
} | {
  type: "ornament-control-hemming";
  p1: number;
} | {
  type: "builtin-sound-replay";
  p1: number;
} | {
  type: "scroll-designation";
  p1: number;
  p2: number;
};

export type AribString = AribChar[];

// time.rs

/** UTCな秒単位のUNIX時間。 */
export type UnixTime = number;

/** ミリ秒単位のUNIX時間。 */
export type Timestamp = number;
