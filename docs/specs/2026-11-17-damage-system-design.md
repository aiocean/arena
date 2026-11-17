# Hệ sát thương, nguyên tố và pool thần thông chung: thiết kế

Trạng thái: **đề xuất đã chốt hướng**, làm phần backend trước (bộ tính, dữ liệu, môi trường huấn luyện), chưa làm
UI. Đi kèm hai bản nghiên cứu `2026-11-17-skill-system-design.md` (gọi là **bản nghiên cứu**) và
`2026-11-17-skill-system-rules.md` (gọi là **bản luật**). Khi ba bản khác nhau, **bản này đúng**; khi một con số ở đây
khác `data.rs` sau này, `data.rs` đúng.

Bản này là một thay đổi luật lớn: mùa mới, huấn luyện lại, viết lại `docs/rules.md` và `docs/bot.md` khi nối vào trận
(`CLAUDE.md`, "Traps"). Mọi con số là số khởi điểm; con số thật đến từ huấn luyện và ladder (mục 12).

---

## 0. Những gì đổi so với hiện tại và so với bản nghiên cứu

**Bỏ hẳn:**

| Bỏ | Ở đâu hiện nay | Thay bằng |
|---|---|---|
| Hai phái **Thunder / Frost** (`School`, `SchoolData`, `stats(school)`) | `data.rs`, `bar.rs`, mọi nơi đọc `school` | Hai **ghế** (trái 0, phải 1) giống hệt nhau; khác nhau chỉ ở **bộ đồ** (loadout) người chơi chọn (mục 2). Hình vẽ của mỗi ghế là chuyện của `arena-fighter`, không phải của luật. |
| Đặc tính phái: `fury`, `unyielding`, `attuned` | `Stats`, `status::take` | Thành **đạo tâm** (keystone) ai cũng chọn được (mục 9), hoặc bỏ. |
| **Chill, Freeze, đóng băng**, `CHILL`, `FREEZE`, luật `GRIT.thaw`, `Status::Chill` | `data.rs`, `status.rs`, `act.rs`, `sense.rs` | Thủy có ailment mới **Thấm** (mục 4), không làm đứng hình. Không còn khái niệm băng hay đóng băng ở bất kỳ đâu, kể cả trong dung hợp. |
| Kỹ năng băng do mình bịa (không có trong truyện): `icicle`, `shard`, `shatter`, `frostfire`, `flurry`, `prison` | `data.rs` | Bỏ. `jet` thành chiêu Thủy. |
| Căn cơ, bản nguyên (lớp nhân vật) của bản nghiên cứu, mục 9 | bản nghiên cứu | Không có lớp. Mọi chiêu chung một pool. |
| Băng phong, "Lôi phá băng", "Phong + Thủy → Băng", "vùng băng đóng băng đạn sét" | bản nghiên cứu 3, bản luật 3.6, 4.5 | Mục 4.3 và 6 (dẫn điện, tản lôi). |
| Máu/chỉ số khác nhau theo căn cơ | bản luật 1 | Một bộ chỉ số gốc cho mọi đấu sĩ (mục 1). |

**Giữ:** tick 60/s và tất định theo seed; khung frame data (windup, recover, counter, punish, unsafe on block);
guard; thanh chiêu xoay (bar) nhưng rút từ bộ đồ của người chơi; ultimate theo nộ.

---

## 1. Đấu sĩ: một gốc cho mọi người

| Chỉ số | Gốc | Ghi chú |
|---|---|---|
| Máu (life) | 3000 | Như `Stats.hp`. |
| Hộ thân linh quang (ES) | 600 | Hồi 60/s sau 2 s không bị trúng. Vỡ (về 0) thì 1 s không hồi thêm. |
| Thần thức (sense) | 100 | Hồi 4/s. Ý cảnh giữ chỗ. Về 0 thì **thất hồn**: câm 1,5 s, tắt ý cảnh. |
| Linh lực (qi) | 100 | Hồi 3/s, thiền 16/s (như `QI_REGEN`, `MEDITATE_REGEN`). |
| Lực đạo (might) | 400 | Gốc của mọi **Đòn** (attack). Thay `atk`. |
| Hộ thể (armour) | 600 | Chỉ giảm sát thương Lực (mục 5.3). Thay `pdef`. |
| Kháng 9 loại còn lại | 0% | Trần 75%, sàn −100%. Thay `mdef`. |
| Chí mạng | 5%, ×1,5 | |
| Né (evasion) | 5% | Chỉ né được Đòn và Đạn; không né được chiêu **Thuấn**. |
| Kháng ailment | 0% | Trần 60%. |
| Bản mệnh | không | Người chơi chọn một hành trong bộ đồ (mục 2), có thể bỏ trống. |

Người chơi khác nhau ở bộ đồ, không ở chỉ số gốc. Muốn trâu hơn thì chọn chiêu và bí quyết phòng thủ, đạo tâm.

---

## 2. Bộ đồ (loadout): người chơi tự chọn

| Phần | Số lượng | Luật |
|---|---|---|
| **Thần thông** (active) | đúng 8, không trùng | Thanh chiêu (bar) rút từ 8 chiêu này, như `POOL_SIZE` hiện nay. |
| **Bí quyết** (support) gắn vào từng thần thông | tối đa 3 mỗi chiêu, tối đa 12 cả bộ | Phải khớp tag (mục 8). Không trùng tên trên một chiêu. |
| **Tối thượng** (ultimate) | đúng 1 | Chọn trong nhóm Tối thượng. Đánh khi nộ đầy. |
| **Đạo tâm** (keystone) | 0–2 | Mục 9. |
| **Bản mệnh hành** | 0–1 | Một trong Kim, Mộc, Thủy, Hỏa, Thổ, Dương, Âm. |
| Ba chiêu cơ bản (chém, lao, độn) | luôn có | Như `basics` hiện nay. |

**Ngân sách đạo điểm** là cần gạt cân bằng chính: cả bộ không quá **50 điểm**. Thần thông 3–6 điểm theo **bậc** (C 3,
B 4, A 5, S 6), bí quyết 1–3, đạo tâm 4, tối thượng 0. Bậc ban đầu do ước lượng (mục 12.1), sau đó do số đo quyết
(mục 12.3). Bộ đồ hợp lệ là một hàm thuần trong `pixfx-duel` (`loadout::check`), dùng chung cho huấn luyện, ladder và
khi nhận bot.

