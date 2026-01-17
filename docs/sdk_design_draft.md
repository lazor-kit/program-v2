# Thiết kế LazorKit Rust SDK

## Tổng quan
LazorKit SDK được chia thành hai phân hệ rõ ràng để phục vụ hai nhóm đối tượng phát triển khác nhau. Kiến trúc đề cao tính **Modular** để đảm bảo khả năng mở rộng khi số lượng Plugin tăng lên.

---

## 1. Phân hệ Cơ bản (Low-Level / Basic Integration)
*Dành cho: App Developer, Wallet UI Developer.*
*Mục tiêu: Đơn giản hóa các tác vụ ví thông thường.*

(Giữ nguyên nội dung cũ...)

---

## 2. Phân hệ Nâng cao (High-Level / Protocol Interface)
*Dành cho: Protocol Integrators, Contract Experts.*
*Mục tiêu: Kiểm soát tuyệt đối cấu hình Policy.*

(Giữ nguyên nội dung cũ...)

---

## 3. Kiến trúc Plugin (Modular Architecture)

### Tại sao Modular tốt cho "Nhiều Plugin"?
Sự lo ngại về việc quản lý hàng chục plugin là hợp lý. Tuy nhiên, kiến trúc Modular giải quyết vấn đề này tốt hơn Monolithic (Gộp chung):

1.  **Vấn đề Bloat (Phình to)**: Nếu gộp 50 plugin vào 1 SDK, app của user sẽ phải gánh code của cả 50 plugin dù chỉ dùng 1. Modular giải quyết triệt để việc này (Tree-shaking).
2.  **Xung đột Dependency**: Plugin A dùng thư viện X v1.0, Plugin B dùng X v2.0. Tách crate giúp Rust/NPM quản lý phiên bản độc lập dễ hơn.
3.  **Giải pháp tiện lợi ("Kitchen Sink")**: Để user không phải import thủ công từng cái, ta cung cấp các gói tổng hợp.

### Cấu trúc Rust
- **Core**: `lazorkit-sdk`
- **Plugin lẻ**: `lazorkit-policy-sol-limit`, `lazorkit-policy-whitelist`...
- **Gói tổng hợp (Optional)**: `lazorkit-policies` (Re-export tất cả các plugin phổ biến).

```rust
// Cách 1: Dùng lẻ (Tối ưu production)
use lazorkit_sdk::prelude::*;
use lazorkit_policy_sol_limit::SolLimit;

// Cách 2: Dùng gói tổng hợp (Tiện cho dev/test)
use lazorkit_policies::{SolLimit, Whitelist, TimeLock};
```

---

## 4. Kiến trúc TypeScript SDK

Đối với TypeScript/JavaScript (Frontend), vấn đề bundle size là cực kỳ quan trọng. Kiến trúc Modular ánh xạ sang hệ sinh thái NPM như sau:

### Cấu trúc Gói (NPM Packages)

1.  **`@lazorkit/sdk` (Core)**
    - Chứa: `createWallet`, `LazorClient`, `TransactionBuilder`.
    - Không chứa: Logic encode của từng Policy cụ thể.

2.  **`@lazorkit/policy-sol-limit` (Plugin Package)**
    - Chứa: Hàm `encodeSolLimit(amount)`, `decodeSolLimit(buffer)`.
    - Dependencies: Chỉ phụ thuộc `@lazorkit/sdk-core`.

3.  **`@lazorkit/policies` (Umbrella Package - Optional)**
    - Re-export toàn bộ các policy.

### Ví dụ Sử dụng (TypeScript)

```typescript
// 1. Chỉ import những gì cần dùng (Tối ưu Bundle Size)
import { LazorWallet, Network } from '@lazorkit/sdk';
import { SolLimitPolicy } from '@lazorkit/policy-sol-limit';

const wallet = await LazorWallet.connect(provider, walletAddress);

// 2. Sử dụng Policy một cách độc lập
// Policy builder trả về cấu trúc Config chuẩn mà Core SDK hiểu được
const limitConfig = new SolLimitPolicy()
    .amount(1_000_000)
    .interval('1d')
    .build();

// 3. Inject vào Transaction
await wallet.grantPermission({
    signer: newSignerPubkey,
    policy: limitConfig, // Core SDK nhận config này và đóng gói
    roleId: 1
});
```

### Lợi ích cho Frontend
- **Tree Shaking**: Các bundler (Vite, Webpack) sẽ tự động loại bỏ code của các plugin không được import. Ví dụ: App chỉ dùng `SolLimit` sẽ không bao giờ phải tải code của `Whitelist`.
- **Phiên bản**: Dễ dàng nâng cấp `@lazorkit/policy-defi-v2` mà không ảnh hưởng code đang chạy ổn định của `@lazorkit/policy-social-v1`.
