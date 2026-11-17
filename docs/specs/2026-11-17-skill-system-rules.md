# Hệ thống thần thông: luật để code

Bản này đi kèm `2026-11-17-skill-system-design.md` (gọi tắt là **bản thiết kế**). Bản thiết kế nói hệ có những gì và
vì sao. Bản này nói **chính xác** chúng tính ra sao, đủ để viết `pixfx-duel` theo. Mọi con số là số khởi điểm, đo
lại bằng ladder. Khi một con số ở đây khác với `data.rs` sau này, `data.rs` đúng.

Luật chung: mô phỏng chạy 60 tick mỗi giây và tất định theo seed. Mọi phép tính dùng `f64`, làm tròn về số nguyên
chỉ khi trừ vào thanh. Mọi "ngẫu nhiên" (chí mạng, bài xích) rút từ RNG của trận theo thứ tự cố định ở mục 9.

---

## 1. Đơn vị và thang

| Đại lượng | Giá trị khởi điểm | Ghi chú |
|---|---|---|
| Tick | 1/60 giây | Như `duel::TICK`. |
| **T** (một thân) | chiều cao đấu sĩ | Mọi tầm tính bằng T. Cận < 1T, Trung 1–4T, Xa > 4T. |
| Máu | 3000 | Thay theo căn cơ: Cổ +20%, Tiên −10%. |
| Hộ thân linh quang | 600 | Hồi 60/giây sau 2 giây không bị trúng. Vỡ thì 1 giây không hồi. |
| Thần thức | 100 | Hồi 4/giây. Ý cảnh giữ chỗ (mục 7). |
| Linh lực | 100 | Hồi 3/giây, thiền 16/giây. |
| Thọ nguyên | 100 | **Không hồi trong trận.** Về 0 thì "già": −20% sát thương, tốc độ và hồi phục tới hết trận. |
| Sát khí | 0–100 | +4 mỗi đòn trúng, +10 mỗi triệu hồi bị hạ, −2/giây khi không đánh. |
| Hồn | 0–30 | +1 mỗi triệu hồi chết trong 4T quanh mình, +1 mỗi 100 sát thương Hồn gây ra. |
| Tinh điểm | 0–3 mỗi loại (Cổ tu 0–5) | Thần, Yêu, Ma. |
| Bài xích | 0–100 | Mục 8.3. |
| Kiên | 0–3 | Mục 4.4. |

---

## 2. Thần thông, bí quyết, kinh mạch

### 2.1 Định nghĩa một thần thông

```text
ThanThong {
  id, ten, phẩm_khởi: 1..5,
  hình: Form, tầm: Range, đưa: Delivery, tags_thêm: [Tag],
  hành: [(Loại, tỉ lệ)],         // sát thương gốc chia theo loại, tổng 1.0
  gốc(hỏa_hầu): f64,             // sát thương gốc ở hỏa hầu đó
  hiệu_lực: f64,                 // hệ số cho sát thương cộng thêm (mục 3.2), 0.5..2.0
  chi_phí: [(TàiNguyên, f64)],
  hồi_chiêu, windup, recover: giây,
  số_lần_mỗi_trận: Option<u8>,   // lá bài một lần
  có_lộ_bài: bool,
  hiệu_ứng: [HiệuỨng],           // ailment ép, khống chế, dấu, vùng, triệu hồi...
}
```

`gốc(L) = gốc(1) × 1,07^(L−1)`, hỏa hầu L từ 1 tới 20. Ngộ (0–20%) thêm vào hiệu quả riêng của từng ngọc (ghi theo
ngọc).

### 2.2 Gắn bí quyết

1. Một bí quyết tác động lên **mọi thần thông cùng mạch** có đủ tag nó đòi. Thiếu tag thì nó nằm đó vô hiệu, giao
   diện báo đỏ. Bot thấy cờ "vô hiệu".
2. Không gắn hai bí quyết cùng tên trên một mạch.
3. **Hệ số chi phí** của các bí quyết nhân với nhau: Tàn Ảnh 1,5, Quy Tắc 1,4, Chắc Trúng 1,2… tức chi phí cuối =
   chi phí gốc × tích các hệ số.
4. Bí quyết **loại trừ nhau**:

| Cặp | Vì sao |
|---|---|
| Hóa Ma + Âm Hàn | Hai chuyển hoá 100% đích khác nhau. |
| Cực Cảnh (đạo tâm) + mọi bí quyết chuyển hoá hoặc Dung | "không dung hợp với bất cứ thuộc tính nào khác" |
| Cửu Chuyển + Tàn Ảnh | Cả hai đổi nhịp thi triển. |
| Súc Địa + chiêu Tích | Lao tới thì không giữ phím được. |
| Huyết Luyện + Phun Máu | Cùng tiêu máu theo hai luật khác nhau. |