Trong giai đoạn cân bằng, các yếu tố ngẫu nhiên của thanh chiêu (phẩm hiếm, trait, lucky, cặp tiến hóa) **tắt**, để số
đo chỉ phản ánh chiêu và bộ đồ. Bật lại hay bỏ hẳn là quyết định của chủ dự án sau khi có số.

---

## 3. Danh mục chiêu (tag), như gem tag của PoE

Một thần thông mang tag thuộc 4 trục. Tag quyết định bí quyết nào gắn được, đòn nào guard/né/khiên chặn được, và
"tăng X%" nào áp lên nó. Bot thấy tag (one-hot), không thấy tên.

### 3.1 Trục 1: bản chất (đúng một)

| Tag | PoE | Gốc sát thương | Đặc điểm |
|---|---|---|---|
| **Đòn** | Attack | `Lực đạo × hiệu_lực` | Ăn tốc đánh, né được, guard được. |
| **Pháp** | Spell | `gốc(hỏa hầu)` của chiêu | Ăn tốc niệm, không né được trừ khi là Đạn. |
| **Thân** | (không sát thương) | — | Buff, biến thân, độn, khiên. |

### 3.2 Trục 2: cách đưa đòn (một hoặc nhiều)

| Tag | PoE | Luật | `Kind` dùng tạm |
|---|---|---|---|
| **Cận chiến** | Melee | Tầm < 1T. Guard được. | `Melee`, `Blast` |
| **Trọng kích** | Strike | Cận chiến một mục tiêu, nhanh. | `Melee` |
| **Dậm** | Slam | Cận chiến vùng trước mặt, chậm, gây Chấn. | `Blast` |
| **Đạn** | Projectile | Bay, né được, chặn được, nuốt được. | `Bolt`, `Orb` |
| **Liên** | Chaining | Nảy hoặc lặp nhịp. | `Bolt` lặp |
| **Vùng** | Area | Có bán kính; guard không chặn. | `Zone` |
| **Nổ quanh thân** | Nova | Vùng tâm là người dùng. | `Blast` |
| **Thuấn** | (instant) | Không bay, **không né được**, không chí mạng, windup dài để đọc được. | `Bolt` tốc độ vô hạn |
| **Kênh** | Channelling | Giữ phím, bị ngắt khi trúng đòn. | `Void` |
| **Tích** | Charged | Giữ để nạp tầng, thả để đánh. | windup kéo dài |
| **Lao** | (dash) | Mang người dùng qua đối thủ. | `Dash` |
| **Ấn** | Brand / delayed | Đặt dấu, nổ sau. | `Zone` có `sigil` |
| **Thiên kiếp** | Meteor / sky strike | Từ trời xuống, có báo trước, đón đầu. | `Zone` có `aim_ahead` |
| **Bẫy** | Trap | Đặt xuống, đối thủ chạm thì kích. | `Zone` dài |
| **Mìn** | Mine | Đặt xuống, chủ ra lệnh kích. | (mới) |
| **Phiên** | Totem | Vật đứng trên sân tự đánh, có máu. | `Void` |
| **Triệu hồi** | Minion | Thực thể có máu, đánh theo. | `Void` (tạm) |
| **Cấm chế** | (mới) | Bẫy hoặc lồng ngoài thang phẩm: không giải, không phản, chỉ phá bằng sát thương. | `Zone` |

### 3.3 Trục 3: vai trò (không hoặc một)

| Tag | PoE | Luật | `Kind` dùng tạm |
|---|---|---|---|
| **Ý cảnh** | Aura | Giữ chỗ Thần thức, có hiệu lực tới khi tắt hoặc thất hồn. | (mới) |
| **Chúc phúc** | Blessing / buff | Buff có thời hạn, không giữ Thần thức. | `Heal` |
| **Nguyền** | Curse / Hex | Debuff lên đối thủ, 1 nguyền mỗi đấu sĩ. | `Zone`, `Bolt` |
| **Hống** | Warcry | Nổ quanh thân, buff mình, ngắt windup đối thủ gần. | `Blast` |
| **Hiệu lệnh** | Herald | Đòn của mình kích thêm hiệu ứng khi trúng một ailment. | (mới) |
| **Độn** | Movement | Thuấn di, đổi chỗ, súc địa. | `Blink`, `Dash` |
| **Hộ thể** | Guard | Khiên, ward, phản. | `Ward`, `Barrier` |
| **Hồi phục** | (heal) | Hồi máu, giải ailment. | `Heal` |
| **Biến thân** | Stance / transform | Đổi chỉ số có thời hạn. | `Heal` (tạm) |
| **Lĩnh vực** | (field) | Biến một nửa hoặc cả sân. Mỗi bên tối đa một. | `Void` |
| **Kích hoạt** | Trigger | Không tự dùng: tự chạy khi điều kiện xảy ra. | (mới) |
| **Liều mạng** | Vaal | Một lần mỗi trận, có Lộ bài. | (mới) |
| **Tối thượng** | (ultimate) | Đánh bằng nộ đầy. | như `tribulation` |

### 3.4 Trục 4: loại sát thương và đạo

Một hoặc nhiều trong 10 loại (mục 4) và các đạo của bản nghiên cứu mục 4 (Sinh tử, Nhân quả, Chân giả, Tuế nguyệt,
Không gian, Duyên, Mệnh, Sát lục, Nguyền, Cấm). Đạo đổi luật, không đổi bộ tính; giai đoạn đầu chỉ nối các đạo có
trong mục 11.

---

## 4. Nguyên tố: 10 loại sát thương

Thứ tự trong bảng là **thứ tự chuyển hoá** (mục 5.3) và thứ tự trong mọi vector bot thấy.

