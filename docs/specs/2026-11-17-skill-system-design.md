# Hệ thống thần thông: thiết kế mới, gốc Tiên Nghịch và Cầu Ma, khung tương tác Path of Exile

Trạng thái: **đề xuất**, chưa có dòng code nào. Bản này thay toàn bộ hệ skill hiện có (`docs/rules.md`), nên
cần mùa giải mới, huấn luyện lại, và chủ dự án quyết các luật và cân bằng (`CLAUDE.md`, "Traps"). Mọi con số ở đây
chỉ là số khởi điểm, phải đo bằng ladder (`--seeds 150 --telemetry`).

Nguồn: 14 bản khai thác truyện ở `references/skill-mining/` (Tiên Nghịch 8 đoạn, Cầu Ma 6 đoạn, khoảng 1.400 mục kỹ
thuật kèm số chương). Khi trích, bản này ghi `TN<chương>` hoặc `CM<chương>`.

---

## 0. Ý tưởng một câu

Thần thông là **viên ngọc** (gem) gắn vào **khiếu huyệt**. Các khiếu nối với nhau bằng **kinh mạch**. Mỗi thần
thông chính được nối với những **bí quyết** (support) làm biến đổi nó. Mỗi chiêu mang **tag**: hình thức, tầm, hành,
đạo, phẩm. Tag quyết định chiêu ăn bí quyết nào, khắc gì, bị gì khắc. Luật tương tác lấy từ truyện, không bịa:
ngũ hành sinh khắc, băng hỏa hợp thành gió, lôi phá băng, lửa màu cao nuốt lửa màu thấp, cấm chế không phải pháp thuật
nên không giải được, quy tắc đè thần thông, bản nguyên đè quy tắc.

Ba trụ thiết kế:

1. **Kết hợp sâu như PoE.** Một chiêu cộng 3–5 bí quyết ra hàng trăm biến thể. Có chuyển hoá sát thương, kích hoạt,
   ailment, nguyền rủa, aura, keystone.
2. **Đọc nhau như đối kháng 1v1.** Lá bài úp, dọa mà không đánh, mồi nhử, ngắt chiêu đang tích, xem thử ai tin ảo
   ảnh. Đây là cách Vương Lâm và Tô Minh thắng trận (mục 10).
3. **Bot đọc được.** Mọi thứ là tag và con số, không là tên. Bot thấy "Chỉ · Xa · Thuấn · Lôi · Thần thông", không cần
   biết đó là Cực Cảnh Thần Thức.

---

## 1. Từ vựng: viên ngọc, khiếu, kinh mạch

| PoE | Ở đây | Nghĩa |
|---|---|---|
| Active skill gem | **Thần thông** | Một chiêu dùng được. |
| Support gem | **Bí quyết** | Gắn vào thần thông để biến đổi nó. Không tự dùng được. Gốc: Thanh Trần tán "một mình nó không có tác dụng" (CM1). |
| Socket | **Khiếu** | Chỗ gắn một viên ngọc. |
| Link | **Kinh mạch** | Nối các khiếu. Khiếu nào chung một mạch thì bí quyết ở đó tác động lên thần thông ở đó. |
| Item with sockets | **Pháp bảo bản mệnh** | Mỗi đấu sĩ mang 3 pháp bảo bản mệnh, mỗi món có một mạch 2–6 khiếu. |
| Passive tree | **Đạo đồ** | Cây điểm bị động. |
| Keystone | **Đạo tâm** | Nút lớn làm đổi luật chơi, luôn có cái giá. |
| Ascendancy | **Bản nguyên** | Nhánh chuyên sâu, chọn 1 trong 3 của mỗi căn cơ. |
| Gem level / quality | **Hỏa hầu / Ngộ** | Hỏa hầu là cấp (1–20), Ngộ là chất lượng (0–20%). |
| Awakened / Vaal gem | **Ngộ đạo / Liều mạng** | Ngộ đạo nâng phẩm (mục 5). Liều mạng là bản đổi máu, thọ nguyên hay một lần dùng mỗi trận. |

Số khiếu trên mỗi mạch tăng theo **cảnh giới** của đấu sĩ: mạch 3 khiếu ở Kết Đan, 4 ở Nguyên Anh, 5 ở Hóa Thần,
6 ở Vấn Đỉnh. Ở duel xếp hạng, mọi người chơi cùng cảnh giới, nên cảnh giới chỉ dùng cho chế độ luyện tập và các
bậc giải đấu.

---

## 2. Tag

Một thần thông mang 4 nhóm tag. Bí quyết đòi tag ("chỉ gắn được vào chiêu có tag Đạn").

### 2.1 Hình thức (form)

| Tag | Hành vi | Gốc trong truyện |
|---|---|---|
| **Chỉ** (ngón) | Tia mảnh hoặc đạn nhỏ, nhanh. | Tịch Diệt Chỉ, Hoàng Tuyền Chỉ, Định Thân điểm ngón (TN) |
| **Chưởng** | Đòn bàn tay ở tầm gần–trung, có thể đẩy. | Phiên Thiên Ấn, Man Thần chưởng |
| **Quyền** | Đòn tay không, chuỗi đòn chồng sức. | Thập Băng Quyền Ý, Trăm quyền sinh cơ, Man Thần Biến 7 quyền |
| **Kiếm** | Chém, phóng kiếm, kiếm quay về. | Trảm La, Tuyệt Ý Kiếm, vỏ kiếm 1/5–4/5 |
| **Ấn** | Dấu đặt lên mục tiêu hoặc mặt đất, nổ sau. | Sinh Tử Ấn, Nhân Quả Ấn, Dịch Linh Ấn |
| **Phiên** (cờ) | Đặt một vùng, sinh đạn, nuốt hồn. | Cấm Phiên, Hồn Phiên, Tôn Hồn Phiên |
| **Trận** | Vùng đặt trước, có điều kiện kích. | Đấu Long trận, Thất Phong trận, Cửu Tuyệt Tổn Thần trận |
| **Cấm chế** | Bẫy hoặc lồng, **không phải pháp thuật**: không giải, không phản, chỉ phá bằng sát thương. | "Cấm chế không phải pháp thuật, không thuộc quy tắc" (TN1274) |
| **Phù** | Vật ném, thường có số lần dùng. | Ngọc phù chín phù văn, giấy phù Tiên Di |
| **Độn** (di chuyển) | Thuấn di, na di, độn thổ, súc địa. | Súc Địa Thành Thốn, Quỷ Sát, huyết ảnh độn |
| **Thân** (biến thân) | Đổi hình thể có thời hạn. | Man Thần Biến, Cổ Thần hóa, Thập Hóa (Chúc Cửu Âm) |
| **Âm** (âm thanh) | Lan quanh người, xuyên khiên vật lý. | Tâm Động Lôi Âm, tiếng chuông Hàm Sơn, Đạo Linh Âm |
| **Ảo** | Bản sao, ảo cảnh, có luật tin hay không tin. | Nhật Nguyệt Tinh Thần Huyễn, Mộng Đạo |
| **Hồn** (triệu hồi) | Du hồn, ma đầu, thú, khôi lỗi, phân thân. | Du hồn (TN295), Lôi Oa, Xạ Thần Xa, Nguyệt Dực |
| **Pháp bảo** | Chiêu gắn với một vật, có thể bị cướp, bị cắt duyên, có thể tự bạo. | Cổ Kính, chuông đồng, hồ lô |
| **Ý cảnh** | Aura quanh thân, giữ thần thức. | Sát lục, Tuế nguyệt, An nhàn, Vô tình |
| **Lĩnh vực** | Biến cả sân, mỗi bên chỉ có một. | Hoàng Tuyền, Tàn Dạ, Ly Phong |
| **Thiên kiếp** | Gọi đòn từ trời xuống, có báo trước. | Sợi tơ Thiên kiếp, Tô Kiếp, Mountain Collapse |

### 2.2 Tầm

| Tag | Tầm (tính theo chiều cao đấu sĩ, khoảng 240 px) | Ghi chú |
|---|---|---|
| **Cận** | dưới 1 thân | Quyền, chưởng, kiếm chém. |
| **Trung** | 1–4 thân | Roi, chưởng xa, âm, hình quạt. |
| **Xa** | trên 4 thân | Đạn, tia, thiên kiếp. |
| **Toàn sân** | cả sân | Lĩnh vực, uy áp, một số tối thượng. |

Một số bí quyết đổi tầm của chiêu. Súc Địa đưa chiêu Cận thành "lao tới rồi đánh". Dịch Linh làm đạn mạnh dần theo
quãng bay, nên thưởng cho người đánh Xa. Đấu Long trận chỉ mạnh trong bán kính của nó, nên đối thủ thắng bằng cách
kéo giãn khoảng cách (TN154).

### 2.3 Cách đưa đòn

**Đơn** (một mục tiêu) · **Tuyến** (xuyên theo đường thẳng) · **Quạt** (hình nón) · **Vùng** · **Đạn** (bay, né
được, chặn được) · **Thuấn** (tức thì, không bay, **không né được**: "Sét = điện đất + điện trời… không thể trốn,
không thể tránh", CM195) · **Kênh** (giữ phím, bị ngắt) · **Tích** (giữ để nạp tầng, thả để bắn) · **Trễ** (dấu,
nổ sau) · **Liên** (chuỗi nhiều nhịp).