5. **Dung**: gắn trên một mạch có đúng hai thần thông. Hai chiêu thành một chiêu mới:
   - hình, tầm, đưa, windup và recover của chiêu **thứ hai**;
   - hành và hiệu ứng của chiêu **thứ nhất**;
   - sát thương = 0,6 × (gốc₁ + gốc₂);
   - chi phí = tổng hai chi phí;
   - hồi chiêu = cái dài hơn.

   Mỗi lần dùng có 15% bài xích: nổ vào chính người dùng 20% sát thương chiêu, rút từ RNG.

---

## 3. Thứ tự tính sát thương

Mỗi đòn đi qua 12 bước theo đúng thứ tự này. Chiều sâu của hệ nằm ở thứ tự, nên không được đổi chỗ các bước.

### 3.1 Bước 1: sát thương gốc

`G = gốc(L)`, chia theo `hành` của chiêu thành các phần theo loại.

### 3.2 Bước 2: cộng thêm

Mỗi nguồn cộng thêm phẳng (bí quyết Trọng thêm Lực, Phun Máu, đạo đồ) nhân với `hiệu_lực` của chiêu, rồi cộng vào
phần của loại đó.

### 3.3 Bước 3: chuyển hoá

Thứ tự loại cố định:

**Lực → Kim → Mộc → Thủy → Hỏa → Thổ → Lôi → Dương → Âm → Hồn**

- Chỉ chuyển **về phía sau** trong chuỗi, không bao giờ ngược lại. Ví dụ không có chuyển Lôi thành Thủy. Riêng Âm Hàn
  (Hỏa, Lôi → Thủy) là ngoại lệ có tên, đặt ở đạo tâm, và nó được tính **trước** mọi chuyển hoá khác.
- Thứ tự áp: chuyển hoá của chính chiêu, rồi của bí quyết, rồi của đạo tâm. Tổng chuyển từ một loại vượt 100% thì cả
  ba được thu nhỏ theo cùng tỉ lệ cho đủ 100%.
- Phần đã chuyển **mang theo tag của loại gốc**. Mọi "tăng sát thương Lực" vẫn tác động lên phần Lực đã thành Hỏa.
  Đây chính là luật của PoE và là nguồn của nhiều build.

### 3.4 Bước 4: tăng và nhân

- **Tăng** (cộng gộp): cộng mọi % "tăng" áp được, gồm tăng theo loại gốc và loại đích. `× (1 + Σtăng)`.
- **Nhân**: mỗi nguồn "nhân" nhân riêng. Ví dụ Cực Cảnh ×1,6, Tiên Trì ×1,4, Tàn Ảnh dồn đòn, máu trả (mỗi 1% máu tối
  đa đã trả thì ×1,03). `× Π(1 + nhân_i)`.

### 3.5 Bước 5: chí mạng

- Cơ hội gốc 5%, +3% mỗi Ma tinh. Hệ số 1,5, +0,1 mỗi Ma tinh.
- Rút một lần cho cả đòn.
- Đòn Thuấn không chí mạng, đổi lại không né được.

### 3.6 Bước 6: dung hợp (Tạo)

Mỗi đấu sĩ giữ **cửa sổ dung hợp**: lượng sát thương theo từng loại đã nhận trong 1,0 giây vừa qua.

Khi đòn có loại B trúng, và cửa sổ đang có loại A tạo thành một cặp trong bảng dưới:

- Sinh ra phần thêm loại F = **0,4 × min(A trong cửa sổ, B của đòn này)**.
- Phần F **không tính kháng** của A hay B. Chỉ kháng riêng của F mới tác động, và chỉ đạo tâm mới cho kháng F.
- Xóa A khỏi cửa sổ, nên mỗi lần A chỉ dùng được cho một lần dung hợp.
- Hiệu ứng thêm của F áp lên mục tiêu.

| A | B | F | Hiệu ứng thêm |
|---|---|---|---|
| Hỏa | Thủy (hoặc ngược lại) | **Phong** | Đẩy 1T và khóa hồi linh lực 1 giây. |
| Phong | Thủy | **Băng** | Đóng băng ngay 0,6 giây (vẫn tính Kiên). |
| Lôi | Hỏa (hoặc ngược lại) | **Lôi hỏa** | Nổ lần hai sau 0,3 giây vào Thần thức, bằng 50% F. |
| Thổ | Hỏa | **Sơn băng** | Để lại vùng khói 2T trong 3 giây, mỗi giây 10% F loại Âm. |
| Âm | Dương (hoặc ngược lại) | **Lưỡng nghi** | Bỏ qua mọi kháng. Người gây đòn nhận 25% F. |

Luân hồi (Sinh + Tử) không ở bảng này vì nó là luật của đạo Sinh tử (mục 6.1).

Phong và Băng sinh từ dung hợp cũng là "loại" trong cửa sổ, nên có chuỗi: Hỏa, rồi Thủy (sinh Phong), rồi Thủy nữa
(sinh Băng). Đây là chuỗi Hô Phong cộng Hoán Vũ.

### 3.7 Bước 7: ngũ hành sinh khắc