| # | Loại | Mã | Nhóm | Ailment | Mỗi tầng | Đầy tầng thì | Bị giảm bởi |
|---|---|---|---|---|---|---|---|
| 0 | **Lực** | `force` | Thân | **Thương** (đòn Kiếm, Chỉ, Trọng kích) hoặc **Chấn** (Quyền, Chưởng, Dậm) | Thương: mất 0,4% máu tối đa/s, ×2 khi di chuyển. Chấn: — | Thương: —. Chấn: choáng 0,5 s | Hộ thể |
| 1 | **Kim** | `metal` | Ngũ hành | **Phá** | −10% hộ thể | Hộ thân vỡ ngay | Kháng Kim |
| 2 | **Mộc** | `wood` | Ngũ hành | **Triền** | −8% tốc chạy; đòn Mộc hút 2% thành máu | Trói chân 0,8 s (vẫn đánh được) | Kháng Mộc |
| 3 | **Thủy** | `water` | Ngũ hành | **Thấm** | −8% hồi linh lực, +5% sát thương Lôi phải nhận | **Ngạt** 2 s: không hồi linh lực, hồi chiêu chạy ×0,5 | Kháng Thủy |
| 4 | **Hỏa** | `fire` | Ngũ hành | **Thiêu** | Mất 0,5% máu tối đa/s, xuyên hộ thể và hộ thân | — (bậc lửa, bản luật 4.3) | Kháng Hỏa |
| 5 | **Thổ** | `earth` | Ngũ hành | **Trọng** | −10% quãng lao và độn; người đang bay rơi | Quật ngã 0,6 s | Kháng Thổ |
| 6 | **Lôi** | `thunder` | Thiên | **Tê** | +4% mọi sát thương phải nhận | Giật 0,3 s, ngắt windup | Kháng Lôi |
| 7 | **Dương** | `light` | Thiên | **Chiếu** | Lộ ảo ảnh, tàng hình; buff Âm −15% | Xóa mọi buff tag Âm | Kháng Dương |
| 8 | **Âm** | `yin` | Minh | **Tịch** | −10% hồi máu, +3% sát thương Âm phải nhận | 3 s: hồi máu thành mất máu | Kháng Âm; **bỏ qua hộ thân** |
| 9 | **Hồn** | `soul` | Minh | **Nhiếp** | +6% windup | (Thần thức về 0 thì thất hồn) | Kháng Hồn; **đánh vào Thần thức** |

Khống chế cứng chỉ còn: choáng (Chấn), trói chân (Triền), quật ngã (Trọng), giật (Tê), định thân (chiêu), thất hồn.
Không chiêu nào làm đứng hình bằng băng.

### 4.1 Ngũ hành sinh khắc

- Khắc: Kim → Mộc → Thổ → Thủy → Hỏa → Kim (Kim khắc Mộc, Mộc khắc Thổ, Thổ khắc Thủy, Thủy khắc Hỏa, Hỏa khắc Kim).
- Sinh: Kim → Thủy → Mộc → Hỏa → Thổ → Kim.
- Âm và Dương khắc nhau hai chiều.

**Với bản mệnh của mục tiêu** (bước 7 của mục 5):

| Quan hệ phần sát thương với bản mệnh mục tiêu | Hệ số |
|---|---|
| Phần sát thương khắc bản mệnh | ×1,25 |
| Bản mệnh khắc phần sát thương | ×0,8 |
| Cùng hành | ×0,7, mục tiêu hồi 10% phần đó thành linh lực |
| Âm đánh bản mệnh Dương, Dương đánh bản mệnh Âm | ×1,5 |
| Còn lại (sinh, được sinh, Lực, Lôi, Hồn) | ×1,0 |

Chọn bản mệnh là được lợi và nhận điểm yếu cùng lúc: bản mệnh Hỏa giảm Kim nhận vào, nhưng Thủy đánh vào ×1,25.

**Với ailment đang dính trên mục tiêu:**

| Đòn có phần loại A, mục tiêu đang dính ailment của B | Kết quả |
|---|---|
| A khắc B (ngũ hành) | Xóa mọi tầng ailment B; thêm loại A bằng `tầng × 2% máu tối đa`. Thủy dập Thiêu, Hỏa nung Phá, Kim cắt Triền, Mộc phá Trọng, Thổ hút Thấm. |
| B sinh A | Ailment A của đòn này **+50% tầng**. Đang Triền thì Thiêu dồn nhanh; đang Thấm thì Triền dồn nhanh. |
| A = Lôi, B = Thấm | **Dẫn điện**: xóa Thấm; thêm Lôi bằng `tầng × 2% máu tối đa`. (Nước dẫn sét: thay luật "Lôi phá băng".) |
| A = Dương, B = Tịch, hoặc A = Âm, B = Chiếu | Hai bên triệt tầng một đổi một; phần dư mới thêm vào. |

**Trên đường bay** (cần thực thể vùng, làm ở giai đoạn sau): đạn Lôi của đối thủ bay qua vùng Thủy thì **tản lôi**: mất
50% sát thương và không gây Tê. Lôi thắng Thủy trên người (dẫn điện), Thủy thắng Lôi trên đường bay.

Ngoài vòng: Lôi ×1,5 lên triệu hồi tag Âm hoặc Trùng.

### 4.2 Dung hợp (Tạo)

Mỗi đấu sĩ giữ **cửa sổ dung hợp**: lượng sát thương theo loại đã nhận trong 1 s gần nhất (đo trước giảm thương). Một
đòn có loại B trúng khi cửa sổ có loại A của cùng một cặp:

- Thêm phần F = `0,4 × min(A trong cửa sổ, B của đòn)`.
- F **bỏ qua mọi kháng** và hộ thể (chỉ đạo tâm mới cho kháng F).
- A trong cửa sổ bị tiêu hết, nên một lần A chỉ dung một lần.
- Mỗi đòn dung tối đa một cặp, theo thứ tự trong bảng.

| A + B | F | Hiệu ứng thêm | Gốc |
|---|---|---|---|
| Hỏa + Thủy | **Phong** | Đẩy 1T, khóa hồi linh lực 1 s | Táng Âm: "nóng lạnh giao nhau, gió nổi lên" (CM415) |
| Lôi + Hỏa | **Lôi hỏa** | Nổ lần hai sau 0,3 s vào Thần thức bằng 50% F | Cửu Huyền (TN1089) |
| Thổ + Hỏa | **Dung nham** | Vùng 2T trong 3 s, mỗi giây 10% F loại Hỏa | Sơn Băng, núi lửa (TN) |
| Âm + Dương | **Lưỡng nghi** | Người gây đòn nhận lại 25% F | TN1306 |

Không có cặp nào sinh băng.

---

## 5. Thứ tự tính một đòn

