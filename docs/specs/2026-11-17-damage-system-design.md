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
Không gian, Duyên, Mệnh, Sát lục, Nguyền, Cấm). Đạo đổi luật, không đổi bộ tính. Luật của từng đạo là bản luật mục 6,
làm đủ trong `calc::dao` và `calc::curse` (mục 15).

---

## 4. Nguyên tố: 10 loại sát thương

Thứ tự trong bảng là **thứ tự chuyển hoá** (mục 5.3) và thứ tự trong mọi vector bot thấy.

| # | Loại | Mã | Nhóm | Ailment | Mỗi tầng | Đầy tầng thì | Bị giảm bởi |
|---|---|---|---|---|---|---|---|
| 0 | **Lực** | `force` | Thân | **Thương** (đòn Kiếm, Chỉ, Trọng kích) hoặc **Chấn** (Quyền, Chưởng, Dậm) | Thương: mỗi đòn một vết chảy máu, 35% phần Lực/s trong 5 s, ×2 khi di chuyển (mục 17.5). Chấn: — | Thương: — (đủ 8 vết). Chấn: choáng 0,5 s | Hộ thể |
| 1 | **Kim** | `metal` | Ngũ hành | **Phá** | −10% hộ thể | Hộ thân vỡ ngay | Kháng Kim |
| 2 | **Mộc** | `wood` | Ngũ hành | **Triền** | −8% tốc chạy; đòn Mộc hút 2% thành máu | Trói chân 0,8 s (vẫn đánh được) | Kháng Mộc |
| 3 | **Thủy** | `water` | Ngũ hành | **Thấm** | −8% hồi linh lực, +5% sát thương Lôi phải nhận | **Ngạt** 2 s: không hồi linh lực, hồi chiêu chạy ×0,5 | Kháng Thủy |
| 4 | **Hỏa** | `fire` | Ngũ hành | **Thiêu** | Mỗi đòn một vết cháy, 50% phần Hỏa/s trong 4 s, chỉ vết mạnh nhất cháy; xuyên hộ thể và hộ thân (mục 17.5) | — (đủ 8 vết; lửa đen thì hút Thần thức) | Kháng Hỏa |
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
| A khắc B (ngũ hành) | Xóa mọi tầng ailment B; thêm loại A bằng `tầng × 2% máu tối đa`. Thủy dập Thiêu, Hỏa nung Phá, Kim cắt Triền, Mộc phá Trọng, Thổ hút Thấm. Với Thiêu và Thương (vết, mục 17.5) thì thêm **phần sát thương còn lại** của các vết bị xóa, đúng như bản nghiên cứu 3.1 ("gây thêm sát thương bằng phần ailment còn lại"): Thiêu chỉ tính vết mạnh nhất (`dps × thời gian còn`, rồi vết kế tiếp khi nó hết), Thương tính mọi vết, nhân `OVERCOME_REMAINING` (1,0). Nhiều vết cháy nhỏ vì thế chỉ đổi được đúng phần chúng còn cháy. |
| B sinh A | Ailment A của đòn này **+50% tầng**; với Thiêu, vết cháy mới **+50% hiệu lực** (không thêm vết). Đang Triền thì Thiêu cháy mạnh; đang Thấm thì Triền dồn nhanh. |
| A = Lôi, B = Thấm | **Dẫn điện**: xóa Thấm; thêm Lôi bằng `tầng × 2% máu tối đa`. (Nước dẫn sét: thay luật "Lôi phá băng".) |
| A = Dương, B = Tịch, hoặc A = Âm, B = Chiếu | Hai bên triệt tầng một đổi một; phần dư mới thêm vào. |

**Trên đường bay**: đạn Lôi của đối thủ bay qua vùng Thủy thì **tản lôi**: mất
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
    `tầng thêm = k × phần / máu tối đa × 20 × (1 − kháng ailment) × hiệu_lực_ailment`, phần lẻ cộng dồn; mỗi tầng sống
    `thời hạn × hệ_số_thời_gian_ailment`. Kháng cao giảm sát thương nhưng không giảm ailment theo cùng tỉ lệ; muốn chống
    ailment phải lấy kháng ailment. Thiêu và Thương không đặt tầng mà đặt **một vết** mỗi đòn (mục 17.5). Loại mà mục
    tiêu miễn nhiễm (Hệ Đại Thành) không đặt ailment.

Ghi chú về con số 20 ở bước 12: bản luật dùng ×100, tức một đòn 10% máu đầy luôn 8 tầng Thiêu. Với 20, một đòn 10%
máu thêm 2 tầng (k = 1), đầy sau 4–5 đòn.

### 5.1 Ailment: số

| Ailment | k | Trần | Thời hạn tầng | Khống chế khi đầy |
|---|---|---|---|---|
| Thương | — | 8 vết | 5 s mỗi vết | — |
| Chấn | 1,2 | 10 | 3 s | choáng 0,5 s |
| Phá | 1,0 | 5 | 5 s | (vỡ hộ thân) |
| Triền | 1,0 | 6 | 3 s | trói 0,8 s |
| Thấm | 1,0 | 6 | 4 s | (Ngạt 2 s, không phải khống chế cứng) |
| Thiêu | — | 8 vết | 4 s mỗi vết | — |
| Trọng | 1,0 | 5 | 3 s | quật ngã 0,6 s |
| Tê | 1,0 | 5 | 3 s | giật 0,3 s |
| Chiếu | 1,0 | 3 | 3 s | (xóa buff Âm) |
| Tịch | 1,0 | 6 | 5 s | (hồi thành mất 3 s) |
| Nhiếp | 1,0 | 5 | 3 s | — |

Mỗi tầng hết hạn riêng, rụng cũ trước. Đầy tầng: hiệu ứng đầy, tầng về 0, khóa tích 2 s. Thiêu và Thương là DoT
(mục 17.5): "đầy" là lúc số vết chạm 8 từ dưới lên; vết không về 0 và không bị khóa. Hiệu lực ailment (`AilmentEffect`)
nhân số tầng thêm, thời gian ailment (`AilmentDuration`) nhân thời hạn mỗi tầng; tầng được sinh (mục 4.1) không kéo dài
thời hạn.

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

Đủ mọi bí quyết của bản nghiên cứu mục 11 (kể cả các bí quyết đổi hành vi: Dung, Lôi Bạo Trễ, Thôn Cấm, Sao Chép,
Phản Ấn, Khí Nhân Quả, Tự Bạo…), các bí quyết bản này thêm, và 72 bí quyết theo support của PoE (từ Phụ Hỏa trở
xuống). Luật gắn và loại trừ là bản luật mục 2.2: chiêu phải có mọi tag ở `all`, ít nhất một tag ở `any`, không tag
nào ở `none` (tag gồm cả loại sát thương của chiêu); loại trừ đọc hai chiều; `loadout::check` từ chối bí quyết không
vừa tag hoặc bị loại trừ, `loadout::random` chỉ bốc bộ đồ hợp lệ. Hệ số chi phí của các bí quyết nhân với nhau. Số của
mọi bí quyết là `pub const` trong `calc::support`; bảng dưới là toàn bộ, theo thứ tự trong `SUPPORTS` (119).

Bí quyết nói bằng modifier giàu (mục 17.1): `Per` (mỗi tầng, mỗi nguyền, mỗi T...), điều kiện mới (mỗi lần thứ n,
mục tiêu có hồn, đã tiêu linh lực gần đây...) và stat mới (gain as extra, trảm, hồi khi trúng, tinh khi X). Việc chỉ
trận đấu làm được là một `Behaviour` (đánh lại sau, gieo nguyền khi trúng, nguyền thành vùng, không lên mình, ảo
linh, phóng khi kênh, ấn tích, bẫy/mìn/đàn); các stat cần vị trí có hành vi trên sân (mục 17.8). Bí quyết kích hoạt mang một
`event::Trigger` (`Support::trigger`), `Support::trigger_for(chiêu)` nhắm nó vào chính chiêu được gắn. Bẫy, mìn, đàn
thêm tag cho chiêu (`Mods::becomes`): mod đòi tag đó (một nút "chiêu Đàn nhân +50%") áp được, và đòn mang tag ấy.
Nước gây Thấm, không bao giờ lạnh hay đóng băng: Thủy Phệ là bản Thủy của Hypothermia.