Mỗi đấu sĩ có **bản mệnh hành** (một trong năm, chọn theo căn cơ hoặc đạo đồ, có thể không có).

| Quan hệ giữa hành của phần sát thương và bản mệnh của mục tiêu | Hệ số |
|---|---|
| Phần sát thương khắc bản mệnh (ví dụ Thủy đánh vào người bản mệnh Hỏa) | ×1,25 |
| Bản mệnh khắc phần sát thương | ×0,8 |
| Cùng hành | ×0,7; mục tiêu hồi 10% phần đó vào linh lực ("hỏa đánh vào hỏa linh thú còn làm nó mạnh thêm") |
| Sinh hoặc được sinh | ×1,0 |

Ngoài ngũ hành:

- Lôi ×1,5 lên triệu hồi có tag Âm hoặc Trùng.
- Âm và Dương ×1,5 lên đấu sĩ hay triệu hồi mang đạo thống bên kia.
- Hỏa bậc đạo hỏa (mục 4.2) đánh vào bản mệnh Thủy thì tính như **khắc** (×1,25), không phải bị khắc.

### 3.8 Bước 8: giảm thương

| Loại | Giảm bởi | Công thức |
|---|---|---|
| Lực | Hộ thể H | `giảm = H / (H + 5 × D)`, D là sát thương phần Lực của đòn, trần 90%. Đòn càng to thì hộ thể càng ít tác dụng, như PoE. Mỗi tầng Phá −10% H. |
| Kim, Mộc, Thủy, Hỏa, Thổ, Lôi, Dương | Kháng R của loại đó | `× (1 − R)`. R từ −100% tới trần 75%. Phá kháng của người đánh trừ thẳng vào R trước khi tính. |
| Âm | Kháng Âm | Như trên. Thêm: **bỏ qua Hộ thân** (bước 11). |
| Hồn | Kháng Hồn | Như trên. Thêm: đánh vào Thần thức (bước 11). Mục tiêu không hồn thì miễn. |

### 3.9 Bước 9: phẩm và khiên

Xem mục 5. Đòn bị một khiên có phẩm ≥ phẩm của đòn chặn, thì phần bị chặn dừng ở đây. Cấm chế đứng ngoài thang.

### 3.10 Bước 10: frame data

Nhân cuối, giữ như hiện tại:

- **Phản đòn**: trúng người đang windup thì ×1,2 và làm khựng thêm 0,10 giây.
- **Phạt hớ**: trúng người đang recover thì ×1,2 và làm khựng thêm 0,20 giây.

### 3.11 Bước 11: trừ vào thanh

- Mọi loại trừ Âm và Hồn: trừ Hộ thân trước, phần dư trừ Máu.
- Âm: trừ thẳng vào Máu.
- Hồn: trừ Thần thức trước, phần dư trừ Máu với hệ số 0,5.

### 3.12 Bước 12: ailment

Mục 4. Ailment tính trên sát thương **trước giảm thương** của phần loại tương ứng (sau bước 7). Vì vậy kháng cao làm
giảm sát thương, nhưng không làm giảm ailment tới cùng tỉ lệ. Ngược lại, kháng ailment là một chỉ số riêng.

---

## 4. Ailment

### 4.1 Luật chung

- Mỗi đòn gây một phần loại X thì thêm **tầng ailment X**:
  `tầng thêm = k_X × (phần X sau bước 7) / (Máu tối đa của mục tiêu) × 100`, rồi làm tròn xuống. Phần lẻ cộng dồn
  vào một bộ đếm riêng, nên đòn nhỏ vẫn tích được.
- Tầng có trần. Mỗi tầng có thời hạn riêng và rụng theo thứ tự cũ trước.
- Kháng ailment K_X (0 tới 60%) giảm số tầng thêm vào: `× (1 − K_X)`.
- "Đủ tầng" thì có **hiệu ứng đầy**. Sau đó tầng về 0 và có 2 giây không tích lại được.

### 4.2 Bảng ailment