Mỗi đòn đi đúng 12 bước theo thứ tự này. Module `calc` trong `pixfx-duel` làm bước 1–8 và 10–12; bước 9 (phẩm, khiên)
cần thực thể nên nằm ở `act`.

1. **Gốc.** Đòn: `Lực đạo × hiệu_lực`. Pháp: `gốc(L) = gốc(1) × 1,07^(L−1)`, L là hỏa hầu 1–20. Chia theo tỉ lệ loại
   của chiêu. Đạo tâm đổi loại gốc (như Âm Hàn: mọi chiêu thành Thủy) đổi **tỉ lệ ở bước này**, nên chuyển hoá không bao
   giờ cần chạy ngược.
2. **Cộng thêm.** Mỗi nguồn cộng phẳng × `hiệu_lực` của chiêu, cộng vào phần loại đó.
3. **Chuyển hoá.** Chỉ chạy **về sau** theo thứ tự mục 4. Xét từng loại theo thứ tự; tổng chuyển ra từ một loại quá
   100% thì thu nhỏ đều về 100%. Phần đã chuyển **nhớ mọi loại nó từng là** (bước 4 dùng).
4. **Tăng và nhân.** `× (1 + tăng_chung + Σ tăng của mọi loại phần đó từng là) × Π(1 + nhân_i)`. Tăng cộng gộp, nhân
   nhân riêng.
5. **Chí mạng.** Một lần cho cả đòn. Đòn Thuấn không chí mạng.
6. **Dung hợp** (mục 4.2).
7. **Ngũ hành** với bản mệnh mục tiêu, rồi khắc/sinh với ailment trên mục tiêu (mục 4.1).
8. **Giảm thương.**
   - Lực: `giảm = H / (H + 5 × D)`, H là hộ thể sau Phá, D là phần Lực của đòn, trần 90%. Đòn to thì hộ thể ít tác dụng.
   - 9 loại còn lại: `× (1 − (R − xuyên kháng))`, R trong [−100%, 75%].
   - Nhận thêm: `× (1 + 4% × tầng Tê)` mọi loại, `× (1 + 5% × tầng Thấm)` cho Lôi, `× (1 + 3% × tầng Tịch)` cho Âm.
9. **Phẩm và khiên** (bản luật mục 5), ở `act`.
10. **Frame data.** Phản đòn (trúng windup) ×1,2; phạt hớ (trúng recover) ×1,2 (như `FRAMES` hiện nay).
11. **Trừ vào thanh.** Lực, ngũ hành, Lôi, Dương và phần dung hợp: hộ thân trước, dư trừ máu. Âm: thẳng vào máu. Hồn:
    Thần thức trước, phần dư × 0,5 vào máu; mục tiêu không hồn thì phần Hồn mất.
12. **Ailment.** Tính trên phần mỗi loại **sau bước 7, trước giảm thương**:
    `tầng thêm = k × phần / máu tối đa × 20 × (1 − kháng ailment)`, phần lẻ cộng dồn. Kháng cao giảm sát thương nhưng
    không giảm ailment theo cùng tỉ lệ; muốn chống ailment phải lấy kháng ailment.

Ghi chú về con số 20 ở bước 12: bản luật dùng ×100, tức một đòn 10% máu đầy luôn 8 tầng Thiêu. Với 20, một đòn 10%
máu thêm 2 tầng (k = 1), đầy sau 4–5 đòn.

### 5.1 Ailment: số

| Ailment | k | Trần | Thời hạn tầng | Khống chế khi đầy |
|---|---|---|---|---|
| Thương | 1,0 | 10 | 4 s | — |
| Chấn | 1,2 | 10 | 3 s | choáng 0,5 s |
| Phá | 1,0 | 5 | 5 s | (vỡ hộ thân) |
| Triền | 1,0 | 6 | 3 s | trói 0,8 s |
| Thấm | 1,0 | 6 | 4 s | (Ngạt 2 s, không phải khống chế cứng) |
| Thiêu | 0,8 | 8 | 4 s | — |
| Trọng | 1,0 | 5 | 3 s | quật ngã 0,6 s |
| Tê | 1,0 | 5 | 3 s | giật 0,3 s |
| Chiếu | 1,0 | 3 | 3 s | (xóa buff Âm) |
| Tịch | 1,0 | 6 | 5 s | (hồi thành mất 3 s) |
| Nhiếp | 1,0 | 5 | 3 s | — |

Mỗi tầng hết hạn riêng, rụng cũ trước. Đầy tầng: hiệu ứng đầy, tầng về 0, khóa tích 2 s.

### 5.2 Kiên (giảm khống chế cứng), thay `GRIT`

`thời gian thật = cơ bản × 0,5^Kiên`. Mỗi khống chế cứng Kiên +1, trần 3; Kiên −1 mỗi 2 s không bị khống chế.

---

## 6. Phòng thủ

| Lớp | Chặn gì | Ghi chú |
|---|---|---|
| **Né** | Đòn và Đạn, không chặn Thuấn | Một lần rút RNG mỗi đòn. |
| **Guard** | Cận chiến, Đạn, Lao (như `GUARDED`) | Giữ như nay: 8 linh lực mỗi đòn đỡ, 20% lọt qua. Không chặn Vùng. |
| **Khiên / ward** | Theo phẩm (bản luật 5) | Ward xóa cả một đòn mỗi lần. |
| **Hộ thể** | Lực | Bước 8. |
| **Kháng** | 9 loại | Bước 8. |
| **Hộ thân** | Mọi loại trừ Âm và Hồn | Bước 11. |
| **Thần thức** | Hồn | Bước 11. |

---

## 7. Tầm

**T** = chiều cao đấu sĩ, 60 px của engine (240 px của sân, `UNIT` 4). Tầm của chiêu hiện nay nằm trong 96–232 px
(engine), tức khoảng 1,6–3,9T.

| Băng tầm | Khoảng | Hệ số ngân sách sát thương (mục 12.1) |
|---|---|---|
| Cận | < 1T | 1,30 |
| Trung | 1–4T | 1,00 |
| Xa | > 4T | 0,80 |
| Toàn sân | cả sân | 0,65 |

Tầm của mỗi chiêu là một con số đo được trong ladder: ở khoảng cách nào chiêu được dùng và trúng (mục 12.2). Khi tầm
thật lệch băng thiết kế, sửa tầm hoặc sửa băng.

---