| id | Bí quyết | Đòi tag | Làm gì (trong bộ tính) | Chi phí × | Điểm | Loại trừ | PoE |
|---|---|---|---|---|---|---|---|
| `focus` | **Thanh Trần** | any | nhân +25% | 1,2 | 2 | — | Increased Effect |
| `multishot` | **Liên Châu** | any projectile | +2 đạn, nhân −30% | 1,3 | 2 | — | Greater Multiple Projectiles |
| `afterimage` | **Tàn Ảnh Tốc Thần** | all attack; any melee+strike+slam | lặp +1, nhân −25% | 1,5 | 3 | nineturns | Multistrike |
| `nineturns` | **Cửu Chuyển** | any charged | nhân +15% mỗi lượt giữ (tối đa 9); bị chặn thì đổi chủ | 1,3 | 3 | afterimage | (Cửu Chuyển) |
| `ninechanges` | **Cửu Biến** | any | nhân +80%, windup ×2,5 | 1,3 | 2 | — | (Cửu Biến) |
| `rule` | **Quy Tắc** | any sword+palm | lên một phẩm, tối đa Quy Tắc | 1,4 | 3 | — | (rank) |
| `fuse` | **Dung** | any | dung hai chiêu (60% cả hai gốc); 15% phản phệ 20% | 1 | 3 | — | (Dung) |
| `demonize` | **Hóa Ma** | any | chuyển 100% mọi loại trước Âm sang Âm; triệu hồi địch chết có thể nổ | 1,2 | 2 | coldqi | Conversion |
| `coldqi` | **Âm Hàn** | any fire+thunder | viết lại 50% Hỏa và Lôi thành Thủy ở bước 1 | 1,1 | 1 | demonize | (backward conversion) |
| `bloodforge` | **Huyết Luyện** | any treasure | trả toàn bộ linh lực bằng máu (`CostAsLife` 1); bảo vật vỡ mất 10% máu | 1 | 2 | bloodspit | Blood Magic (treasure) |
| `bloodspit` | **Phun Máu** | any | trả 10% máu: nhân theo `BLOOD_POWER`; xuyên một lớp phòng | 1 | 1 | bloodforge | Blood Magic |
| `leap` | **Súc Địa** | any near+melee+strike+slam; none charged | nhảy tới mục tiêu rồi đánh | 1,2 | 2 | — | Leap Slam |
| `spiritshift` | **Dịch Linh** | any projectile | nhân +10% mỗi T bay (tối đa +60%); gọi đạn về | 1,2 | 2 | — | Far Shot (bay) |
| `delayedthunder` | **Lôi Bạo Trễ** | any projectile | đạn bị chặn thành mìn | 1,2 | 2 | — | (Lôi Bạo Trễ) |
| `bait` | **Nhử Đòn** | any control | khống chế ×2 nếu mục tiêu vừa dùng Hộ thể | 1,2 | 1 | — | (yomi) |
| `surehit` | **Chắc Trúng** | any | không né được; windup ×1,4 | 1,2 | 1 | — | Accuracy |
| `chainweaken` | **Liên Hoàn Suy Yếu** | any guard | khiên vỡ thì phóng một Đòn (kích hoạt `castonshieldbreak`) | 1,3 | 2 | — | Cast when Shield Breaks |
| `returnseal` | **Phản Ấn** | any guard | hộ thể hút một đạn và trả lại | 1,3 | 2 | — | (reflect) |
| `karmaqi` | **Khí Nhân Quả** | any guard+formation | khiên/trận vỡ thì phản một bản sao đòn | 1,3 | 2 | — | (reflect) |
| `selfdestruct` | **Tự Bạo** | any spirit+minion+treasure | thứ nó tạo nổ khi vỡ, 50% máu | 1,1 | 1 | — | (detonate) |
| `soulharvest` | **Thu Hồn** | any seal+banner | mỗi mạng chết dưới nó +10% cho cả hiệp | 1,2 | 2 | — | Soul Eater |
| `devourban` | **Thôn Cấm** | any banner+formation | nuốt bẫy, cột, vùng của địch để nạp | 1,3 | 2 | — | (devour) |
| `copy` | **Sao Chép** | any transform+spirit+soul | học bản sao 50% chiêu cuối của địch | 1,5 | 3 | — | (copy) |
| `allure` | **Mê Dục** | any projectile | trúng: địch không dùng Hộ thể 2 s | 1,2 | 2 | — | (debuff) |
| `heavy` | **Trọng** | any | thêm Thổ 15% gốc; windup ×1,2; rơi kẻ bay | 1,1 | 1 | — | Added damage |
| `hollowfire` | **Hư Hỏa** | any fire | lửa tím, hư hỏa | 1,2 | 2 | — | (fire tier) |
| `years` | **Tuế Nguyệt** | any area+aura | đạn trong vùng chậm 40% | 1,2 | 2 | — | (slow projectiles) |
| `noretreat` | **Chặn Lui** | any seal+palm | trúng: không thoát 1 s | 1,2 | 1 | — | (debuff) |
| `spacelock` | **Không Khóa** | any | trúng: không thuấn di 2 s | 1,2 | 1 | — | (debuff) |
| `soulburst` | **Hồn Bạo** | any spirit+minion | hiến một triệu hồi nổ theo máu còn | 1,1 | 1 | — | (detonate minion) |
| `soulfeed` | **Nạp Hồn** | any | sát thương Hồn sinh hồn | 1,1 | 1 | — | (souls) |
| `drain` | **Đoạt Tu Vi** | any | hút máu 5%; rút 2% thành linh lực địch | 1,2 | 2 | — | Life Leech |
| `fireforge` | **Hỏa Luyện** | any | chuyển 50% Lực sang Hỏa | 1,1 | 1 | — | Physical to Fire |
| `metalwind` | **Kim Phong** | any | chuyển 50% Lực sang Kim | 1,1 | 1 | — | Conversion |
| `waterintent` | **Thủy Ý** | any | chuyển 50% Lực sang Thủy | 1,1 | 1 | — | Physical to Cold (water) |
| `thunderlead` | **Lôi Dẫn** | any | chuyển 50% Lực sang Lôi | 1,1 | 1 | — | Physical to Lightning |
| `woodgrowth` | **Mộc Sinh** | any | chuyển 50% Lực sang Mộc | 1,1 | 1 | — | Conversion |
| `pierce` | **Xuyên Giáp** | any attack | xuyên 30% hộ thể | 1,2 | 2 | — | Impale / Penetration |
| `breakresist` | **Phá Kháng** | any spell | xuyên 20% kháng mọi loại trừ Lực | 1,2 | 2 | — | Penetration |
| `malignant` | **Ác Tính** | any | ailment +50%, đòn −15% | 1,2 | 2 | — | Unbound Ailments |
| `critical` | **Chí Mạng** | none instant | chí mạng +10%, hệ số +0,3 | 1,2 | 2 | — | Increased Critical Strikes |
| `lingering` | **Thời Gian Kéo Dài** | any | thời gian ailment +50% | 1,1 | 1 | — | Ailment duration |
| `venomheart` | **Độc Tâm** | any force+fire+yin | DoT multiplier +0,3 | 1,2 | 2 | — | Damage over time multiplier |
| `inferno` | **Hỏa Ngục Thiêu Đốt** | any fire | Thiêu nhân +50%, đòn −25% | 1,2 | 2 | — | Burning Damage / Immolation |
| `outburst` | **Bạo Phát** | any | nhân +30% mỗi nguyền trên mục tiêu, tối đa +60% (`Per::Curses`) | 1,2 | 2 | — | Hex/curse payoff |
| `pressadvantage` | **Thừa Thắng** | all attack | nhân +35% khi mục tiêu ≥ 5 vết Thương | 1,2 | 2 | — | (vs bleeding) |
| `bloodthirst` | **Thị Huyết** | any | nhân +25% trong 4 s sau khi giết | 1,1 | 1 | — | Feeding Frenzy-ish |
| `addedfire` | **Phụ Hỏa** | any attack+spell | thêm Hỏa 25% gốc | 1,2 | 1 | — | Added Fire Damage |
| `addedwater` | **Phụ Thủy** | any attack+spell | thêm Thủy 25% gốc (Thấm, không lạnh) | 1,2 | 1 | — | Added Cold Damage |
| `addedthunder` | **Phụ Lôi** | any attack+spell | thêm Lôi 25% gốc | 1,2 | 1 | — | Added Lightning Damage |
| `addedyin` | **Phụ Âm** | any attack+spell | thêm Âm 25% gốc | 1,2 | 1 | — | Added Chaos Damage |
| `addedmetal` | **Phụ Kim** | any attack+spell | thêm Kim 25% gốc | 1,2 | 1 | — | Added damage |
| `addedwood` | **Phụ Mộc** | any attack+spell | thêm Mộc 25% gốc | 1,2 | 1 | — | Added damage |
| `addedearth` | **Phụ Thổ** | any attack+spell | thêm Thổ 25% gốc | 1,2 | 1 | — | Added damage |
| `addedlight` | **Phụ Quang** | any attack+spell | thêm Quang 25% gốc | 1,2 | 1 | — | Added damage |
| `addedsoul` | **Phụ Hồn** | any attack+spell | thêm Hồn 25% gốc | 1,2 | 1 | — | Added damage |
| `elementalfocus` | **Ngũ Hành Ngưng** | any metal+wood+water+fire+earth | nhân +35%; không gây ailment nào (hiệu lực ailment −100%) | 1,3 | 2 | — | Elemental Focus |
| `controlleddestruction` | **Khống Diệt** | all spell | pháp nhân +30%; chí mạng −100% (increased) | 1,3 | 2 | — | Controlled Destruction |
| `concentrated` | **Tụ Lực** | any area+nova+cone | vùng nhân −30%, sát thương nhân +35% | 1,3 | 2 | widearea | Concentrated Effect |
| `widearea` | **Khoách Vực** | any area+nova+cone+aura+field | vùng +40% | 1,2 | 1 | concentrated | Increased Area of Effect |
| `brutality` | **Thuần Lực** | all force | Lực nhân +40%; mọi loại khác nhân −100% (kể cả phần Lực đã chuyển) | 1,3 | 2 | — | Brutality |
| `piercing` | **Xuyên Thấu** | any projectile | đạn xuyên 2; nhân +10% | 1,15 | 1 | — | Pierce |
| `fork` | **Phân Nhánh** | any projectile | đạn tách 1; nhân −10% | 1,2 | 1 | — | Fork |
| `chain` | **Liên Kích** | any projectile+chain | nảy 2; nhân −25% | 1,3 | 2 | — | Chain |
| `fasterprojectiles` | **Tật Tiễn** | any projectile | tốc đạn +50%; nhân +10% | 1,15 | 1 | slowerprojectiles | Faster Projectiles |
| `slowerprojectiles` | **Hoãn Tiễn** | any projectile | tốc đạn −30%; nhân +20% | 1,15 | 1 | fasterprojectiles | Slower Projectiles |
| `lessermultiple` | **Tiểu Liên Châu** | any projectile | +1 đạn; nhân −15% | 1,2 | 1 | multishot | Lesser Multiple Projectiles |
| `pointblank` | **Cận Xạ** | any projectile | trong 2 T nhân +30%; ngoài 6 T nhân −20% | 1,1 | 1 | farshot | Point Blank |
| `farshot` | **Viễn Xạ** | any projectile | nhân +5% mỗi T (`Per::Distance`), tối đa +30% | 1,2 | 2 | pointblank | Far Shot |
| `meleephysical` | **Cương Lực** | all attack; any melee+strike+slam | Lực nhân +35%; tốc nhân −10% | 1,3 | 2 | — | Melee Physical Damage |
| `closecombat` | **Áp Sát** | all attack; any melee+strike+slam+dash | trong 1,5 T nhân +30% | 1,2 | 2 | — | Close Combat |
| `ruthless` | **Tàn Nhẫn** | all attack; any melee+strike+slam | mỗi lần thứ 3 (`EveryNth`): nhân +90%, Chấn +50% | 1,2 | 2 | — | Ruthless |
| `fistofwar` | **Chiến Quyền** | all attack; any slam+fist | mỗi lần thứ 4: nhân +60%, vùng +50% | 1,3 | 2 | — | Fist of War |
| `impale` | **Lưu Lực** | all attack+force | 30% Lực đánh lại sau 1 s (trận đấu) | 1,2 | 2 | — | Impale |
| `culling` | **Trảm Tận** | any attack+spell | mục tiêu còn ≤ 10% máu sau đòn thì chết (`Execute`) | 1,1 | 1 | — | Culling Strike |
| `maim` | **Phế Cước** | all attack | Lực nhân +10%; trúng: mục tiêu chạy −30%, nhận Lực +10% trong 4 s | 1,1 | 1 | — | Maim |
| `fasterattacks` | **Tốc Kích** | all attack | tốc đánh +30% | 1,15 | 1 | — | Faster Attacks |
| `triplestrike` | **Tam Liên Ảnh** | all attack; any melee+strike+slam | lặp +2, nhân −35%, tốc nhân +20% | 1,6 | 3 | afterimage, nineturns | Multistrike (variant) |
| `fastercasting` | **Tốc Chú** | all spell | tốc thi triển +30% | 1,15 | 1 | — | Faster Casting |
| `spellecho` | **Hồi Âm** | all spell; none channel+instant+charged+ultimate | lặp +1, tốc nhân +50%, nhân −10% | 1,4 | 3 | unleash | Spell Echo |
| `arcanesurge` | **Linh Triều** | all spell | sau khi tiêu ≥ 30 linh lực gần đây (`QiSpent`): nhân +15%, tốc +10% | 1,1 | 1 | — | Arcane Surge |
| `efficacy` | **Hiệu Lực** | all spell | nhân +15%, DoT nhân thêm +20%, thời gian chiêu −20% | 1,2 | 2 | — | Efficacy |
| `inspiration` | **Linh Cảm** | all spell; none instant | chi phí ×0,75; chí mạng +25% mỗi Ma tinh, tối đa +75% | 0,75 | 1 | — | Inspiration |
| `burningdamage` | **Liệt Diễm** | any fire | Thiêu nhân +45% | 1,2 | 2 | — | Burning Damage |
| `immolate` | **Hỏa Thiêu Thân** | all attack | thêm Hỏa 30% gốc khi mục tiêu đang Thiêu | 1,2 | 2 | — | Immolate |
| `swiftaffliction` | **Tốc Độc** | any force+fire+yin | DoT nhân +35%; ailment và thời gian chiêu nhân −35% | 1,2 | 2 | — | Swift Affliction |
| `deadlyailments` | **Độc Ngấm** | any force+fire | DoT nhân +45%; đòn nhân −20% | 1,3 | 2 | — | Deadly Ailments |
| `cruelty` | **Tàn Khốc** | any force+fire | DoT nhân tới +40% theo cỡ đòn (đủ khi đòn ≥ 10% máu mục tiêu) | 1,2 | 2 | — | Cruelty |
| `bloodlust` | **Khát Huyết** | all attack | nhân +8% mỗi vết Thương trên mục tiêu (`Per::Stacks`), tối đa +48% | 1,2 | 2 | — | Bloodlust |
| `witheringtouch` | **Tịch Thủ** | all attack | mỗi đòn trúng gây thêm 1 tầng Tịch (`Inflict`) | 1,2 | 2 | — | Withering Touch |
| `voidmanipulation` | **Hư Không** | any yin | Âm nhân +25%; ngũ hành nhân −25% | 1,2 | 2 | — | Void Manipulation |
| `combustion` | **Nhiên Thiêu** | any fire | Hỏa nhân +10%; vết Thiêu nó gây hạ kháng Hỏa mục tiêu 15% trong 4 s | 1,2 | 2 | — | Combustion |
| `intensify` | **Tăng Ý** | all spell; any area+nova | nhân +8% và vùng −5% mỗi đòn trúng gần đây (`Per::Hits`), tối đa 3 | 1,2 | 2 | — | Intensify |
| `unleash` | **Phong Ấn** | all spell; none channel+instant+ultimate | ấn tích mỗi 1,5 s (tối đa 2), mỗi ấn một lần lặp; nhân −25% | 1,3 | 2 | spellecho | Unleash |
| `overcharge` | **Quá Tải** | any thunder | hiệu lực Tê nhân +50%; Lôi nhân +10% | 1,2 | 2 | — | Overcharge |
| `soaked` | **Thủy Phệ** | any water | nhân +25% khi mục tiêu đang Thấm; hiệu lực Thấm +30% (không lạnh, không đóng băng) | 1,2 | 2 | — | Hypothermia (as water) |
| `critdamage` | **Bạo Thương** | none instant | hệ số chí mạng +0,5 | 1,2 | 2 | — | Increased Critical Damage |
| `powercharge` | **Ma Tinh Dẫn** | none instant | chí mạng: 50% được Ma tinh (`StarOn`); chí mạng +20% | 1,1 | 1 | — | Power Charge On Critical |
| `endurancecharge` | **Thần Tinh Dẫn** | all attack; any melee+strike+slam | đòn gây khống chế cứng: 100% được Thần tinh; Chấn nhân +25% | 1,1 | 1 | — | Endurance Charge on Melee Stun |
| `frenzycharge` | **Yêu Tinh Dẫn** | all attack | trúng mục tiêu có hồn: 25% được Yêu tinh | 1,1 | 1 | — | Frenzy Charge on Hit (vs rare) |
| `trinity` | **Tam Tài** | any attack+spell | nhân +5% mỗi loại sát thương đã gây gần đây (`Per::KindsDealt`), tối đa +30% | 1,2 | 2 | — | Trinity |
| `lifeonhit` | **Hồi Huyết** | any attack+spell | mỗi đòn trúng +20 máu | 1,1 | 1 | — | Life Gain on Hit |
| `qileech` | **Hấp Khí** | any attack+spell | hút 2% sát thương thành linh lực | 1,1 | 1 | — | Mana Leech |
| `lifeonkill` | **Sát Sinh Bổ Huyết** | any attack+spell | giết: hồi 5% máu (trận đấu) | 1,1 | 1 | — | Life Gain on Kill |
| `hextouch` | **Chú Ấn Kích** | all curse | nguyền được gieo bằng đòn trúng; hiệu lực nguyền nhân −20% | 1,2 | 2 | blasphemy | Hextouch |
| `blasphemy` | **Chú Vực** | all curse | nguyền thành vùng quanh mình, giữ 30 thần thức | 1 | 2 | hextouch | Blasphemy |
| `enlighten` | **Khai Ngộ** | any aura+curse | thần thức giữ chỗ nhân −30% | 1 | 2 | — | Enlighten |
| `generosity` | **Khảng Khái** | any aura | hiệu lực ý cảnh nhân +30%, không lên mình | 1 | 2 | — | Generosity |
| `trap` | **Bẫy Trận** | any spell+attack; none melee+dash+channel+trap+mine+totem+minion+aura+ultimate+movement+guard+field+domain | thành bẫy (tag Bẫy); nhân −10%; 1 bẫy | 1,1 | 2 | mine, totem, critcast, hurtcast, lowlifecast, channelcast | Trap |
| `mine` | **Địa Lôi Trận** | any spell+attack; none melee+dash+channel+trap+mine+totem+minion+aura+ultimate+movement+guard+field+domain | thành mìn (tag Mìn); nhân −10%; 1 mìn | 1,1 | 2 | trap, totem, critcast, hurtcast, lowlifecast, channelcast | Blastchain Mine |
| `totem` | **Pháp Đàn** | any spell; none melee+dash+channel+trap+mine+totem+minion+aura+ultimate+movement+guard+field+domain | thành pháp đàn (tag Đàn); nhân −30%; 1 đàn | 1,1 | 2 | trap, mine, critcast, hurtcast, lowlifecast, channelcast, leap | Spell Totem |
| `miniondamage` | **Ngự Linh Lực** | any minion | sát thương triệu hồi nhân +35% | 1,2 | 2 | — | Minion Damage |
| `minionlife` | **Ngự Linh Thể** | any minion | máu triệu hồi nhân +40% | 1,2 | 1 | — | Minion Life |
| `phantasm` | **Ảo Linh** | all spell; none minion+channel | trúng: 25% gọi một ảo linh | 1,2 | 2 | — | Summon Phantasm |
| `critcast` | **Bạo Kích Dẫn Pháp** | all spell; none channel+ultimate+trap+mine+totem+minion+aura | chí mạng của một Đòn phóng chiêu này (`castoncrit`) | 1,2 | 2 | hurtcast, lowlifecast, channelcast, trap, mine, totem | Cast On Critical Strike |
| `hurtcast` | **Thụ Thương Phản Chú** | all spell; none channel+ultimate+trap+mine+totem+minion+aura | mỗi 300 sát thương nhận phóng chiêu này (`castwhenhurt`) | 1,2 | 2 | critcast, lowlifecast, channelcast, trap, mine, totem | Cast when Damage Taken |
| `lowlifecast` | **Lâm Nguy Phản Kích** | all spell; none channel+ultimate+trap+mine+totem+minion+aura | máu rơi qua 35% phóng chiêu này (`castonlowlife`) | 1,2 | 2 | critcast, hurtcast, channelcast, trap, mine, totem | Cast on Death (at low life) |
| `channelcast` | **Kênh Dẫn Pháp** | all spell; none channel+ultimate+trap+mine+totem+minion+aura | phóng mỗi 0,5 s khi đang kênh | 1,2 | 2 | critcast, hurtcast, lowlifecast, trap, mine, totem | Cast while Channelling |
| `lifetap` | **Huyết Tế** | any attack+spell | trả toàn bộ linh lực bằng máu; nhân +15% | 1 | 1 | bloodforge, bloodspit | Lifetap |

