# 成果與落差說明（SDR 與軟體模擬）

本文件用白話說明：這個專案用兩片 ESP32 開發板「模擬」了 SDR（軟體無線電）與軟體模擬的哪些部分、是怎麼做到的，以及和「真正的 SDR」「真正的軟體模擬」相比，還差哪些東西。

分成三個面向來看：**軟體（Software）**、**韌體（Firmware）**、**硬體（Hardware）**。

---

## 一、先講結論（一句話版本）

- 我用 **兩片便宜的 ESP32 開發板 + ESP-NOW 無線傳輸**，把「一個指令從一端送到另一端、被解出來、進到後端處理、顯示在儀表板上」這條完整鏈路真的跑通了。
- 同一條後端處理鏈路，也接上了一個**純軟體模擬器（hil-simulator）**，不用任何硬體也能產生一樣格式的資料、推進到後端、在儀表板上看到波形與誤碼分析。
- **但是**：真正 SDR 的核心——「處理原始無線電訊號樣本（raw RF samples）」——我目前是用**軟體假裝**出來的，開發板本身收到的是已經解碼好的封包，不是原始電波。這是最大的落差。

可以把它想成：**整條「資料管線」是真的，但「無線電物理層」一半是真硬體（ESP-NOW）、一半是軟體模擬（SDR/OOK）。**

---

## 二、整體架構（兩條軌道）

```
硬體軌道（真實開發板）：
  ESP32 發送端 ──ESP-NOW無線──▶ ESP32-S3 閘道 ──USB線──▶ edge-gateway ──▶ control-plane ──▶ 儀表板

軟體模擬軌道（不用硬體）：
  hil-simulator 模擬器 ──ZMQ 或 加密通道──▶ control-plane ──▶ 儀表板
```

兩條軌道送進後端的資料格式完全一樣（都是 `TelemetryFrame`），所以後端不需要分辨資料是來自真板子還是軟體，處理方式一致。這是設計上最關鍵的一點：**真硬體與軟體模擬可以無痛互換**。

---

## 三、已經做到的部分

### 軟體面（已完成且穩定）

| 項目 | 說明 |
| --- | --- |
| 共用資料格式 | 不論真板子或模擬器，資料都用同一個 `TelemetryFrame` 格式，搭配 CRC 校驗、COBS 封包切割。 |
| 後端處理鏈路 | `control-plane` 接收資料 → 套用規則（例如防重送）→ 存進資料庫 → 對外提供即時狀態。 |
| 軟體模擬器 | `hil-simulator` 不用任何硬體，就能模擬通道條件並產生有效封包，推到後端。 |
| 即時儀表板 | Next.js 網頁可調整 SNR、雜訊、閾值、傳輸模式，即時顯示 OOK 波形與 BER／CRC 分析。 |
| 安全傳輸通道 | 模擬器可改走 TLS 1.3 + 雙向憑證（mTLS）的加密通道送資料，並有完整的「拒絕沒憑證／錯憑證／明文／舊版 TLS」測試。 |
| 自動化測試 | 有單元測試、整合測試、Robot Framework 端對端測試，連「防重送」「指令翻譯」都涵蓋。 |
| 雙模式自動切換 | 儀表板 `/gateway` 頁面會自動偵測現在是「真硬體」還是「模擬模式」，並顯示對應標籤。 |

### 韌體面（已完成）

| 項目 | 說明 |
| --- | --- |
| ESP-NOW 收發 | 發送端真的用 ESP-NOW 無線把封包送出去，閘道端真的收下並解碼。 |
| 序號與防重送基礎 | 韌體會送出遞增序號，後端據此判斷重複封包。 |
| 實體按鈕觸發 | 按開發板上的 BOOT（GPIO0）按鈕會送出 `BoolCmd(true)`，平常每 2 秒送一次心跳 `BoolCmd(false)`。 |
| 執行期可調參數 | 可從儀表板即時設定發送端的 TX 發射功率、8 位元資料內容，透過 USB→閘道→ESP-NOW 下發。 |
| 安全閘道指令集 | ESP32-S3 可接受 USB 指令並轉成 ESP-NOW，操作另一片 ESP32 閘道：健康檢查、開關下游 AP、踢除連線裝置、查連線數、模擬 SNMP 讀寫等，全部在實機驗證過。 |