## 8. Bí quyết (support)

Giai đoạn đầu dùng các bí quyết đổi **con số** (bộ tính làm được ngay). Bí quyết đổi **hành vi** (Dung, Lôi Bạo Trễ,
Thôn Cấm, Sao Chép…) vào sau, theo bản nghiên cứu mục 11.

| Bí quyết | Đòi tag | Làm gì (trong bộ tính) | Chi phí × | PoE |
|---|---|---|---|---|
| **Thanh Trần** | bất kỳ | nhân +25% | 1,2 | Increased Effect |
| **Liên Châu** | Đạn | thêm 2 đạn, mỗi đạn nhân −30% | 1,3 | Multiple Projectiles |
| **Tàn Ảnh Tốc Thần** | Đòn, Cận chiến | đánh lặp 2 lần, nhân −25% | 1,5 | Multistrike |
| **Trọng** | bất kỳ | thêm Thổ bằng 15% gốc; windup +20% | 1,1 | Added damage |
| **Hỏa Luyện / Kim Phong / Thủy Ý / Lôi Dẫn / Mộc Sinh** | bất kỳ | chuyển 50% Lực sang hành đó | 1,1 | Conversion |
| **Hóa Ma** | bất kỳ | chuyển 100% về Âm (chỉ từ các loại trước Âm) | 1,2 | Conversion |
| **Phun Máu** | bất kỳ | trả 10% máu: nhân +30% | máu | Blood magic |
| **Chắc Trúng** | bất kỳ | không né được; windup +40% | 1,2 | Accuracy |
| **Xuyên Giáp** | Đòn | xuyên 30% hộ thể | 1,2 | Impale / Penetration |
| **Phá Kháng (hành)** | Pháp | xuyên 20% kháng của một loại | 1,2 | Penetration |
| **Ác Tính** | bất kỳ | ailment +50% tầng, sát thương đòn −15% | 1,2 | Unbound Ailments |
| **Chí Mạng** | không Thuấn | chí mạng +10%, hệ số +0,3 | 1,2 | Increased Critical |
| **Đoạt Tu Vi** | bất kỳ | hút 5% sát thương thành máu | 1,2 | Life Leech |
| **Nhử Đòn** | khống chế | khống chế ×2 nếu mục tiêu vừa dùng chiêu Hộ thể | 1,2 | (yomi) |

Hệ số chi phí của các bí quyết nhân với nhau.

---

## 9. Đạo tâm (keystone), giai đoạn đầu

Chỉ các đạo tâm bộ tính làm được. Mỗi cái có được có mất.

| Đạo tâm | Được | Mất |
|---|---|---|
| **Cực Cảnh** (chọn 1 loại) | Loại đó nhân +60% | Không gắn được bí quyết chuyển hoá |
| **Âm Hàn Linh Lực** | Mọi chiêu thành Thủy ở bước 1 | Không gắn bí quyết hành khác |
| **Tiên Trì** | Mọi Pháp nhân +40% | Không bật được Ý cảnh |
| **Hai Mươi Năm Khổ Luyện** | Chiêu phẩm 1 nhân +100% | Không dùng chiêu phẩm 3+ |
| **Bất Khuất** (từ `unyielding`) | Một lần mỗi hiệp, đòn chết để lại 1 máu | Máu tối đa −15% |
| **Sát Ý** (từ `fury`) | Mỗi đòn trúng +4% sát thương, 5 tầng, tới hết hiệp | Hộ thân tối đa −50% |
| **Tâm Pháp** (từ `attuned`) | Mỗi hai pháp tốn linh lực cho 1 nộ | Đòn không cho nộ |
| **Hệ Đại Thành** (theo bản mệnh) | Miễn loại bản mệnh | Hành khắc bản mệnh đánh vào ×1,5 thay ×1,25 |

---

## 10. Chi phí, hồi chiêu, nộ

- Chi phí trả khi windup bắt đầu; bị phá giữa windup là mất (như nay).
- Nộ: 4 điểm, +1 mỗi đòn trúng (trừ tối thượng). Đầy thì đánh tối thượng đã chọn.
- Chiêu tốn máu: không bao giờ chết vì trả máu; mỗi 1% máu tối đa đã trả thì nhân +3% (`BLOOD_POWER`).

---

## 11. Pool thần thông chung

Mọi chiêu dưới đây ai cũng chọn được. Cột **Tạm** là `Kind` và hiệu ứng pixelfx đang dùng, giữ nguyên cho tới khi có
effect riêng. Chiêu đã có trong `data.rs` giữ tên mã; chiêu mới từ bản nghiên cứu mang tên mã mới và dùng tạm một
`Kind` gần nhất. Loại sát thương ghi theo tỉ lệ khi chiêu chia nhiều loại.

### 11.1 Chiêu cơ bản (luôn có, không tốn đạo điểm)

`slash` (Đòn · Trọng kích · Lực), `dive` (Đòn · Lao · Lực), `blink` (Thân · Độn).

### 11.2 Đòn cận chiến (Trọng kích, Dậm, Lao)

| Chiêu | Mã | Tag | Loại | Tạm |
|---|---|---|---|---|
| Bộ (step) | `step` | Đòn · Lao | Lực | Dash |
| Chưởng (palm) | `palm` | Đòn · Cận · Dậm | Lực | Blast |
| Bạo (burst) | `burst` | Đòn · Nổ quanh thân | Lôi | Blast |
| Cổ Thần Huyết | `ancientgod` | Đòn · Dậm | Lực | Blast |
| Trảm La | `slashluo` | Đòn · Lao | Kim | Dash |
| Nhất Bộ Đăng Thiên | `onestep` | Đòn · Lao | Lực | Dash |
| Cửu Chuyển Luyện | `nineturn` | Đòn · Dậm · Tích | Lực | Blast |
| Chu Tước Biến | `vermilion` | Đòn · Cận | Hỏa | Blast |
| Thiên Trảm | `heavenlychop` | Đòn · Trọng kích | Kim | Blast |
| Hỏa Độn | `fireescape` | Đòn · Lao | Hỏa | Dash |
| Chiến Hồn Ấn | `warprint` | Đòn · Dậm | Lực 50 / Kim 50 | Blast |
| Thất Thải Đạo Thương | `sevencolor` | Đòn · Lao | Dương | Dash |
| Phệ Linh | `devour` | Đòn · Cận | Âm | Blast |
| Trăm Quyền Sinh Cơ | `hundredfist` | Đòn · Trọng kích · Liên | Lực | Melee (mới) |
| Thập Băng Quyền Ý | `tenfist` | Đòn · Trọng kích · Liên | Lực | Melee (mới) |
| Man Thần Biến | `manshen` | Đòn · Trọng kích · Liên | Lực | Melee (mới) |
| Thôn Linh Phệ Thể | `devourbody` | Đòn · Lao | Âm | Dash (mới) |
| Thu Ý | `autumn` | Đòn · Cận · Vùng | Kim 50 / Âm 50 | Blast (mới) |
| Man Thương | `manspear` | Đòn · Lao · Tuyến | Lực | Dash (mới) |
| Thần Chấn | `godquake` | Đòn · Dậm | Thổ | Blast (mới) |

