//! groove-gd1_25 viết thân hàm (đặc tả 5.5). `main` gọi hàm này trước khi dựng GPUI nên chữ ký đã khai sẵn.

/// Đã có phiên chính thì chuyển `argv` + `cwd` cho nó và trả `true`, tiến trình này thoát luôn.
pub fn forward_to_primary() -> bool {
    false
}