### 硬體面（已完成）

| 項目 | 說明 |
| --- | --- |
| 兩板實機連線 | ESP32 發送端 ×2 與 ESP32-S3 閘道，已在實機上端對端驗證過整條鏈路。 |
| USB 橋接 | 閘道透過 USB 序列埠把解碼後的封包送進電腦端程式。 |
| 多板燒錄流程 | 有 `flash_tx.sh` / `flash_gw.sh` 一鍵燒錄腳本，並記錄了實際的板子 MAC、連接埠、鮑率。 |

---

## 四、和「真正的 SDR / 軟體模擬」相比，還差哪些（落差）

這一節是重點。下面把每個落差講清楚：**現在是怎麼做的、真正版本應該怎樣、要補上需要什麼。**

### 硬體面的落差（最關鍵）

| 落差項目 | 現在的狀況 | 真正 SDR 應該要的 | 補上需要 |
| --- | --- | --- | --- |
| **原始 RF 樣本** | 開發板收到的是 ESP-NOW **已解碼好的封包**，看不到原始電波。 | SDR 接收器要拿到的是連續的 I/Q 原始取樣資料。 | 加一個真正的 SDR 接收硬體（RTL-SDR、HackRF、USRP 等）。 |
| **SNR（訊雜比）** | 由軟體模擬器算出來的數字，板子無法自己製造。 | 由真實通道與接收訊號決定。 | 用 RF 衰減器、控制距離、或從 RSSI／封包遺失率估算。 |
| **雜訊注入** | 純軟體模擬。 | 真實雜訊或干擾源。 | RF 雜訊源、SDR 訊號注入或受控干擾發射器。 |
| **濾波器頻寬** | 軟體參數，對真硬體沒有作用。 | 在 DSP 中對原始樣本做濾波。 | 需要 SDR 接收路徑＋實際 DSP 處理原始樣本。 |
| **判決閾值（OOK 解調）** | 軟體模擬的 OOK 切片器。 | 韌體或後端對真實振幅樣本做切片。 | 需要 SDR／OOK 解調路徑。 |
| **非 ESP-NOW 傳輸模式** | 「433 MHz OOK」「SoftwareSim」目前只是模擬行為。 | 真實的 433 MHz 或 BLE 收發。 | 加 BLE 韌體模式，或加 433 MHz OOK 收發硬體。 |

> 一句話：**SDR 最核心的「處理原始電波樣本」這件事，目前完全是軟體假裝的，沒有真正的射頻前端硬體。** ESP-NOW 是 2.4GHz 的封包傳輸，不是 SDR。

### 韌體面的落差

| 落差項目 | 現在的狀況 | 缺口 |
| --- | --- | --- |
| 解調器 | 韌體裡**沒有** OOK 解調器，閘道只負責收 ESP-NOW 封包再轉 USB。 | 若要真 SDR，需在韌體或後端實作切片／解調。 |
| 通道參數控制 | SNR、雜訊、濾波頻寬、閾值這些**韌體無法控制**，因為板子拿不到原始樣本。 | 這些本質上要有 SDR 路徑才有意義。 |
| 防重送 | 韌體只負責送遞增序號，真正的重複拒絕在後端（control-plane）。 | 算合理分工，但要注意這不是 RF 層的保護。 |
| 命名與角色 | 韌體 crate 名稱是舊的，和實際角色**相反**（容易誤解）。 | 屬於技術債，建議日後正名。 |

### 軟體面的落差