### 11.3 Đạn và Thuấn

| Chiêu | Mã | Tag | Loại | Tạm |
|---|---|---|---|---|
| Lôi (bolt) | `bolt` | Pháp · Đạn | Lôi | Bolt |
| Cầu (orb) | `orb` | Pháp · Đạn | Lôi | Orb |
| Hoa (spark) | `spark` | Pháp · Đạn | Hỏa | Bolt |
| Hỏa Cầu | `fireball` | Pháp · Đạn | Hỏa | Bolt |
| Diệt Chỉ | `finger` | Pháp · Đạn | Kim | Bolt |
| Cực Lôi | `jithunder` | Pháp · Đạn | Lôi | Bolt |
| Hóa Ma Chỉ | `demonfinger` | Pháp · Đạn | Âm | Bolt |
| Thiên Hổ Phiên | `tigerflag` | Pháp · Đạn · Tuyến | Kim | Bolt |
| Thiên Lục | `celestialslaughter` | Pháp · Đạn · Tích | Lôi 50 / Lực 50 | Bolt |
| Hóa Linh | `spiritchange` | Pháp · Đạn | Hồn | Bolt |
| Thiên Nhãn | `thirdeye` | Pháp · Thuấn | Dương | Bolt |
| Đoạt Cơ | `seize` | Pháp · Đạn | Âm | Bolt |
| Thất Thải Hư Không Đinh | `voidnail` | Pháp · Đạn | Kim | Bolt |
| Ngưng Tuyền Hồn | `springsoul` | Pháp · Đạn | Hồn (bỏ chill) | Bolt |
| Thủy Tiễn (jet) | `jet` | Pháp · Cận · Tuyến | Thủy (bỏ chill) | Blast |
| Cực Cảnh Thần Thức | `extremesense` | Pháp · Thuấn | Hồn | Bolt (mới) |
| Định Thân Thuật | `bodyfix` | Pháp · Thuấn · khống chế | Hồn | Zone → Bolt |
| Tịch Diệt Chỉ | `silentfinger` | Pháp · Đạn | Âm | Bolt (mới) |
| Ngũ Tiễn Liên Châu | `fivearrows` | Đòn · Đạn · Liên | Lực | Bolt (mới) |
| Cung Lý Quảng | `libow` | Đòn · Đạn · Tuyến · Liều mạng | Kim | Bolt (mới) |
| Cổ Thần Chỉ | `godfinger` | Đòn · Đạn | Lực | Bolt (mới) |
| Sát Lục Lôi | `slaughterbolt` | Pháp · Thuấn | Lôi | Bolt (mới) |
| Hô Phong | `summonwind` | Pháp · Đạn | Thủy 50 / Âm 50 (bỏ chill) | Blast → Bolt |

### 11.4 Vùng, Ấn, Thiên kiếp, Bẫy, Cấm chế

| Chiêu | Mã | Tag | Loại | Tạm |
|---|---|---|---|---|
| Ấn (seal) | `seal` | Pháp · Ấn | Lôi | Zone |
| Sụp (collapse) | `collapse` | Pháp · Vùng | Thổ | Zone |
| Chấn Địa (quake) | `quake` | Pháp · Vùng | Thổ | Zone |
| Trọng Lực Thuật | `gravity` | Pháp · Vùng | Thổ | Zone |
| Địa Liệt | `earthsplit` | Pháp · Vùng | Thổ | Zone |
| Tụ Lôi Ấn | `gather` | Pháp · Ấn | Lôi | Zone |
| Sơn Băng | `mountain` | Pháp · Thiên kiếp | Thổ 60 / Hỏa 40 | Zone |
| Thái Cổ Lôi Vực | `ancientthunder` | Pháp · Thiên kiếp | Lôi | Zone |
| Tử Chỉ | `fingerdeath` | Pháp · Thiên kiếp | Âm | Zone |
| Tam Giới Hỏa Nguyên | `flameorigin` | Pháp · Vùng | Hỏa | Zone |
| Bát Cực Đạo | `eightextremes` | Pháp · Thiên kiếp · Liên | Hỏa | Zone |
| Âm Nguyệt | `yinmoon` | Pháp · Ấn | Âm | Zone |
| Diệt Cấm | `restriction` | Pháp · Cấm chế | Kim | Zone |
| Thập Bát Mai Hoa Cấm | `plumblossom` | Pháp · Cấm chế | Mộc | Zone |
| Sinh Tử Ấn | `lifedeath` | Pháp · Ấn · Sinh tử | Âm 50 / Mộc 50 | Zone |
| Tàn Dạ Thiên Liệt | `remnantnight` | Pháp · Thiên kiếp | Âm 50 / Dương 50 | Zone |
| Phiên Thiên Ấn | `heavenflip` | Pháp · Thiên kiếp | Thổ | Zone |
| Phần Thế Tán | `umbrella` | Pháp · Vùng | Hỏa | Zone |
| Lạc Tinh | `starfall` | Pháp · Thiên kiếp | Dương | Zone |
| Thời Cấm | `timerestrict` | Pháp · Cấm chế | Hồn | Zone |
| Linh Nô Ấn | `servitude` | Pháp · Ấn | Hồn | Zone |
| Hồn Xoáy | `soulvortex` | Pháp · Vùng | Hồn | Zone |
| Tinh Chuyển | `starrotation` | Pháp · Thiên kiếp · Liên | Dương | Zone |
| Chân Giả Ấn | `truefalse` | Pháp · Ấn · Chân giả | Hồn 50 / Dương 50 | Zone |
| Diệt Sinh Triệu Linh | `spiritsummon` | Pháp · Thiên kiếp · Liều mạng | Âm 50 / Hỏa 50 | Zone |
| Tam Đại Cấm Chế | `threerestrict` | Pháp · Cấm chế | Lực | Zone (mới) |
| Di Sơn | `movemountain` | Pháp · Thiên kiếp | Thổ | Zone (mới) |
| Nhân Quả Ấn | `karmaseal` | Pháp · Ấn · Nhân quả | Hồn | Zone (mới) |

