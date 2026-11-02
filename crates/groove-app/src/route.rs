//! groove-gd1_03: enum Route, lịch sử.
//!
//! Bản Tauri dùng `vue-router`; bản này là một enum và ngăn xếp lịch sử trong app (đặc tả 1.1, 4). Chỉ có "lùi", không có
//! "tiến" như bản cũ.

use groove_core::model::CategoryRef;
use groove_core::settings::StartPage;
use groove_core::t;

/// Số bước lùi tối đa giữ lại; vượt thì bỏ bước cũ nhất.
const HISTORY_LIMIT: usize = 64;

/// Một màn của app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Route {
    MyMusic,
    NowPlaying,
    Playlists,
    /// Danh sách bài của một album, thư mục hay playlist (`?view=list` của bản cũ). Không lưu qua lần mở sau.
    Category(CategoryRef),
}

impl Route {
    /// Ba mục của cột điều hướng, theo thứ tự hiện.
    pub const NAV: [Route; 3] = [Route::MyMusic, Route::NowPlaying, Route::Playlists];

    /// Trang mở lại khi khởi động (`settings.last_active_route`).
    pub fn from_start_page(page: StartPage) -> Route {
        match page {
            StartPage::MyMusic => Route::MyMusic,
            StartPage::NowPlaying => Route::NowPlaying,
            StartPage::Playlists => Route::Playlists,
        }
    }

    /// Giá trị ghi vào `settings.last_active_route`. Không nhớ tham số xem danh sách: về trang lưới chứa nó.
    pub fn start_page(self) -> StartPage {
        match self.nav_section() {
            Route::NowPlaying => StartPage::NowPlaying,
            Route::Playlists => StartPage::Playlists,
            _ => StartPage::MyMusic,
        }
    }

    /// Mục của cột điều hướng được tô sáng khi ở màn này.
    pub fn nav_section(self) -> Route {
        match self {
            Route::Category(CategoryRef::Playlist(_)) => Route::Playlists,
            Route::Category(_) => Route::MyMusic,
            other => other,
        }
    }

    /// Tên trang hiện ở cột điều hướng và tiêu đề cửa sổ.
    pub fn title(self) -> &'static str {
        match self.nav_section() {
            Route::NowPlaying => t!("shell.nav.now_playing"),
            Route::Playlists => t!("shell.nav.playlists"),
            _ => t!("shell.nav.my_music"),
        }
    }
}

/// Ngăn xếp lịch sử. Luôn có ít nhất một phần tử (màn hiện tại).
#[derive(Clone, Debug)]
pub struct History {
    stack: Vec<Route>,
}

impl History {
    pub fn new(start: Route) -> History {
        History { stack: vec![start] }
    }

    pub fn current(&self) -> Route {
        *self.stack.last().expect("history is never empty")
    }

    pub fn can_back(&self) -> bool {
        self.stack.len() > 1
    }

    /// Sang `route`. Trả `false` (không thêm bước) khi đã ở đúng màn đó.
    pub fn push(&mut self, route: Route) -> bool {
        if self.current() == route {
            return false;
        }
        self.stack.push(route);
        if self.stack.len() > HISTORY_LIMIT {
            self.stack.remove(0);
        }
        true
    }

    /// Lùi một bước; trả màn mới, `None` khi không còn gì để lùi.
    pub fn back(&mut self) -> Option<Route> {
        if !self.can_back() {
            return None;
        }
        self.stack.pop();
        Some(self.current())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// C4 — mở app về trang cuối, trang xem danh sách không được nhớ.
    #[test]
    fn start_page_round_trip_drops_category() {
        for page in [StartPage::MyMusic, StartPage::NowPlaying, StartPage::Playlists] {
            assert_eq!(Route::from_start_page(page).start_page(), page);
        }
        assert_eq!(Route::Category(CategoryRef::Album(1)).start_page(), StartPage::MyMusic);
        assert_eq!(Route::Category(CategoryRef::Folder(2)).start_page(), StartPage::MyMusic);
        assert_eq!(Route::Category(CategoryRef::Playlist(3)).start_page(), StartPage::Playlists);
    }

    #[test]
    fn nav_section_highlights_the_parent_grid() {
        assert_eq!(Route::Category(CategoryRef::Playlist(3)).nav_section(), Route::Playlists);
        assert_eq!(Route::Category(CategoryRef::Album(3)).nav_section(), Route::MyMusic);
        assert_eq!(Route::NowPlaying.nav_section(), Route::NowPlaying);
    }

    /// C3 — nút lùi theo lịch sử, mờ khi không lùi được.
    #[test]
    fn history_back_follows_pushes() {
        let mut history = History::new(Route::MyMusic);
        assert!(!history.can_back());
        assert_eq!(history.back(), None);

        assert!(history.push(Route::Category(CategoryRef::Album(7))));
        assert!(history.push(Route::NowPlaying));
        assert!(history.can_back());
        assert_eq!(history.back(), Some(Route::Category(CategoryRef::Album(7))));
        assert_eq!(history.back(), Some(Route::MyMusic));
        assert_eq!(history.back(), None);
        assert_eq!(history.current(), Route::MyMusic);
    }

    #[test]
    fn pushing_the_current_route_adds_nothing() {
        let mut history = History::new(Route::Playlists);
        assert!(!history.push(Route::Playlists));
        assert!(!history.can_back());
        // Cùng loại nhưng khác tham số là màn khác.
        assert!(history.push(Route::Category(CategoryRef::Playlist(1))));
        assert!(history.push(Route::Category(CategoryRef::Playlist(2))));
    }

    #[test]
    fn history_is_bounded() {
        let mut history = History::new(Route::MyMusic);
        for i in 0..(HISTORY_LIMIT as i64 * 2) {
            history.push(Route::Category(CategoryRef::Album(i)));
        }
        let mut steps = 0;
        while history.back().is_some() {
            steps += 1;
        }
        assert_eq!(steps, HISTORY_LIMIT - 1);
    }
}