| 落差項目 | 現在的狀況 | 缺口 |
| --- | --- | --- |
| DSP 真實性 | 波形、BER、CRC 分析都是基於**模擬通道模型**，不是真實樣本。 | 接上真 SDR 後，DSP 才會處理真資料。 |
| 真實 SDR 工具鏈 | 雖有 GNU Radio / ZMQ 注入的選項，但目前主要走模擬注入，**真正的 SDR 還沒啟用**。 | 需整合 GNU Radio + 實體 SDR。 |
| 後量子加密（PQC） | 目前是 TLS 1.3 + 傳統憑證，PQC 只列為未來升級點。 | 等相依套件成熟後再導入 ML-KEM／ML-DSA 等。 |

---

## 五、用一張表看「真硬體 / 軟體模擬 / 未實作」對照

> 注意：「軟體模擬」**不等於「假的」**。它是用程式正式實作出來的軟體版本，本來就是這個專案要交付的東西之一。下表的 🟦 代表「已用軟體完整做出來」，只是還沒接上對應的實體射頻硬體。詳見第八節。

| 環節 | 狀態 |
| --- | --- |
| 指令端對端傳輸（ESP-NOW） | ✅ 真硬體 |
| 序號、心跳、BOOT 按鈕觸發 | ✅ 真硬體 |
| 閘道收封包＋USB 轉送 | ✅ 真硬體 |
| 後端規則處理、存檔、即時狀態 | ✅ 真軟體（正式邏輯） |
| 加密通道（TLS 1.3 / mTLS） | ✅ 真軟體（正式邏輯） |
| 安全閘道指令（健康、踢除、SNMP…） | ✅ 真硬體驗證過 |
| **SNR / 雜訊 / 濾波頻寬 / 判決閾值** | 🟦 軟體模擬已完成（缺真 SDR 硬體） |
| **OOK 波形 / 解調 / BER 分析** | 🟦 軟體模擬已完成（缺真 SDR 硬體） |
| **板子 / 韌體的軟體孿生** | 🟦 軟體模擬已完成（無需任何 ESP 板即可跑） |
| **真實原始 RF 樣本（I/Q）** | ⚠️ 無射頻前端，尚未取得 |
| **433 MHz、BLE 等其他無線模式** | ⚠️ 尚未實作 |

---

## 六、若要「補成真正的 SDR」，建議順序

1. **先加一個 SDR 接收硬體**（最便宜可從 RTL-SDR 起步），讓系統第一次拿到真正的原始 I/Q 樣本。
2. **在 DSP 端實作真正的解調**（OOK 切片、濾波），讓 SNR／雜訊／頻寬／閾值這些參數開始有真實意義。
3. **把模擬器當成對照組**：同樣的資料格式，可以隨時切換「真 SDR 接收」和「軟體模擬」，方便驗證 DSP 是否正確。
4. **逐步加入其他無線模式**（433 MHz OOK、BLE），擴大涵蓋範圍。
5. **資安升級**：等套件成熟後，把 TLS 換成混合式後量子加密。

---

## 七、總結

- **已完成且可信賴的是「資料管線」與「軟體模擬框架」**：從封包格式、後端處理、加密傳輸、儀表板、到自動化測試，都是紮實的正式實作，而且真硬體與模擬可無痛互換。
- **尚未補上的是「射頻前端與真實 DSP」**：目前所有跟原始電波樣本有關的 SDR 行為都是軟體模擬，沒有實體 SDR 硬體。
- 換句話說：**這是一個「架構完整、模擬到位，但還沒接上真正天線端」的 SDR 原型。** 補上一個 SDR 接收器與 DSP 解調路徑，就能讓目前模擬的部分逐項變成真的。

---

## 八、重要釐清：這個專案有「兩種」軟體模擬（software-sim）

之前容易把不同的東西都叫「軟體模擬」而混淆。其實本專案的 software-sim 是**兩個不同的東西**，各自要解決的問題不同、落差也不同。先講最重要的觀念：

> **「軟體模擬」不是「假裝」。** 它是刻意用程式正式做出來的版本，目的是「不靠實體硬體也能跑」，這本身就是一項成果，不是缺陷。真正的落差只在於「還沒接上對應的實體射頻硬體」。