### 2.4 Hành, đạo, phẩm

Xem mục 3 (loại sát thương), 4 (đạo) và 5 (phẩm).

---

## 3. Loại sát thương và ailment

Mười loại, chia bốn nhóm. Mỗi loại có **một ailment riêng** và **một kháng riêng**.

| Nhóm | Loại | Ailment | Ailment làm gì | Gốc |
|---|---|---|---|---|
| Thân | **Lực** | **Thương** (chảy máu) và **Chấn** (dồn choáng) | Thương: mất máu theo thời gian, nặng hơn khi di chuyển. Chấn: đủ ngưỡng thì choáng. | Huyết tán chỉ giết được người đang chảy máu (CM1) |
| Ngũ hành | **Kim** | **Phá** | Mỗi tầng giảm hộ thể. Kim là "không gì chắc". | Tâm Ý Ngũ Hành quyết, Tiên Cực Kiếm (TN1934) |
| | **Mộc** | **Triền** | Dây trói: đủ tầng thì trói chân. Đòn Mộc hút máu. | Long cân (TN), dây leo |
| | **Thủy** | **Hàn** rồi **Băng phong** | Hàn làm chậm. Đủ tầng thì đóng băng. Đòn kết liễu lên người đóng băng thì vỡ (Băng điêu). | Hoàng Tuyền Thăng Khiếu, sương lam (TN276) |
| | **Hỏa** | **Thiêu** | Đốt theo thời gian. Có bậc màu (3.2). | Hỏa Man, Chu Tước, Tàn Dạ |
| | **Thổ** | **Trọng** | Nặng: làm chậm dash, kéo người đang bay xuống đất, đủ tầng thì quật ngã. | Trọng bản nguyên "nặng như cả một đại lục" (TN1812) |
| Thiên | **Lôi** | **Tê** | Tăng sát thương phải nhận. Lôi trúng người đang đóng băng thì phá băng và thêm sát thương. | "Lôi phá băng" (CM197) |
| | **Dương** (quang) | **Chiếu** | Làm lộ ảo ảnh và người tàng hình, đốt mạnh lên kẻ mang Âm. | Hạo Dương Thạch, Thái Sơ, Cực Minh Quang |
| Minh | **Âm** (tử) | **Tịch** | Héo: giảm hồi máu, tăng sát thương Âm phải nhận. Đủ tầng thì hồi máu thành mất máu. | Tịch Diệt Chỉ hút sinh cơ hóa tử lực (TN) |
| | **Hồn** | **Nhiếp** | Đánh vào thần thức (3.4): làm chậm thi pháp. Thần thức vỡ thì "thất hồn" (câm và đứng yên). | Cực Cảnh Thần Thức, Côn Cực Tiên, tiếng chuông |

### 3.1 Ngũ hành sinh khắc

Vòng khắc: **Kim khắc Mộc, Mộc khắc Thổ, Thổ khắc Thủy, Thủy khắc Hỏa, Hỏa khắc Kim.**
Vòng sinh: **Kim sinh Thủy, Thủy sinh Mộc, Mộc sinh Hỏa, Hỏa sinh Thổ, Thổ sinh Kim.**

- **Khắc** (đánh vào): đòn hành A trúng người đang dính ailment của hành bị A khắc thì **xóa ailment đó** và gây thêm
  sát thương bằng phần ailment còn lại. Thủy dập Thiêu (TN95: âm hàn dập Lam Viêm Ma hỏa). Hỏa nung chảy Phá giáp.
- **Sinh** (nuôi): đòn hành B trúng người đang dính ailment của hành sinh ra B thì **ailment của B kéo dài và nặng
  hơn**. Mộc sinh Hỏa: đốt người đang bị trói thì cháy lâu hơn. Thủy sinh Mộc: người đang Hàn bị trói nhanh hơn.