| Ailment | Loại | k | Trần | Mỗi tầng | Thời hạn tầng | Hiệu ứng đầy |
|---|---|---|---|---|---|---|
| **Thương** | Lực (đòn Kiếm, Chỉ, Kim) | 1,0 | 10 | Mất 0,4% máu tối đa mỗi giây; ×2 khi đang di chuyển | 4 s | — (chỉ để Huyết Tán bắt) |
| **Chấn** | Lực (đòn Quyền, Chưởng, Thổ) | 1,2 | 10 | — | 3 s | Choáng 0,5 s (tính Kiên) |
| **Phá** | Kim | 1,0 | 5 | −10% hộ thể | 5 s | Hộ thân vỡ ngay |
| **Triền** | Mộc | 1,0 | 6 | −8% tốc chạy; đòn Mộc hút 2% sát thương thành máu | 3 s | Trói chân 0,8 s (vẫn đánh và niệm được) |
| **Hàn** | Thủy | 1,0 | 6 | −8% tốc chạy và tốc niệm | 3 s | **Băng phong** 0,9 s (tính Kiên). Đòn kết liễu lên người đang băng phong: mục tiêu dưới 10% máu thì vỡ (chết). |
| **Thiêu** | Hỏa | 0,8 | 8 | Mất 0,5% máu tối đa mỗi giây, xuyên hộ thể | 4 s | — (bậc lửa, mục 4.3) |
| **Trọng** | Thổ | 1,0 | 5 | −10% quãng dash và độn; người đang bay rơi 0,5T mỗi tầng | 3 s | Quật ngã 0,6 s |
| **Tê** | Lôi | 1,0 | 5 | +4% sát thương phải nhận | 3 s | Giật 0,3 s và ngắt chiêu đang windup |
| **Chiếu** | Dương | 1,0 | 3 | Lộ ảo ảnh và tàng hình; buff Âm trên người giảm 15% hiệu lực | 3 s | Xóa mọi buff có tag Âm |
| **Tịch** | Âm | 1,0 | 6 | −10% hồi máu, +3% sát thương Âm phải nhận | 5 s | 3 s: mọi hồi máu thành mất máu |
| **Nhiếp** | Hồn | 1,0 | 5 | +6% windup | 3 s | Xem Thần thức: Thần thức về 0 thì thất hồn |

### 4.3 Bậc lửa

- Mỗi chiêu Hỏa có **bậc**:
  - đỏ = 1 (mặc định);
  - lam = 2 (hỏa hầu 8);
  - tím = 3 (hỏa hầu 14, hoặc bí quyết Hư Hỏa);
  - đen = 4 (chỉ Hỏa Bản Nguyên).
- Thiêu bậc cao thay Thiêu bậc thấp, giữ số tầng.
- **Lửa nuốt lửa**: đạn Hỏa bậc thấp hơn bay vào vùng Hỏa (lĩnh vực, trận, khiên lửa) bậc cao hơn của đối thủ thì
  bị hủy, và chủ vùng hồi linh lực bằng 10% sát thương của đạn.
- **Hư hỏa**: mỗi chiêu mục tiêu dùng trong lúc đang Thiêu thì thêm 1 tầng Thiêu, không cần đòn.
- **Đạo hỏa** (bậc 4): khi Thiêu đầy, rút thêm 20 Thần thức.

### 4.4 Kiên (giảm khống chế cứng)

Khống chế cứng gồm choáng, băng phong, định thân, quật ngã, giật và trói chân.

- Thời gian thật = `cơ_bản × 0,5^Kiên`.
- Mỗi lần dính khống chế cứng thì Kiên +1, trần 3.
- Kiên −1 sau mỗi 2 giây không dính khống chế.
- Thêm: mục tiêu có **phẩm ý cảnh** cao hơn người khống chế thì thời gian thật × 0,5 (luật "tự giải" của Định Thân).

### 4.5 Khắc và sinh giữa ailment

Đòn có phần loại A trúng mục tiêu đang mang ailment của loại B:

| Quan hệ | Kết quả |
|---|---|
| A **khắc** B theo ngũ hành | Xóa toàn bộ tầng ailment B. Thêm sát thương loại A bằng `tầng B × 2% máu tối đa của mục tiêu`. Ví dụ Thủy dập Thiêu, Hỏa nung Phá, Kim cắt Triền, Mộc ăn Trọng, Thổ vùi Hàn. |
| A **được sinh** bởi B (B sinh A) | Ailment A của đòn này **+50% tầng** và thời hạn ×1,5. Ví dụ đang Triền (Mộc) thì Thiêu (Hỏa) dồn nhanh; đang Hàn (Thủy) thì Triền (Mộc) dồn nhanh. |
| A = Lôi, B = Băng phong | Phá băng: xóa băng phong, thêm Lôi bằng 15% máu tối đa. |
| A = Thủy (vùng hoặc khiên), B = đạn Lôi bay qua | Đạn Lôi bị đóng băng, rơi xuống, không nổ. |
| A = Dương, B = Tịch, hoặc ngược lại | Hai bên triệt tầng cho nhau một đổi một. Phần dư mới tính. |

---

## 5. Phẩm

### 5.1 Ai có phẩm

| Có phẩm | Ví dụ |
|---|---|
| Mỗi đòn | Lấy phẩm của chiêu sinh ra nó. |
| Mỗi khiên, ward, barrier | Lấy phẩm của chiêu dựng nó. |
| Mỗi vùng, lĩnh vực, ý cảnh, buff, nguyền | Lấy phẩm của chiêu sinh ra nó. |
| Mỗi giải trừ (dispel) | Lấy phẩm của chiêu giải trừ. |

Phẩm của chiêu = `phẩm_khởi`, +1 nếu gắn bí quyết Quy Tắc (trần 3), +1 nếu ngọc Ngộ đạo (trần 4). Phẩm 4 chỉ có ở
chiêu bản nguyên, phẩm 5 chỉ có ở chiêu tín niệm.