### 第一種：SDR / 射頻通道的「DSP 軟體模擬」

- **位置**：`hil-simulator` 的 `sim/pipeline.rs`、`sim/ook.rs`。
- **它做什麼**：真的用程式產生 OOK 波形 → 依 SNR 注入雜訊（`noise_amp = 10^(-SNR/20)…`）→ 降取樣 → 用閾值切片 → 計算 BER 與 CRC。
- **這代表什麼**：SNR、雜訊、濾波頻寬、判決閾值這些參數**在軟體裡是真的有作用、會改變波形與誤碼率的**，這是一個貨真價實的 DSP 模擬，不是寫死的假數字。
- **和「真正的軟體模擬」相比**：基本上沒有落差，該有的訊號處理流程都做出來了。
- **唯一的落差是硬體**：它處理的是「程式產生的樣本」，不是「天線收到的真實 I/Q 樣本」。要補的是 **SDR 射頻前端硬體**，不是補軟體。

### 第二種：板子 / 韌體的「軟體孿生（software twin）」

- **位置**：`firmware/software-sim` crate（`firmware-software-sim`），含 `gateway.rs`。
- **它做什麼**：在電腦上產生和真板子**位元組完全相同**的 `TelemetryFrame`、`SDRCTL,…` 控制線、以及安全閘道指令模型（健康檢查、開關下游、踢除、SNMP 讀寫等）。
- **這代表什麼**：**完全不插任何 ESP32 板子**，整套系統也能跑起來，後端分不出資料是來自真板還是軟體孿生。
- **和「真正的軟體模擬」相比**：這正是 software-sim 的標準定義——用軟體取代硬體——而且已經做到，並在儀表板上能自動切換「真硬體 / 模擬模式」。
- **落差**：它是「行為與位元組層級」的孿生，不模擬晶片內部時序、射頻細節，也不能取代燒進板子的真實韌體映像。

### 兩種 software-sim 對照表

| 比較項目 | 第一種：DSP 通道模擬 | 第二種：板子/韌體軟體孿生 |
| --- | --- | --- |
| 程式位置 | `hil-simulator/src/sim/` | `firmware/software-sim/` |
| 模擬對象 | 射頻通道、波形、雜訊、解調 | 板子送出的封包與控制指令 |
| 主要產出 | OOK 波形、BER、CRC、KPI | `TelemetryFrame`、`SDRCTL`、閘道指令 |
| 完成度 | 訊號處理鏈大致完成 | 封包/指令孿生完成、實機驗證過 |
| 真正的落差 | 缺 SDR **硬體**（拿不到真 I/Q） | **無佈建（provisioning）流程**、不模擬晶片時序、不取代真實韌體映像 |
| 該補什麼 | 加 SDR 接收器 + DSP 處理真樣本 | 補裝置佈建/註冊生命週期、視需要做 HIL 對照測試 |

> 更正：先前把 software-sim 寫成「已完整」並不準確。它能產生正確的封包與指令，但**整套裝置佈建（provisioning / onboarding）功能目前還沒有**，詳見第十節。

---

## 九、修正先前的說法

本文件第三、四節原本把 SNR／雜訊／OOK 解調描述成「軟體假裝」，這個用詞**不準確**，在此更正：

- ❌ 舊說法：「SDR 最核心的處理原始電波樣本，目前完全是軟體假裝的。」
- ✅ 正確說法：「SDR 的 DSP 處理**已經用軟體正式模擬出來**（會依參數改變波形與 BER）。目前缺的是**實體 SDR 射頻前端**，因此處理的是程式產生的樣本，而非天線收到的真實 I/Q 樣本。」

簡單記：**SDR 的 DSP 處理軟體做了；缺的是天線端的那塊硬體。** 但要注意，這只說 DSP 部分；software-sim 在「裝置佈建」上仍有明顯缺口，見下一節。

---

## 十、software-sim 目前的明確缺口：沒有佈建（provisioning）功能

