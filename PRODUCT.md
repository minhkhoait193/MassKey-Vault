# MassKey Vault

<!-- impeccable:product-schema 1 -->

## Platform
web

## Users
Người dùng cá nhân quản lý mật khẩu và lưu website, tài liệu trực tuyến, repo GitHub để tìm lại trên máy Windows.

## Product Purpose
Một vault mã hóa cục bộ cho hai không gian riêng: Mật khẩu và Tài nguyên. Sau khi mở khóa, người dùng chọn không gian cần làm việc và có thể chuyển thẳng sang không gian kia.

## Operating Context
Ứng dụng desktop Tauri 2 trên Windows, hoạt động offline-first. Metadata và thông tin GitHub được lấy khi người dùng thêm tài nguyên; GitHub tự làm mới sau mỗi lần mở khóa. Khi mất mạng, dữ liệu đã lưu vẫn xem được.

## Capabilities and Constraints
- Tài khoản, danh mục, thùng rác, cài đặt và backup ZIP mã hóa đang tồn tại.
- Tài nguyên có URL, tiêu đề, mô tả, ghi chú, thẻ, bộ sưu tập và đường dẫn thư mục tùy chọn. Chỉ lưu đường dẫn, không sao chép file vào vault.
- Dữ liệu và backup cũ phải tiếp tục đọc được. Không dùng vault sản xuất để kiểm thử.
- Không dùng AI hoặc tài khoản dịch vụ ngoài để lấy metadata.

## Brand Commitments
Giữ tên MassKey Vault và nhận diện màu sắc hiện tại. Màn chào Hai cánh cửa là điểm nhấn điều hướng.

## Accessibility & Inclusion
Hỗ trợ bàn phím, trạng thái focus rõ ràng, cửa sổ Windows hẹp và `prefers-reduced-motion`.