- **Cùng hành thì nuôi nhau**: đánh Hỏa vào kẻ hệ Hỏa làm nó mạnh lên ("hỏa đánh vào hỏa linh thú còn làm nó mạnh
  thêm", TN136). Kháng cùng hành của một đấu sĩ hệ đó cao, và vài keystone biến đòn cùng hành nhận vào thành tài
  nguyên (mục 7).
- **Ngoại lệ theo bậc**: đạo hỏa (3.2) khắc ngược Thủy: "thần thông của Oán Ngụy là lửa, về thiên tính có thể đối kháng
  với bản nguyên thủy" (CM1094).

Lôi và Băng khắc nhau theo hai chiều khác nhau, đúng như truyện: **sét đánh vào người đang đóng băng thì phá băng**
(CM197), nhưng **vùng băng đóng băng đạn sét bay qua** (lam quang Tuyết Vực đóng băng lôi cầu, TN293). Tức là Lôi thắng
Thủy trên người, Thủy thắng Lôi trên đường bay.

Ngoài vòng: **Lôi khắc tử vật và trùng** (Lôi ×1,5 lên triệu hồi Âm và bầy trùng; TN84, TN293). **Âm và Dương khắc nhau
hai chiều**: mỗi bên nhận ×1,5 từ bên kia, nhưng Âm đủ dày thì dập được Dương (CM684). Ai tu Âm thì yếu trước Dương
và ngược lại, nên chọn đạo cũng là chọn điểm yếu.

### 3.2 Bậc của lửa, bậc của sét

- **Lửa có màu**: đỏ, lam, tím, đen (CM4; TN1280: thực hỏa, hư hỏa, đạo hỏa; lửa bảy màu, chín màu). Một chiêu Hỏa có
  bậc theo hỏa hầu và bí quyết. **Lửa bậc cao nuốt lửa bậc thấp**: đạn lửa thấp hơn bay vào vùng lửa cao hơn thì bị
  hút và hồi linh lực cho chủ vùng lửa ("lấy lửa đối lửa", CM82).
  - **Hư hỏa** đốt theo hành động: mỗi chiêu mục tiêu vừa dùng thêm một tầng ("đốt tâm tình", TN1280).
  - **Đạo hỏa** đốt cả ý cảnh: rút thần thức đang giữ aura.
  - **Oán hỏa** đen là nhánh riêng: mạnh lên theo số lần chính mình từng bị trúng đòn (CM4).
- **Sét có loại**: lôi thường, lôi tiên thiên (khắc con rối), thiên kiếp, Cực Cảnh. Lục Đạo Lôi cộng 3 loại tự định
  của Vương Lâm, Cực Cảnh tính là loại thứ bảy (TN1269). Keystone **Thái Cổ Lôi Long** cho quyền chỉ huy mọi loại sét
  thường trên sân (mục 7).

### 3.3 Dung hợp (Tạo)

"Băng và hỏa dung hợp… khí hoàn toàn khác với băng hỏa" (CM468). Cửu Phong gọi việc dung hợp hai cực là "Tạo".
Luật: **hai loại sát thương trong một cặp dưới đây cùng trúng một mục tiêu trong 1 giây thì sinh loại thứ ba**, phần
sát thương mới bỏ qua kháng của cả hai loại gốc.

| Cặp | Sinh ra | Hiệu ứng thêm | Nguồn |
|---|---|---|---|
| Hỏa + Thủy (nóng + lạnh) | **Phong** | Gió hút và đẩy, tắt hồi linh lực trong chốc lát. | Táng Âm: "nóng lạnh giao nhau, gió nổi lên" (CM415) |
| Phong + Thủy | **Băng** | Đóng băng ngay, không cần dồn tầng. | Hô Phong cộng Hoán Vũ thành tuyết (TN1008) |
| Lôi + Hỏa | **Lôi hỏa** | Nổ hai nhịp, nhịp sau đánh vào thần thức. | Cửu Huyền: "tuy hai mà một" (TN1089) |
| Thổ + Hỏa (+ Âm) | **Sơn Băng** | Khói hỏa độc, phủ vùng. | Sơn Băng (TN) |
| Âm + Dương | **Lưỡng Nghi** | Bỏ qua mọi kháng, nhưng người dùng cũng nhận một phần. | "lực lượng của âm dương… cũng là nhân quả" (TN1306) |
| Sinh + Tử (mục 4) | **Luân hồi** | Mục tiêu bị kéo về trạng thái của 1 giây trước. | Sinh Tử Luân Hồi |

Dung hợp có rủi ro: hai thứ mang ý chí khác nhau có thể bài xích nhau ("thủy hỏa bất dung", TN1322). Bí quyết **Dung**
(mục 6) gộp hai chiêu làm một nhưng có 15% bài xích nổ vào người dùng.

### 3.4 Ba thanh sống

| Thanh | Như PoE | Ai đánh vào | Khi cạn |
|---|---|---|---|
| **Máu** | Life | Mọi loại | Thua hiệp. |
| **Hộ thân linh quang** | Energy shield | Mọi loại trừ Âm. Âm đi xuyên qua, như Chaos đi qua ES. | Vỡ: 1 giây không tự hồi. |
| **Thần thức** | (mới) | Hồn và một ít Lôi. Ý cảnh và aura **giữ chỗ** trên thanh này. | **Thất hồn**: câm 1,5 giây, mọi ý cảnh tắt. |

Sinh vật không hồn (khôi lỗi, hỏa linh) miễn Hồn (TN133). Cổ tu khóa thức hải vào xương nên giảm Hồn nhận vào
("thức hải đã tan vào xương", TN243).

---

## 4. Đạo: những thứ đổi luật

Các đạo không phải loại sát thương mà là **cơ chế**. Một chiêu có tag đạo thì được hoặc chịu các luật dưới đây.

| Đạo | Luật | Chiêu mẫu | Gốc |
|---|---|---|---|
| **Sinh tử** | Có **dấu Sinh** (hồi một ít cho mục tiêu, lưu lại lượng đã hồi) và **chiêu Tử** (kích dấu: gây sát thương bằng X% lượng đã hồi). Cạn sinh cơ thì dùng tử khí: dưới 10% máu, chiêu tốn máu chuyển sang tốn Tử khí. | Sinh Tử Ấn, chỉ "Sinh" đoạt phi kiếm (TN281) | "gieo sinh rồi kích tử" (TN1632), "Sau sinh cơ chính là tử khí" (TN1279) |
| **Nhân quả** | Gắn **sợi nhân quả** giữa hai thứ. Đòn đánh vào một đầu thì vang sang đầu kia. Phá khiên của người mang Khí Nhân Quả thì nhận lại đòn đó. | Nhân Quả Ấn, Karmic Lash, Tử Ấn Tổ Quyết (nối mạng) | "nhân là do ta, quả thuộc về ta" (TN740) |
| **Chân giả** | Ảo gây 50% sát thương. Nhưng nếu đối thủ **đáp lại** (dùng phòng thủ hay chiêu lớn) trong 3 giây thì ảo hóa thật, gây đủ 100% và xuyên giáp. Chiêu Chân giả mở mắt xóa mọi ảo, triệu hồi và vùng của đối thủ trong tầm. Hai Chân giả đụng nhau thì cả hai bị phản chấn. | Nhật Nguyệt Huyễn, Chân Giả (nhắm mắt né) | "tin thì tồn tại" 3 giây (CM4), TN1632 |
| **Tuế nguyệt** (thời gian) | Làm chậm, quay lui, làm già. Quay lui hủy được tự bạo và chiêu đang kênh. Ai đứng yên với tâm bất động thì miễn ("người xem cây", CM1339). Kẻ dùng thời gian lên người mang Nguyền Bẩm Sinh thì bị phản. | Lưu Nguyệt, Đảo Ngược Thời Gian, Bướm Tang Tương | CM1051, CM1339, TN304 |
| **Không gian** | Thuấn di, xé khe, **khóa thuấn di**. Mở khe ngay chỗ đối thủ định thuấn di thì đối thủ bị xé (TN331). Quỷ Sát (di chuyển bằng thời gian) đi qua được khóa không gian. | Súc Địa, Tê Thiên, khe Diệt Sinh | TN331, CM1203 |
| **Duyên** | Mỗi đấu sĩ có **sợi duyên** tới pháp bảo, triệu hồi, aura và buff môi trường của mình. Trảm Duyên cắt sợi: thứ ở đầu kia tắt tới hết hiệp. Duyên của chính mình khó cắt nhất. | Trảm Tam Sát, Trảm Thần Quyết | "Trảm Tam Sát là khắc tinh của tất cả duyên" (CM992) |
| **Mệnh** | Khí vận, mệnh cách: chém mệnh cách gỡ một mạng phụ hay một buff lớn. Có **thọ nguyên** làm tài nguyên (mục 8). | Búa Chẻ Củi, Thu ý | CM1425 |
| **Sát lục** | Mỗi lần trúng đòn hay hạ triệu hồi thì tích sát khí. Tích nhiều thì mạnh, quá ngưỡng thì Tâm ma (tự mất máu). | Sát Lục Tiên Quyết, Hắc ấn chữ Tử | TN575, TN272 |
| **Nguyền** | Nguyền cần **vật dẫn** (máu, tóc, vật của đối thủ, lấy khi đánh trúng ở cự ly gần). Nguyền lan sang mọi thứ **cùng nguồn** (triệu hồi, phân thân của mục tiêu). Nguyền là cặp với chúc phúc. Bị nguyền ngược thì kẻ nguyền chịu phản phệ. | Thảo Nhân, Cửu Lê, Bát Chú | CM1275, CM1330, CM (Cửu Lê) |
| **Cấm** | Lồng, bẫy, khóa. Cấm chế đứng **ngoài** thang phẩm (mục 5). | Cấm Phiên, Tam đại cấm chế | TN1274 |

---

## 5. Phẩm: thang đè nhau

Luật xương sống của tu tiên: cái cao hơn đè cái thấp hơn bất kể con số.

| Phẩm | Tên | Mở bằng | Đè được gì |
|---|---|---|---|
| 1 | **Thuật** | Mặc định | — |
| 2 | **Thần thông** | Hỏa hầu 10 | Khiên và giải trừ phẩm Thuật. |
| 3 | **Quy tắc** | Ngộ đạo (gem Ngộ đạo) hoặc bí quyết **Quy Tắc** | Đi xuyên khiên phẩm 2. Phá vùng phẩm 2 khi đi qua. Chém đứt dây nối (Trảm La "chém quy tắc"). |
| 4 | **Bản nguyên** | Chỉ chiêu của Bản nguyên (thăng hoa, mục 9) | Buff phẩm 4 không bị giải trừ bởi phẩm 3 ("ngón tay vàng" không xóa được hư bản nguyên, TN1956). |
| 5 | **Tín niệm** | Một chiêu tối thượng mỗi đấu sĩ | Xóa mọi lĩnh vực, aura, vùng trên sân (Đạp Thiên Nhất Bộ, TN2088). |
| — | **Cấm chế** | Tag Cấm chế | Đứng ngoài thang. **Không giải được, không phản được**, chỉ phá bằng sát thương. Vì "không phải pháp thuật" nên vẫn cản được cả người phẩm cao (TN1274). |

Hai luật đi kèm thang:

- **Tu vi và ý cảnh là hai thanh riêng** (TN621). Phẩm của chiêu khác cảnh giới của người. Một chiêu phẩm 3 do người
  cảnh giới thấp dùng vẫn đè khiên phẩm 2 của người cảnh giới cao, nhưng sát thương vẫn theo chỉ số của người dùng.
- **Ý cảnh tự ngộ thắng ý cảnh mượn** (TN826). Trong tranh đạo niệm (mục 6.3), aura từ Bản nguyên của chính đấu sĩ
  thắng aura gắn từ pháp bảo khi hai bên ngang phẩm.

---

## 6. Hiệu ứng mạnh, yếu và luật vùng

### 6.1 Nguyền (curse)

Mỗi đấu sĩ dính tối đa **1 nguyền** (2 với đạo tâm Vu Chú). Nguyền mới đè nguyền cũ, trừ khi nguyền cũ là phẩm cao
hơn. Mọi nguyền cần vật dẫn (mục 4). Chỉ chiêu có tag Hồn, Âm hay Phù đặt được nguyền từ Xa.

| Nguyền | Làm gì | Gốc |
|---|---|---|
| **Thảo Kết Ký Sự** (búp bê cỏ) | Mỗi nút thắt (kênh ngắn) thêm một tầng. 10 tầng: đòn của mục tiêu có thể trượt hoặc tự hại. 11 tầng: khống chế ngắn. Từ nút thứ 7, mỗi nút làm người nguyền mất máu. | CM (tám câu, thảo kết) |
| **Cửu Tử** | Mục tiêu càng dùng chiêu phòng thủ thì nguyền càng nặng. Chỉ giải được bằng một nguyền khác hoặc sống qua 9 nhịp. | CM6 |
| **Trớ Thuật Tiên Tổ** | Như Doom: mỗi lần mục tiêu tự ra đòn thì nhận thêm X% sát thương. Chỉ giảm khi không ra đòn (lui lại "bế quan"). | TN8 |
| **Tử Ấn Tổ Quyết** | Nối mạng: mục tiêu không thể cách người nguyền quá N thân. Có cửa sổ lộ sợi tơ để bị chém đứt. | TN8 |
| **Ngũ Tạng** | 5 nhịp, mỗi nhịp phá một "tạng": hồi máu, hét, một loại tài nguyên… Nhịp 5 là đòn nặng. | TN7 |
| **Bói Ba Nhịp Thở** | Dấu đếm ngược 3 giây. Không phá được thì nhận sát thương Hồn lớn. Phá được thì người bói bị phản. | TN7 |
| **Lời Giam Cầm** | Đặt một lời hứa: nếu mục tiêu dùng chiêu loại X trong N giây thì bị **Tâm Nứt** (giảm kháng Hồn, mở Đoạt Xá). | CM5 |
| **Mộng Đạo** | Ẩn, không hiện trên thanh trạng thái. Lần tới mục tiêu dùng chiêu mạnh nhất thì khựng 0,5 giây. | TN6 |
| **Phong Thần** | Phong ấn một tag hành của mục tiêu (mọi chiêu mang tag đó bị khóa vài giây). Trả bằng máu tối đa của người nguyền. | TN5 |
| **Hóa Ma** | Biến buff tiên lực của mục tiêu thành debuff ma khí. Lôi thuần dương trục được nó. | TN3–4 |
| **Tuế Nguyệt** | Làm già: chậm và rút thọ nguyên. Không ăn được lên kẻ "người xem cây" đang đứng yên. | TN260, CM6 |
| **Khắc Ấn** | Lộ vị trí (phá tàng hình), đạn tự dẫn vào, giảm kháng một hành (như Exposure). | CM1 |

### 6.2 Ý cảnh (aura) và chúc phúc (buff)

Ý cảnh giữ chỗ trên thanh Thần thức. Một đấu sĩ bật được 1–3 ý cảnh tùy đạo đồ. Có hai loại:

- **Ý cảnh thân**: buff cho mình. Ví dụ **Sát lục** (tăng sát thương theo sát khí), **Tuế nguyệt** (làm chậm đạn đi vào
  vùng của mình, TN267: một chỉ làm chậm lưới kiếm), **Niềm tin** (Thập Băng: chuỗi quyền không thể ngắt), **Chiến**
  (miễn khựng khi đang ra đòn).
- **Ý cảnh áp**: debuff cho đối thủ trong vùng. Ví dụ **An nhàn** (giảm tốc độ đánh, mất sát ý, TN260), **Vô tình**
  (đòn xuyên tràn sang thần thức, TN299), **Điên cuồng** (đồng hóa linh lực của đối thủ trong vùng: mỗi lần đối thủ
  dùng chiêu trong vùng thì người giữ aura hồi linh lực, TN258), **Hỉ Nộ Ai Oán** (bốn trạng thái đổi lần lượt, CM5).

Chúc phúc là buff có thời hạn, không giữ thần thức: Hồn Khải (TN8, tăng sức 30 giây, hết thì vỡ thành khiên),
Hơi Thở Bất Diệt (không thể chết khi lửa còn cháy, mỗi giây đốt máu tối đa, CM5), Túc Mệnh (15 giây, mọi chiêu thời
gian không tốn hồi chiêu, CM481).

### 6.3 Tranh đạo niệm

Khi hai ý cảnh áp của hai bên chồng lên nhau, mở một thanh **đạo niệm** giữa hai người. Mỗi giây, bên có ý cảnh phẩm
cao hơn (rồi tới bên tự ngộ, rồi tới bên còn nhiều thần thức hơn) đẩy thanh về phía đối thủ. Thanh chạm đầu thì bên
thua **thất hồn** và mất ý cảnh. Một lần **Nghịch chuyển** mỗi hiệp: tiêu toàn bộ linh lực để dựng lại aura. Gốc:
TN740–741, "Ý cảnh sơ kỳ đấu trung kỳ là trứng chọi đá" (TN326). Đây là một mini-game đọc nhau cho đấu 1v1.

**Ý cảnh khắc nhau.** Truyện nêu từng cặp. Trong tranh đạo niệm, bên khắc đẩy thanh nhanh gấp đôi, nên chọn ý cảnh
cũng là chơi oẳn tù tì:

| Ý cảnh | Khắc | Gốc |
|---|---|---|
| Luân hồi | Tuyệt tình, Tuế nguyệt ("Tuế Nguyệt chỉ là nét bút nghiêng bắt chước luân hồi") | TN265, TN365 |
| Dục (dâm tà) | Vô tình | TN449 |
| Sát lục | An nhàn, mê hoặc | TN260 |
| Tỏa Thần ("vạn tà bất xâm") | Dục, mê hoặc | TN404 |
| Vô tình | Hồi ức, Si (cắt tình cảm thì ký ức không giữ được người) | TN299 (đoán: suy từ trận Hồng Điệp) |

Ý cảnh còn **cất được** (vào ngọc giản, tượng gỗ, TN411) và **bị nuốt** (TN435). Bí quyết Sao Chép gắn lên chiêu
Thân có thể lấy ý cảnh của đối thủ vừa thất hồn, dùng tới hết hiệp.

### 6.4 Lĩnh vực

Mỗi bên có tối đa một lĩnh vực. Lĩnh vực mới của cùng một bên thay lĩnh vực cũ. Hai lĩnh vực của hai bên thì chia
đôi sân theo vị trí người dựng. Lĩnh vực bị xóa bởi chiêu phẩm 5 và bởi Ly Phong.

| Lĩnh vực | Luật | Gốc |
|---|---|---|
| **Hoàng Tuyền** | Sông oán dưới sàn: làm chậm và nuốt đạn của đối thủ. Thức 3 kéo đối thủ về tâm. Kẻ chết trong vùng (triệu hồi) thành oan hồn phe chủ. | TN3–4 |
| **Tàn Dạ** | Đêm phủ sân, mặt trời mọc gây Dương không né được. Tốn rất nhiều, nhưng **miễn phí trong vài giây đầu mỗi hiệp** ("bình minh"). | TN986, TN1213 |
| **Ly Phong** | Vùng không gió: không ai hồi linh lực, không dùng được chiêu tiêu linh lực, chỉ đánh tay. Xóa đạn và hiệu ứng gió. | CM (Ly Phong Tam Thức) |
| **Định Quy Tắc** | Ai **di chuyển** trong vùng thì bị luật áp lên (cấm độn, cấm bay). Đứng yên thì không bị. | TN1755 |
| **Uy Áp Chân Giới** | Đối thủ trong vùng chỉ dùng được 40% tu vi. Phải giữ tâm sàn mới dựng được. | CM6 |
| **Hô Phong Hoán Vũ** | Đối thủ không hồi linh lực từ môi trường. Triệu hồi chết trong vùng cho chủ một điểm hương hỏa. | TN6 |

### 6.5 Khống chế cứng và Kiên

Định Thân và các khống chế cứng **ngắn** (0,3–0,5 giây, TN4, TN8) và có luật giảm:

- Mỗi lần khống chế lên cùng mục tiêu trong 4 giây thì ngắn đi một nửa (**Kiên**: hệ số giảm dần, như grit cũ).
- Mục tiêu có phẩm ý cảnh cao hơn tự giải sau một nửa thời gian (TN3).
- Định Thân tốn máu của người dùng theo chênh lệch sức mạnh: càng yếu hơn mục tiêu thì càng tốn (TN8).

---

## 7. Đạo tâm (keystone)

Mỗi đạo tâm đổi luật chơi và luôn có giá. Một đấu sĩ chọn tối đa 3.

| Đạo tâm | Được | Mất | Gốc |
|---|---|---|---|
| **Cực Cảnh** | Mọi sát thương của một loại (chọn khi lấy) ×1,6, chiêu Thuấn của loại đó kết liễu đối thủ cùng phẩm hoặc thấp hơn khi dưới 15% máu. | Không gắn được bí quyết chuyển hoá hay dung hợp: "không dung hợp với bất cứ thuộc tính nào khác". | TN91, TN125 |
| **Âm Hàn Linh Lực** | 100% sát thương thành Thủy. Lôi nhận vào chuyển 50% thành Thủy. | Không gắn được bí quyết hành khác. | TN104 |
| **Cổ Thần Quyết** | Không có trần linh lực. Hút linh lực rơi trên sân, kể cả của đối thủ. | Không dùng được đan dược hồi phục. | TN1 |
| **Sát Lục Tiên Quyết** | Mỗi lần trúng đòn thì tích một lạc ấn. Lạc ấn thành lớp hộ thể tự hồi rất nhanh. | Quá 1.000 tầng thì Tâm ma: mỗi giây tự mất máu. Đòn phẩm 3 trở lên xuyên nhiều lớp một lúc. | TN575 |
| **Tiên Trì** | Mọi chiêu Thuật và Thần thông ×1,4 sức. | Không bật được ý cảnh, không vào được tranh đạo niệm. | TN879–890 |
| **Tóc Đen** (Vương Lâm) | Mọi sát thương thành Sát lục (không có kháng). Miễn giải trừ buff. | Tắt mọi chân thân, aura và buff ngoại lai. −30% chỉ số. | TN8 |
| **Thái Cổ Lôi Long** | Hút Lôi nhận vào thành linh lực. Lôi của mình bỏ qua kháng Lôi. Lôi của đối thủ có 30% bị cướp quyền (bay ngược). | Chỉ được dùng một loại sát thương ngoài Lôi. | TN702, TN873 |
| **Hệ Đại Thành** | Miễn một hành đã chọn; đòn hành đó của đối thủ có X% bị khuất phục (đổi phe). | Đòn của hành khắc nó gây ×1,5 lên mình. | TN1657 |
| **Tam Mệnh** | Hai mạng phụ: chết thì sống lại tại chỗ với 30% máu, cắt mọi nhân quả trên người (nguyền, sợi khóa). | Không bao giờ nạp lại. Mỗi mạng dùng làm giảm sát thương 10%. | TN7, TN8 |
| **Ngọn Đèn Hồn** | Chia hồn vào N đèn. Chết thì tắt một đèn và sống lại. Có thể chủ động tắt đèn để lấy một đợt sức. | Đèn có thể bị đóng băng hay bị dập từ xa. Hết đèn thì thua cả trận, không chỉ hiệp. | CM5 |
| **Hai Mươi Năm Khổ Luyện** | Chiêu phẩm 1 ×2 sức. | Không dùng được chiêu phẩm 3 trở lên. | TN1 (Dẫn Lực thuật) |
| **Tâm Xám** | Miễn mọi hiệu ứng cảm xúc (An nhàn, Hỉ Nộ Ai Oán, Mê Dục). | Không hưởng buff cảm xúc nào. | CM5 |
| **Người Xem Cây** | Đứng yên thì miễn chậm, quay lui, già. Đối thủ dùng chiêu thời gian lên mình thì tốn gấp đôi thọ nguyên. | Di chuyển thì nhận thêm 15% sát thương từ Tuế nguyệt. | CM1339 |
| **Nguyền Bẩm Sinh** | Miễn "già". Ai dùng chiêu thời gian lên mình thì dính nguyền ngược. | Không dùng được chiêu thời gian của chính mình. | CM1051 |
| **Đạo Dung Thân** | Mỗi chiêu vừa dùng nạp một khí xoáy vào đòn thường kế tiếp (tối đa 9). | Đòn thường chậm hơn 20%. Mỗi lớp đỡ của đối thủ triệt một khí xoáy. | TN1919 |
| **Phá Rồi Lập** | Mọi phân thân hay ảnh thân bị phá trong 5 giây thì nhận buff lên cảnh giới 10 giây. | Mất một ảnh thân là mất vĩnh viễn trong hiệp. | CM6 |
| **Vu Chú** | Dính được 2 nguyền lên đối thủ. | Mỗi nguyền dùng tăng thanh "Tam Hoang thức tỉnh". Đầy thì bị phạt (chính mình dính nguyền mạnh nhất vừa dùng). | CM1261 |
| **Niềm Tin Tất Thắng** | Chuỗi quyền mạnh thêm theo số trận thắng liên tiếp trong mùa. | Thua một trận là mất sạch. | TN544 |

---

## 8. Tài nguyên

| Tài nguyên | Như PoE | Có từ | Tiêu vào | Gốc |
|---|---|---|---|---|
| **Linh lực / tiên lực** | Mana | Tự hồi, nhanh hơn khi thiền | Phần lớn chiêu | — |
| **Thần thức** | (giữ chỗ) | Cố định, hồi chậm | Ý cảnh giữ chỗ; Hồn đánh vào | TN225 (Cực Cảnh tiêu thần thức) |
| **Máu** | Life, Blood magic | — | Chiêu "huyết": mỗi điểm máu trả cộng X% sức | Phun máu cường kích (TN1) |
| **Thọ nguyên** | (mới) | **Không hồi trong trận** | Chiêu thời gian, mệnh, Trảm Tam Sát, Hơi Thở Bất Diệt. Cạn thì "già": −20% mọi chỉ số. | CM4, CM1339 |
| **Sát khí** | Rage | Trúng đòn, hạ triệu hồi | Chiêu Sát lục. Quá ngưỡng thì Tâm ma. | TN271 |
| **Hồn** | Corpses | Triệu hồi chết, sát thương Hồn, kết liễu | Du hồn, Tát Đậu Thành Binh, Tôn Hồn Phiên; tiêu để hồi thần thức | TN295, TN913 |
| **Tinh điểm** | Endurance / Frenzy / Power charges | Xem dưới | Toái tinh | TN1035 |
| **Bài xích** | (thanh nhiệt) | Dùng chiêu Tiên và chiêu Cổ liền nhau | Đầy thì nổ vào mình | "Tiên và Cổ trộn lẫn sinh lực bài xích" (TN1586) |

**Tinh điểm của tam tộc Cổ** khớp với ba loại charge của PoE (TN1035: "Cổ Thần tinh điểm ở mi tâm, Cổ Ma ở mắt trái,
Cổ Yêu ở mắt phải"):

- **Thần tinh** (mi tâm): như Endurance. Giảm sát thương nhận vào, tăng kháng.
- **Yêu tinh** (mắt phải): như Frenzy. Tăng tốc đánh và tốc độ di chuyển.
- **Ma tinh** (mắt trái): như Power. Tăng chí mạng, tăng sức Hồn.
- **Toái tinh**: đập nát toàn bộ tinh một loại để có một đợt sức rất lớn (kích thước, sát thương), hoặc hồi đầy máu.

**Lá bài một lần**: một số chiêu có số lần dùng mỗi trận cố định, không hồi: Sợi Tơ Thiên Kiếp (1), Cung Lý Quảng
(2–3), Sợi Tóc Man Thần (2), Kiếm Khí Lăng Thiên Hậu (1–2). Mọi chiêu loại này có hành động **Lộ bài**: giơ ra mà
không bắn, gây Sợ hãi hoặc khiến đối thủ mất hồi chiêu phòng thủ nếu họ đáp lại, không tốn lần dùng (mục 10).

---

## 9. Căn cơ và bản nguyên (lớp nhân vật)

Bốn căn cơ là bốn điểm xuất phát trên đạo đồ. Mỗi căn cơ có ba bản nguyên để thăng hoa (ascendancy).

| Căn cơ | Lối đánh | Bản nguyên 1 | Bản nguyên 2 | Bản nguyên 3 |
|---|---|---|---|---|
| **Tiên tu** (Vương Lâm tiền kỳ) | Pháp thuật, cấm chế, pháp bảo | **Cấm Chế**: bẫy và lồng không giải được, phiên tự chạy | **Lôi Bản Nguyên**: lôi đồ, quyền chỉ huy sét | **Hỏa Bản Nguyên**: lửa nhiều màu, hư hỏa, Tàn Dạ |
| **Cổ tu** | Thân thể, tinh điểm, cận chiến | **Cổ Thần**: thân to, Toái tinh, miễn Hồn | **Cổ Yêu**: tốc độ, nghịch ý (đánh vào thì nhận phản lực, tắt khiên để đổi phản lực lấy buff) | **Cổ Ma**: hút linh lực, Cổ Ma chỉ |
| **Ma tu** | Hiến tế, chuyển hoá, hồn | **Sát Lục**: sát khí, lạc ấn, Tóc Đen | **Sinh Tử Luân Hồi**: dấu Sinh, kích Tử, Hoàng Tuyền | **Nhân Quả / Chân Giả**: sợi nhân quả, ảo hóa thật |
| **Man tu** (Tô Minh) | Máu, nguyền, triệu hồi, thời gian | **Man Thần**: Man Thần Biến, Phong/Lôi/Hỏa Man | **Mệnh Tu Bốn Mùa**: đổi Đông→Thu→Hạ→Xuân, ảnh thân | **Tuế Nguyệt / Nguyền**: Túc Mệnh, quay lui, Thảo Nhân |

Bản nguyên mở chiêu phẩm 4 và cho một chiêu phẩm 5 (tín niệm) cuối nhánh.

---

## 10. Đọc nhau: những gì làm 1v1 hay

Đây là phần làm game này khác PoE. PoE đánh quái; ở đây mỗi trận là hai người đoán nhau. Tất cả đều lấy từ cách hai
nhân vật chính thắng trận trong truyện.

1. **Lộ bài, không đánh.** Vương Lâm giơ sợi tơ Thiên kiếp hay Cấm Phiên dẫn kiếp để ép đối thủ lùi (TN226, TN248,
   TN309). Chiêu một lần có hành động Lộ bài (mục 8). Đối thủ phải đoán: bắn thật hay dọa.
2. **Mồi nhử.** Ném sợi tơ lên trời, đối thủ ngẩng nhìn, rồi đánh thần thức vào kẽ hở (TN226). Bí quyết **Nhử Đòn**:
   khống chế kéo dài gấp đôi nếu mục tiêu vừa dùng chiêu phòng thủ.
3. **Đòn dọa "Một chưởng".** Hiện dấu tay khổng lồ nhưng không có sức. Ai dùng phòng thủ để đỡ thì mất hồi chiêu đó
   (TN299, TN3 hook 26).
4. **Ảo hóa thật.** Luật Chân giả (mục 4): đáp lại ảo là cho nó thành thật.
5. **Lời giam cầm.** Đặt điều kiện lên chiêu đối thủ sắp dùng (6.1).
6. **Bóc lớp đòn lớn.** Đòn lớn của đối thủ bị hao dần qua nhiều lớp: du hồn, rồi lôi cầu, rồi cấm khí (TN295–297).
   Mỗi lớp chặn được làm đòn lớn yếu đi theo tỉ lệ, thay vì chặn hết hoặc không chặn gì.
7. **Ngắt chiêu đang tích.** Tích và Kênh đều bị ngắt. Huyết Nguyệt nhãn (CM1) là chiêu ngắt thuần.
8. **Chặn tự bạo.** Tự bạo là nước cuối thường thấy. Quay lui (Tuế nguyệt) hủy tự bạo và trả chi phí cho người tự bạo
   (CM3 hook 4).
9. **Câu giờ cho buff đối thủ hết.** Buff mạnh có thời hạn (Hồn Khải, Túc Mệnh); phe yếu chạy hoặc ném triệu hồi tự bạo
   để chờ (TN8).
10. **Đánh lúc sân sụp.** Một số lĩnh vực có thời hạn. Khi lĩnh vực sụp, cả hai không độn được trong 1 giây (TN316).

---

## 11. Bí quyết (support), danh mục đầu

| Bí quyết | Đòi tag | Làm gì | PoE gần nhất | Gốc |
|---|---|---|---|---|
| **Thanh Trần** | bất kỳ | +X% hiệu quả của chiêu đi kèm. Một mình vô dụng. | Increased Effect | CM1 |
| **Liên Châu** | Đạn | Bắn thêm 2–4 đạn. | Multiple Projectiles | Ngũ tiễn liên châu (CM1) |
| **Tàn Ảnh Tốc Thần** | Chỉ, Quyền, Kiếm | Lặp N lần rồi dồn làm một đòn. | Spell Echo / Multistrike | TN8 |
| **Cửu Chuyển** | Tích | Giữ để xoay 1–9 vòng, mỗi vòng nhân sát thương. Đối thủ chặn đúng chiêu thì đạn đổi chủ. | Charged | TN499 |
| **Cửu Biến** | bất kỳ | Hiệu quả tăng mạnh, nhưng thời gian thi triển dài gấp nhiều lần. | (mới: đổi tốc độ lấy sức) | CM |
| **Quy Tắc** | Kiếm, Chưởng | Nâng chiêu lên phẩm 3. Tốn thêm 40%. | Awakened | Trảm La (TN) |
| **Dung** | 2 thần thông | Gộp hai chiêu cùng mạch: hành của chiêu đầu, hình của chiêu sau. 15% bài xích nổ vào mình. | (mới) | TN1322 |
| **Hóa Ma** | bất kỳ | Chuyển 100% sang Âm. Có cơ hội làm triệu hồi đối thủ tự bạo khi chết. | Conversion | TN3 |
| **Âm Hàn** | Hỏa, Lôi | Chuyển 50% sang Thủy. | Conversion | TN104 |
| **Huyết Luyện** | Pháp bảo | Dùng máu thay linh lực. Mạnh hơn, nhưng pháp bảo vỡ thì mất % máu tối đa. | Blood Magic | TN1 |
| **Phun Máu** | bất kỳ | Trả 10% máu: đòn kế tiếp xuyên một lớp phòng ngự. | (mới) | TN1 |
| **Súc Địa** | Cận | Lao tới mục tiêu rồi đánh. | Leap Slam / Blink | TN |
| **Dịch Linh** | Đạn | Đạn to và nhanh dần theo quãng bay; sát thương tăng theo quãng đường. Gọi đạn quay về được. | Projectile range scaling | TN5 |
| **Lôi Bạo Trễ** | Đạn | Đạn bị chặn hay bị nuốt thành mìn trong vật chặn, kích bằng lệnh riêng. | Delayed / Mine | "Lôi, bạo!" (TN740) |
| **Nhử Đòn** | Khống chế | Thời gian khống chế ×2 nếu mục tiêu vừa dùng phòng thủ. | (mới, yomi) | TN226 |
| **Chắc Trúng** | bất kỳ | Không thể bị né. +40% thời gian thi triển. | Accuracy / "cannot be evaded" | Trảm Thần Quyết (CM5) |
| **Liên Hoàn Suy Yếu** | Phòng thủ + Công | Khi chiêu phòng thủ bị phá thì kích chiêu công cùng mạch với sát thương bằng phần đã hấp thụ. | Cast when Damage Taken | CM6 |
| **Phản Ấn** | Phòng thủ | Hấp thu một đạn rồi bắn trả, đổi sang hành của chiêu này. | Absorb and Return | TN511 |
| **Khí Nhân Quả** | Khiên, Trận | Khi khiên hay trận vỡ, bắn ngược bản sao đòn đã phá nó. | (mới) | TN7 |
| **Tự Bạo** | Hồn, Pháp bảo | Khi bị hạ thì nổ. | Detonate Dead | TN, CM |
| **Thu Hồn** | Ấn, Phiên | Thứ gì chết dưới chiêu này làm nó mạnh vĩnh viễn trong hiệp. | (mới, scaling) | Phong Tiên Ấn (TN4) |
| **Thôn Cấm** | Phiên, Trận | Bẫy, totem hay vùng đối thủ nằm trong vùng này thì bị phân giải, cộng charge cho chiêu. | (mới) | Tam Xoa Kích nuốt cấm chế (TN5) |
| **Sao Chép** | Thân, Hồn | Học bản yếu hơn của chiêu cuối đối thủ vừa dùng. | (mới) | Huyết Yến (TN4), Đoạt Xá (CM5) |
| **Mê Dục** | Đạn | Trúng thì đối thủ không dùng được chiêu phòng thủ 2 giây. | (mới) | Thập Tam Đào Hoa Sát (CM2) |
| **Trọng** | bất kỳ | Thêm Thổ "nặng": chậm hơn 20% nhưng đè, ngắt bay, thêm Lực. | Added damage | TN1812 |
| **Hư Hỏa** | Hỏa | Đổi Thiêu thành hư hỏa (đốt theo số chiêu đối thủ vừa dùng). | (mới) | TN1280 |
| **Tuế Nguyệt** | Vùng, Ý cảnh | Đạn đi vào vùng bị chậm 40%. | (mới) | TN267 |
| **Chặn Lui** | Ấn, Chưởng | Trúng thì đối thủ không độn được 1 giây. | (mới) | Dịch Linh Ấn (TN8) |
| **Không Khóa** | bất kỳ | Trúng thì khóa thuấn di 2 giây. | (mới) | Tịch Diệt Chỉ (TN3) |
| **Hồn Bạo** | Hồn | Hy sinh triệu hồi để nổ, sát thương theo máu còn lại của nó. | Detonate | TN913 |
| **Nạp Hồn** | bất kỳ | Gây sát thương Hồn thì nạp tài nguyên Hồn. | Leech (mới) | TN |
| **Đoạt Tu Vi** | bất kỳ | Gây sát thương thì hút một phần tài nguyên đối thủ. | Leech | Ấn Trăng (CM6) |

---

## 12. Thần thông, danh mục đầu

Gom theo căn cơ. Ký hiệu cột: **Hình** · **Tầm** · **Đưa** · **Hành/Đạo** · **Phẩm khởi**. Mỗi căn cơ dùng được mọi
thần thông, nhưng đạo đồ của nó thưởng cho nhóm của nó.

### 12.1 Tiên tu

| Thần thông | Tag | Làm gì | Gốc |
|---|---|---|---|
| **Cực Cảnh Thần Thức** | Chỉ · Xa · Thuấn · Hồn · 2 | Tia đỏ từ mắt, không né được. Đánh vào thần thức. Kết liễu đối thủ cùng phẩm dưới ngưỡng, còn với phẩm cao hơn thì chỉ choáng. Mỗi lần dùng trừ thần thức của chính mình. | TN125–278 |
| **Cấm Phiên** | Phiên · Trung · Vùng · Cấm · — | Dựng cờ: màn đen giam, bắn 9 cấm khí hình rồng. Khắc đủ 99 tổ thì phiên dẫn kiếp (lá bài một lần, có Lộ bài). | TN226–315 |
| **Cấm Khí Trường Thương** | Kiếm · Trung · Tuyến · Cấm · — | Mấy chục cấm khí kết một cây thương. Đâm tới đâu thu cấm khí trên đất tới đó, càng đâm càng dài. | TN309 |
| **Tam Đại Cấm Chế** | Cấm chế · Vùng · Trễ · — | Ba lớp vỡ lần lượt: khóa di chuyển, khóa chiêu thân, khóa chiêu hồn. Không giải được. | TN6 |
| **Lôi Đồ** | Ấn · Toàn sân · Vùng · Lôi · 4 | Ra lệnh: một hiệu ứng bất kỳ đang dính trên người (kể cả của đối thủ) "là sấm", biến thành sát thương Lôi. | "ta nói nó là sấm… thì nó là như vậy" (TN1269) |
| **Hô Phong** | Hồn · Xa · Đạn · Thủy/Âm · 2 | N rồng đen đuổi theo phun âm phong. Gây "tắt lửa sinh mệnh" (giảm hồi máu). | TN4 |
| **Hoán Vũ** | Ý cảnh · Trung · Vùng · Thủy · 2 | Mưa quanh người, mỗi giọt là đạn nhỏ. Có Hô Phong cùng lúc thì hai chiêu chuyển cho nhau và dung thành băng. | TN913 |
| **Tát Đậu Thành Binh** | Hồn · Trung · — · Lực · 2 | Triệu số binh bằng số Hồn đang có. Binh lao vào tự bạo. | TN4 |
| **Sơn Băng** | Thiên kiếp · Xa · Vùng · Thổ/Hỏa · 2 | Lần 1 dựng núi (vật cản thật, chặn đạn). Lần 2 nổ đá, lửa, khói độc. Lần 3 phá núi thành vòng sông lửa quanh mình. | TN5 |
| **Phiên Thiên Ấn** | Ấn · Xa · Vùng · Thổ · 2 | Ấn khổng lồ úp xuống, phá giáp. | TN |
| **Thuấn Di** | Độn · Xa · — · Không gian · 1 | Biến mất rồi hiện ở chỗ khác. | TN201 |
| **Định Thân Thuật** | Chỉ · Trung · Thuấn · Hồn · 3 | Đứng hình 0,3–0,5 giây, ngắt chiêu đang thi. Tốn máu theo chênh lệch sức (6.5). | TN, xuyên suốt |
| **Tàn Dạ** | Lĩnh vực · Toàn sân · Hỏa/Dương · 5 | Xem 6.4. Tín thuật của Vương Lâm. | TN986–2072 |
| **Cung Lý Quảng** | Đạn · Xa · Tuyến · Kim · 3 | Xuyên mọi khiên; mỗi lớp xuyên làm tên nhỏ đi. 2–3 phát mỗi trận, có Lộ bài. | TN7 |

### 12.2 Cổ tu

| Thần thông | Tag | Làm gì | Gốc |
|---|---|---|---|
| **Cổ Thần Hóa** | Thân · Cận · — · Lực · 2 | To lên, +máu, +sức đấm. Mỗi Thần tinh làm biến thân dài thêm. | TN284, TN339 |
| **Trăm Quyền Sinh Cơ** | Quyền · Cận · Liên · Lực · 2 | Mỗi cú tốn % máu tối đa thay cho linh lực, càng đấm liền càng mạnh. Dưới 10% máu thì chuyển sang tử khí. | TN6 |
| **Thập Băng Quyền Ý** | Quyền · Cận · Liên · Lực · 2 | 10 đòn chồng sức lên nhau. Ý cảnh Niềm tin: không thể ngắt giữa chừng. | TN544 |
| **Toái Tinh** | Thân · Cận · Vùng · Lực · 3 | Đập một loại tinh: nổ vùng, hoặc hồi đầy máu. | TN1035 |
| **Cổ Thần Chỉ** | Chỉ · Trung · Đạn · Lực · 3 | Mạnh dần theo sát thương đã nhận. Ở ngưỡng tối đa đổi thành Cổ Thần Tí. | TN716 |
| **Nghịch Ý** | Thân · — · — · Lực · 2 | Đánh vào mình thì mình nhận phản lực tăng dần. Tắt khiên trước khi nhận thì phản lực thành buff Tẩy tủy. | Yêu Cổ (TN3) |
| **Thần Chấn** | Chưởng · Trung · Quạt · Thổ · 2 | Kéo đối thủ về một hàng trước mặt, phá từng lớp khiên. | TN6 |
| **Cổ Tộc Chiến Ấn** | Ấn · Toàn sân · — · Lực · 3 | Nếu cả hai cùng ấn chiến ấn: khóa trận thành đấu thân thể (thần thông −50%, đòn thân thể +X%, không ai rời vùng). | TN7 |
| **Đạo Cổ Tứ Thức** | Ấn · Trung · Liên · Lực/Âm · 3 | Bốn nhấn: kéo về, lốc hút, chuyển hồi máu của đối thủ thành sát thương, hóa đá nếu trúng đủ bốn. Bỏ dở là mất hết. | TN1829 |

### 12.3 Ma tu

| Thần thông | Tag | Làm gì | Gốc |
|---|---|---|---|
| **Tịch Diệt Chỉ** | Chỉ · Xa · Đạn · Âm · 2 | Đạn hút sinh cơ trên đường bay. Mỗi đạn hay khiên nó xuyên qua cộng sát thương. Trúng thì khóa thuấn di 2 giây. | TN3 |
| **Hóa Ma Chỉ** | Chỉ · Xa · Đạn · Âm · 2 | Chuyển buff tiên lực của đối thủ thành ma khí. | TN3 |
| **Hoàng Tuyền Chỉ** | Chỉ · Xa · Kênh · Âm · 3 | Đòn kết liễu. Trong lúc kênh, người dùng không hồi gì. Sát thương cộng theo số thứ đã chết trong vùng. | TN3 |
| **Sinh Tử Ấn** | Ấn · Cận · Trễ · Sinh tử · 3 | Chưởng trái gieo dấu Sinh, chưởng phải kích Tử. | TN7 |
| **Nhân Quả Ấn** | Chưởng · Cận · Kênh · Nhân quả · 3 | Giật một bản nguyên của đối thủ (gỡ một buff hay một loại sát thương N giây) và hồi cho mình. Dưới ngưỡng máu thì thành đòn kết liễu. | TN7 |
| **Chân Giả** | Thân · — · Kênh · Chân giả · 3 | Nhắm mắt 0,5 giây: miễn phép và đạn. Mở mắt: xóa mọi ảo, triệu hồi, vùng của đối thủ trong tầm. | TN7 |
| **Du Hồn** | Hồn · Xa · — · Hồn · 2 | Thả bầy hồn: chui vào người rút máu, làm mồi, chặn đạn (mỗi lớp hồn bóc đạn lớn đi 1/5). | TN295–344 |
| **Hồn Phiên** | Phiên · Trung · Vùng · Hồn · 2 | Nuốt hồn, lệnh "Dung!" gộp N hồn thành một hồn chủ, lệnh "Vỡ!" cho cả bầy tự bạo. | TN3 |
| **Sát Lục Lôi** | Chỉ · Xa · Thuấn · Lôi/Sát lục · 4 | Mạnh nhưng có thanh bất ổn tích dần. Phải có aura Cấm chế bật để giữ, không thì nổ vào mình. | TN8 |
| **Thôn Linh Phệ Thể** | Quyền · Cận · Đơn · Âm · 2 | Lao tới, hút máu lớn. | TN6 |

### 12.4 Man tu

| Thần thông | Tag | Làm gì | Gốc |
|---|---|---|---|
| **Man Thương** | Kiếm · Trung · Tuyến · Lực · 2 | Một nét chém đánh trúng cả mục tiêu vô hình, phá đạn hay vùng trên đường. | CM2 |
| **Huyết Tán** | Phù · Cận · Vùng · Lực · 1 | Sát thương cực lớn, nhưng chỉ lên người đang chảy máu. Mình đang chảy máu thì tự dính. | CM1 |
| **Ngũ Tiễn Liên Châu** | Đạn · Xa · — · Lực · 1 | Năm mũi liền, mỗi mũi gây Thương. Chiêu mở cho Huyết Tán. | CM1 |
| **Trảm Tam Sát** | Kiếm · Trung · Liên · Duyên/Mệnh · 3 | Ba nhát, mỗi nhát trả thọ nguyên. Nhát 1 xóa buff môi trường, nhát 2 xóa buff không gian và dịch chuyển, nhát 3 xóa hiệu ứng thời gian. Phải chém về **phương Tam Sát** (đổi theo thời gian trong hiệp), không phải về phía đối thủ. | CM81, CM992 |
| **Man Thần Biến** | Thân · Cận · Liên · Lực · 3 | To lên; đấm liền cùng mục tiêu tăng dần tới 7 quyền. | CM5 |
| **Ly Phong Tam Thức** | Quyền · Trung · Liên · Phong · 3 | Khai Dương (đấm mắt bão, nạp bằng di chuyển), Táng Âm (trộn nóng lạnh sinh gió), Ly Phong (lĩnh vực không gió). | CM2–4 |
| **Đảo Ngược Thời Gian** | Chưởng · Trung · Thuấn · Tuế nguyệt · 3 | Kéo đối thủ về vị trí 0,5 giây trước, hủy chiêu đang dở. Xung lực lớn chỉ bị chậm. | CM481 |
| **Túc Mệnh** | Thân · — · — · Tuế nguyệt · 4 | 15 giây: mọi chiêu thời gian không tốn hồi chiêu. Hết thì kiệt sức. | CM481 |
| **Thảo Kết Ký Sự** | Phù · Xa · Kênh · Nguyền · 2 | Xem 6.1. | CM |
| **Thuật Nguyền Rủa** (tám câu) | Âm · Xa · Liên · Nguyền · 4 | Tốn toàn bộ máu trừ 1. 8 hiệu ứng nối nhau mỗi giây: mù, mất máu, hồn oán, mục rữa, gai xương… Chỉ kích được khi mục tiêu không còn khiên. | CM3 |
| **Mệnh Cách Bốn Mùa** | Ý cảnh · Trung · — · Mệnh · 3 | Đổi theo chu kỳ Đông (chậm rồi băng), Thu (hút máu), Hạ (lửa đổi bằng máu mình), Xuân (hồi). Đổi đúng thứ tự thì được thêm một tầng "Chuyển mùa". | CM4 |
| **Thu Ý** | Kiếm · Cận · Quạt · Mệnh · 3 | Quét hình nón, gây sát thương theo % thọ nguyên còn của đối thủ, hồi một nửa cho mình. | CM4 |
| **Di Sơn** | Thiên kiếp · Xa · Vùng · Thổ · 3 | Gọi N núi từ hồi ức. Bí quyết Di Sơn Hóa Nhất gộp mọi núi làm một đạn đơn mục tiêu. | CM5 |
| **Nguyệt Dực** | Hồn · Trung · — · Hỏa · 2 | Bầy hồn trăng, hợp thể thành Tàn Thần "Hình". Kẻ có tag Hỏa Man cướp được bầy Nguyệt Dực của đối thủ. | CM31, CM99 |
| **Tô Kiếp** | Thiên kiếp · Xa · Vùng · đổi theo kiếp · 4 | Vẽ vòng, gọi kiếp 1 đến 8, mỗi kiếp một loại sát thương khác. | CM6 |

### 12.5 Nguồn trong bách khoa

Mỗi thần thông ở trên ứng với một mục trong `references/encyclopedia/techniques.json` (khóa `key`, số lần xuất hiện,
chương đầu). Mỗi mục giữ mọi lần xuất hiện kèm câu trích đã kiểm với bản gốc. Hai thần thông là **gộp hoặc chuyển
thể** của thiết kế, không phải một chiêu có tên trong truyện; chúng được ghi rõ.

| Thần thông | Khóa trong bách khoa | Lần | Chương đầu |
|---|---|---|---|
| Cực Cảnh Thần Thức | TN `cực cảnh` | 19 | 91 |
| Cấm Phiên | TN `cấm phiên` | 14 | 181 |
| Cấm Khí Trường Thương | TN `cấm khí trường thương` | 1 | 309 |
| Tam Đại Cấm Chế | TN `tam đại cấm chế dung hợp phá diệt sinh tử tuế nguyệt` | 1 | — |
| Lôi Đồ | TN `lôi đồ mắt phải` | 1 | — |
| Hô Phong / Hoán Vũ | TN `hô phong`, `hoán vũ` | 11 / 6 | 783 / 794 |
| Tát Đậu Thành Binh | TN `tát đậu thành binh` | 8 | 856 |
| Sơn Băng | TN `sơn băng` | 6 | 857 |
| Phiên Thiên Ấn | TN `phiên thiên ấn` | 1 | 1319 |
| Định Thân Thuật | TN `định thân thuật` | 22 | 625 |
| Tàn Dạ | TN `tàn dạ` | 12 | 987 |
| Cung Lý Quảng | TN `cung lý quảng` | 9 | 1468 |
| Cổ Thần Hóa | **chuyển thể**: gộp các lần Vương Lâm hiện thân Cổ Thần (TN `cổ thần quyết`, `hợp thể bản tôn phân thân…`) | — | 190 |
| Trăm Quyền Sinh Cơ | TN `một trăm quyền quyền sinh cơ` | 1 | 1277 |
| Thập Băng Quyền Ý | TN `thập băng quyền ý thức 1 của mạc lệ hải` | 1 | — |
| Toái Tinh | TN `cổ thần toái tinh` | — | — |
| Cổ Thần Chỉ | TN `cổ thần chỉ` | 5 | 716 |
| Nghịch Ý | **chuyển thể** từ khảo nghiệm Yêu Cổ (TN `yêu cổ trống khảo nghiệm`) | 1 | 588 |
| Thần Chấn | TN `thần chấn` | 2 | 1468 |
| Cổ Tộc Chiến Ấn | TN `cổ tộc chiến ấn` | 1 | 1672 |
| Đạo Cổ Tứ Thức | TN `đạo cổ tứ thức` | 2 | 1778 |
| Tịch Diệt / Hóa Ma / Hoàng Tuyền Chỉ | TN `tịch diệt chỉ`, `hóa ma chỉ`, `hoàng tuyền chỉ` | 4 / 3 / 3 | 483–500 |
| Sinh Tử Ấn | TN `sinh tử ấn thần thông thứ năm` | 1 | 1616 |
| Nhân Quả Ấn | TN `nhân quả ấn` | 2 | 1614 |
| Chân Giả | TN `chân giả đạo`, `chân giả bổn nguyên` | — | — |
| Du Hồn | TN `du hồn` | 3 | 295 |
| Hồn Phiên | TN `tôn hồn phiên`, `hồn phiên vương lâm` | 17 / — | 390 |
| Sát Lục Lôi | TN `sát lục lôi` | 1 | 1900 |
| Thôn Linh Phệ Thể | TN `thôn linh phệ thể tuyệt thiên` | 3 | 885 |
| Man Thương | CM `man thương` | 3 | 238 |
| Huyết Tán | CM `huyết tán huyết dược` | 1 | 10 |
| Ngũ Tiễn Liên Châu | CM `ngũ tiễn liên châu` | 1 | 13 |
| Trảm Tam Sát | CM `trảm tam sát` | 4 | 38 |
| Man Thần Biến | CM `man thần biến` | 3 | 1203 |
| Ly Phong Tam Thức | CM `ly phong tam thức` | 3 | 382 |
| Đảo Ngược Thời Gian | CM `đảo ngược thời gian` | 5 | 498 |
| Túc Mệnh | CM `túc mệnh` | 4 | 477 |
| Thảo Kết Ký Sự | CM `thảo kết ký sự` | 2 | 600 |
| Thuật Nguyền Rủa (tám câu) | CM `thuật nguyền rủa` | 8 | 429 |
| Mệnh Cách Bốn Mùa | CM `mệnh cách bốn mùa` | 4 | 692 |
| Thu Ý | CM `thu ý huyết thu bổn mệnh thần thông` | — | — |
| Di Sơn | CM `di sơn` | 6 | 1007 |
| Nguyệt Dực | CM `nguyệt dực` | 2 | 25 |
| Tô Kiếp | CM `tô kiếp tự giáng kiếp kiểu tam hoang` | 1 | — |

"—" là chỗ một khóa có nhiều mục con. Tra trực tiếp trong file để xem số chương.

---

## 13. Ba build mẫu

Để thấy hệ chồng lên nhau ra sao.

**"Vương Lâm Hóa Thần" (Tiên tu, Lôi Bản Nguyên)**
- Mạch 5: Cực Cảnh Thần Thức + Nhử Đòn + Chắc Trúng + Không Khóa + Thanh Trần.
- Mạch 4: Cấm Phiên + Thôn Cấm + Khí Nhân Quả + Tự Bạo.
- Mạch 3: Thuấn Di + Tuế Nguyệt (vùng ý cảnh).
- Đạo tâm: Thái Cổ Lôi Long, Sát Lục Tiên Quyết.
- Kế hoạch: dựng Cấm Phiên để giữ khoảng cách và nuốt bẫy đối thủ, chờ đối thủ dùng phòng thủ, rồi Cực Cảnh với Nhử
  Đòn (choáng ×2), kết liễu khi dưới ngưỡng.

**"Tô Minh Mệnh Tu" (Man tu, Mệnh Tu Bốn Mùa)**
- Mạch: Ngũ Tiễn Liên Châu + Liên Châu (mở Thương), Huyết Tán + Phun Máu.
- Mạch: Mệnh Cách Bốn Mùa + Thanh Trần. Trảm Tam Sát + Chắc Trúng.
- Đạo tâm: Người Xem Cây, Phá Rồi Lập.
- Kế hoạch: Đông làm chậm, Thu hút máu, mở Thương rồi Huyết Tán. Gặp đối thủ dựa vào pháp bảo hay buff môi trường thì
  Trảm Tam Sát cắt duyên.

**"Cổ Thần Đập Tinh" (Cổ tu, Cổ Thần)**
- Mạch: Thập Băng Quyền Ý + Tàn Ảnh Tốc Thần + Trọng + Súc Địa.
- Toái Tinh riêng một mạch. Cổ Tộc Chiến Ấn để ép đối thủ pháp thuật vào đấu thân thể.
- Đạo tâm: Cổ Thần Quyết, Hai Mươi Năm Khổ Luyện.
- Điểm yếu cố ý: không có đòn Xa, bị Hoàng Tuyền và Tuế Nguyệt làm chậm, sợ đối thủ giữ khoảng cách.

---

## 14. Bot, frame data, cân bằng

- **Bot thấy tag, không thấy tên.** Vector mỗi slot: hình (one-hot 17), tầm (4), đưa (10), hành/đạo (10 + 10), phẩm
  (6), chi phí theo từng tài nguyên, hồi chiêu, windup, recover, số lần dùng còn lại, và cờ "có thể Lộ bài". Cộng
  trạng thái hai bên: ailment theo tầng, nguyền, ý cảnh, lĩnh vực, ba thanh sống, tinh điểm, thọ nguyên, thanh đạo
  niệm. Đây là thay đổi input của mạng, nên kéo theo huấn luyện lại và mùa mới (`docs/bot.md`).
- **Không gian build quá lớn để bot học hết.** Bot học trên một **bộ build cố định mỗi mùa** (ví dụ 12 build đóng sẵn,
  4 mỗi căn cơ), còn người chơi build tự do ở chế độ luyện tập. Giải đấu mở dần.
- **Frame data giữ nguyên khung cũ**: windup, recover, counter, punish, unsafe on block. Kênh và Tích là windup dài bị
  ngắt được. Thuấn không né được nhưng windup dài và có báo trước để còn đọc được.
- **Đo**: mỗi căn cơ đấu mỗi căn cơ 150 seed, mục tiêu 45–55% mỗi cặp. Đạo tâm nào có tỉ lệ chọn trên 60% hay dưới 5%
  thì xem lại (`game-balance`, `competitive-game-design`).

---

## 15. Làm theo thứ tự nào

1. **Lõi luật** (crate `pixfx-duel`): ba thanh sống, 10 loại sát thương và ailment, vòng ngũ hành, thang phẩm, kinh
   mạch và bí quyết theo tag. Chưa cần đạo tâm hay lĩnh vực.
2. **Lát dọc 2 căn cơ**: Tiên tu (Lôi) và Man tu (Mệnh), mỗi bên 8 thần thông, 10 bí quyết, 4 đạo tâm. Huấn luyện bot,
   chạy ladder.
3. Thêm nguyền, ý cảnh, tranh đạo niệm, lĩnh vực.
4. Thêm Cổ tu, Ma tu, bản nguyên, chiêu tín niệm.
5. Lộ bài và các luật đọc nhau (mục 10), đo xem bot có học được dọa và nhử không.

Mỗi bước là một thay đổi luật, nên cần chủ dự án duyệt và đợi mùa mới.
