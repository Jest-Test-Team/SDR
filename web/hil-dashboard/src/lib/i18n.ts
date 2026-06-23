export type Locale = "en" | "zh-Hant";

type Explanation = {
  title: string;
  body: string;
};

export type Dictionary = {
  languageName: string;
  languageToggle: string;
  title: string;
  subtitle: string;
  connected: string;
  connecting: string;
  backendFetchError: string;
  backendWsError: string;
  triggerError: string;
  tabs: {
    hil: string;
    ook: string;
    bits: string;
  };
  sections: {
    flow: string;
    events: string;
    kpis: string;
    ook: string;
    bits: string;
    controls: string;
  };
  pipeline: {
    nodes: {
      icon: string;
      label: string;
      sub: string;
      explanation: string;
    }[];
  };
  events: {
    intro: string;
    time: string;
    node: string;
    payload: string;
    rssi: string;
    status: string;
    nodePrefix: string;
    empty: string;
  };
  kpis: {
    prr: Explanation;
    latency: Explanation;
    bool: Explanation;
    alerts: Explanation;
  };
  charts: {
    baseband: Explanation;
    rfTx: Explanation;
    rfRx: Explanation;
    magnitude: Explanation;
    bitMagnitude: Explanation;
    threshold: string;
  };
  bitCompare: {
    original: string;
    recovered: string;
    packetComplete: string;
    crcError: string;
    bitError: string;
    ber: string;
    empty: string;
    explanation: Explanation;
  };
  controls: {
    intro: string;
    mode: Explanation;
    dataBits: Explanation;
    txPower: Explanation;
    snr: Explanation;
    noise: Explanation;
    filter: Explanation;
    threshold: Explanation;
    replayGuard: Explanation;
    sending: string;
    send: string;
    modeOptions: Record<"EspNow" | "BleAdvertisement" | "Ook433Mhz", string>;
  };
  concepts: Explanation[];
};