### 11.5 Kênh, Phiên, Triệu hồi, Lĩnh vực

| Chiêu | Mã | Tag | Loại | Tạm |
|---|---|---|---|---|
| Hư Không (void) | `void` | Pháp · Kênh | Âm | Void |
| Vạn Kiếm Trận | `arsenal` | Pháp · Phiên | Kim | Void |
| Hoán Vũ | `summonrain` | Pháp · Vùng · Ý cảnh | Thủy (bỏ chill) | Void |
| Tát Đậu Thành Binh | `beans` | Pháp · Triệu hồi | Lực | Void |
| Sinh Tử Cấm | `lifedeathrestrict` | Pháp · Cấm chế | Âm | Void |
| Hồn Phiên | `soulbanner` | Pháp · Phiên | Hồn | Void |
| Triệu Quỷ | `ghostsummon` | Pháp · Triệu hồi | Âm | Void |
| Hoàng Tuyền Đạo Hóa | `daoyellow` | Pháp · Lĩnh vực | Âm (bỏ chill) | Void |
| Du Hồn | `wanderingsouls` | Pháp · Triệu hồi | Hồn | Void (mới) |
| Cấm Phiên | `restrictbanner` | Pháp · Phiên · Cấm chế | Âm 50 / Lực 50 | Void (mới) |
| Hoàng Tuyền Chỉ | `yellowfinger` | Pháp · Kênh | Âm | Void (mới) |
| Nguyệt Dực | `moonwing` | Pháp · Triệu hồi | Hỏa | Void (mới) |

### 11.6 Nguyền, Hống, Hồn

| Chiêu | Mã | Tag | Loại | Tạm |
|---|---|---|---|---|
| Tâm Động Lôi Âm | `heartbeat` | Pháp · Nổ quanh thân · Hống | Lôi 50 / Hồn 50 | Blast |
| Côn Cực Tiên | `kunjiwhip` | Pháp · Cận · Tuyến | Hồn | Blast |
| Sưu Hồn | `soulsearch` | Pháp · Cận | Hồn | Blast |
| Mộng Đạo | `dreamdao` | Pháp · Nguyền | Hồn | Blast |
| Hoàng Tuyền Chỉ (cũ) | `yellowspring` | Pháp · Cận | Âm (bỏ chill; ăn Tịch thay Chill) | Blast |
| Tinh Hồn Trích | `starsoul` | Pháp · Cận | Hồn | Blast |
| Hex | `hex` | Pháp · Nguyền | Âm | Ward → nguyền (mới) |
| Thảo Kết Ký Sự | `strawknot` | Pháp · Nguyền · Kênh | Hồn | Zone (mới) |
| Trớ Thuật Tiên Tổ | `ancestorcurse` | Pháp · Nguyền | Âm | Zone (mới) |
| Khắc Ấn | `exposemark` | Pháp · Nguyền | — (−20% kháng một loại) | Bolt (mới) |

### 11.7 Hộ thể, Hồi phục, Biến thân, Độn, Ý cảnh

| Chiêu | Mã | Tag | Tạm |
|---|---|---|---|
| Ward | `ward` | Thân · Hộ thể | Ward |
| Chuông (bell) | `bell` | Thân · Hộ thể | Barrier |
| Gương (mirror) | `mirror` | Thân · Hộ thể | Barrier |
| Quang Ảnh Thuẫn | `lightshield` | Thân · Hộ thể | Barrier |
| Cổ Hộ | `ancientprotect` | Thân · Hộ thể | Ward |
| Nhân Quả Thuẫn | `karma` | Thân · Hộ thể · Nhân quả | Barrier |
| Mộng Hồi Viễn Cổ | `dreamback` | Thân · Hộ thể | Ward |
| Công Đức Linh | `meritspirit` | Thân · Hộ thể | Ward |
| Hồi (heal) | `heal` | Thân · Hồi phục | Heal |
| Bất Diệt Thân | `indestructible` | Thân · Hồi phục | Heal |
| Lưu Nguyệt | `flowingmoon` | Thân · Hồi phục · Tuế nguyệt | Heal |
| Luyện Hồn | `soulrefine` | Thân · Hồi phục | Heal |
| Tố Thân | `bodyformation` | Thân · Hồi phục | Heal |
| Cổ Thần Hóa | `godform` | Thân · Biến thân | Heal (mới) |
| Túc Mệnh | `fate` | Thân · Biến thân · Tuế nguyệt | Heal (mới) |
| Súc Địa | `inchearth` | Thân · Độn | Blink |
| Thổ Độn | `earthescape` | Thân · Độn | Blink |
| Cổ Thần Đỉnh | `ancientcauldron` | Thân · Độn (đổi chỗ) | Blink |
| Tinh Bàn | `starcompass` | Thân · Độn (đổi chỗ) | Blink |
| Thuấn Di | `teleport` | Thân · Độn | Blink (mới) |
| Sát Lục Ý Cảnh | `slaughteraura` | Thân · Ý cảnh | (mới) |
| Tuế Nguyệt Ý Cảnh | `timeaura` | Thân · Ý cảnh | (mới) |

### 11.8 Tối thượng

| Chiêu | Mã | Loại | Tạm |
|---|---|---|---|
| Thiên Kiếp (tribulation) | `tribulation` | Lôi | Zone |
| Tàn Dạ | `remnantdusk` | Hỏa 50 / Dương 50 | Zone (mới) |
| Tô Kiếp | `sukiep` | đổi theo kiếp (mỗi nhịp một loại) | Zone (mới) |
| Đạo Pháp Dung Hợp | `daofusion` | Lực 34 / Lôi 33 / Hỏa 33 | Blast (đưa từ pool lên) |