### 5.2 Luật

1. **Khiên**: khiên phẩm P chặn được đòn phẩm ≤ P. Đòn phẩm > P đi xuyên khiên mà không làm khiên mất gì.
2. **Giải trừ**: giải trừ phẩm P xóa được hiệu ứng phẩm ≤ P.
3. **Vùng gặp đòn**: đạn phẩm > P bay qua vùng phẩm P (như Tuế Nguyệt làm chậm đạn) thì không bị vùng tác động. Kiếm
   hay Chưởng phẩm 3 trở lên đi qua vùng phẩm thấp hơn thì phá vùng đó.
4. **Ý cảnh đối ý cảnh**: xem mục 7.2.
5. **Cấm chế** không có phẩm:
   - không bị giải trừ ở bất kỳ phẩm nào;
   - không bị phản, không bị cướp, không bị nuốt (kể cả bởi Thôn Cấm: Thôn Cấm chỉ ăn trận và bẫy, không ăn cấm
     chế);
   - có **độ bền** và chỉ vỡ khi nhận đủ sát thương bằng độ bền;
   - chỉ chiêu phẩm 5 xóa được nó (Đạp Thiên Nhất Bộ xóa mọi thứ trên sân).
6. **Hư bản nguyên**: buff phẩm 4 có cờ `hư` (Sinh tử, Nhân quả, Chân giả) không bị xóa kể cả bởi đòn phẩm 4. Chỉ phẩm
   5 xóa được nó.

---

## 6. Đạo: luật cụ thể

### 6.1 Sinh tử

- **Dấu Sinh** (từ chiêu tag Sinh): hồi cho mục tiêu H máu, và dấu ghi nhớ H. Thời hạn 5 giây. Tối đa 1 dấu mỗi
  mục tiêu; dấu mới cộng dồn H, trần 30% máu tối đa của mục tiêu.
- **Kích Tử** (đòn có tag Tử lên mục tiêu mang dấu): gây thêm Âm bằng `2,5 × H`, rồi xóa dấu.
- **Cạn sinh hóa tử**: người tu Sinh tử dưới 10% máu thì mọi chi phí máu chuyển thành chi phí Tử khí (một thanh 0–50
  chỉ mở khi đó, nạp bằng sát thương Âm gây ra).
- **Luân hồi**: trong 1 giây vừa kích Tử lên một mục tiêu, nếu lại gieo Sinh lên chính nó, thì mục tiêu bị kéo về vị
  trí và máu của 1 giây trước, hủy chiêu đang windup.

### 6.2 Nhân quả

**Sợi nhân quả** nối hai thực thể A và B: hai đấu sĩ, đấu sĩ với triệu hồi, đấu sĩ với khiên. Thời hạn ghi theo
chiêu. Đòn đánh vào A thì 30% (theo chiêu) vang sang B, cùng loại, cùng phẩm.

- Khí Nhân Quả: khi khiên K của X vỡ bởi đòn D, tạo một đòn D′ = 50% D bắn từ X vào người gây D.
- **Tử Ấn Tổ Quyết**: sợi giữa hai đấu sĩ, độ dài tối đa N T. Kéo quá thì bị giật lại.
  - Người nguyền chết thì người bị nguyền mất 50% máu hiện có. Đây là điểm khác truyện, vì giết ngay thì quá mạnh trong
    1v1.
  - Mỗi 3 giây sợi lộ ra 0,5 giây. Chém trúng (Kiếm, phẩm ≥ phẩm của sợi) trong lúc lộ thì đứt.

### 6.3 Chân giả

Chiêu Ảo sinh **đòn ảo**: hiển thị y như đòn thật, nhưng gây 50% sát thương và 0 ailment.

- **Ảo hóa thật**: trong 3 giây sau khi đòn ảo xuất hiện, nếu mục tiêu dùng một chiêu tag Phòng thủ, Độn hoặc một chiêu
  phẩm cao nhất của mình, thì đòn ảo (nếu còn bay) thành thật: 100% sát thương, đủ ailment, xuyên hộ thể 30%.
- Chiếu (Dương) và chiêu Chân giả nhìn thấu: đòn ảo hiện viền khác cho riêng người bị Chiếu. Bot nhận cờ `ảo` cho đòn
  ấy.
- Hai chiêu Chân giả của hai bên cùng hoạt động trong 0,5 giây: cả hai nhận 10% máu tối đa sát thương Hồn.

### 6.4 Tuế nguyệt

- **Chậm**: mọi thực thể trong vùng chạy ở tốc độ × (1 − s). Tick vẫn chạy đủ, chỉ vận tốc và đồng hồ chiêu bị nhân.
- **Quay lui** (Đảo Ngược Thời Gian, Đảo Ngược):
  - Ghi đệm vị trí và máu 2 giây gần nhất của mỗi đấu sĩ (120 mẫu).
  - Quay lui t giây thì đặt lại vị trí, máu, Hộ thân của mục tiêu về mẫu `now − t`.
  - Không quay lại tài nguyên đã tiêu, trừ tự bạo (dưới).