這是先前漏講、而且很重要的一點：**software-sim 目前沒有任何裝置佈建 / 註冊上線（provisioning / onboarding）流程。** 因此不能說它「已完整」。

### 什麼是佈建（provisioning）

佈建是指一台新裝置要正式加入系統時，所需要的「身份與信任建立」流程，通常包含：

- 產生／指派裝置唯一身份（device identity）。
- 安全地配發金鑰或憑證（key / certificate 發放）。
- 把裝置註冊（enroll / claim）到系統並建立信任關係。
- 後續可撤銷（revoke）或輪替（rotate）憑證。

### 現在的實際狀況

| 項目 | 現況 | 落差 |
| --- | --- | --- |
| ESP-NOW 配對 | 對端 MAC 在燒錄時**寫死**（`GATEWAY_MAC` 編進韌體）。 | 沒有動態配對 / 上線流程，換板就要重燒。 |
| TLS 憑證 | 由本地 CA**手動**事先產生的靜態檔案。 | 沒有自動簽發、沒有輪替、沒有撤銷機制。 |
| `CMD_REGISTER_NODE` | 只是在記憶體裡記一筆「有節點加入 AP」，且**硬體模式下只回一句說明、不真的做事**。 | 不是真正的註冊：沒有身份、沒有發證、沒有信任建立。 |
| 裝置身份管理 | 無集中管理、無生命週期。 | 缺整套 provisioning 後端與資料模型。 |

### 結論

- software-sim 能正確產生封包與指令、能跑 DSP 模擬，這些是真的。
- **但它假設「裝置早就被信任、金鑰早就配好」**，跳過了真實系統最重要的第一步——安全佈建。
- 因此和「真正的軟體模擬 / 真實系統」相比，software-sim 還缺：**裝置佈建、身份管理、憑證簽發/輪替/撤銷**這一整塊，屬於尚未實作的功能，而非已完成。

### 更新（已補上佈建生命週期）

上述缺口已著手補上。現在 software-sim 具備完整的**裝置佈建生命週期**，三層都打通：

- **韌體層（`firmware/software-sim`）**：`GatewaySim` 新增裝置身份狀態機 `Pending → Active → Revoked`，並提供 `EnrollDevice`（簽發身份＋憑證指紋）、`ClaimDevice`（啟用上線）、`RotateCredential`（輪替憑證、版本遞增）、`RevokeDevice`（撤銷下線）四個指令，含完整的負面測試（重複註冊、未註冊就啟用、撤銷後不可輪替）。
- **後端層（`hil-simulator`）**：模擬模式會真正套用上述指令；硬體模式則把指令轉成序列埠命令送到板子（見第十一節）。
- **前端層（`web/hil-dashboard`）**：`/gateway` 頁新增「裝置佈建」面板與裝置身份表（狀態、憑證指紋、版本）。

仍未實作（列為未來工作，詳見根目錄 `GAP_ELIMINATION_PLAN.md`）：真實金鑰/憑證簽章（目前是確定性的假指紋）、NVS 持久化（佈建狀態跨重啟保存）、把憑證輪替接到真正的 TLS 1.3/mTLS 通道。

---

## 十一、佈建已上板：真實硬體路徑（雙語 / Bilingual）

> 中文先、English follows. 這一節記錄佈建從「純軟體模擬」進一步做到「真的在開發板上執行」。

### 中文

佈建已經不只是軟體模擬，而是**真的在開發板上執行**。指令會經由 ESP-NOW 在兩片板子之間往返，**裝置註冊表存在 ESP32 閘道板的記憶體裡**：

```
儀表板 / CLI ─USB─▶ ESP32-S3（模擬節點）─ESP-NOW(GwMsg)─▶ ESP32（閘道：註冊表＋狀態機）
                                         ◀─GwMsg::ProvisionResp─
```

已完成並驗證：