### 11.8 Bỏ

`icicle`, `shard`, `shatter`, `frostfire`, `flurry`, `prison` (băng tự chế). `lotus` đổi thành Dương (Đòn · Cận),
giữ mã.

---

## 12. Cân bằng: bậc và tầm từ số đo

Phần này là lý do làm backend trước. Không đoán bậc bằng cảm giác; đo.

### 12.1 Ngân sách khởi điểm (trước khi đo)

Mỗi chiêu nhận một **điểm sức** dự kiến:

```text
sức = (sát thương mong đợi mỗi lần dùng) / (windup + recover + 0,25 × hồi chiêu)
      / hệ_số_tầm (mục 7) / hệ_số_an_toàn
```

`hệ_số_an_toàn`: Thuấn 1,25 (không né), Vùng 1,10 (guard không chặn), unsafe on block 0,85. Ailment quy ra sát thương
theo hiệu ứng đầy. Chiêu trong cùng băng tầm được chỉnh về cùng điểm sức ± 10%, rồi xếp bậc C–S theo hiệu quả ngoài sát
thương (khống chế, độn, phòng thủ).

### 12.2 Đo gì

Ladder và huấn luyện chạy trên **bộ đồ ngẫu nhiên** (rút đều trong các bộ hợp lệ), nên mỗi chiêu có mặt trong hàng nghìn
trận với đủ loại bạn đồng hành. Telemetry ghi cho mỗi chiêu:

| Số đo | Nghĩa |
|---|---|
| **Tỉ lệ thắng khi có mặt** (inclusion WR) | Thắng bao nhiêu % trận có chiêu trong bộ, trừ đi trung bình. |
| **Tỉ lệ thắng khi dùng** | Như trên, chỉ tính trận chiêu được dùng ≥ 1 lần. |
| **Tỉ lệ dùng** | Bot đã huấn luyện chọn chiêu khi nó có trên thanh bao nhiêu %. |
| **Sát thương / linh lực**, **sát thương / giây cam kết** | Hiệu quả thô. |
| **Tầm dùng thật** | Phân bố khoảng cách hai đấu sĩ lúc dùng và lúc trúng. |
| **Tỉ lệ trúng, bị guard, bị né, bị phạt** | Chiêu có an toàn như thiết kế không. |
| Với bí quyết, đạo tâm, bản mệnh | Inclusion WR và tỉ lệ chọn như trên. |

Hồi quy tuyến tính (hoặc logistic) kết quả trận theo các chiêu có mặt ở hai bên cho mỗi chiêu một hệ số, tách được đóng
góp của nó khỏi bạn đồng hành.

### 12.3 Bậc và mục tiêu

- **Bậc** = hệ số hồi quy chia vào 4 nhóm theo phân vị (S 15% trên cùng, A 35%, B 35%, C 15% dưới). Bậc đổi chi phí đạo
  điểm (mục 2), nên bộ đồ "toàn S" không vừa ngân sách.
- **Mục tiêu**: mọi chiêu có inclusion WR trong 50% ± 3; tỉ lệ dùng trong 3–40%; mọi nhóm ở mục 11 có ít nhất hai chiêu
  ở bậc A trở lên; mọi loại sát thương có ít nhất một build thắng ≥ 50% trước bot nhà.
- **Tầm**: chiêu có tầm dùng thật (trung vị) nằm ngoài băng thiết kế quá 20% thì sửa.
- Một vòng: sửa số trong `data.rs`, huấn luyện tiếp từ bot trước (không từ đầu), ladder 150 seed × bộ đồ ngẫu nhiên, đọc
  bảng. Thay đổi lớn về luật vẫn là quyết định của chủ dự án.

---

## 13. Bot và huấn luyện

- **Quan sát mỗi slot**: tag one-hot (3 bản chất, 18 cách đưa, 13 vai trò), 10 tỉ lệ loại sát thương sau chuyển hoá,
  sát thương mong đợi (đã qua bộ tính với mục tiêu hiện tại), chi phí, hồi chiêu, windup, recover, tầm.
- **Quan sát mỗi đấu sĩ**: máu, hộ thân, thần thức, linh lực, nộ, 11 ailment (tầng và khóa), cửa sổ dung hợp (10),
  Kiên, bản mệnh (8 one-hot), kháng (10), hộ thể.
- **Hành động**: không đổi số slot (3 cơ bản, 3 thanh, tối thượng).
- **Bộ đồ trong huấn luyện**: rút ngẫu nhiên mỗi trận cho cả hai bên, để một mạng chơi được mọi bộ.
- **Bộ đồ trong giải**: module bot khai báo bộ đồ (một export mới của ABI, `docs/bot.md`), arena kiểm bằng
  `loadout::check`. Đây là thay đổi ABI, cần mùa mới.

Kích thước đầu vào đổi, nên `pixfx-mind` (`policy`) và mọi bot đổi theo; bot nhà phải huấn luyện lại.

---

## 14. Làm theo thứ tự nào (backend trước, chưa UI)

1. **Bộ tính** (`crates/pixfx-duel/src/calc.rs`): loại, ailment, chuyển hoá, tăng/nhân, chí mạng, dung hợp, ngũ hành,
   giảm thương, ba thanh, Kiên. Hàm thuần, test theo từng bước. **Đã làm cùng bản này.**
2. **Danh mục** (`gem.rs`): mỗi chiêu mục 11 có tag, loại, hiệu_lực/gốc, bậc; bí quyết và đạo tâm mục 8–9 thành dữ
   liệu áp lên `Hit`. `loadout::check`.
3. **Nối vào trận**: bỏ `School` và chill/freeze; `Fighter` mang bộ đồ và trạng thái của bộ tính; `status::damage`
   gọi `calc`; thanh chiêu rút từ bộ đồ. Re-pin goldens (`crates/pixfx-duel/AGENTS.md`).
4. **Môi trường**: quan sát mục 13, bộ đồ ngẫu nhiên trong `train`, `pixfx-duel-py`.
5. **Telemetry cân bằng**: ladder ghi các số đo mục 12.2 theo chiêu; một example in bảng bậc.
6. **Huấn luyện và vòng cân bằng** (mục 12.3).
7. Sau cùng: hành vi mới (triệu hồi, ý cảnh, lĩnh vực, nguyền, phẩm), effect riêng, UI.