- **Chặn tự bạo**: quay lui trúng người đang windup một chiêu Tự bạo thì hủy chiêu và **trả lại** chi phí cho người
  tự bạo. Người tự bạo không mất máu và không gây nổ.
- **Người xem cây**: đấu sĩ đứng yên (vận tốc 0 suốt 0,5 giây) với đạo tâm Người Xem Cây thì miễn chậm và quay lui.
- Mọi chiêu Tuế nguyệt tốn thọ nguyên.

### 6.5 Không gian

- **Khóa thuấn di** (trạng thái): không dùng được chiêu tag Độn, trừ Độn có tag Tuế nguyệt (Quỷ Sát).
- **Khe nứt**: vùng 1T tồn tại 1 giây. Đấu sĩ kết thúc một lần thuấn di bên trong khe thì nhận 20% máu tối đa sát thương
  Lực, xuyên hộ thể. Đấu sĩ đi bộ qua thì không sao.
- Thuấn di bỏ qua mọi đạn đang bay tới trong windup của nó, trừ đòn Thuấn.

### 6.6 Duyên

- Mỗi thực thể "của" một đấu sĩ có một sợi duyên tới đấu sĩ đó: pháp bảo đặt ra, triệu hồi, vùng, buff có nguồn ngoài
  (mọi buff không do chính chiêu của đấu sĩ sinh ra: lĩnh vực, sân, đồng minh).
- **Trảm duyên** cắt một sợi: thực thể ở đầu kia tắt **tới hết hiệp**, không chỉ hết thời hạn.
- Thứ tự cắt khi chiêu không chỉ định: buff môi trường, rồi pháp bảo, rồi triệu hồi.
- Duyên của chính thân (máu, tu vi) không cắt được.

### 6.7 Mệnh

- **Chém mệnh cách** gỡ, theo thứ tự ưu tiên:
  1. một mạng phụ (Tam Mệnh, Ngọn Đèn Hồn);
  2. buff phẩm cao nhất không có cờ `hư`;
  3. 10 thọ nguyên.
- Rút thọ nguyên của đối thủ: chỉ chiêu tag Mệnh hoặc Tuế nguyệt làm được.

### 6.8 Sát lục

- Sát khí ≥ 50: +1% sát thương mỗi điểm trên 50.
- Sát khí = 100 trong 3 giây liền: **Tâm ma**, mỗi giây mất 1% máu tối đa cho tới khi sát khí xuống dưới 80.

### 6.9 Nguyền

- **Vật dẫn**: một đòn Cận trúng đích thì người đánh nhận một "vật dẫn" của mục tiêu: máu, tóc, giữ 20 giây, trần 3.
  - Chiêu nguyền tag Phù hay Hồn dùng từ Xa thì tiêu một vật dẫn.
  - Không có vật dẫn thì chỉ nguyền được ở Cận.
- **Trần**: 1 nguyền mỗi đấu sĩ (2 với đạo tâm Vu Chú). Nguyền mới thay nguyền cũ nếu phẩm ≥. Không thì nguyền mới
  thất bại và vẫn tốn chi phí.
- **Lan cùng nguồn**: mọi triệu hồi và phân thân của mục tiêu nhận cùng nguyền với 50% hiệu lực.
- **Phản phệ**: đấu sĩ X đang nguyền Y mà bị Y nguyền lại, thì nguyền của Y lên X có hiệu lực ×1,5 và nguyền của X lên
  Y rụng.
- **Chúc phúc** (chiêu tag Chúc) xóa một nguyền phẩm ≤ trên chính mình và đổi nó thành buff ngược chiều trong 3 giây.

---

## 7. Ý cảnh, tranh đạo niệm, lĩnh vực

### 7.1 Ý cảnh

- Bật ý cảnh thì giữ chỗ c Thần thức. Thần thức tối đa còn lại = 100 − Σc. Giữ chỗ khởi điểm: 20–35 tùy ý cảnh.
- Ý cảnh tắt khi thất hồn. Bật lại thì windup 1 giây.
- Bán kính ý cảnh: Trung (3T).

### 7.2 Tranh đạo niệm

Mở khi vùng ý cảnh **áp** của hai bên chồng lên nhau, tức hai đấu sĩ cách nhau dưới tổng bán kính và cả hai đang bật
ý cảnh áp.

- Thanh đạo niệm `x` từ −1 tới 1, bắt đầu ở 0. Mỗi giây: `x += 0,25 × (lực_A − lực_B)`.
- Lực của một bên = `phẩm + 0,5 × [tự ngộ] + 1,0 × [ý cảnh của mình khắc ý cảnh của đối thủ, bảng ở bản thiết kế mục
  6.3] + (Thần thức hiện tại / 100)`.