- **協定層（`protocol/gwlink`）**：新增 `EnrollReq / ClaimReq / RotateReq / RevokeReq` 與 `ProvisionResp` 訊息，含 round-trip 測試。
- **ESP32 閘道板（`esp32-tx-node`）**：在板上維護裝置註冊表，執行 `pending → active → revoked` 狀態機，並拒絕非法操作（重複註冊、未 pending 不能 claim、撤銷後不能輪替）。
- **ESP32-S3 模擬節點（`esp32s3-gateway`）**：解析 `GW,ENROLL/CLAIM/ROTATE/REVOKE` 的 USB 指令、轉成 ESP-NOW，並把回覆印成 `GWRESP PROVISION …`。
- **後端硬體模式（`hil-simulator`）**：把佈建指令轉成序列埠命令送到板子，再把 `GWRESP PROVISION` 解析回 `GatewaySnapshot.devices`，前端表格即時反映真實板況。
- **兩個 ESP 韌體映像皆可交叉編譯通過**；在實機上已看到 `GWRESP PROVISION … ok=true` 的真實回覆。

一鍵測試腳本：

```bash
./scripts/provision_full_flow.sh            # 自動偵測板子（硬體模式），跑 enroll→claim→rotate→revoke 自我檢查
./scripts/provision_full_flow.sh --sim      # 不接板子，純模擬
./scripts/provision_full_flow.sh --restart  # 先停掉舊的模擬後端，改用硬體模式重啟
./scripts/provision_demo.sh --usb           # 不開後端，直接走 USB 對板子斷言 GWRESP
```

「這是真硬體」的證明：**把 ESP32 閘道板斷電重開**，註冊表會清空（因為存在板子的 RAM 裡），證明狀態活在板子上、不是活在電腦上。

仍未補上的差距：憑證指紋目前是確定性的**假指紋**（非真實金鑰/憑證），且註冊表**尚未寫入 NVS**，所以一斷電就消失。

### English

Provisioning is no longer only a software simulation — it now **runs on the real
dev boards**. Commands travel over ESP-NOW between the two boards, and **the device
registry lives in RAM on the ESP32 gateway board**:

```
Dashboard / CLI ─USB─▶ ESP32-S3 (sim node) ─ESP-NOW(GwMsg)─▶ ESP32 (gateway: registry + state machine)
                                            ◀─GwMsg::ProvisionResp─
```

Done and verified:

- **Protocol (`protocol/gwlink`)** — new `EnrollReq / ClaimReq / RotateReq /
  RevokeReq` and `ProvisionResp` messages, with round-trip tests.
- **ESP32 gateway (`esp32-tx-node`)** — keeps the device registry on-device and
  enforces the `pending → active → revoked` state machine, rejecting bad ops
  (duplicate enroll, claim of a non-pending device, rotate after revoke).
- **ESP32-S3 node (`esp32s3-gateway`)** — parses `GW,ENROLL/CLAIM/ROTATE/REVOKE`
  USB lines, relays them over ESP-NOW, and prints `GWRESP PROVISION …` replies.
- **Backend hardware mode (`hil-simulator`)** — turns provisioning commands into
  serial lines to the boards and parses `GWRESP PROVISION` back into
  `GatewaySnapshot.devices`, so the dashboard table reflects real board state.
- **Both ESP firmware images cross-compile**, and real `GWRESP PROVISION … ok=true`
  replies were observed on hardware.

One-command tests:

```bash
./scripts/provision_full_flow.sh            # auto-detect boards (hardware), run the self-check
./scripts/provision_full_flow.sh --sim      # no boards, pure simulation
./scripts/provision_full_flow.sh --restart  # stop a stale sim backend, relaunch in hardware mode
./scripts/provision_demo.sh --usb           # no backend; assert GWRESP straight from the boards
```

Proof it is real hardware: **power-cycle the ESP32 gateway** and the registry clears
(it lives in the board's RAM), proving the state lives on the board, not the host.

Remaining gaps: the credential fingerprint is still a deterministic **fake**
(not real key/cert material), and the registry is **not yet persisted to NVS**, so
it is lost on power-cycle.