---

## 9. Đạo tâm (keystone)

Đủ mọi đạo tâm của bản nghiên cứu mục 7 (Cực Cảnh, Âm Hàn Linh Lực, Cổ Thần Quyết, Sát Lục Tiên Quyết, Tiên Trì, Tóc
Đen, Thái Cổ Lôi Long, Hệ Đại Thành, Tam Mệnh, Ngọn Đèn Hồn, Hai Mươi Năm Khổ Luyện, Tâm Xám, Người Xem Cây, Nguyền Bẩm
Sinh, Đạo Dung Thân, Phá Rồi Lập, Vu Chú, Niềm Tin Tất Thắng), không đạo tâm nào gắn với lớp. Thêm ba đạo tâm thay cho
đặc tính phái cũ. Bảng dưới nhắc những cái đổi bộ tính trực tiếp; số của mọi đạo tâm nằm ở `calc::keystone`.

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
      / hệ_số_tầm (mục 7) × hệ_số_an_toàn
```

`hệ_số_an_toàn`: Thuấn 1,25 (không né), Vùng 1,10 (guard không chặn), unsafe on block 0,85. Chiêu càng an toàn thì
điểm sức càng cao, nên cùng một bậc nó phải gây ít sát thương hơn. Ailment quy ra sát thương
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

1. **Bộ tính đủ** (`crates/arena-calc/src/`, mục 15): loại, ailment, chuyển hoá, tăng/nhân, chí mạng, dung hợp,
   ngũ hành, giảm thương, ba thanh, mọi tài nguyên, Kiên, phẩm, mọi đạo, nguyền, ý cảnh, tranh đạo niệm, lĩnh vực, lá
   bài và Lộ bài, mọi bí quyết, mọi đạo tâm, toàn bộ pool chiêu, bộ đồ và ngân sách. Hàm thuần, có test.
2. **Nối vào trận**: bỏ `School` và chill/freeze; `Fighter` mang bộ đồ và trạng thái của bộ tính; `status::damage`
   gọi `calc`; thanh chiêu rút từ bộ đồ. Re-pin goldens (`crates/pixfx-duel/AGENTS.md`).
3. **Môi trường**: quan sát mục 13, bộ đồ ngẫu nhiên trong `train`, `pixfx-duel-py`.
4. **Telemetry cân bằng**: ladder ghi các số đo mục 12.2 theo chiêu; một example in bảng bậc.
5. **Huấn luyện và vòng cân bằng** (mục 12.3).
6. Sau cùng: effect riêng cho từng chiêu, UI.

### 14.1 Nối vào trận (bước 2): đã làm, và quyết định khi thiết kế im lặng

Trận thật (`crates/pixfx-duel`, bản Bevy) giờ đánh bằng `arena-calc`, gọi như `arena-sim` gọi
(`crates/arena-sim/src/fight.rs` là mẫu): bỏ `School`, chill, freeze và sáu chiêu băng; hai ghế giống hệt nhau, khác
nhau ở bộ đồ (ngẫu nhiên theo seed trên xúc xắc riêng, hoặc truyền vào `Duel::with_loadouts`); Thunder và Frost chỉ còn
là hình vẽ (`data::Look`). Mọi đòn qua `build_skill` + `resolve_with` (né, chí mạng, nhân đôi, tinh theo thứ tự);
`tick_with` mỗi tick; hàng đợi sự kiện chạy sau mỗi lượt đòn và kích chiêu ngay; đạn là `projectile::Flight`; đẩy lùi,
tốc chạy, tầm lướt từ `motion`; hồi chiêu là `charge::Uses`; vùng và thời gian từ `play`; bẫy, mìn, đàn, triệu hồi là
thực thể trong world, xét theo id.

Những chỗ thiết kế không nói, trận chọn thế này (mỗi cái một dòng, đổi được):

- Chiêu đi theo **motion** nó mượn (`Gem::spell`): Melee, Dash, Blast, Bolt, Orb, Zone, Void, Heal, Ward, Barrier,
  Blink. Vai trò theo tag đứng trước motion (`kit::Way`): ý cảnh, lĩnh vực, nguyền không sát thương, buff (tư thế: nhân
  +25% sát thương; khác: nhận −20%, 8 s), bẫy/mìn/đàn/triệu hồi. Chiêu Thuấn vẫn bay theo motion của nó; Thuấn chỉ đổi
  né và chí mạng.
- `Hits` của gem thành **đòn lặp** cách nhau 0,15 s (phần lẻ đánh cuối với hệ số lẻ); Zone có nhiều nhát và Void đã tự
  đếm nhát của mình. Nhát Void = đòn × `tick/damage` của motion.
- **Nộ**: một điểm mỗi lần thi triển có đòn trúng (đòn đầu), không phải mỗi đòn, để chiêu nhiều nhát không đầy nộ ngay.
- **Hồi chiêu** tính từ lúc bắt đầu thi triển (`Uses`), không cộng windup như luật cũ.
- **Guard**: 8 linh lực, xác suất đỡ theo cỡ đòn như cũ; phần lọt 20% đi qua `resolve_with` không xúc xắc.
- **Khống chế lên thân**: choáng và quật ngã giữ thân (hitstun) và ngắt windup; định thân và giật giữ thân (shocked) và
  ngắt windup; trói = không đi, không độn; thất hồn = câm (chỉ Đòn).
- **Bay** dừng hồi linh lực và tốn 3 linh lực/s.
- **Hồi máu** và **ward** xóa mọi ailment (như cleanse cũ).
- **Chuông** (bell) phản đòn gần bằng `counter.damage × POWER` sát thương Lực, choáng như cũ.
- **Sức** của đòn cho thân (bị hất, hitstun) = sát thương gây ra / `data::POWER` (≈ 33,9, một điểm power cũ).
- **Triệu hồi** đi bằng tốc đi của đấu sĩ tới tầm kiếm (`slash`), đánh 1 lần/s như nhát kiếm. **Đàn** phóng chiêu từ chỗ
  nó đứng theo motion của chiêu. **Bẫy**: chiêu vùng nổ quanh bẫy, chiêu khác đánh kẻ đạp. **Mìn**: luật tự cho nổ thay
  chủ khi đối thủ đứng trong tầm nổ (ABI bot chưa có hành động kích mìn); mọi mìn đã gài nổ cùng tick, không cách 0,1 s.
- **Hồn**: +1 mỗi 100 sát thương Hồn gây ra, +1 mỗi mạng có hồn giết (kể cả triệu hồi), +1 mỗi triệu hồi chết trong 4T
  (`RES.souls_*`, `Gains::souls`, `Cultivator::minion_died`); Nạp Hồn cho thêm một lần nữa.
- **Pháp đàn**: nhịp = max(windup + recover, hồi chiêu) nhưng không quá ¼ đời đàn (`TOTEM_CADENCE_SHARE`), nên Vạn Kiếm
  Trận (hồi 9 s, đàn sống 8 s) phóng bốn lần.
- **Liên Hoàn Suy Yếu** giữ đòn liên kết (Chưởng, gốc = phần khiên hút), không bị nhắm lại vào chiêu hộ thể nó gắn.
- **Lưu Nguyệt** tốn thọ nguyên: để nguyên (chiêu Tuế nguyệt).
- DoT gây mỗi tick, hiện (`Out::Tick`) mỗi 0,5 s.
- Tâm tính bộ não chung: trung bình của hai phái cũ (`data::MIND`).

### 14.2 Trận chưa kích hoạt (vẫn gọi được trong `arena-calc`)

- Kênh (Kênh, Kênh Dẫn Pháp) và Tích (giữ nạp tầng, Cửu Chuyển): không có phím giữ; chiêu kênh thành chiêu thường,
  `turns` luôn 0.
- Bước 9 (phẩm chặn khiên, `grade`), Lộ bài và đòn dọa (`card`), các luật đạo riêng (`dao`: Sinh tử, Nhân quả, Chân giả,
  Tuế nguyệt quay lui...), tranh đạo niệm giữa hai ý cảnh (`aura::Contest`), luật đạn trong lĩnh vực, số phận triệu hồi
  và sụp lĩnh vực (`domain::Field::shot`, `minion_dies`, `Collapse`), vật dẫn và tầm nguyền (`curse::Conduits`),
  bài xích (`cast_school`).
- Hậu quả dung hợp: nổ lần hai của Lôi hỏa, vùng Dung nham (như `arena-sim`).
- Trên đường bay: tản lôi qua vùng Thủy, Tuế Nguyệt làm chậm đạn; Trọng làm rơi người đang bay; Lôi ×1,5 lên triệu hồi
  Âm/Trùng (`Situation::target_tags` của đơn vị).
- Hành vi bí quyết chỉ trận làm được, trừ NoBlink, NoEscape, NoGuard, LifeOnKill, SoulsFromSoul, StoredForce, Rejection,
  Triggered và Device: TurnsOnBlock, Fuse (đã gộp ở `kit`), MinionsBurst, TreasureBreak, PierceLayer, LeapToTarget,
  Recall, MineOnBlock, Bait, CastOnGuardBreak (kích bằng `ShieldBroke` của hộ thân), AbsorbAndReturn, ReflectBreaker,
  SelfDestruct, GrowOnKill, DevourField, CopyLast, Grounds, SlowShots, SacrificeMinion, DrainQi, CurseOnHit, CurseAura,
  NotOnSelf, SummonPhantasm, CastWhileChannelling, Seals.
- Quả cầu (Orb) vẫn là đạn dò của duel, chỉ trúng đối thủ, không phải `Flight`.
- Bot chưa chọn bộ đồ (ABI, mục 13) và chưa thấy trạng thái mới (mục 16). Bot nhà phải huấn luyện lại.

---

## 15. Bộ tính trong code

Mọi luật của bản nghiên cứu và bản luật (trừ lớp nhân vật và băng) nằm trong `crates/arena-calc/src/`. Mỗi
module mang số của nó dưới dạng `pub const`; bản này và bản luật nói vì sao, module nói bao nhiêu.

| Module | Nội dung | Bản luật |
|---|---|---|
| `element` | 10 loại, thứ tự chuyển hoá, ngũ hành sinh khắc, bản mệnh, dung hợp | 3.6, 3.7 |
| `tag` | Mọi tag theo 7 nhóm (bản chất, hình thức, tầm, cách đưa, vai trò, đạo, loại), tập tag, one-hot cho bot | — |
| `ailment` | 11 ailment, tầng, đầy, khóa, khống chế | 4 |
| `dot` | Thiêu và Thương thành vết: snapshot theo người đánh, chồng (mạnh nhất / 8 vết), giảm theo người chịu, hư hỏa (mục 17.5) | — |
| `cultivator` | Chỉ số gốc chung; máu, hộ thân, thần thức; linh lực, thọ nguyên, sát khí, hồn, tinh điểm, bài xích, tử khí; Kiên; cửa sổ dung hợp; tick | 1, 4.4, 8 |
| `grade` | Phẩm 1–5, cấm chế, khiên, giải trừ, vùng gặp đạn, hư bản nguyên | 5 |
| `hit` | `Hit`, 12 bước (`scale` cho 1–4, `resolve` cho cả đòn) | 3 |
| `dao` | Sinh tử, Nhân quả, Chân giả, Tuế nguyệt (đệm quay lui), Không gian, Duyên, Mệnh, Sát lục | 6.1–6.8 |
| `curse` | Vật dẫn, trần nguyền, lan cùng nguồn, phản phệ, chúc phúc; mọi nguyền của bản nghiên cứu 6.1 | 6.9 |
| `aura` | Ý cảnh thân và áp, giữ chỗ thần thức, bảng khắc, tranh đạo niệm, Nghịch chuyển | 7.1, 7.2 |
| `domain` | Lĩnh vực, chia sân, sụp, Ly Phong, phẩm 5 | 7.3 |
| `card` | Lá bài một lần, Lộ bài, đòn dọa, mồi nhử | 8.4 |
| `support` | Mọi bí quyết, tag đòi, hệ số chi phí, loại trừ, Dung | 2.2 |
| `keystone` | Mọi đạo tâm | — |
| `gem` | Pool chiêu chung (mục 11), tài nguyên, bậc, công thức ngân sách | 2.1 |
| `build` | Chiêu + hỏa hầu + bí quyết + đạo tâm + người đánh → `Hit`, chi phí và nhịp sau bí quyết | 2 |
| `loadout` | Bộ đồ, `check`, đạo điểm, bộ đồ ngẫu nhiên cho huấn luyện | — |
| `play` | Sân nhìn ngang theo T, mục tiêu, vùng tròn và dải, bán kính theo `Area`, thời gian theo `SkillDuration` (mục 17.8) | — |
| `projectile` | Đạn: xuyên, tách, nảy theo thứ tự PoE, xòe, tốc độ; `trace` và `Flight::step` | — |
| `motion` | Tốc chạy, đẩy lùi chặn tường, tầm lướt dưới Trọng | — |
| `charge` | Lượt dùng tích (`ExtraCooldownUse`) | — |
| `device` | Bẫy, mìn, đàn, triệu hồi: `spawn`, `Placed` | — |

Test: `crates/arena-calc/tests/*.rs`.

---

## 16. Bot inputs, next (đầu vào bot, bước sau)

Trận đã đánh bằng bộ tính nhưng vector quan sát của policy (`arena-mind`, `policy::observe`, 354 số) và ABI bot giữ
nguyên, để bot nhà, module và trang vẫn chạy. Trạng thái mới được đặt vào ô cũ nào hợp, còn lại là 0 (`docs/bot.md`):

| Ô cũ | Giờ đọc |
|---|---|
| hp, qi (mình, đối thủ) | máu, linh lực theo phần trăm của tối đa |
| silenced | thất hồn |
| sealed | khóa độn (Không Khóa, Chặn Lui) hoặc bị trói |
| foe frozen | số giây bị trói còn lại (không còn đóng băng) |
| foe shocked | số giây bị giữ thân (định thân, giật, choáng) |
| slot damage, power | sát thương mong đợi theo đơn vị power cũ (`data::POWER`); power = 1 |
| inflicts: freeze, stun, silence | định thân; choáng, quật ngã, giật; thất hồn (giây, từ `Effect::Control`) |
| inflicts: def_down, def_loss | Phá (theo phần Kim) |
| inflicts: burn, burn_rate | Thiêu (theo phần Hỏa) |
| inflicts: chill, chill_max, slow | Thấm (phần Thủy, trần 6); chậm của Triền (phần Mộc) |
| inflicts: shock... | Tê (phần Lôi, trần 5, giật 0,3 s, +4% nhận mỗi tầng) |
| inflicts: steal_rage, sense.power, sense.stacks | 0 (không còn) |
| tag: school (2) | ghế (0 trái, 1 phải) |

**Cần ô riêng** (mùa sau, kèm huấn luyện lại, đổi `OBS` và `docs/bot.md`):

- Mỗi đấu sĩ: hộ thân và thần thức (phần trăm), hồn, sát khí, thọ nguyên, tinh điểm (3), 11 ailment (tầng hoặc số vết, và
  khóa), Kiên, bản mệnh (8 one-hot), kháng 10 loại, hộ thể, cửa sổ dung hợp (10), ý cảnh đang bật, nguyền đang dính,
  lĩnh vực đang đứng (mục 13).
- Mỗi slot: tag one-hot (3 + 18 + 13), tỉ lệ 10 loại sau chuyển hoá, sát thương mong đợi qua bộ tính với mục tiêu
  hiện tại, lượt dùng tích còn và tối đa, tầm thật, chi phí theo từng tài nguyên.
- Trên sân: bẫy, mìn, đàn, triệu hồi của hai bên (loại, vị trí, máu, đã gài), như threats.
- Hành động: kích mìn (nay luật tự kích thay chủ), chọn bộ đồ lúc vào giải (export ABI mới, `loadout::check`).

---

## 17. Bộ máy modifier, đồ, cây passive, DoT, kích hoạt

Đây là phần làm build có chiều sâu như PoE. Lõi ở mục 5 giữ nguyên; mục này nằm phía trên và đổ số vào nó.

### 17.1 Modifier (`stat`)

Mọi nguồn (bí quyết, đạo tâm, đồ, nút passive, ý cảnh, chúc phúc, nguyền, ailment, tinh điểm, lĩnh vực) phát ra
danh sách `Mod`:

| Trường | Nghĩa |
|---|---|
| `stat` | Con số bị đổi: sát thương (chung, theo loại, chỉ đòn, chỉ DoT, theo ailment), DoT multiplier, cộng thêm, chuyển hoá, chí mạng, xuyên, hiệu lực và thời gian ailment, số đạn, lặp, tốc độ, hồi chiêu, chi phí, thời gian chiêu, vùng, hiệu lực nguyền và ý cảnh; phía chịu: máu, hộ thân, thần thức, lực đạo, hộ thể, né, kháng và trần kháng, kháng ailment, sát thương nhận (chung, theo loại, DoT), khống chế nhận, hồi phục, tốc chạy. |
| `kind` | **Flat** (cộng vào gốc), **Increased** (cộng gộp), **More** (nhân riêng), **Override** (đặt thẳng). |
| `need` | Tag chiêu phải có (ví dụ chỉ áp lên Đạn). |
| `when` | Điều kiện dạng dữ liệu: mục tiêu bị nguyền, mục tiêu có ≥ n tầng ailment X, mục tiêu đang chịu DoT, máu mục tiêu ≤ x, bản mệnh mục tiêu, khiên mục tiêu đã vỡ, máu mình ≤ x hoặc đầy, có tinh, sát khí ≥ n, "vừa X trong N giây" (trúng, chí mạng, giết, bị trúng, đỡ, độn, dùng chiêu phòng thủ, nguyền ai đó, khiên vỡ, gây ailment, né, bị nguyền, triệu hồi chết, máu thấp, quay lui, lĩnh vực sụp, chịu DoT), gần/xa, đang chạy/đứng yên, chiêu có tag, và Not/All/Any. |
| `source` | Nguồn, để bot, tooltip hay bảng phân tích đọc. |

`Table::gather(mods, ctx)` giữ các mod có tag và điều kiện đúng lúc đó, rồi cộng theo stat:
`giá trị = (gốc + Σ flat) × (1 + Σ increased) × Π(1 + more)`; override thắng. `build` và `resolve` chỉ đọc bảng.
Không còn chỗ nào người gọi phải nhớ cộng tay một nguồn.

Hai trường nữa: **`per`** và **`max`**. Giá trị của mod tính một lần cho mỗi `per` (kiểu "x% mỗi y" của PoE), rồi bị
`max` chặn theo độ lớn (giữ dấu); override không bao giờ nhân. `Table::gather` cộng giá trị đã nhân, và `used` giữ
mod đã nhân (một "more" mỗi tầng là `1 + x × n`, không phải `(1 + x)^n`). Dựng bằng `.per(..)` và `.max(..)`.

| `Per` | Đếm gì |
|---|---|
| `One` | một lần (mặc định) |
| `Stacks(a)` | tầng ailment `a` trên mục tiêu (vết, với Thiêu và Thương) |
| `DistinctAilments` | số ailment khác nhau trên mục tiêu |
| `Curses` | số nguyền trên mục tiêu (`Ctx::curses`) |
| `Stars(s)` | tinh loại `s` của mình |
| `Intent(n)` | mỗi `n` sát khí (bậc nguyên) |
| `MissingLife` / `TargetMissingLife` | mỗi 1% máu đã mất, của mình / của mục tiêu |
| `Souls` | mỗi hồn |
| `Distance(n)` | mỗi `n` T tới mục tiêu (0 khi chưa biết khoảng cách) |
| `Kills` / `Hits` | số mạng giết, số đòn trúng gần đây |
| `QiSpent(n)` | mỗi `n` linh lực đã tiêu gần đây |
| `KindsDealt` | số loại sát thương đã gây gần đây |

"Gần đây" là `Counts` (giết, trúng, linh lực đã tiêu, loại đã gây) mà trận đấu đếm trong `RECENT` = 4 s và đưa qua
`Situation::counts`. `Ctx` mang thêm: số nguyền trên mục tiêu (`curses`; `other_cursed` giữ để tương thích), mục tiêu
có đang chạy (`other_moving`), lần dùng thứ mấy của chiêu (`use_count`, trận đấu đếm từ 1) và `counts`.

**Điều kiện mới:** `TargetCurses(n)`, `TargetDistinctAilments(n)`, `TargetFullLife`, `TargetMoving`,
`TargetHasSoul` (không phải thân vô hồn, không đang thất hồn), `TargetHasEffect(nguồn)`, `SelfHasEffect(nguồn)`,
`SelfQiAbove(tỉ lệ)`, `QiSpent(n)` (đã tiêu ≥ n linh lực gần đây), `EveryNth(n)` (lần dùng thứ n, 2n, 3n...).
`Happened` có thêm `LaidControl` (gây khống chế cứng; sự kiện `Controlled`).

**Stat mới**, mỗi cái được đọc ở một chỗ trong bộ tính:

| Stat | Nghĩa | Ai đọc |
|---|---|---|
| `Extra(từ, sang)` | gain as extra: một phần mỗi phần đang là `từ` sau chuyển hoá thêm thành phần `sang`, phần mới đã là cả hai loại; mọi hướng (không còn gì chuyển sau nó) | `build` → `Hit::extra`; `hit::scale` và `dot_scale` |
| `Execute` | trảm: mục tiêu còn ≤ x máu sau đòn thì chết | `resolve`, cuối bước 11 |
| `LifeOnHit`, `QiOnHit`, `SenseOnHit`, `IntentOnHit`, `QiLeech` | hồi khi trúng, hút linh lực | `resolve` → `Outcome::gains` (trận đấu trao cho người đánh) |
| `ChanceDoubleDamage` | cơ hội nhân đôi, xúc xắc riêng `Rolls::double` | `resolve`, bước 5 |
| `ShieldBypass` | một phần sát thương lẽ ra vào khiên đi thẳng vào máu (Âm và Hồn vốn không qua khiên) | `resolve`, bước 11 |
| `HitTaken` | sát thương đòn nhận (Fortify); DoT nhận vẫn là `DotTaken` | `resolve`, bước 8 |
| `Inflict(a)` | mỗi đòn trúng gây thêm `n` tầng `a` (Tịch Thủ) | `resolve`, bước 12 |
| `StarOn(việc, tinh)` | cơ hội được tinh khi việc xảy ra, một xúc xắc `Rolls::star` | `resolve`: người đánh khi `Hit`, `Crit`, `Kill`, `LaidControl` (`Outcome::stars`); người chịu khi `BeenHit`, `BeenCrit`; việc khác: trận đấu |
| `ControlDuration` / `ControlTaken` | khống chế mình gây dài hơn / khống chế mình chịu | `build` → `Hit::control`, `Built::control`; `resolve` (`full_effect_with`) |
| `ControlEffect` | hiệu lực các ailment không gây sát thương (tầng tới đầy) | `Table::ailment_factors` |
| `AilmentResist(a)` | (có từ trước) kháng ailment, nay đọc từ bảng phòng thủ | `resolve`, bước 12 |
| `CostAsLife` | một phần chi phí linh lực trả bằng máu, `LIFE_PER_QI` máu mỗi điểm (Huyết Luyện, Huyết Tế) | `build` |
| `QiMax`, `StarsMax` | linh lực đầy; tinh tối đa | `Sheet::apply`; `resolve` (tinh né, tinh thân, `StarOn`) |
| `Reservation`, `Reserved` | hệ số thần thức giữ chỗ; thần thức bí quyết giữ thêm (Chú Vực) | `build` → `Built::reservation`, `Built::reserve`; `Auras::light_with` |
| `CurseCount` | số nguyền giữ được thêm | `curse::curse_limit` |

Rolls của một đòn, theo thứ tự trận đấu rút: né, chí mạng, nhân đôi, tinh (`Rolls::NONE`: không gì xảy ra).

**`FIGHT_READS`**: các stat bộ tính gom nhưng không tự áp. Trước đây gồm `MoveSpeed`, `SkillDuration`, `Area`,
`Chain`, `Pierce`, `Fork`, `ProjectileSpeed`, `Knockback`, `MinionDamage`, `MinionLife`, `MinionCount`, `TrapCount`,
`MineCount`, `TotemCount`, `DeviceLife`, `ExtraCooldownUse`; nay mỗi cái có hành vi trong bộ tính (mục 17.8) nên danh
sách rỗng. Một test (`tests/stat.rs`) bắt mỗi biến thể `Stat` hoặc được đọc trong bộ tính hoặc nằm trong danh sách này,
không cả hai.

### 17.2 Trang nhân vật (`sheet`)

`Sheet` = đồ đang đeo + cây passive và các nút đã chọn + mod cố định khác. `Sheet::apply` đặt chỉ số lâu dài lúc vào
trận. `Sheet::table(ctx, mod của chiêu)` gom mod cố định, hiệu ứng đang dính trên người (`Cultivator.effects`) và bí
quyết của chiêu.

### 17.3 Hiệu ứng có thời hạn

`Cultivator.effects` là danh sách `Active { nguồn, mods, thời gian còn }`: chúc phúc, ý cảnh (giữ tới khi tắt),
nguyền, luật lĩnh vực, ảnh hưởng của ailment lên người mang. Cùng nguồn thì thay nhau, không chồng. Nguyền mạnh theo
`CurseEffect` của người nguyền (`curse::curse_effect`, `curse::cast_with`: nhân hiệu lực, không nhân thời gian); ý
cảnh theo `AuraEffect` của người giữ (`aura::sync`).

### 17.4 Đồ và cây passive: chỉ có base

- **Đồ** (`item`): 11 ô (vũ khí, tay phụ, mũ, áo, găng, giày, đai, dây chuyền, hai nhẫn, pháp bảo bản mệnh). Base vũ khí
  có sát thương theo loại, tốc đánh, chí mạng, tầm; base giáp có hộ thể, né, hộ thân. Mỗi món mang danh sách `Mod`.
  Không đeo gì là **tay không**: sát thương Đòn = lực đạo, đúng như trận hiện nay.
- **Cây passive** (`passive`): nút mang mod hoặc đạo tâm, nối thành đồ thị; một bộ nút hợp lệ khi mọi nút nối về điểm
  xuất phát qua nút đã chọn và không vượt số điểm. Mặc định cây rỗng.
- Chưa có món đồ hay nút nào: chế tạo đồ, affix, cây cụ thể làm sau. Base đủ để chúng chỉ cần thêm dữ liệu.

### 17.5 DoT: nhánh sát thương riêng, scale theo người đánh

Thay luật "Thiêu, Thương mất % máu tối đa của mục tiêu" ở mục 4 và 5.1. Code: module `dot`.

- Một đòn có phần Hỏa thì đặt một **vết cháy**, có phần Lực mà Lực của chiêu là Thương (đòn cắt) thì đặt một **vết chảy
  máu**. Sát thương mỗi giây của vết lấy từ **phần loại đó của đòn**, sau bước 7, trước giảm thương:
  - Thiêu: 50% phần Hỏa mỗi giây, 4 giây (`BURN_RATE`, `BURN_TIME`);
  - Thương: 35% phần Lực mỗi giây, 5 giây, ×2 khi mục tiêu đang chạy (`BLEED_RATE`, `BLEED_TIME`, `BLEED_MOVING`).
- "Phần sau bước 7" của DoT được tính như phần của đòn nhưng **bước 4 đọc mod DoT thay cho mod đòn**: bước 5 (chí mạng),
  sức mạnh người đánh (sát khí, già), bước 7 (bản mệnh, phần thêm khi khắc) giữ nguyên. Vì thế `HitDamage` không chạm
  DoT, `DotDamage` không chạm đòn.
- Scale lúc đặt (snapshot, như PoE):
  `dps = tỉ_lệ × gốc × (1 + Σ tăng của Damage, DotDamage, AilmentDamage(a), DamageOf(mọi loại phần đó từng là)) × Π more
  × (1 + DotMulti + DotMultiOf(loại)) × hiệu_lực_ailment × hệ_số_bậc_lửa × (1 − kháng ailment) × sinh`;
  `thời gian = gốc × hệ_số_thời_gian_ailment`. Hệ số bậc lửa (`FIRE_TIER`) đỏ, lam, tím, đen: 1; 1,25; 1,5; 2.
  Máu trả cho chiêu (`BLOOD_POWER`) là more cho mọi sát thương của chiêu, nên cũng nhân DoT. `build` mang các hệ số này
  trên `Hit.dot` (theo ailment), cùng `Hit.ailment_effect` và `Hit.ailment_duration`.
- Chồng:
  - **Thiêu: chỉ vết mạnh nhất cháy**, các vết khác chạy hết thời gian riêng, như Ignite. Vết mạnh nhất hết giữa tick thì
    vết kế tiếp cháy phần còn lại của tick.
  - **Thương: tối đa 8 vết, tất cả cùng chảy.** Bleed build và ignite build vì thế chơi khác nhau.
  - Mỗi loại giữ tối đa 8 vết (`DOT_CAP`); vết thứ 9 thay vết yếu nhất nếu không yếu hơn nó, yếu hơn cả 8 thì bị bỏ.
    Chạm 8 từ dưới lên là "đầy" (sự kiện `AilmentFull`; lửa đen hút Thần thức).
- Giảm, mỗi tick, theo bảng phòng thủ sống của người chịu (trang, hiệu ứng, trạng thái):
  - Thiêu: theo kháng Hỏa (trần `ResistCap`), đi thẳng vào máu (xuyên hộ thể và hộ thân). **Xuyên kháng là của đòn,
    không áp lên DoT.**
  - Thương: bỏ qua hộ thể, hộ thân đỡ trước.
  - Cả hai ăn `Taken`, `TakenOf(loại)`, `DotTaken` của người chịu. Loại mà người chịu miễn nhiễm thì không đặt vết.
- Tầng ailment của Thiêu và Thương (dùng cho khắc/sinh, mục 4.1, và điều kiện `TargetHas`) bằng số vết đang có. Xóa
  ailment là xóa mọi vết. Khắc: thêm **phần sát thương các vết còn gây** nếu để chúng chạy hết
  (`Instances::remaining`, trước giảm thương, không tính ×2 khi chạy) × `OVERCOME_REMAINING` (1,0), cộng vào phần của
  hành khắc ở bước 7. Thiêu: vết mạnh nhất cháy hết thời gian còn lại, rồi vết kế tiếp; Thương: mọi vết. Luật cũ
  `số vết × 2% máu tối đa` bị bỏ vì mỗi đòn Hỏa dù nhỏ cũng đặt một vết: nhiều đòn Hỏa tí hon rồi một đòn Thủy là lỗ
  hổng. Sinh: vết mới +50% hiệu lực. Các ailment khác giữ luật tầng ở 5.1 (và `tầng × 2% máu tối đa` khi bị khắc).
- Lửa bậc 4 (đen, `DAO_FIRE_TIER`) là đạo hỏa: khắc bản mệnh Thủy ở bước 7 dù đòn có cờ `dao_fire` hay không.
- **Hư hỏa**: mỗi lần người đang cháy (với hư hỏa) xuất chiêu, thêm một vết chép vết mạnh nhất với 50% sát thương
  (`HOLLOW_FIRE_EFFECT`), cùng bậc lửa, thời gian đầy lại. Yếu hơn vết mạnh nhất nên nó giữ lửa cháy lâu hơn chứ không
  cháy mạnh hơn.
- `Cultivator::tick_with(dt, đang_chạy, trang)` gây DoT và trả `Ticked` (mỗi ailment: loại, vào hộ thân, vào máu);
  `tick(dt, đang_chạy)` là bản với trang rỗng. `event::dot_events` biến nó thành sự kiện `Dotted(a)`.
- Hồi linh lực, hộ thân, máu đọc `QiRegen`, `ShieldRegen`, `LifeRegen` từ bảng của người mang: Thấm (−8% mỗi tầng),
  Ly Phong và ý cảnh đè đi qua mod, không cộng tay. Hồi máu đi qua `heal` nên Tịch cắt nó, thối rữa biến nó thành mất máu.
- Bí quyết và mod chỉ tăng DoT (`DotDamage`, `DotMulti`, `AilmentDuration`…) không đụng tới đòn, và ngược lại
  (`HitDamage`).

### 17.6 Sự kiện và kích hoạt (`event`)

- Mỗi đòn sinh danh sách sự kiện theo thứ tự cố định: trúng, chí mạng, giết, bị trúng, đỡ, khiên vỡ, né, gây
  ailment, ailment đầy, nguyền, dùng chiêu, độn, triệu hồi chết, máu xuống ngưỡng, quay lui, lĩnh vực sụp; mỗi tick DoT
  sinh "chịu DoT" (`Dotted`). Sự kiện cập nhật "vừa X" của hai bên (cả né, bị nguyền, triệu hồi chết, máu thấp, quay lui,
  lĩnh vực sụp, chịu DoT).
- Trigger có thể nghe "ailment bất kỳ": `AnyAilmentLaid`, `AnyAilmentFull`, `AnyDotted`.
- `Trigger`: khi sự kiện X, nếu điều kiện đúng (và roll < cơ hội), thì kích chiêu Y ở hỏa hầu L với hệ số sát thương,
  hoặc đặt một hiệu ứng, hoặc cộng tài nguyên. Mỗi trigger có hồi chiêu riêng.
- Hàng đợi tất định: thứ tự theo ghế rồi theo sự kiện, trần số kích mỗi tick và trần độ sâu chuỗi (để không lặp vô
  hạn). Roll được truyền vào theo đúng thứ tự.
- `arena-calc` trả về các chiêu được kích; trận dựng và giải chúng, vì cần vị trí.

### 17.7 Golden theo ca PoE

`tests/golden.rs`: 10–15 ca chồng phức tạp, mỗi ca có con số kỳ vọng tính tay trong comment. Ví dụ: Hỏa Cầu + Liên Châu +
support DoT, người dùng bật ý cảnh Hỏa, mục tiêu đang Triền và bị Khắc Ấn; build chảy máu 8 vết đánh mục tiêu đang chạy;
kích chiêu khi chí mạng lên mục tiêu bị nguyền; đồ và nút passive chồng với bí quyết; chuyển hoá Lực sang Hỏa rồi sang Âm
với tăng ở cả ba loại.

Ca 16–19 thử modifier giàu và bí quyết PoE: chuyển 50% Lực sang Hỏa rồi 40% Hỏa gain thành Lôi (phần Lôi mang tăng của cả
Lực và Lôi); Khát Huyết đếm 6 vết Thương (+48%) và dừng ở trần; Trảm Tận kết liễu mục tiêu còn ≤ 10% máu sau đòn;
Pháp Đàn nhân −30% và nhận tag Đàn để một nút "chiêu Đàn" áp được.

Ca 20–21 thử hành vi trên sân (mục 17.8): Hỏa Cầu + Liên Kích bắn vào 3 mục tiêu, nảy A → B (gần hơn) → C, mỗi lần
trúng 0,75 Bf qua `resolve`; Hỏa Cầu trên Pháp Đàn trong 3 s phóng đúng 2 lần (0,26 s và 1,76 s), tổng 1,4 Bf.

### 17.8 Hành vi trên sân

Các stat cần vị trí và thân thể (trước kia nằm trong `FIGHT_READS`) giờ có hành vi là hàm thuần trên hình học đơn
giản, trong bộ tính. Trận đấu (bộ mô phỏng không hình `arena-sim`, trận đấu hiện nay, bản Bevy sau này) chỉ di chuyển
thân thể và gọi chúng. Vẫn không có xúc xắc, vẫn `libm`.

**Sân** (`play`): nhìn ngang, đơn vị T (chiều cao đấu sĩ). `x` dọc mặt đất từ tường trái (0) tới tường phải
(`Field::width`), `y` hướng lên. Thân thể là hình tròn. Mục tiêu là `Target { id, x, y, radius, alive, hit_before }`;
`hit_before` là "đạn này đã trúng nó rồi" (trận thời gian thực giữ cờ này giữa các khung hình). Vùng là `Shape::Circle`
hoặc `Shape::Band` (chữ nhật đứng); `Shape::inside` trả id các mục tiêu còn sống có thân chạm vào vùng.

**Vùng và thời gian.** `Area` tăng *diện tích*, nên bán kính nhân căn bậc hai: `radius(gốc, bảng) = gốc × √(hệ số Area)`
(Khoách Vực +40% → bán kính ×1,183; Tụ Lực −30% → ×√0,7). `duration(gốc, bảng)` = gốc qua `SkillDuration` (vùng, buff,
nguyền chiêu đặt), không âm.

**Đạn** (`projectile`). `Powers::of(bảng, số đạn thêm)` đọc `Pierce`, `Fork` (tối đa `MAX_FORKS` = 4), `Chain`,
`ProjectileSpeed`. Đạn là một điểm bay thẳng tới hết tầm. Khi trúng, theo thứ tự PoE:

1. còn xuyên: đi xuyên, bay tiếp;
2. hết xuyên, còn tách: tách đôi ở ±`FORK_ANGLE` (30°) từ chỗ trúng rồi tắt; mỗi nửa giữ tầm còn lại và không bao giờ
   trúng lại mục tiêu vừa tách;
3. hết tách, còn nảy: quay về mục tiêu sống gần nhất (tính từ tâm mục tiêu vừa trúng) trong `CHAIN_RANGE` (4T) mà đạn
   này chưa trúng, bay thẳng tới (vật mới chắn đường thì trúng trước); không có ai trong tầm thì tắt;
4. còn lại: tắt.

- Một đạn (cùng các nhánh tách và nảy) không trúng một mục tiêu hai lần; mỗi đạn thêm có trí nhớ riêng.
- Hòa khoảng cách thì lấy mục tiêu đứng trước trong danh sách.
- Đạn thêm xòe cách nhau `SPREAD` (10°) quanh hướng ngắm.
- Tốc độ = tốc độ của phát bắn × hệ số `ProjectileSpeed`; với tầm cho trước, thời gian bay chia theo
  (`flight_time`).
- Hệ số sát thương theo đường đi: `CHAIN_FACTOR`, `FORK_FACTOR`, `PIERCE_FACTOR` đều là 1. PoE không giảm theo mỗi lần
  nảy; cái "ít hơn" là của bí quyết (Liên Kích −25%, Phân Nhánh −10%), đã nằm trong `Built::hit`.
- `trace(shot, powers, targets)` trả mọi lần trúng của phát bắn: mục tiêu, thời điểm, hệ số, đạn thứ mấy, nhánh
  (tách nhánh `b` ra `2b+1` ngược kim đồng hồ và `2b+2`), đi bằng đường nào (`Direct`, `Pierce`, `Fork`, `Chain`), chỗ
  trúng, quãng đã bay (cho Dịch Linh, `Situation::travelled`).
- `Flight::step(dt, targets)` cho đạn bay một khung hình trong trận có mục tiêu di chuyển; trả các lần trúng và các nửa
  vừa tách. `Strike::blow(hit)` đặt hệ số thành một "more".

**Đẩy lùi và chạy** (`motion`).
- `knockback_distance(gốc, bảng người đánh)` = gốc qua `Knockback` (T).
- `knockback(x, bán kính, x người đánh, khoảng, sân)` đẩy ra xa người đánh, chặn ở tường (tâm cách tường ít nhất một
  bán kính).
- Trọng không làm ngắn đẩy lùi: luật ailment chỉ nói nó làm ngắn lướt và độn (`dash_reach`: −10% mỗi tầng,
  `AIL.heavy_reach`).
- `run_speed(gốc, bảng người mang, bị trói)` = gốc qua `MoveSpeed`, không âm; bị trói (Triền đầy, `Control::Root`) thì 0.
- Bảng người mang đã có Triền (−8% mỗi tầng), Phế Cước (−30%), nguyền Tuế Nguyệt (−20% × hiệu lực). Tinh Yêu tăng tốc
  đánh và thi triển (`Speed`), không tăng tốc chạy.
- `RUN_SPEED` = 3 T/s là gốc cho trận chưa có số riêng. `step_x` là một bước chạy chặn ở tường.

**Lượt dùng tích** (`charge`).
- `Uses { max, stored, recharge_left, cooldown }`, với `max` = 1 + `ExtraCooldownUse` (tối đa `MAX_USES` = 16).
- Vào trận đầy lượt. Mỗi lần dùng tiêu một lượt. Thiếu lượt thì hồi từng lượt một, mỗi lượt mất
  `Built::cooldown` (đã chia `CooldownRecovery`); phần `dt` dư chạy sang lượt sau.
- Ngạt: trận truyền `dt × AIL.choke_cooldown`.

**Bẫy, mìn, đàn, triệu hồi** (`device`). `spawn(built)` đọc chiêu đã thành bẫy, mìn, đàn (tag từ `Mods::becomes`) hoặc
chiêu triệu hồi:

| Loại | Máu | Sống | Trần cùng lúc | Khác |
|---|---|---|---|---|
| Bẫy | 200 qua `DeviceLife` | 8 s | `TrapCount` (bí quyết cho 1) | gài 0,5 s; bật khi thân kẻ địch trong 1T; kẻ gần nhất |
| Mìn | 200 qua `DeviceLife` | 12 s | `MineCount` | gài 0,3 s; lệnh nổ: mọi mìn đã gài nổ, cũ nhất trước, cách nhau 0,1 s |
| Đàn | 600 qua `DeviceLife` | 8 s | `TotemCount` | phóng lần đầu sau windup, rồi mỗi max(windup + recover, cooldown), ít nhất 0,1 s |
| Triệu hồi | 500 qua `MinionLife` | tới khi chết | 2 + `MinionCount` | sát thương nhân hệ số `MinionDamage` |

- Mỗi lần thi triển tạo một cái.
- Hệ số sát thương của bẫy, mìn, đàn là 1, vì cái "ít hơn" của bí quyết (−10%, −10%, −30%) đã nằm trong đòn.
- `Placed` giữ những cái đang đứng, cũ nhất trước:
  - `place` vượt trần thì bỏ cái cũ nhất;
  - `tick` làm già, cho đàn phóng (đúng thời điểm trong tick) và bỏ cái hết giờ;
  - `spring` cho bẫy bật;
  - `detonate` cho mìn nổ;
  - `hurt` gây sát thương lên một cái.
- Triệu hồi chỉ là bản ghi có máu; trận quyết cách nó di chuyển và đánh.

**`FIGHT_READS` giờ rỗng.** Mọi stat đều được đọc trong bộ tính; test phủ (`tests/stat.rs`) đọc cả các module trên. Stat
mới mà bộ tính chưa đọc được thì vào lại danh sách này.