- `x` chạm 1: B thất hồn. Chạm −1: A thất hồn. Thanh về 0 khi vùng không còn chồng.
- **Nghịch chuyển**: mỗi hiệp một lần, tiêu toàn bộ linh lực hiện có (tối thiểu 50) để đặt `x` về 0 phía mình.
- Bot nhận `x` và hai giá trị lực.

### 7.3 Lĩnh vực

- Mỗi bên tối đa một lĩnh vực.
- Hai lĩnh vực của hai bên thì sân chia đôi tại trung điểm giữa hai tâm. Mỗi nửa theo luật của lĩnh vực chủ nửa đó.
- Lĩnh vực có thời hạn (6–10 giây). Khi lĩnh vực **sụp** (hết hạn hoặc bị xóa), mọi đấu sĩ trong vùng bị khóa thuấn
  di 1 giây (luật "đánh lúc sân sụp").
- Ly Phong đè mọi lĩnh vực khác nếu phẩm ≥.
- Phẩm 5 xóa tất cả.

---

## 8. Tài nguyên: luật chi tiết

### 8.1 Chi phí

- Một chiêu có thể có nhiều chi phí (linh lực và thọ nguyên…).
- Chiêu không dùng được nếu thiếu bất kỳ chi phí nào, trừ máu: máu được trả tới khi còn 1, không bao giờ chết vì trả
  máu.
- Chi phí trả khi windup **bắt đầu**. Chiêu bị phá giữa windup thì mất hết chi phí, trừ khi bị Tuế nguyệt quay lui
  (mục 6.4).

### 8.2 Tinh điểm

- Nhận tinh:
  - **Thần tinh**: mỗi lần chịu một đòn trên 8% máu tối đa.
  - **Yêu tinh**: mỗi lần né hoặc thuấn di qua một đòn.
  - **Ma tinh**: mỗi lần chí mạng.
- Mỗi tinh tồn tại 10 giây. Nhận tinh mới thì làm mới thời hạn của mọi tinh cùng loại.
- Hiệu ứng mỗi tinh:
  - Thần: −4% sát thương nhận vào, +4% mọi kháng.
  - Yêu: +5% tốc đánh và tốc chạy.
  - Ma: như mục 3.5, +5% sát thương Hồn.
- **Toái tinh**: tiêu mọi tinh một loại. Mỗi tinh tiêu cho ×1,25 sức của chiêu Toái Tinh, hoặc hồi 8% máu tối đa.

### 8.3 Bài xích

- Dùng một chiêu tag Tiên trong 2 giây sau một chiêu tag Cổ, hoặc ngược lại, thì Bài xích +20.
- Bài xích −10 mỗi giây.
- Đủ 100: nổ vào chính mình 15% máu tối đa, rồi về 0.
- Đạo tâm Đạo Cổ (bản nguyên Tiên Cổ, sau này) đổi luật: Bài xích đầy cho ×1,5 đòn kế tiếp thay vì nổ.

### 8.4 Lá bài một lần và Lộ bài

- `số_lần_mỗi_trận` đếm trên cả trận, không hồi giữa hiệp.
- **Lộ bài**: hành động riêng của slot đó. Windup 0,4 giây, hiện hình chiêu, không tốn lần dùng.
  - Trong 1,5 giây sau đó, nếu đối thủ dùng một chiêu tag Phòng thủ hoặc Độn, thì chiêu ấy vào hồi chiêu gấp đôi và
    người lộ bài nhận 10 Sát khí.
  - Đối thủ đứng yên hoặc tấn công thì không có gì xảy ra.
- Mỗi lá bài lộ được tối đa 2 lần mỗi hiệp. Lần thứ ba trở đi không còn hiệu lực dọa.

---

## 9. Thứ tự xử lý trong một tick

Cố định, để mô phỏng tất định:

1. Đọc hành động của hai ghế: ghế 0 rồi ghế 1.
2. Cập nhật lĩnh vực, ý cảnh, thanh đạo niệm.
3. Di chuyển, độn, quay lui (Tuế nguyệt).
4. Tiến đồng hồ windup và recover. Chiêu tới cuối windup thì sinh đòn hoặc thực thể.
5. Đạn bay. Đạn gặp vùng thì áp luật vùng (mục 5.2, 4.3, 4.5).
6. **Giải quyết đòn trúng**. Hai đòn trúng cùng tick thì giải theo thứ tự: phẩm cao trước; cùng phẩm thì đòn Thuấn
   trước; cùng nữa thì ghế có `(seed + tick) mod 2` nhỏ hơn trước. Mỗi đòn đi đủ 12 bước ở mục 3. Chí mạng và bài xích
   rút RNG theo đúng thứ tự giải đòn này.
7. Ailment: rụng tầng hết hạn, tick sát thương theo thời gian.
8. Khống chế và Kiên.
9. Hồi tài nguyên.
10. Kiểm tra chết, mạng phụ, hết hiệp.

---

## 10. Các trường hợp đụng nhau