export const dictionaries: Record<Locale, Dictionary> = {
  en: {
    languageName: "English",
    languageToggle: "Language",
    title: "ESP32-S3 to SDR HIL Simulator",
    subtitle:
      "Software simulation mode (ESP32-S3 + ESP32). Real SDR hardware mode is not enabled yet.",
    connected: "Live connection",
    connecting: "Connecting...",
    backendFetchError:
      "Cannot reach the HIL backend at http://127.0.0.1:8090. In another terminal, run: cargo run -p hil-simulator --release",
    backendWsError:
      "WebSocket connection failed. Check that hil-simulator is running on :8090.",
    triggerError: "Trigger failed: backend is not running or cannot be reached on :8090.",
    tabs: {
      hil: "System overview",
      ook: "OOK demodulation",
      bits: "Bit analysis",
    },
    sections: {
      flow: "Signal path",
      events: "Live data stream",
      kpis: "Run health",
      ook: "OOK waveform stages",
      bits: "Recovered bit check",
      controls: "Control panel",
    },
    pipeline: {
      nodes: [
        {
          icon: "S3",
          label: "ESP32-S3",
          sub: "Source command",
          explanation:
            "Creates the command bits that the rest of the simulated radio chain must carry.",
        },
        {
          icon: "RF",
          label: "RF space",
          sub: "Modulation plus noise",
          explanation:
            "Turns bits into an on-off keyed radio-like signal and adds channel noise.",
        },
        {
          icon: "SDR",
          label: "SDR receive",
          sub: "RTL-SDR model",
          explanation:
            "Represents what the receiver sees after signal loss, noise, and filtering.",
        },
        {
          icon: "ZMQ",
          label: "ZMQ pipe",
          sub: "Recovered result",
          explanation:
            "Carries decoded telemetry from the signal processing side to the control side.",
        },
        {
          icon: "CP",
          label: "Control plane",
          sub: "Rule engine",
          explanation:
            "Uses the recovered packet to update KPIs, detect faults, and publish status.",
        },
      ],
    },
    events: {
      intro:
        "Each row is one telemetry packet produced by the simulator. Use it to confirm timing, node identity, payload content, signal strength, and decode status.",
      time: "Time",
      node: "Source node",
      payload: "JSON payload",
      rssi: "RSSI",
      status: "Status",
      nodePrefix: "Node",
      empty: "No packets yet. Send a command to generate telemetry.",
    },
    kpis: {
      prr: {
        title: "PRR (packet reception rate)",
        body: "The percent of sent packets that were decoded successfully. A lower value usually points to noise, a weak signal, or a threshold problem.",
      },
      latency: {
        title: "Latency",
        body: "The simulated time from command transmission to the recovered control-plane event.",
      },
      bool: {
        title: "Boolean state",
        body: "The last command value recovered from the packet. It is useful for checking that the data path still preserves the command meaning.",
      },
      alerts: {
        title: "Security alerts",
        body: "The number of safety or replay-rule violations detected by the control logic.",
      },
    },
    charts: {
      baseband: {
        title: "ESP32 baseband bits",
        body: "This is the clean digital bit pattern before radio modulation. High means 1, low means 0.",
      },
      rfTx: {
        title: "Transmitted RF signal (OOK)",
        body: "OOK means on-off keying. The carrier is present for a 1 and mostly absent for a 0.",
      },
      rfRx: {
        title: "RTL-SDR received signal with noise",
        body: "This shows the signal after the channel adds noise. It is closer to what an SDR receiver would sample.",
      },
      magnitude: {
        title: "GNU Radio magnitude and slicer",
        body: "Magnitude converts the received wave into signal strength. The slicer compares it with the threshold to decide each bit.",
      },
      bitMagnitude: {
        title: "Demodulated magnitude",
        body: "This focused view shows the values used to recover bits. Values above the threshold become 1; values below it become 0.",
      },
      threshold: "Threshold",
    },
    bitCompare: {
      original: "Original bits:",
      recovered: "Recovered bits:",
      packetComplete: "Packet complete",
      crcError: "Packet damaged (CRC error)",
      bitError: "Bit error",
      ber: "Bit error rate (BER):",
      empty: "No bit snapshot yet. Send a command to run the decoder and compare bits.",
      explanation: {
        title: "How to read this check",
        body: "The original row is what the transmitter sent. The recovered row is what the receiver decoded. Highlighted positions are bit mismatches. BER is the error percentage.",
      },
    },
    controls: {
      intro:
        "These controls change the simulated radio channel and receiver. After changing values, send a command to see how the waveform, decoded bits, and KPIs respond.",
      mode: {
        title: "Transmission mode",
        body: "Chooses the simulated wireless format. OOK exposes the radio waveform stages used by this dashboard.",
      },
      dataBits: {
        title: "Payload bits (8-bit)",
        body: "The exact command bits sent by the simulated ESP32-S3. Use only 0 and 1 so the bit comparison remains meaningful.",
      },
      txPower: {
        title: "Transmit power (dBm)",
        body: "Raises or lowers the signal level before the channel. Higher power usually improves reception.",
      },
      snr: {
        title: "Signal-to-noise ratio (SNR dB)",
        body: "Compares useful signal strength with noise. Higher SNR means a cleaner signal.",
      },
      noise: {
        title: "Noise level",
        body: "Adds random disturbance to the received signal. More noise makes decoding harder.",
      },
      filter: {
        title: "Filter bandwidth (MHz)",
        body: "Controls how much of the received spectrum passes into the decoder. Too narrow can cut signal; too wide can admit noise.",
      },
      threshold: {
        title: "Decision threshold",
        body: "The magnitude level used to decide 0 or 1. A poor threshold can turn a clean signal into wrong bits.",
      },
      replayGuard: {
        title: "Sequence replay check",
        body: "Rejects packets that repeat an old sequence number. This models a basic replay-protection rule.",
      },
      sending: "Sending...",
      send: "Send boolean command",
      modeOptions: {
        EspNow: "ESP-NOW",
        BleAdvertisement: "BLE Advertisement",
        Ook433Mhz: "433 MHz OOK",
      },
    },
    concepts: [
      {
        title: "HIL",
        body: "Hardware-in-the-loop connects real or simulated hardware behavior to software tests. Here it lets the control dashboard observe a radio pipeline as if hardware were attached.",
      },
      {
        title: "SDR",
        body: "Software-defined radio moves radio processing into software. This makes it easier to inspect waveforms, adjust filters, and test receivers.",
      },
      {
        title: "RSSI",
        body: "Received signal strength indicator. It is a rough measure of how strong the received signal is, shown in dBm.",
      },
      {
        title: "CRC",
        body: "Cyclic redundancy check. It helps detect whether a packet was damaged during transmission.",
      },
      {
        title: "ZMQ",
        body: "ZeroMQ is a messaging layer used here to move decoded data between processing blocks.",
      },
    ],
  },
  "zh-Hant": {
    languageName: "繁體中文",
    languageToggle: "語言",
    title: "ESP32-S3 至 SDR HIL 模擬器",
    subtitle: "軟體模擬模式（ESP32-S3 + ESP32）。真實 SDR 硬體模式尚未啟用。",
    connected: "即時連線",
    connecting: "連線中...",
    backendFetchError:
      "無法連線至 HIL 後端 http://127.0.0.1:8090。請在另一個終端執行：cargo run -p hil-simulator --release",
    backendWsError: "WebSocket 連線失敗。請確認 hil-simulator 正在 :8090 執行。",
    triggerError: "觸發失敗：後端未啟動或無法連線到 :8090。",
    tabs: {
      hil: "系統總覽",
      ook: "OOK 解調",
      bits: "位元分析",
    },
    sections: {
      flow: "訊號路徑",
      events: "即時資料流",
      kpis: "執行狀態",
      ook: "OOK 波形階段",
      bits: "還原位元檢查",
      controls: "控制面板",
    },
    pipeline: {
      nodes: [
        {
          icon: "S3",
          label: "ESP32-S3",
          sub: "原始指令",
          explanation: "產生要傳送的指令位元，後面的模擬無線鏈路都要把這些位元保留下來。",
        },
        {
          icon: "RF",
          label: "RF 空間",
          sub: "調變加雜訊",
          explanation: "把位元轉成 OOK 無線訊號，並加入通道中的雜訊。",
        },
        {
          icon: "SDR",
          label: "SDR 接收",
          sub: "RTL-SDR 模型",
          explanation: "表示接收端看到的訊號，包含衰減、雜訊與濾波後的影響。",
        },
        {
          icon: "ZMQ",
          label: "ZMQ 管道",
          sub: "還原結果",
          explanation: "把解碼後的遙測資料，從訊號處理端送到控制端。",
        },
        {
          icon: "CP",
          label: "控制層端",
          sub: "規則引擎",
          explanation: "使用還原後的封包更新 KPI、判斷錯誤，並發布目前狀態。",
        },
      ],
    },
    events: {
      intro:
        "每一列都是模擬器產生的一筆遙測封包。可以用來確認時間、節點、載荷內容、訊號強度與解碼狀態。",
      time: "時間",
      node: "來源節點",
      payload: "JSON 內容",
      rssi: "RSSI",
      status: "狀態",
      nodePrefix: "節點",
      empty: "尚無封包。請發送指令來產生遙測資料。",
    },
    kpis: {
      prr: {
        title: "PRR（封包接收率）",
        body: "成功解碼的封包比例。數值下降通常代表雜訊太多、訊號太弱，或判定閾值不合適。",
      },
      latency: {
        title: "延遲",
        body: "從指令送出到控制端收到還原事件的模擬時間。",
      },
      bool: {
        title: "布林狀態",
        body: "上一個從封包還原出的指令值。可用來確認資料路徑是否保留原本的指令意思。",
      },
      alerts: {
        title: "安全警報",
        body: "控制邏輯偵測到的安全規則或重放規則違反次數。",
      },
    },
    charts: {
      baseband: {
        title: "ESP32 原始數位訊號",
        body: "這是調變前的乾淨位元型態。高電位代表 1，低電位代表 0。",
      },
      rfTx: {
        title: "發射端 RF 訊號（OOK）",
        body: "OOK 是開關鍵控。傳 1 時載波存在，傳 0 時載波大多關閉。",
      },
      rfRx: {
        title: "RTL-SDR 接收訊號（含雜訊）",
        body: "這是通道加入雜訊後的訊號，比較接近 SDR 接收器實際取樣到的資料。",
      },
      magnitude: {
        title: "GNU Radio 幅度與切片判定",
        body: "幅度會把接收波形轉成訊號強度。切片器再用閾值判斷每個位元是 0 還是 1。",
      },
      bitMagnitude: {
        title: "解調後幅度",
        body: "這個視圖聚焦在用來還原位元的數值。高於閾值會判為 1，低於閾值會判為 0。",
      },
      threshold: "閾值",
    },
    bitCompare: {
      original: "原始位元：",
      recovered: "還原位元：",
      packetComplete: "封包完整",
      crcError: "封包損壞（CRC 錯誤）",
      bitError: "位元錯誤",
      ber: "誤碼率（BER）：",
      empty: "尚無位元快照。請發送指令，讓解碼器執行並比較位元。",
      explanation: {
        title: "如何閱讀這個檢查",
        body: "原始位元是發射端送出的資料，還原位元是接收端解出的資料。標示的位置代表位元不一致。BER 是錯誤比例。",
      },
    },
    controls: {
      intro:
        "這些控制項會改變模擬無線通道與接收器設定。調整後發送指令，就能觀察波形、還原位元與 KPI 如何變化。",
      mode: {
        title: "傳輸模式",
        body: "選擇模擬的無線格式。OOK 會顯示此儀表板使用的無線波形階段。",
      },
      dataBits: {
        title: "傳輸資料（8-bit）",
        body: "模擬 ESP32-S3 送出的指令位元。請只輸入 0 和 1，位元比較才有意義。",
      },
      txPower: {
        title: "發射功率（dBm）",
        body: "調整訊號進入通道前的強度。功率越高，通常越容易接收成功。",
      },
      snr: {
        title: "信噪比（SNR dB）",
        body: "比較有效訊號與雜訊的強度。SNR 越高，訊號越乾淨。",
      },
      noise: {
        title: "雜訊強度",
        body: "加入接收訊號中的隨機干擾。雜訊越多，解碼越困難。",
      },
      filter: {
        title: "濾波器頻寬（MHz）",
        body: "控制多少接收頻譜能進入解碼器。太窄可能切掉訊號，太寬可能放進更多雜訊。",
      },
      threshold: {
        title: "判定閾值",
        body: "用來判斷 0 或 1 的幅度門檻。閾值不合適時，乾淨訊號也可能被判錯。",
      },
      replayGuard: {
        title: "序列號重放校驗",
        body: "拒絕重複舊序列號的封包。這是在模擬基本的重放攻擊防護規則。",
      },
      sending: "發送中...",
      send: "發送布林指令",
      modeOptions: {
        EspNow: "ESP-NOW",
        BleAdvertisement: "BLE Advertisement",
        Ook433Mhz: "433 MHz OOK",
      },
    },
    concepts: [
      {
        title: "HIL",
        body: "Hardware-in-the-loop 是把真實或模擬硬體行為接進軟體測試。這裡用來讓控制儀表板像接上硬體一樣觀察無線流程。",
      },
      {
        title: "SDR",
        body: "Software-defined radio 是用軟體處理無線訊號。這樣比較容易觀察波形、調整濾波器，也方便測試接收器。",
      },
      {
        title: "RSSI",
        body: "Received signal strength indicator，代表接收訊號強度的粗略指標，單位是 dBm。",
      },
      {
        title: "CRC",
        body: "Cyclic redundancy check，用來偵測封包在傳輸過程中是否損壞。",
      },
      {
        title: "ZMQ",
        body: "ZeroMQ 是訊息傳遞工具。此處用來在處理模組之間傳送解碼後的資料。",
      },
    ],
  },
};