| Trường hợp | Kết quả | Vì sao |
|---|---|---|
| Quay lui lên người đang đứng trong lồng cấm chế | Vị trí quay lui, nhưng không ra khỏi lồng: lồng chặn như tường. | Cấm chế không phải pháp thuật. |
| Thuấn di ra khỏi lồng cấm chế | Không được, trừ khi lồng đã vỡ. | Như trên. |
| Tam Mệnh kích khi đang bị Tử Ấn | Hồi sinh, sợi Tử Ấn đứt ("cắt nhân quả"). | Bản thiết kế, Tam Mệnh. |
| Người nguyền chết, nguyền còn | Nguyền rụng. Riêng Tử Ấn thì áp luật 6.2. | — |
| Cùng lúc hai lĩnh vực phẩm bằng nhau | Chia đôi sân (7.3). | — |
| Chuyển hoá đưa phần sát thương sang Hồn, mục tiêu không hồn | Phần Hồn mất. | Miễn Hồn. |
| Âm Hàn (đạo tâm) và chiêu có hành Lôi gốc | Âm Hàn tính trước mọi chuyển hoá (3.3). 50% Lôi thành Thủy, phần còn lại vẫn là Lôi. | — |
| Phản ấn bắt một đạn phẩm cao hơn khiên phản | Không bắt được; đạn đi xuyên. | 5.2.1 |
| Đạn Lôi bay qua vùng Thủy của chính người bắn | Không bị đóng băng. Vùng chỉ tác động đạn của đối thủ. | — |
| Hỏa đạo hỏa (bậc 4) bay qua vùng Thủy của đối thủ | Không bị dập. Vùng Thủy bậc thấp hơn thì vỡ. | Ngoại lệ 3.7. |
| Đối thủ "tin" ảo bằng cách dùng Độn | Đòn ảo hóa thật **nhưng** người đã độn khỏi đường bay thì không trúng. Thật hay ảo, né được vẫn né. | — |
| Lộ bài khi đối thủ đang bị khống chế | Không có hiệu lực dọa, vì đối thủ không thể đáp. | — |
| Băng phong và Kiên ≥ 2 | Băng phong 0,9 × 0,25 = 0,225 giây. | 4.4 |
| Dung hai chiêu, một chiêu là lá bài một lần | Không cho phép. | Tránh nhân bản lá bài. |
| Toái tinh khi không có tinh | Không dùng được. | — |
| Sinh tử: gieo Sinh lên chính mình | Được: hồi máu, và đối thủ kích Tử lên mình cũng ăn dấu ấy. Rủi ro có chủ ý. | — |
| Trảm duyên lên đấu sĩ không có thực thể ngoài | Rút 10 thọ nguyên của đối thủ thay vào. | — |
| Hai bên cùng hạ máu về 0 trong một tick | Bên có mạng phụ sống lại; không ai có thì hiệp hòa. | — |
| Tự bạo khi Đảo Ngược Thời Gian đang windup lên mình | Tự bạo nổ nếu tới cuối windup trước. Thứ tự tick: bước 4 trước bước 6. | Mục 9. |

---

## 11. Bot

Thay đổi interface (`docs/bot.md` phải viết lại cùng lúc):

- **Mỗi slot chiêu**:
  - tag one-hot: 17 hình, 4 tầm, 10 đưa;
  - phần sát thương theo 10 loại sau chuyển hoá;
  - phẩm;
  - chi phí theo 6 tài nguyên;
  - hồi chiêu còn, windup, recover;
  - số lần dùng còn;
  - các cờ `có_lộ_bài`, `ảo`, `vô_hiệu`.
- **Mỗi đấu sĩ**:
  - 3 thanh sống, 6 tài nguyên, 3 loại tinh, Kiên, bài xích;
  - tầng 11 ailment;
  - nguyền (loại, phẩm, thời hạn);
  - ý cảnh đang bật;
  - thanh đạo niệm, lĩnh vực mỗi nửa sân;
  - danh sách thực thể có duyên.
- **Hành động**: 7 slot + Lộ bài cho mỗi slot có lá bài + Nghịch chuyển.

Tổng số đặc trưng tăng mạnh, nên mạng policy đổi và mọi bot phải huấn luyện lại.

---

## 12. Kiểm thử phải có trước khi chạy ladder

- Golden cho 12 bước sát thương: một đòn cố định qua từng bước, so từng con số.
- Chuyển hoá: tổng vượt 100% thì thu nhỏ đúng; không có chuyển ngược.
- Dung hợp: mỗi cặp ở 3.6, cửa sổ 1 giây, tiêu A đúng một lần.
- Phẩm: ma trận khiên phẩm 1–5 đấu đòn phẩm 1–5, cộng cấm chế.
- Quay lui chặn tự bạo và trả chi phí.
- Thứ tự giải hai đòn cùng tick, chạy với nhiều seed và ra cùng kết quả mọi máy.
- Mọi dòng của bảng mục 10.
